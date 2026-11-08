//! HTTP/1.1 报文层：请求序列化 + 响应解析（content-length / chunked / EOF 终止）。
//! 语义对齐 LuaSocket socket.http：每请求新建连接、Connection: close、
//! 不注入 User-Agent、不自动跟随重定向（3xx 原样返回给 Lua 侧处理）。

use std::io::{Read, Write};

pub const MAX_HEADER_BLOCK: usize = 64 * 1024;

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    /// 原始 "Name: value" 行（保留大小写），按出现顺序
    pub header_lines: Vec<String>,
    pub body: Vec<u8>,
}

/// 序列化请求。headers 为 "Name: value" 字符串切片。
/// Host/Content-Length/Connection 由本函数统一负责，调用方重复提供则拒绝。
pub fn serialize_request(
    method: &str,
    target_path: &str,
    host_header: &str,
    headers: &[&str],
    body: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    if !method.bytes().all(|b| b.is_ascii_uppercase()) || method.is_empty() || method.len() > 16 {
        return Err(format!("invalid method: {method:?}"));
    }
    for h in headers {
        let bytes = h.as_bytes();
        if !bytes.contains(&b':') {
            return Err(format!("header missing colon: {h:?}"));
        }
        let name = h.split(':').next().unwrap_or("");
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(format!("invalid header name: {h:?}"));
        }
        let lname = name.to_ascii_lowercase();
        if matches!(
            lname.as_str(),
            "host" | "content-length" | "connection" | "transfer-encoding"
        ) {
            return Err(format!("reserved header provided by caller: {name}"));
        }
        if h.contains('\r') || h.contains('\n') {
            return Err("header injection (CR/LF)".to_string());
        }
    }
    if target_path.contains(' ') || target_path.contains('\r') || target_path.contains('\n') {
        return Err("invalid request target".to_string());
    }

    let mut buf = Vec::with_capacity(256 + headers.len() * 32 + body.map_or(0, |b| b.len()));
    buf.extend_from_slice(format!("{method} {target_path} HTTP/1.1\r\n").as_bytes());
    buf.extend_from_slice(format!("Host: {host_header}\r\n").as_bytes());
    for h in headers {
        buf.extend_from_slice(h.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    if let Some(b) = body {
        buf.extend_from_slice(format!("Content-Length: {}\r\n", b.len()).as_bytes());
    }
    buf.extend_from_slice(b"Connection: close\r\n\r\n");
    if let Some(b) = body {
        buf.extend_from_slice(b);
    }
    Ok(buf)
}

/// 从 TLS 流读响应。`max_bytes` 上限约束 header 块与 body 总量。
pub fn read_response<S: Read>(stream: &mut S, max_bytes: usize) -> Result<Response, String> {
    let mut head = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    let head_end;
    loop {
        if let Some(pos) = find_head_end(&head) {
            head_end = pos;
            break;
        }
        if head.len() > MAX_HEADER_BLOCK {
            return Err("header block too large".to_string());
        }
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("connection closed before response head".to_string());
        }
        head.extend_from_slice(&chunk[..n]);
    }
    let (status, header_lines) = parse_head(&head[..head_end])?;
    let rest = &head[head_end + 4..];
    let mut body = Vec::new();
    let te_chunked = header_lines.iter().any(|l| {
        l.split_once(':').is_some_and(|(n, v)| {
            n.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        })
    });
    let content_length = header_lines.iter().find_map(|l| {
        let (n, v) = l.split_once(':')?;
        if n.trim().eq_ignore_ascii_case("content-length") {
            v.trim().parse::<usize>().ok()
        } else {
            None
        }
    });

    if te_chunked {
        let mut leftover = rest.to_vec();
        loop {
            match decode_chunk_step(&mut body, &mut leftover)? {
                ChunkState::Done => break,
                ChunkState::NeedMore => {
                    let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
                    if n == 0 {
                        return Err("connection closed mid-chunk".to_string());
                    }
                    leftover.extend_from_slice(&chunk[..n]);
                }
            }
            if body.len() > max_bytes {
                return Err("body too large (chunked)".to_string());
            }
        }
    } else if let Some(len) = content_length {
        if len > max_bytes {
            return Err("body too large (content-length)".to_string());
        }
        body.extend_from_slice(rest);
        while body.len() < len {
            let want = (len - body.len()).min(chunk.len());
            let n = stream
                .read(&mut chunk[..want])
                .map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                return Err("connection closed mid-body".to_string());
            }
            body.extend_from_slice(&chunk[..n]);
        }
        if body.len() > len {
            body.truncate(len);
        }
    } else {
        body.extend_from_slice(rest);
        loop {
            let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
            if body.len() > max_bytes {
                return Err("body too large (eof-terminated)".to_string());
            }
        }
    }
    Ok(Response {
        status,
        header_lines,
        body,
    })
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_head(head: &[u8]) -> Result<(u16, Vec<String>), String> {
    let text = std::str::from_utf8(head).map_err(|_| "head not utf-8".to_string())?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().ok_or("empty head")?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/1.") {
        return Err(format!("unsupported version: {version}"));
    }
    let status: u16 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad status line: {status_line:?}"))?;
    let headers: Vec<String> = lines
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    Ok((status, headers))
}

