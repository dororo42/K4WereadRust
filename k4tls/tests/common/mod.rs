//! hermetic TLS 测试服务器：rcgen 自签 + rustls ServerConnection，单连接服务。
//! 每个测试 spawn 一个实例，端口随机，证书 PEM 供客户端 trust_pem 注入。

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;

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
            if hold {
                // 不握手不响应：供客户端超时路径测试
                std::thread::sleep(std::time::Duration::from_secs(30));
                drop(sock);
                return;
            }
            serve_one(sock, &cert_der, &key, &response);
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
    // close_notify：让 EOF-终止式响应的客户端读到干净的流结束
    stream.conn.send_close_notify();
    let _ = stream.conn.complete_io(&mut stream.sock);
    let _ = stream.write_all(b"");
}
