//! FFI 层测试：C 结构直接驱动 k4tls_perform / k4tls_free_response。
//! 校验：参数非法返回 -1、成功路径内存回收、响应字段填充。

use std::ffi::CString;

use k4tls::{K4TlsRequest, K4TlsResponse};

mod common;
use common::TestServer;

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn call(req: &K4TlsRequest) -> (i32, K4TlsResponse) {
    let mut resp: K4TlsResponse = unsafe { std::mem::zeroed() };
    // Safety: resp 栈上有效；req 字段由各测试构造并保证覆盖本调用
    let rc = unsafe { k4tls::k4tls_perform(req, &mut resp) };
    (rc, resp)
}

fn err_text(resp: &K4TlsResponse) -> String {
    unsafe { std::ffi::CStr::from_ptr(resp.err.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

#[test]
fn invalid_args_return_minus_one() {
    let url = cstring("https://127.0.0.1:1/");
    let method = cstring("GET");

    let req = K4TlsRequest {
        url: url.as_ptr(),
        method: method.as_ptr(),
        headers: std::ptr::null(),
        nheaders: 0,
        body: std::ptr::null(),
        body_len: 0,
        block_timeout_ms: 0, // 非法：必须 > 0
        total_timeout_ms: 5000,
        max_response_bytes: 4096,
        trust_pem: std::ptr::null(),
        trust_pem_len: 0,
    };
    let (rc, resp) = call(&req);
    assert_eq!(rc, k4tls::ERR_INVALID_ARGS);
    assert!(
        err_text(&resp).contains("timeout"),
        "err: {}",
        err_text(&resp)
    );

    let req = K4TlsRequest {
        url: std::ptr::null(),
        headers: std::ptr::null(),
        nheaders: 0,
        body: std::ptr::null(),
        body_len: 0,
        block_timeout_ms: 1000,
        total_timeout_ms: 5000,
        max_response_bytes: 4096,
        trust_pem: std::ptr::null(),
        trust_pem_len: 0,
        method: method.as_ptr(),
    };
    let (rc, resp) = call(&req);
    assert_eq!(rc, k4tls::ERR_INVALID_ARGS);
    assert!(err_text(&resp).contains("url"), "err: {}", err_text(&resp));
    let _ = url;
}

#[test]
fn null_pointers_rejected() {
    let mut resp: K4TlsResponse = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { k4tls::k4tls_perform(std::ptr::null(), &mut resp) },
        k4tls::ERR_INVALID_ARGS
    );
    assert_eq!(
        unsafe { k4tls::k4tls_perform(std::ptr::null(), std::ptr::null_mut()) },
        k4tls::ERR_INVALID_ARGS
    );
}

#[test]
fn version_string_nonempty() {
    let s = unsafe { std::ffi::CStr::from_ptr(k4tls::k4tls_version()) };
    let s = s.to_string_lossy();
    assert!(s.starts_with("k4tls "), "version: {s}");
    assert!(s.contains("rustls"), "version: {s}");
}

#[test]
fn full_roundtrip_via_ffi_with_free() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nX-K4tls: yes\r\nContent-Length: 4\r\n\r\nbody".to_string(),
        false,
    );
    let url = cstring(&format!("https://127.0.0.1:{}/x", server.addr.port()));
    let method = cstring("GET");
    let header = cstring("Accept: application/json");
    let headers = [header.as_ptr()];
    let trust = server.root_pem.clone();
    let req = K4TlsRequest {
        url: url.as_ptr(),
        method: method.as_ptr(),
        headers: headers.as_ptr(),
        nheaders: 1,
        body: std::ptr::null(),
        body_len: 0,
        block_timeout_ms: 2000,
        total_timeout_ms: 6000,
        max_response_bytes: 65536,
        trust_pem: trust.as_ptr(),
        trust_pem_len: trust.len(),
    };
    let (rc, mut resp) = call(&req);
    assert_eq!(rc, 0, "err: {}", err_text(&resp));
    assert_eq!(resp.status, 200, "err: {}", err_text(&resp));
    assert_eq!(resp.header_count, 2);
    let blob = unsafe { std::ffi::CStr::from_ptr(resp.headers_blob) }
        .to_string_lossy()
        .into_owned();
    assert!(blob.contains("X-K4tls: yes"));
    assert_eq!(resp.body_len, 4);
    let body = unsafe { std::slice::from_raw_parts(resp.body, resp.body_len) };
    assert_eq!(body, b"body");
    unsafe { k4tls::k4tls_free_response(&mut resp) };
    assert!(resp.headers_blob.is_null());
    assert!(resp.body.is_null());
}