enum ChunkState {
    Done,
    NeedMore,
}

/// 增量 chunked 解码：把 leftover 中已完整的 chunk 落入 body。
fn decode_chunk_step(body: &mut Vec<u8>, leftover: &mut Vec<u8>) -> Result<ChunkState, String> {
    loop {
        let Some(line_end) = find_crlf(leftover) else {
            if leftover.len() > 64 {
                return Err("chunk-size line too long".to_string());
            }
            return Ok(ChunkState::NeedMore);
        };
        let size_line = String::from_utf8_lossy(&leftover[..line_end]).to_string();
        let size_str = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|_| format!("bad chunk size: {size_str:?}"))?;
        let after_size = line_end + 2;
        if size == 0 {
            // 终止块后跟 trailer + CRLF；宽松处理：只要收到终止块即完成
            return Ok(ChunkState::Done);
        }
        if leftover.len() < after_size + size + 2 {
            return Ok(ChunkState::NeedMore);
        }
        body.extend_from_slice(&leftover[after_size..after_size + size]);
        let tail = &leftover[after_size + size..];
        if tail.len() < 2 || &tail[..2] != b"\r\n" {
            return Err("missing chunk CRLF".to_string());
        }
        leftover.drain(..after_size + size + 2);
    }
}

fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}

/// 供测试/工具构造响应写入器（服务端方向不需要，这里只保留流约定）。
pub fn _write_all<S: Write>(stream: &mut S, buf: &[u8]) -> Result<(), String> {
    stream.write_all(buf).map_err(|e| format!("write: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(bytes: &[u8]) -> Result<Response, String> {
        let mut cursor = std::io::Cursor::new(bytes.to_vec());
        read_response(&mut cursor, 1 << 20)
    }

    #[test]
    fn serializes_get_with_defaults() {
        let b = serialize_request(
            "GET",
            "/a?b=1",
            "weread.qq.com",
            &["Accept: application/json"],
            None,
        )
        .unwrap();
        let s = String::from_utf8(b).unwrap();
        assert!(s.starts_with("GET /a?b=1 HTTP/1.1\r\n"));
        assert!(s.contains("Host: weread.qq.com\r\n"));
        assert!(s.contains("Accept: application/json\r\n"));
        assert!(s.ends_with("Connection: close\r\n\r\n"));
        assert!(!s.to_lowercase().contains("user-agent"));
    }

    #[test]
    fn serializes_post_with_length() {
        let b = serialize_request("POST", "/x", "h", &[], Some(b"payload")).unwrap();
        let s = String::from_utf8(b).unwrap();
        assert!(s.contains("Content-Length: 7\r\n"));
        assert!(s.ends_with("\r\n\r\npayload"));
    }

    #[test]
    fn rejects_reserved_and_injection() {
        assert!(serialize_request("GET", "/", "h", &["Host: evil.com"], None).is_err());
        assert!(serialize_request("GET", "/", "h", &["X: a\r\nX2: b"], None).is_err());
        assert!(serialize_request("get", "/", "h", &[], None).is_err());
        assert!(serialize_request("GET", "/a b", "h", &[], None).is_err());
    }

    #[test]
    fn parses_content_length_body() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello";
        let r = resp(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"hello");
        assert_eq!(r.header_lines[0], "Content-Type: text/plain");
    }

    #[test]
    fn preserves_header_case() {
        let raw =
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: https://x/Y\r\nContent-Length: 0\r\n\r\n";
        let r = resp(raw).unwrap();
        assert_eq!(r.status, 301);
        assert!(r.header_lines.iter().any(|l| l.starts_with("Location:")));
    }

    #[test]
    fn parses_chunked() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n3;x=y\r\nefg\r\n0\r\n\r\n";
        let r = resp(raw).unwrap();
        assert_eq!(r.body, b"abcdefg");
    }

    #[test]
    fn parses_eof_terminated() {
        let raw = b"HTTP/1.0 200 OK\r\n\r\nstreamed-data";
        let r = resp(raw).unwrap();
        assert_eq!(r.body, b"streamed-data");
    }

    #[test]
    fn enforces_max_bytes() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort";
        assert!(resp_with_cap(raw, 10).is_err());
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        assert!(resp_with_cap(raw, 3).is_err());
    }

    fn resp_with_cap(bytes: &[u8], cap: usize) -> Result<Response, String> {
        let mut cursor = std::io::Cursor::new(bytes.to_vec());
        read_response(&mut cursor, cap)
    }

    #[test]
    fn rejects_bad_status_line() {
        assert!(resp(b"NOT-HTTP\r\n\r\n").is_err());
        assert!(resp(b"HTTP/2 200 OK\r\n\r\n").is_err());
    }
}
