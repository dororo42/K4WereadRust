//! 传输编排：IPv4 DNS（对齐 ipv4_dns 补丁语义，AF_INET only）→ TCP
//! connect_timeout → TLS 握手 → 请求写 → 响应读。全程受 block/total 预算约束。

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use rustls::StreamOwned;

use crate::codes;
use crate::http::{self, Response};
use crate::tls;
use crate::url::{parse_https_url, Budgets};

pub struct Request<'a> {
    pub url: &'a str,
    pub method: &'a str,
    /// "Name: value" 行；Host/Content-Length/Connection 由报文层负责
    pub headers: Vec<&'a str>,
    pub body: Option<&'a [u8]>,
    pub budgets: Budgets,
    pub max_response_bytes: usize,
    pub trust_pem: Option<&'a [u8]>,
}

#[derive(Debug)]
pub enum TransportError {
    Code(i32, String),
}

impl TransportError {
    pub fn code(&self) -> i32 {
        match self {
            TransportError::Code(c, _) => *c,
        }
    }
    pub fn detail(&self) -> &str {
        match self {
            TransportError::Code(_, d) => d,
        }
    }
}

fn code(c: i32, detail: impl Into<String>) -> TransportError {
    TransportError::Code(c, detail.into())
}

fn is_timeout_err(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) || e.raw_os_error() == Some(libc::EAGAIN)
        || e.raw_os_error() == Some(libc::EWOULDBLOCK)
}

const DNS_QUEUE_CAPACITY: usize = 8;
type DnsResult = Result<Vec<SocketAddr>, TransportError>;

struct DnsJob {
    host: String,
    port: u16,
    deadline: Instant,
    reply: mpsc::SyncSender<DnsResult>,
}

struct DnsWorker {
    queue: mpsc::SyncSender<DnsJob>,
}

impl DnsWorker {
    fn start<F>(mut resolver: F) -> io::Result<Self>
    where
        F: FnMut(&str, u16) -> DnsResult + Send + 'static,
    {
        let (queue, jobs) = mpsc::sync_channel::<DnsJob>(DNS_QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("k4tls-dns".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    if Instant::now() >= job.deadline {
                        continue;
                    }
                    let result = resolver(&job.host, job.port);
                    // One-slot reply: a timed-out caller can never block the worker.
                    let _ = job.reply.try_send(result);
                }
            })?;
        Ok(Self { queue })
    }

    fn resolve(&self, host: &str, port: u16, deadline: Instant) -> DnsResult {
        remaining(deadline)?;
        // Bound queued allocations as well as the number of pending requests.
        if host.is_empty() || host.len() > 254 || host.as_bytes().contains(&0) {
            return Err(code(codes::DNS_FAIL, "invalid DNS host"));
        }
        let (reply, response) = mpsc::sync_channel(1);
        self.queue
            .try_send(DnsJob {
                host: host.into(),
                port,
                deadline,
                reply,
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => code(codes::DNS_FAIL, "DNS queue full"),
                mpsc::TrySendError::Disconnected(_) => code(codes::DNS_FAIL, "DNS worker stopped"),
            })?;
        let result = response
            .recv_timeout(remaining(deadline)?)
            .map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => code(codes::TIMEOUT, "DNS deadline exceeded"),
                mpsc::RecvTimeoutError::Disconnected => code(codes::DNS_FAIL, "DNS worker stopped"),
            });
        remaining(deadline)?;
        result?
    }
}

/// One process-level worker and a bounded queue. getaddrinfo itself cannot be
/// cancelled: a stuck call occupies this worker, never the caller or a new thread.
pub fn resolve_ipv4(host: &str, port: u16, deadline: Instant) -> DnsResult {
    remaining(deadline)?;
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return Ok(vec![SocketAddr::from((ip, port))]);
    }
    static WORKER: OnceLock<Result<DnsWorker, String>> = OnceLock::new();
    let worker = WORKER
        .get_or_init(|| DnsWorker::start(system_resolve_ipv4).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| code(codes::DNS_FAIL, format!("DNS worker: {e}")))?;
    worker.resolve(host, port, deadline)
}