#[test]
fn transport_error_via_ffi() {
    // 无人监听的端口 → connect fail
    let url = cstring("https://127.0.0.1:9/");
    let method = cstring("GET");
    let req = K4TlsRequest {
        url: url.as_ptr(),
        method: method.as_ptr(),
        headers: std::ptr::null(),
        nheaders: 0,
        body: std::ptr::null(),
        body_len: 0,
        block_timeout_ms: 500,
        total_timeout_ms: 2000,
        max_response_bytes: 4096,
        trust_pem: std::ptr::null(),
        trust_pem_len: 0,
    };
    let (rc, resp) = call(&req);
    assert_eq!(rc, 0);
    assert_eq!(
        resp.status,
        k4tls::codes::CONNECT_FAIL,
        "err: {}",
        err_text(&resp)
    );
    assert!(
        err_text(&resp).contains("tcp connect"),
        "err: {}",
        err_text(&resp)
    );
}

#[test]
fn regression_nul_header_rejected_before_ffi_allocation() {
    let server = TestServer::spawn(
        "HTTP/1.1 200 OK\r\nX-Test: before\0after\r\nContent-Length: 0\r\n\r\n".into(),
        false,
    );
    let url = cstring(&format!("https://127.0.0.1:{}/", server.addr.port()));
    let method = cstring("GET");
    let req = K4TlsRequest {
        url: url.as_ptr(),
        method: method.as_ptr(),
        headers: std::ptr::null(),
        nheaders: 0,
        body: std::ptr::null(),
        body_len: 0,
        block_timeout_ms: 1000,
        total_timeout_ms: 3000,
        max_response_bytes: 4096,
        trust_pem: server.root_pem.as_ptr(),
        trust_pem_len: server.root_pem.len(),
    };
    let (rc, mut resp) = call(&req);
    assert_eq!(rc, 0);
    // Assert before free: the buggy implementation must not reach its mismatched deallocation.
    assert_eq!(resp.status, k4tls::codes::READ_FAIL, "{}", err_text(&resp));
    assert!(resp.headers_blob.is_null());
    assert!(resp.body.is_null());
    unsafe { k4tls::k4tls_free_response(&mut resp) };
}

#[test]
fn response_framing_limits_and_timeout_codes_survive_ffi() {
    for (method, wire, expected) in [
        (
            "GET",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n",
            k4tls::codes::TOO_LARGE,
        ),
        ("GET", "HTTP/1.0 200 OK\r\n\r\nab", k4tls::codes::TOO_LARGE),
        (
            "HEAD",
            "HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n",
            200,
        ),
        ("GET", "HTTP/1.1 204 No Content\r\n\r\n", 204),
        (
            "GET",
            "HTTP/1.1 304 Not Modified\r\nContent-Length: 999\r\n\r\n",
            304,
        ),
        (
            "GET",
            "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
            200,
        ),
        (
            "GET",
            "HTTP/1.1 101 Switching Protocols\r\n\r\n",
            k4tls::codes::READ_FAIL,
        ),
        (
            "GET",
            "HTTP/1.1 200 OK\r\nContent-Length: invalid\r\n\r\n",
            k4tls::codes::READ_FAIL,
        ),
        (
            "GET",
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n",
            k4tls::codes::TIMEOUT,
        ),
    ] {
        let server = TestServer::stalled(wire);
        let url = cstring(&format!("https://127.0.0.1:{}/", server.addr.port()));
        let method = cstring(method);
        let req = K4TlsRequest {
            url: url.as_ptr(),
            method: method.as_ptr(),
            headers: std::ptr::null(),
            nheaders: 0,
            body: std::ptr::null(),
            body_len: 0,
            block_timeout_ms: 100,
            total_timeout_ms: 800,
            max_response_bytes: 1,
            trust_pem: server.root_pem.as_ptr(),
            trust_pem_len: server.root_pem.len(),
        };
        let (rc, mut resp) = call(&req);
        assert_eq!(rc, 0);
        assert_eq!(resp.status, expected, "{wire:?}: {}", err_text(&resp));
        assert_eq!(resp.body_len, 0);
        unsafe { k4tls::k4tls_free_response(&mut resp) };
        assert!(resp.body.is_null());
        assert!(resp.headers_blob.is_null());
    }
}

#[test]
fn free_response_is_noop_safe() {
    let mut resp: K4TlsResponse = unsafe { std::mem::zeroed() };
    unsafe { k4tls::k4tls_free_response(&mut resp) };
    unsafe { k4tls::k4tls_free_response(std::ptr::null_mut()) };
}
