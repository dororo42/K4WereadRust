//! 传输层错误码（负值走 k4tls_response.status）。
//!
//! 语义对齐 client.lua/socketutil 的故障面：Lua 侧据此区分可重试的
//! 瞬态错误（超时/连接）与确定失败（证书/解析），与现有错误分类保持同构。
//!
//! -18/-19 是确定性参数错误：其 Lua 文本不得命中 plugin_util 的
//! TRANSIENT_TRANSPORT_PATTERNS 或 ipv4_dns 的 RESOLUTION_ERROR_PATTERNS，
//! 否则会被 client 误判成可重试的 DNS/瞬态故障。

pub const DNS_FAIL: i32 = -10;
pub const CONNECT_FAIL: i32 = -11;
pub const HANDSHAKE_FAIL: i32 = -12;
pub const TIMEOUT: i32 = -13;
pub const WRITE_FAIL: i32 = -14;
pub const READ_FAIL: i32 = -15;
pub const TOO_LARGE: i32 = -16;
pub const CERT_VERIFY_FAIL: i32 = -17;
pub const INVALID_URL: i32 = -18;
pub const INVALID_REQUEST: i32 = -19;

pub fn describe(code: i32) -> &'static str {
    match code {
        DNS_FAIL => "dns resolution failed (ipv4)",
        CONNECT_FAIL => "tcp connect failed",
        HANDSHAKE_FAIL => "tls handshake failed",
        TIMEOUT => "timeout (connect/handshake/total)",
        WRITE_FAIL => "request write failed",
        READ_FAIL => "response read failed",
        TOO_LARGE => "response exceeds max_response_bytes",
        CERT_VERIFY_FAIL => "tls certificate verification failed",
        INVALID_URL => "invalid or unsupported https url",
        INVALID_REQUEST => "invalid request (method/headers/target)",
        _ => "unknown transport error",
    }
}