/// getaddrinfo(AF_INET, SOCK_STREAM), with the requested port applied to each A record.
fn system_resolve_ipv4(host: &str, port: u16) -> DnsResult {
    let c_host = std::ffi::CString::new(host).map_err(|_| code(codes::DNS_FAIL, "empty host"))?;
    let hints = libc::addrinfo {
        ai_flags: 0,
        ai_family: libc::AF_INET,
        ai_socktype: libc::SOCK_STREAM,
        ai_protocol: libc::IPPROTO_TCP,
        ai_addrlen: 0,
        ai_addr: std::ptr::null_mut(),
        ai_canonname: std::ptr::null_mut(),
        ai_next: std::ptr::null_mut(),
    };
    let mut res: *mut libc::addrinfo = std::ptr::null_mut();
    let rc = unsafe { libc::getaddrinfo(c_host.as_ptr(), std::ptr::null(), &hints, &mut res) };
    if rc != 0 || res.is_null() {
        return Err(code(codes::DNS_FAIL, format!("getaddrinfo rc={rc}")));
    }
    let mut out = Vec::new();
    let mut cur = res;
    while !cur.is_null() {
        unsafe {
            let ai = &*cur;
            if ai.ai_family == libc::AF_INET && !ai.ai_addr.is_null() {
                let sa = &*(ai.ai_addr as *const libc::sockaddr_in);
                let ip = std::net::Ipv4Addr::from(u32::from_be(sa.sin_addr.s_addr));
                out.push(SocketAddr::from((ip, port)));
            }
            cur = ai.ai_next;
        }
    }
    unsafe { libc::freeaddrinfo(res) };
    if out.is_empty() {
        return Err(code(codes::DNS_FAIL, "no A record"));
    }
    Ok(out)
}

fn remaining(deadline: Instant) -> Result<Duration, TransportError> {
    let now = Instant::now();
    if now >= deadline {
        Err(code(codes::TIMEOUT, "total deadline exceeded"))
    } else {
        Ok(deadline - now)
    }
}

fn block_budget(deadline: Instant, block: Duration) -> Result<Duration, TransportError> {
    let budget = remaining(deadline)?.min(block);
    if budget.is_zero() {
        return Err(code(codes::TIMEOUT, "zero block budget"));
    }
    Ok(budget)
}

/// Guard socket operations: complete_io can perform arbitrarily many I/Os
/// inside a single handshake or plaintext read.
struct DeadlineStream {
    socket: TcpStream,
    deadline: Instant,
    block: Duration,
}

impl DeadlineStream {
    fn budget(&self) -> io::Result<Duration> {
        block_budget(self.deadline, self.block)
            .map_err(|e| io::Error::new(io::ErrorKind::TimedOut, e.detail().to_string()))
    }
}

