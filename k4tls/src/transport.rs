//! 传输编排：IPv4 DNS（对齐 ipv4_dns 补丁语义，AF_INET only）→ TCP
//! connect_timeout → TLS 握手 → 请求写 → 响应读。全程受 block/total 预算约束。

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
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

/// getaddrinfo(AF_INET, SOCK_STREAM)。IPv4-only：K4 无 IPv6 栈，
/// 与 weread ipv4_dns 补丁同语义（AAAA 记录不参与）。
/// DNS 不提供端口：返回地址统一替换为目标端口。
pub fn resolve_ipv4(host: &str, port: u16) -> Result<Vec<SocketAddr>, TransportError> {
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

fn block_budget(deadline: Instant, block: Duration) -> Duration {
    remaining(deadline).map(|r| r.min(block)).unwrap_or(block)
}

pub fn perform(req: Request) -> Result<Response, TransportError> {
    let target = parse_https_url(req.url)
        .ok_or_else(|| code(codes::DNS_FAIL, format!("not an https url: {:?}", req.url)))?;
    let started = Instant::now();
    let deadline = started + req.budgets.total_deadline;
    let block = req.budgets.block_timeout;

    let addrs = resolve_ipv4(&target.host, target.port)?;
    let mut last_err: Option<TransportError> = None;
    let mut sock = None;
    for addr in &addrs {
        match TcpStream::connect_timeout(addr, block_budget(deadline, block)) {
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
    let budget = block_budget(deadline, block);
    sock.set_read_timeout(Some(budget))
        .map_err(|e| code(codes::CONNECT_FAIL, e.to_string()))?;
    sock.set_write_timeout(Some(budget))
        .map_err(|e| code(codes::CONNECT_FAIL, e.to_string()))?;
    sock.set_nodelay(true).ok();

    let server_name = tls::server_name(&target.host)
        .map_err(|e| code(codes::HANDSHAKE_FAIL, format!("sni: {e}")))?;
    let config = tls::build_config(req.trust_pem).map_err(|e| match e {
        tls::TrustError::InvalidPem(m) => code(codes::HANDSHAKE_FAIL, format!("trust pem: {m}")),
    })?;
    let conn = rustls::ClientConnection::new(std::sync::Arc::new(config), server_name)
        .map_err(|e| code(codes::HANDSHAKE_FAIL, format!("conn: {e}")))?;
    let mut stream = StreamOwned::new(conn, sock);

    // 握手（StreamOwned 惰性；显式 complete_io 让超时归类 HANDSHAKE/TIMEOUT）
    loop {
        match stream.conn.complete_io(&mut stream.sock) {
            Ok(_) if stream.conn.is_handshaking() => {
                remaining(deadline)?;
                continue;
            }
            Ok(_) => break,
            Err(e) => {
                let c = if is_timeout_err(&e) {
                    codes::TIMEOUT
                } else if is_cert_error(&e) {
                    codes::CERT_VERIFY_FAIL
                } else {
                    codes::HANDSHAKE_FAIL
                };
                return Err(code(c, format!("handshake: {e}")));
            }
        }
    }

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
    .map_err(|m| code(codes::WRITE_FAIL, m))?;
    write_all_tls(&mut stream, &wire, deadline, block)?;

    let resp = read_all_tls(&mut stream, req.max_response_bytes, deadline, block).map_err(|m| {
        if m.contains("body too large") {
            code(codes::TOO_LARGE, m)
        } else if m.contains("read: ") || m.contains("connection closed") {
            // 底层超时以 io 错误上浮时同样落 TIMEOUT
            if m.contains("timed out") || m.contains("WouldBlock") || m.contains("would block") {
                code(codes::TIMEOUT, m)
            } else {
                code(codes::READ_FAIL, m)
            }
        } else {
            code(codes::READ_FAIL, m)
        }
    })?;
    Ok(resp)
}

fn is_cert_error(e: &io::Error) -> bool {
    let s = e.to_string();
    s.contains("certificate")
        || s.contains("unknown CA")
        || s.contains("AlertReceived(Description(CertificateUnknown")
        || s.to_string().contains("InvalidCertificate")
}

fn write_all_tls(
    stream: &mut StreamOwned<rustls::ClientConnection, TcpStream>,
    buf: &[u8],
    deadline: Instant,
    block: Duration,
) -> Result<(), TransportError> {
    let mut off = 0;
    while off < buf.len() {
        remaining(deadline)?;
        let budget = block_budget(deadline, block);
        stream
            .sock
            .set_write_timeout(Some(budget))
            .map_err(|e| code(codes::WRITE_FAIL, e.to_string()))?;
        match stream.write(&buf[off..]) {
            Ok(0) => return Err(code(codes::WRITE_FAIL, "zero write")),
            Ok(n) => off += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if is_timeout_err(&e) => {
                return Err(code(codes::TIMEOUT, format!("write: {e}")))
            }
            Err(e) => return Err(code(codes::WRITE_FAIL, format!("write: {e}"))),
        }
    }
    // 刷出 TLS 记录
    loop {
        remaining(deadline)?;
        match stream.conn.complete_io(&mut stream.sock) {
            Ok(_) if stream.conn.wants_write() => continue,
            Ok(_) => break,
            Err(e) if is_timeout_err(&e) => {
                return Err(code(codes::TIMEOUT, format!("flush: {e}")))
            }
            Err(e) => return Err(code(codes::WRITE_FAIL, format!("flush: {e}"))),
        }
    }
    Ok(())
}

fn read_all_tls(
    stream: &mut StreamOwned<rustls::ClientConnection, TcpStream>,
    max_bytes: usize,
    deadline: Instant,
    block: Duration,
) -> Result<Response, String> {
    struct Guard<'a> {
        stream: &'a mut StreamOwned<rustls::ClientConnection, TcpStream>,
        deadline: Instant,
        block: Duration,
    }
    impl Read for Guard<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "total deadline exceeded",
                ));
            }
            let budget = self
                .block
                .min(self.deadline.saturating_duration_since(Instant::now()));
            let _ = self.stream.sock.set_read_timeout(Some(budget));
            self.stream.read(out)
        }
    }
    let mut guard = Guard {
        stream,
        deadline,
        block,
    };
    http::read_response(&mut guard, max_bytes)
}
