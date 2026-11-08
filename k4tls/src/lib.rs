//! k4tls — Kindle 4 weread TLS 传输组件（C ABI，LuaJIT FFI 加载）。
//!
//! 单请求同步语义：一次 perform = DNS(IPv4) + TCP + TLS + HTTP/1.1 收发。
//! 重定向/cookie/超时策略归 Lua 侧（client.lua），与 socketutil 语义对齐：
//! block_timeout = 单 socket 操作预算；total_timeout = 整请求硬期限。

pub mod codes;
pub mod http;
pub mod tls;
pub mod transport;
pub mod url;

use std::panic::{catch_unwind, AssertUnwindSafe};

pub const ERR_INTERNAL: i32 = -99;
pub const ERR_INVALID_ARGS: i32 = -1;

#[repr(C)]
pub struct K4TlsRequest {
    pub url: *const libc::c_char,
    pub method: *const libc::c_char,
    pub headers: *const *const libc::c_char,
    pub nheaders: libc::c_int,
    pub body: *const libc::c_uchar,
    pub body_len: libc::size_t,
    pub block_timeout_ms: libc::c_int,
    pub total_timeout_ms: libc::c_int,
    pub max_response_bytes: libc::size_t,
    pub trust_pem: *const libc::c_uchar,
    pub trust_pem_len: libc::size_t,
}

#[repr(C)]
pub struct K4TlsResponse {
    /// HTTP 状态码（1xx-5xx）；status<0 时为 codes.rs 传输错误
    pub status: libc::c_int,
    /// "Name: value\r\n..." NUL 结尾，保留大小写；k4tls_free_response 归还
    pub headers_blob: *mut libc::c_char,
    pub header_count: libc::size_t,
    pub body: *mut libc::c_uchar,
    pub body_len: libc::size_t,
    /// 返回码 != 0 或 status<0 时有内容
    pub err: [libc::c_char; 256],
}

impl K4TlsResponse {
    fn zeroed() -> Self {
        K4TlsResponse {
            status: 0,
            headers_blob: std::ptr::null_mut(),
            header_count: 0,
            body: std::ptr::null_mut(),
            body_len: 0,
            err: [0; 256],
        }
    }
}

fn set_err(out: &mut K4TlsResponse, msg: &str) {
    let bytes = msg.as_bytes();
    let n = bytes.len().min(out.err.len() - 1);
    let dst = &mut out.err[..n];
    for (i, b) in bytes[..n].iter().enumerate() {
        dst[i] = *b as libc::c_char;
    }
    out.err[n] = 0;
}

fn boxed_cstring(s: &str) -> *mut libc::c_char {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend_from_slice(s.as_bytes());
    v.push(0);
    Box::leak(v.into_boxed_slice()).as_mut_ptr() as *mut libc::c_char
}

fn boxed_bytes(b: Vec<u8>) -> *mut libc::c_uchar {
    Box::leak(b.into_boxed_slice()).as_mut_ptr()
}

enum Failure {
    /// C ABI 返回 -1，不产生响应字段
    Args(String),
    /// C ABI 返回 0，status<0
    Transport(transport::TransportError),
}

unsafe fn cstr_arg(p: *const libc::c_char) -> Option<&'static str> {
    if p.is_null() {
        return None;
    }
    std::ffi::CStr::from_ptr(p).to_str().ok()
}

#[no_mangle]
pub extern "C" fn k4tls_version() -> *const libc::c_char {
    concat!("k4tls ", env!("CARGO_PKG_VERSION"), " rustls-0.23/ring\0").as_ptr()
        as *const libc::c_char
}

