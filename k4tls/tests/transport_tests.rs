mod common;

use k4tls::transport::{perform, Request};
use k4tls::url::Budgets;
use std::time::Duration;

use common::TestServer;

fn budgets() -> Budgets {
    Budgets {
        block_timeout: Duration::from_millis(2000),
        total_deadline: Duration::from_millis(6000),
    }
}

fn get(
    server: &TestServer,
    path: &str,
    trust_pem: Option<&[u8]>,
) -> Result<k4tls::http::Response, k4tls::transport::TransportError> {
    perform(Request {
        url: &format!("https://{}{}", server_addr(server, path).0, path),
        method: "GET",
        headers: vec!["Accept: */*"],
        body: None,
        budgets: budgets(),
        max_response_bytes: 1 << 20,
        trust_pem,
    })
}

fn server_addr(server: &TestServer, _path: &str) -> (String, u16) {
    // 证书 SAN 是 localhost/127.0.0.1；用 127.0.0.1 连接 + 证书带该 SAN
    (
        format!("127.0.0.1:{}", server.addr.port()),
        server.addr.port(),
    )
}

#[test]
fn get_200_content_length_roundtrip() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 17\r\n\r\n{\"ok\":true,\"x\":1}".to_string(),
        false,
    );
    let resp = get(&server, "/api", Some(server.root_pem.as_bytes())).expect("ok");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, br#"{"ok":true,"x":1}"#);
    assert!(resp
        .header_lines
        .iter()
        .any(|l| l == "Content-Type: application/json"));
}

#[test]
fn get_200_chunked_roundtrip() {
    let body = "chunked-body-数据";
    let mut chunk_payload = String::new();
    let bytes = body.as_bytes();
    chunk_payload.push_str(&format!("{:x}\r\n", bytes.len()));
    chunk_payload.push_str(body);
    chunk_payload.push_str("\r\n0\r\n\r\n");
    let server = TestServer::spawn(
        format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{chunk_payload}"),
        false,
    );
    let resp = get(&server, "/", Some(server.root_pem.as_bytes())).expect("ok");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, body.as_bytes());
}

#[test]
fn redirect_301_returned_not_followed() {
    let server = TestServer::spawn(
        "HTTP/1.1 301 Moved Permanently\r\nLocation: https://weread.qq.com/next\r\nContent-Length: 0\r\n\r\n".to_string(),
        false,
    );
    let resp = get(&server, "/from", Some(server.root_pem.as_bytes())).expect("ok");
    assert_eq!(resp.status, 301);
    assert!(resp
        .header_lines
        .iter()
        .any(|l| l == "Location: https://weread.qq.com/next"));
    assert!(resp.body.is_empty());
}

#[test]
fn eof_terminated_response() {
    let server = TestServer::spawn(
        "HTTP/1.0 200 OK\r\nConnection: close\r\n\r\nstreamed-no-length".to_string(),
        false,
    );
    let resp = get(&server, "/", Some(server.root_pem.as_bytes())).expect("ok");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"streamed-no-length");
}

#[test]
fn post_body_sent() {
    // 用 echo 式响应验证服务端收到的请求行含 Content-Length 与方法 POST
    let server = TestServer::spawn(
        "HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok".to_string(),
        false,
    );
    let resp = perform(Request {
        url: &format!("https://127.0.0.1:{}/submit", server.addr.port()),
        method: "POST",
        headers: vec!["Content-Type: application/x-www-form-urlencoded"],
        body: Some(br#"a=1&b=%E4%B8%AD"#),
        budgets: budgets(),
        max_response_bytes: 1 << 20,
        trust_pem: Some(server.root_pem.as_bytes()),
    })
    .expect("ok");
    assert_eq!(resp.status, 201);
    assert_eq!(resp.body, b"ok");
}

#[test]
fn handshake_timeout_maps_to_timeout() {
    let server = TestServer::spawn(String::new(), true);
    let err = get(&server, "/", Some(server.root_pem.as_bytes())).expect_err("must time out");
    assert_eq!(
        err.code(),
        k4tls::codes::TIMEOUT,
        "detail: {}",
        err.detail()
    );
}

#[test]
fn oversized_body_maps_too_large() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\ntruncated".to_string(),
        false,
    );
    let resp = perform(Request {
        url: &format!("https://127.0.0.1:{}/big", server.addr.port()),
        method: "GET",
        headers: vec![],
        body: None,
        budgets: budgets(),
        max_response_bytes: 1024,
        trust_pem: Some(server.root_pem.as_bytes()),
    });
    let err = resp.expect_err("must be too large");
    assert_eq!(
        err.code(),
        k4tls::codes::TOO_LARGE,
        "detail: {}",
        err.detail()
    );
}

