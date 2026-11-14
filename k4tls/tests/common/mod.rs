//! hermetic TLS 测试服务器：rcgen 自签 + rustls ServerConnection，单连接服务。
//! 每个测试 spawn 一个实例，端口随机，证书 PEM 供客户端 trust_pem 注入。

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rustls::ServerConnection;
use rustls::StreamOwned;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

pub struct TestServer {
    pub addr: SocketAddr,
    pub root_pem: String,
    _handle: JoinHandle<()>,
}

impl TestServer {
    pub fn spawn(response: String, hold: bool) -> TestServer {
        Self::spawn_mode(response, hold, false, false)
    }

    #[allow(dead_code)] // common is compiled separately by each integration test
    pub fn stalled(response: &str) -> TestServer {
        Self::spawn_mode(response.to_string(), false, false, true)
    }

    #[allow(dead_code)]
    pub fn slow_handshake() -> TestServer {
        Self::spawn_mode(String::new(), false, true, false)
    }

    /// 2026-11-13 (weak-network fix): TLS handshake completes normally, then
    /// the response body arrives in slow chunks (interval > client block
    /// timeout) — the client read must keep retrying until the total deadline
    /// instead of aborting on the first timeout-class error.
    #[allow(dead_code)] // only used by transport_tests; other test targets include this module
    pub fn spawn_chunked(chunks: Vec<String>, interval: Duration) -> TestServer {
        Self::spawn_chunked_impl(chunks, interval)
    }

    #[allow(dead_code)]
    fn spawn_chunked_impl(chunks: Vec<String>, interval: Duration) -> TestServer {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
            .expect("rcgen cert");
        let root_pem = cert.cert.pem();
        let cert_der = CertificateDer::from(cert.cert);
        let key = PrivateKeyDer::Pkcs8(rustls_pki_types::PrivatePkcs8KeyDer::from(
            cert.key_pair.serialize_der(),
        ));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let Ok((sock, _)) = listener.accept() else {
                return;
            };
            sock.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            sock.set_write_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let config = rustls::ServerConfig::builder_with_provider(
                rustls::crypto::ring::default_provider().into(),
            )
            .with_safe_default_protocol_versions()
            .expect("server protocol versions")
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key)
            .expect("server cert");
            let conn = ServerConnection::new(Arc::new(config)).expect("server conn");
            let mut stream = StreamOwned::new(conn, sock);
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            for chunk in &chunks {
                let _ = stream.write_all(chunk.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(interval);
            }
            stream.conn.send_close_notify();
            let _ = stream.conn.complete_io(&mut stream.sock);
        });
        TestServer {
            addr,
            root_pem,
            _handle: handle,
        }
    }

    fn spawn_mode(response: String, hold: bool, drip: bool, stall: bool) -> TestServer {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
            .expect("rcgen cert");
        let root_pem = cert.cert.pem();
        let cert_der = CertificateDer::from(cert.cert);
        let key = PrivateKeyDer::Pkcs8(rustls_pki_types::PrivatePkcs8KeyDer::from(
            cert.key_pair.serialize_der(),
        ));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            sock.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            sock.set_write_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            if hold {
                let mut buf = [0; 8192];
                while matches!(sock.read(&mut buf), Ok(n) if n > 0) {}
                // Hold means "never respond AND never close": closing here
                // surfaces to the client as EOF (HANDSHAKE_FAIL), not a
                // timeout, so the client can never reach its own deadline.
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            }
            if drip {
                // A fragmented, incomplete TLS handshake record: every read makes progress,
                // but the total budget must still expire inside rustls::complete_io.
                let _ = sock.write_all(&[22, 3, 3, 0x10, 0]);
                for _ in 0..25 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    if sock.write_all(&[0]).is_err() {
                        break;
                    }
                }
                return;
            }
            serve_one(sock, &cert_der, &key, &response, stall);
        });
        TestServer {
            addr,
            root_pem,
            _handle: handle,
        }
    }
}

fn serve_one(
    sock: TcpStream,
    cert: &CertificateDer<'static>,
    key: &PrivateKeyDer<'static>,
    response: &str,
    stall: bool,
) {
    let config = rustls::ServerConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .with_no_client_auth()
    .with_single_cert(vec![cert.clone()], key.clone_key())
    .expect("server cert");
    let conn = ServerConnection::new(Arc::new(config)).expect("server conn");
    let mut stream = StreamOwned::new(conn, sock);
    let mut buf = [0u8; 8192];
    let _ = stream.read(&mut buf); // 请求很小，一次读足够；失败也让响应继续
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    if stall {
        let _ = stream.read(&mut buf); // Wait for client timeout/close, without closing the response.
        return;
    }
    // close_notify：让 EOF-终止式响应的客户端读到干净的流结束
    stream.conn.send_close_notify();
    let _ = stream.conn.complete_io(&mut stream.sock);
    let _ = stream.write_all(b"");
}