/// 返回 0：out 已填充（HTTP 状态或负值传输错误）；-1：参数非法（仅 out.err）；
/// -99：内部 panic（out 已清零，无指针可释放）。
///
/// # Safety
///
/// - `req` 与 `out` 必须非空且分别可读/可写一个完整的 `K4TlsRequest`/`K4TlsResponse`
/// - `req` 内所有指针字段（url/method/headers/body/trust_pem）须指向有效内存，
///   生命周期覆盖本调用；返回 0 后必须用 [`k4tls_free_response`] 回收 `out`
/// - 返回 -99 后不得调用 `k4tls_free_response`
/// - 本函数不得从信号处理器/多个线程同时写同一 `out` 调用
#[no_mangle]
pub unsafe extern "C" fn k4tls_perform(req: *const K4TlsRequest, out: *mut K4TlsResponse) -> libc::c_int {
    if req.is_null() || out.is_null() {
        return ERR_INVALID_ARGS;
    }
    let result = catch_unwind(AssertUnwindSafe(|| perform_inner(unsafe { &*req })));
    let out_ref = unsafe { &mut *out };
    *out_ref = K4TlsResponse::zeroed();
    match result {
        Ok(Ok(resp)) => {
            out_ref.status = resp.status as libc::c_int;
            let blob = resp.header_lines.join("\r\n");
            if !blob.is_empty() {
                out_ref.headers_blob = boxed_cstring(&blob);
                out_ref.header_count = resp.header_lines.len();
            }
            if !resp.body.is_empty() {
                out_ref.body_len = resp.body.len();
                out_ref.body = boxed_bytes(resp.body);
            }
            0
        }
        Ok(Err(Failure::Transport(te))) => {
            out_ref.status = te.code();
            set_err(
                out_ref,
                &format!("{}: {}", codes::describe(te.code()), te.detail()),
            );
            0
        }
        Ok(Err(Failure::Args(m))) => {
            set_err(out_ref, &m);
            ERR_INVALID_ARGS
        }
        Err(_) => {
            out_ref.status = ERR_INTERNAL;
            set_err(out_ref, "internal panic (caught at ffi boundary)");
            ERR_INTERNAL
        }
    }
}

fn perform_inner(req: &K4TlsRequest) -> Result<http::Response, Failure> {
    if req.block_timeout_ms <= 0 || req.total_timeout_ms <= 0 || req.max_response_bytes == 0 {
        return Err(Failure::Args(
            "invalid args: block/total timeout must be > 0 and max_response_bytes != 0".into(),
        ));
    }
    let url =
        unsafe { cstr_arg(req.url) }.ok_or_else(|| Failure::Args("invalid args: url".into()))?;
    let method = unsafe { cstr_arg(req.method) }
        .ok_or_else(|| Failure::Args("invalid args: method".into()))?;
    if req.nheaders < 0 {
        return Err(Failure::Args("invalid args: nheaders".into()));
    }
    if req.nheaders > 0 && req.headers.is_null() {
        return Err(Failure::Args("invalid args: headers".into()));
    }
    if req.body.is_null() != (req.body_len == 0) {
        return Err(Failure::Args("invalid args: body/body_len".into()));
    }
    let mut headers: Vec<&str> = Vec::with_capacity(req.nheaders as usize);
    for i in 0..req.nheaders as usize {
        let h = unsafe { cstr_arg(*req.headers.add(i)) }
            .ok_or_else(|| Failure::Args("invalid args: header".into()))?;
        headers.push(h);
    }
    let body: Option<&[u8]> = if req.body.is_null() {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(req.body, req.body_len) })
    };
    let trust_pem: Option<&[u8]> = if req.trust_pem.is_null() || req.trust_pem_len == 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(req.trust_pem, req.trust_pem_len) })
    };

    let rreq = transport::Request {
        url,
        method,
        headers,
        body,
        budgets: url::Budgets {
            block_timeout: std::time::Duration::from_millis(req.block_timeout_ms as u64),
            total_deadline: std::time::Duration::from_millis(req.total_timeout_ms as u64),
        },
        max_response_bytes: req.max_response_bytes,
        trust_pem,
    };
    transport::perform(rreq).map_err(Failure::Transport)
}

/// 回收 k4tls_perform 成功填充的响应内存（返回 0 时，无论 status 正负）。
/// 返回 -99 的调用后不得调用（无指针产生）。
///
/// # Safety
///
/// `out` 必须是此前 k4tls_perform 返回 0 时填充的同一指针（或为空），
/// 且其 response 内存未被释放过。
#[no_mangle]
pub unsafe extern "C" fn k4tls_free_response(out: *mut K4TlsResponse) {
    if out.is_null() {
        return;
    }
    let r = &mut *out;
    if !r.headers_blob.is_null() {
        let n = blob_len(r.headers_blob);
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            r.headers_blob as *mut u8,
            n,
        )));
        r.headers_blob = std::ptr::null_mut();
        r.header_count = 0;
    }
    if !r.body.is_null() && r.body_len > 0 {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            r.body, r.body_len,
        )));
        r.body = std::ptr::null_mut();
        r.body_len = 0;
    }
}

/// boxed_cstring 泄漏的是 len+1 的 boxed slice（含 NUL），按实际长度重建。
unsafe fn blob_len(p: *const libc::c_char) -> usize {
    let mut n = 0usize;
    loop {
        let b = *p.add(n) as u8;
        if b == 0 {
            return n + 1;
        }
        n += 1;
    }
}