impl DeadlineStream {
    // 2026-11-13 (weak-network fix, K4): the Kindle 4's 2.6-era kernel
    // returns EAGAIN (os error 11) instead of ETIMEDOUT when SO_RCVTIMEO
    // expires on a blocking socket, and DeadlineStream.read had no retry
    // loop — every slow read aborted the whole request, producing the EAGAIN
    // storm + crash loop seen on K4 (2026-11-12/14). Retry within the total
    // deadline; only a real deadline expiry surfaces as an error.
    fn retry_on_timeout<T>(
        &mut self,
        mut op: impl FnMut(&mut Self) -> io::Result<T>,
    ) -> io::Result<T> {
        loop {
            match op(self) {
                Ok(v) => return Ok(v),
                Err(e) if is_timeout_err(&e) => {
                    self.budget()?; // deadline exceeded -> real timeout
                    continue; // else: keep waiting (slow delivery)
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl Read for DeadlineStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.retry_on_timeout(|stream| {
            stream.socket.set_read_timeout(Some(stream.budget()?))?;
            stream.socket.read(out)
        })
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.retry_on_timeout(|stream| {
            stream.socket.set_write_timeout(Some(stream.budget()?))?;
            stream.socket.write(buf)
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.retry_on_timeout(|stream| {
            stream.socket.set_write_timeout(Some(stream.budget()?))?;
            stream.socket.flush()
        })
    }
}

fn io_error(e: io::Error, fallback: i32, phase: &str) -> TransportError {
    code(
        if is_timeout_err(&e) {
            codes::TIMEOUT
        } else {
            fallback
        },
        format!("{phase}: {e}"),
    )
}

pub fn perform(req: Request) -> Result<Response, TransportError> {
    let started = Instant::now();
    let target = parse_https_url(req.url).ok_or_else(|| {
        code(
            codes::INVALID_URL,
            format!("not an https url: {:?}", req.url),
        )
    })?;
    let deadline = started
        .checked_add(req.budgets.total_deadline)
        .ok_or_else(|| code(codes::TIMEOUT, "invalid total deadline"))?;
    remaining(deadline)?;
    let block = req.budgets.block_timeout;

    let addrs = resolve_ipv4(&target.host, target.port, deadline)?;
    let mut last_err: Option<TransportError> = None;
    let mut sock = None;
    for addr in &addrs {
        match TcpStream::connect_timeout(addr, block_budget(deadline, block)?) {
            Ok(s) => {
                sock = Some(s);
                break;
            }
            Err(e) => {
                let c = if is_timeout_err(&e) {
                    codes::TIMEOUT
                } else {
                    codes::CONNECT_FAIL
                };
                last_err = Some(code(c, format!("connect {addr}: {e}")));
            }
        }
    }
    let sock = sock.ok_or_else(|| {
        last_err
            .take()
            .unwrap_or_else(|| code(codes::CONNECT_FAIL, "no addresses"))
    })?;
    remaining(deadline)?;
    sock.set_nodelay(true).ok();

    let server_name = tls::server_name(&target.host)
        .map_err(|e| code(codes::HANDSHAKE_FAIL, format!("sni: {e}")))?;
    let config = tls::build_config(req.trust_pem).map_err(|e| match e {
        tls::TrustError::InvalidPem(m) => code(codes::HANDSHAKE_FAIL, format!("trust pem: {m}")),
        tls::TrustError::ProtocolVersions(m) => {
            code(codes::HANDSHAKE_FAIL, format!("protocol versions: {m}"))
        }
    })?;
    let conn = rustls::ClientConnection::new(std::sync::Arc::new(config), server_name)
        .map_err(|e| code(codes::HANDSHAKE_FAIL, format!("conn: {e}")))?;
    let socket = DeadlineStream {
        socket: sock,
        deadline,
        block,
    };
    let mut stream = StreamOwned::new(conn, socket);

    while stream.conn.is_handshaking() {
        remaining(deadline)?;
        if let Err(e) = stream.conn.complete_io(&mut stream.sock) {
            let fallback = if is_cert_error(&e) {
                codes::CERT_VERIFY_FAIL
            } else {
                codes::HANDSHAKE_FAIL
            };
            return Err(io_error(e, fallback, "handshake"));
        }
    }
    remaining(deadline)?;

    let host_header = if target.port == 443 {
        target.host.clone()
    } else {
        format!("{}:{}", target.host, target.port)
    };
    let wire = http::serialize_request(
        req.method,
        &target.path,
        &host_header,
        &req.headers,
        req.body,
    )
    .map_err(|m| code(codes::INVALID_REQUEST, m))?;
    write_all_tls(&mut stream, &wire)?;
    let result = http::read_response(&mut stream, req.method, req.max_response_bytes);
    remaining(deadline)?;
    result.map_err(|e| match e {
        http::ReadError::Io(e) => io_error(e, codes::READ_FAIL, "read"),
        http::ReadError::TooLarge(_) => code(codes::TOO_LARGE, e.to_string()),
        http::ReadError::Invalid(_) => code(codes::READ_FAIL, e.to_string()),
    })
}

fn is_cert_error(e: &io::Error) -> bool {
    matches!(
        e.get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>()),
        Some(rustls::Error::InvalidCertificate(_) | rustls::Error::NoCertificatesPresented)
    )
}

fn write_all_tls(
    stream: &mut StreamOwned<rustls::ClientConnection, DeadlineStream>,
    buf: &[u8],
) -> Result<(), TransportError> {
    remaining(stream.sock.deadline)?;
    stream
        .write_all(buf)
        .map_err(|e| io_error(e, codes::WRITE_FAIL, "write"))?;
    remaining(stream.sock.deadline)?;
    stream
        .flush()
        .map_err(|e| io_error(e, codes::WRITE_FAIL, "flush"))?;
    remaining(stream.sock.deadline)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn regression_dns_timeout_keeps_one_worker_and_bounds_queue() {
        let (release, gate) = mpsc::channel();
        let (events, observed) = mpsc::channel();
        let worker = DnsWorker::start(move |host, port| {
            events
                .send((host.to_string(), std::thread::current().id()))
                .unwrap();
            if host == "slow.test" {
                gate.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))])
        })
        .unwrap();
        let started = Instant::now();
        let err = worker
            .resolve("slow.test", 443, started + Duration::from_millis(30))
            .unwrap_err();
        assert_eq!(err.code(), codes::TIMEOUT);
        assert!(started.elapsed() < Duration::from_millis(200));
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(1)).unwrap().0,
            "slow.test"
        );
        for _ in 0..DNS_QUEUE_CAPACITY {
            let err = worker
                .resolve(
                    "expired.test",
                    443,
                    Instant::now() + Duration::from_millis(2),
                )
                .unwrap_err();
            assert_eq!(err.code(), codes::TIMEOUT);
        }
        // The blocked resolver must not cause a new worker, or an unbounded queue.
        let started = Instant::now();
        let err = worker
            .resolve("full.test", 443, started + Duration::from_secs(1))
            .unwrap_err();
        assert_eq!(err.code(), codes::DNS_FAIL);
        assert!(started.elapsed() < Duration::from_millis(200));
        drop(worker);
        release.send(()).unwrap();
        // Disconnect proves the worker drained its queue and exited without resolving
        // any expired job. A leaked/new worker or an executed job fails this assertion.
        assert!(matches!(
            observed.recv_timeout(Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn regression_dns_worker_reused_and_expired_requests_not_submitted() {
        let (events, observed) = mpsc::channel();
        let worker = DnsWorker::start(move |_, port| {
            events.send(std::thread::current().id()).unwrap();
            Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))])
        })
        .unwrap();
        let err = worker
            .resolve("expired.test", 443, Instant::now())
            .unwrap_err();
        assert_eq!(err.code(), codes::TIMEOUT);
        assert!(observed.try_recv().is_err());
        let mut ids = Vec::new();
        for _ in 0..10 {
            let result = worker
                .resolve("fast.test", 8443, Instant::now() + Duration::from_secs(1))
                .unwrap();
            assert_eq!(result, [SocketAddr::from(([127, 0, 0, 1], 8443))]);
            ids.push(observed.recv_timeout(Duration::from_secs(1)).unwrap());
        }
        assert!(ids.iter().all(|id| *id == ids[0]));
    }

    #[test]
    fn expired_socket_reads_writes_and_flushes_never_restart_block_budget() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        let mut socket = DeadlineStream {
            socket: client,
            deadline: Instant::now(),
            block: Duration::from_secs(10),
        };
        assert_eq!(
            socket.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            socket.write(b"must not be sent").unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(socket.flush().unwrap_err().kind(), io::ErrorKind::TimedOut);
        peer.set_nonblocking(true).unwrap();
        assert_eq!(
            peer.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn expired_block_budget_and_localized_errors_are_timeouts() {
        assert_eq!(
            block_budget(Instant::now(), Duration::from_secs(10))
                .unwrap_err()
                .code(),
            codes::TIMEOUT
        );
        for error in [
            io::Error::from_raw_os_error(libc::EAGAIN),
            io::Error::new(io::ErrorKind::TimedOut, "期限已到"),
            io::Error::new(io::ErrorKind::WouldBlock, "暂不可用"),
        ] {
            assert_eq!(
                io_error(error, codes::READ_FAIL, "read").code(),
                codes::TIMEOUT
            );
        }
    }
}