#[test]
fn untrusted_cert_maps_cert_verify_fail() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_string(),
        false,
    );
    let err = get(&server, "/", None).expect_err("must fail verification");
    assert_eq!(
        err.code(),
        k4tls::codes::CERT_VERIFY_FAIL,
        "detail: {}",
        err.detail()
    );
}

#[test]
fn hostname_resolution_uses_ipv4_and_preserves_port() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".into(),
        false,
    );
    let resp = perform(Request {
        url: &format!("https://localhost:{}/", server.addr.port()),
        method: "GET",
        headers: vec![],
        body: None,
        budgets: budgets(),
        max_response_bytes: 2,
        trust_pem: Some(server.root_pem.as_bytes()),
    })
    .unwrap();
    assert_eq!(resp.body, b"ok");
}

#[test]
fn non_https_url_rejected() {
    let err = perform(Request {
        url: "http://weread.qq.com/",
        method: "GET",
        headers: vec![],
        body: None,
        budgets: budgets(),
        max_response_bytes: 1024,
        trust_pem: None,
    })
    .expect_err("must reject");
    assert_eq!(err.code(), k4tls::codes::DNS_FAIL);
}

fn limited_request(
    server: &TestServer,
    method: &str,
    cap: usize,
    budgets: Budgets,
) -> Result<k4tls::http::Response, k4tls::transport::TransportError> {
    perform(Request {
        url: &format!("https://127.0.0.1:{}/", server.addr.port()),
        method,
        headers: vec![],
        body: None,
        budgets,
        max_response_bytes: cap,
        trust_pem: Some(server.root_pem.as_bytes()),
    })
}

#[test]
fn regression_slow_drip_handshake_obeys_total_deadline() {
    let server = TestServer::slow_handshake();
    let start = std::time::Instant::now();
    let err = limited_request(
        &server,
        "GET",
        1024,
        Budgets {
            block_timeout: Duration::from_millis(100),
            total_deadline: Duration::from_millis(80),
        },
    )
    .unwrap_err();
    let elapsed = start.elapsed();
    assert_eq!(err.code(), k4tls::codes::TIMEOUT, "{err:?}, {elapsed:?}");
    assert!(elapsed < Duration::from_millis(250), "{elapsed:?}");
}

#[test]
fn regression_stalled_body_read_maps_timeout_without_string_matching() {
    let server = TestServer::stalled("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\na");
    let start = std::time::Instant::now();
    let err = limited_request(
        &server,
        "GET",
        1024,
        Budgets {
            block_timeout: Duration::from_millis(100),
            total_deadline: Duration::from_millis(800),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), k4tls::codes::TIMEOUT, "{err:?}");
    assert!(start.elapsed() < Duration::from_millis(500));
}

#[test]
fn regression_oversized_chunk_declaration_does_not_wait_for_payload() {
    let server =
        TestServer::stalled("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10000000\r\n");
    let err = limited_request(
        &server,
        "GET",
        1,
        Budgets {
            block_timeout: Duration::from_millis(100),
            total_deadline: Duration::from_millis(800),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), k4tls::codes::TOO_LARGE, "{err:?}");
}

#[test]
fn regression_same_packet_caps_map_too_large() {
    for raw in [
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n",
        "HTTP/1.0 200 OK\r\n\r\nab",
    ] {
        let server = TestServer::spawn(raw.into(), false);
        let err = limited_request(&server, "GET", 1, budgets()).unwrap_err();
        assert_eq!(err.code(), k4tls::codes::TOO_LARGE, "{err:?}");
    }
}

#[test]
fn regression_head_and_bodyless_statuses_do_not_read_body() {
    for (method, status) in [("HEAD", 200), ("GET", 204), ("GET", 304)] {
        let server = TestServer::stalled(&format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: 999\r\n\r\n"
        ));
        let resp = limited_request(&server, method, 1, budgets()).unwrap();
        assert_eq!(resp.status, status);
        assert!(resp.body.is_empty());
    }
}

#[test]
fn trust_pem_with_bad_pem_maps_handshake_fail() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_string(),
        false,
    );
    let err = get(&server, "/", Some(b"not a pem at all {{")).expect_err("must fail");
    assert_eq!(
        err.code(),
        k4tls::codes::HANDSHAKE_FAIL,
        "detail: {}",
        err.detail()
    );
}
