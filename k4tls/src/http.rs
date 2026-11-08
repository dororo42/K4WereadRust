//! HTTP/1.1 报文层：请求序列化 + 响应解析（content-length / chunked / EOF 终止）。
//! 语义对齐 LuaSocket socket.http：每请求新建连接、Connection: close、
//! 不注入 User-Agent、不自动跟随重定向（3xx 原样返回给 Lua 侧处理）。

use std::io::{self, BufRead, BufReader, Read};

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    Invalid(String),
    TooLarge(&'static str),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "read: {e}"),
            Self::Invalid(message) => f.write_str(message),
            Self::TooLarge(part) => write!(f, "{part} too large"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<String> for ReadError {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}

impl From<&str> for ReadError {
    fn from(message: &str) -> Self {
        Self::Invalid(message.to_string())
    }
}

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

/// `max_bytes` caps the decoded body; headers (including interim responses) and
/// chunk metadata/trailers each have an independent MAX_HEADER_BLOCK budget.
pub fn read_response<S: Read>(
    stream: &mut S,
    method: &str,
    max_bytes: usize,
) -> Result<Response, ReadError> {
    // Fixed read-ahead, never a buffer sized by an untrusted chunk declaration.
    let mut reader = BufReader::with_capacity(2048, stream);
    let mut head_budget = MAX_HEADER_BLOCK;
    let (status, header_lines, framing) = loop {
        let mut head = Vec::new();
        loop {
            let line = read_line(
                &mut reader,
                &mut head_budget,
                MAX_HEADER_BLOCK,
                "header block",
            )?;
            if line.is_empty() {
                break;
            }
            if !head.is_empty() {
                head.extend_from_slice(b"\r\n");
            }
            head.extend_from_slice(&line);
        }
        let (status, headers) = parse_head(&head)?;
        let framing = parse_framing(&headers)?;
        if status == 101 {
            return Err("HTTP protocol switching is unsupported".into());
        }
        if status >= 200 {
            break (status, headers, framing);
        }
    };
    let mut body = Vec::new();
    if method == "HEAD" || matches!(status, 204 | 304) {
        // Content-Length describes the representation, not a body on this response.
    } else {
        match framing {
            Framing::Chunked => read_chunked(&mut reader, &mut body, max_bytes)?,
            Framing::Length(len) => read_body(&mut reader, &mut body, len, max_bytes)?,
            Framing::Eof => {
                let mut buf = [0; 2048];
                loop {
                    let available = max_bytes
                        .checked_sub(body.len())
                        .ok_or(ReadError::TooLarge("body"))?;
                    // At the cap, a one-byte probe distinguishes EOF from overflow.
                    let want = available.min(buf.len()).max(1);
                    let n = read_retry(&mut reader, &mut buf[..want])?;
                    if n == 0 {
                        break;
                    }
                    ensure_body_room(body.len(), n, max_bytes)?;
                    body.extend_from_slice(&buf[..n]);
                }
            }
        }
    }
    Ok(Response {
        status,
        header_lines,
        body,
    })
}

fn read_retry<R: Read>(reader: &mut R, out: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(out) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

/// Both per-line and aggregate limits include CRLF, checked before growing a buffer.
fn read_line<R: BufRead>(
    reader: &mut R,
    budget: &mut usize,
    line_limit: usize,
    part: &'static str,
) -> Result<Vec<u8>, ReadError> {
    let mut line = Vec::new();
    loop {
        let room = line_limit
            .checked_sub(line.len())
            .ok_or(ReadError::TooLarge(part))?
            .min(*budget);
        if room == 0 {
            return Err(ReadError::TooLarge(part));
        }
        let available = match reader.fill_buf() {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if available.is_empty() {
            return Err("connection closed before CRLF".into());
        }
        let take = available
            .iter()
            .position(|&b| b == b'\n')
            .map_or(available.len(), |pos| pos + 1)
            .min(room);
        let ended = available[take - 1] == b'\n';
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        *budget = budget.checked_sub(take).ok_or(ReadError::TooLarge(part))?;
        if ended {
            if !line.ends_with(b"\r\n") {
                return Err("line missing CRLF".into());
            }
            line.truncate(line.len() - 2);
            return Ok(line);
        }
    }
}

fn valid_text(text: &str) -> bool {
    text.bytes().all(|b| b == b'\t' || (b >= 32 && b != 127))
}

fn header_parts(line: &str) -> Result<(&str, &str), ReadError> {
    let (name, value) = line.split_once(':').ok_or("header missing colon")?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        || !valid_text(value)
    {
        return Err("invalid header name or control character".into());
    }
    Ok((name, value.trim_matches([' ', '\t'])))
}

fn parse_head(head: &[u8]) -> Result<(u16, Vec<String>), ReadError> {
    let text = std::str::from_utf8(head).map_err(|_| "head not utf-8")?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().ok_or("empty head")?;
    if !valid_text(status_line) {
        return Err("invalid status control character".into());
    }
    let mut parts = status_line.splitn(3, ' ');
    if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
        return Err("unsupported HTTP version".into());
    }
    let digits = parts.next().ok_or("missing status")?;
    if digits.len() != 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("bad HTTP status".into());
    }
    let status: u16 = digits.parse().map_err(|_| "bad HTTP status")?;
    if !(100..=599).contains(&status) {
        return Err("bad HTTP status".into());
    }
    let mut headers = Vec::new();
    for line in lines {
        header_parts(line)?;
        headers.push(line.to_string());
    }
    Ok((status, headers))
}

enum Framing {
    Chunked,
    Length(usize),
    Eof,
}

fn parse_framing(headers: &[String]) -> Result<Framing, ReadError> {
    let mut length = None;
    let mut chunked = false;
    for header in headers {
        let (name, value) = header_parts(header)?;
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err("invalid or duplicate Content-Length".into());
            }
            length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| "invalid Content-Length")?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !value.eq_ignore_ascii_case("chunked") {
                return Err("unsupported or duplicate Transfer-Encoding".into());
            }
            chunked = true;
        }
    }
    match (chunked, length) {
        (true, Some(_)) => Err("conflicting Transfer-Encoding and Content-Length".into()),
        (true, None) => Ok(Framing::Chunked),
        (false, Some(len)) => Ok(Framing::Length(len)),
        (false, None) => Ok(Framing::Eof),
    }
}

fn ensure_body_room(len: usize, size: usize, cap: usize) -> Result<(), ReadError> {
    match len.checked_add(size) {
        Some(total) if total <= cap => Ok(()),
        _ => Err(ReadError::TooLarge("body")),
    }
}

fn read_body<R: Read>(
    reader: &mut R,
    body: &mut Vec<u8>,
    size: usize,
    cap: usize,
) -> Result<(), ReadError> {
    ensure_body_room(body.len(), size, cap)?;
    let mut left = size;
    let mut buf = [0; 2048];
    while left > 0 {
        let want = left.min(buf.len());
        let n = read_retry(reader, &mut buf[..want])?;
        if n == 0 {
            return Err("connection closed mid-body".into());
        }
        ensure_body_room(body.len(), n, cap)?;
        body.extend_from_slice(&buf[..n]);
        left -= n;
    }
    Ok(())
}

fn read_chunked<R: BufRead>(
    reader: &mut R,
    body: &mut Vec<u8>,
    cap: usize,
) -> Result<(), ReadError> {
    let mut metadata_budget = MAX_HEADER_BLOCK;
    loop {
        let line = read_line(reader, &mut metadata_budget, 1024, "chunk metadata")?;
        let line = std::str::from_utf8(&line).map_err(|_| "bad chunk size line")?;
        if !valid_text(line) {
            return Err("invalid chunk metadata control character".into());
        }
        let digits = line.split(';').next().unwrap_or("");
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("bad chunk size".into());
        }
        let size = usize::from_str_radix(digits, 16).map_err(|_| ReadError::TooLarge("body"))?;
        // Reject a declaration exceeding the remaining cap before reading its payload.
        ensure_body_room(body.len(), size, cap)?;
        if size == 0 {
            loop {
                let trailer = read_line(
                    reader,
                    &mut metadata_budget,
                    MAX_HEADER_BLOCK,
                    "chunk metadata",
                )?;
                if trailer.is_empty() {
                    return Ok(());
                }
                let trailer = std::str::from_utf8(&trailer).map_err(|_| "trailer not utf-8")?;
                let (name, _) = header_parts(trailer)?;
                if name.eq_ignore_ascii_case("content-length")
                    || name.eq_ignore_ascii_case("transfer-encoding")
                {
                    return Err("framing field in trailer".into());
                }
            }
        }
        metadata_budget = metadata_budget
            .checked_sub(2)
            .ok_or(ReadError::TooLarge("chunk metadata"))?;
        read_body(reader, body, size, cap)?;
        let mut crlf = [0; 2];
        reader.read_exact(&mut crlf)?;
        if crlf != *b"\r\n" {
            return Err("missing chunk CRLF".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(bytes: &[u8]) -> Result<Response, ReadError> {
        let mut cursor = std::io::Cursor::new(bytes.to_vec());
        read_response(&mut cursor, "GET", 1 << 20)
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

    fn resp_with_cap(bytes: &[u8], cap: usize) -> Result<Response, ReadError> {
        let mut cursor = std::io::Cursor::new(bytes.to_vec());
        read_response(&mut cursor, "GET", cap)
    }

    #[test]
    fn regression_same_packet_body_caps() {
        for raw in [
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n0\r\n\r\n"[..],
            &b"HTTP/1.0 200 OK\r\n\r\nab"[..],
        ] {
            assert!(
                resp_with_cap(raw, 1).is_err(),
                "accepted oversized response"
            );
            assert_eq!(resp_with_cap(raw, 2).unwrap().body, b"ab");
        }
    }

    struct NoExtraRead<'a>(&'a [u8]);

    impl Read for NoExtraRead<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            assert!(
                !self.0.is_empty(),
                "read payload after oversized declaration"
            );
            let n = out.len().min(self.0.len());
            out[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn regression_chunk_declaration_rejected_before_payload_read() {
        let mut stream =
            NoExtraRead(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10000000\r\n");
        let err = read_response(&mut stream, "GET", 1).unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[test]
    fn regression_chunk_arithmetic_does_not_overflow() {
        let raw = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
            usize::MAX
        );
        assert!(resp_with_cap(raw.as_bytes(), 1).is_err());
    }

    #[test]
    fn regression_rejects_header_controls() {
        for control in [0, 1, 11, 12, 13, 10, 31, 127] {
            let mut raw = b"HTTP/1.1 200 OK\r\nX-Test: before".to_vec();
            raw.push(control);
            raw.extend_from_slice(b"after\r\nContent-Length: 0\r\n\r\n");
            assert!(resp(&raw).is_err(), "accepted control byte {control}");
        }
    }

    #[test]
    fn regression_rejects_ambiguous_framing() {
        for headers in [
            "Content-Length: invalid",
            "Content-Length: +2",
            "Content-Length: -1",
            "Content-Length: 2, 2",
            "Content-Length: 2\r\nContent-Length: 3",
            "Content-Length: 999999999999999999999999999999999999",
            "Transfer-Encoding: notchunked",
            "Transfer-Encoding: gzip, chunked",
            "Transfer-Encoding: chunked\r\nContent-Length: 2",
            "Transfer-Encoding: chunked\r\nTransfer-Encoding: chunked",
        ] {
            let raw = format!("HTTP/1.1 200 OK\r\n{headers}\r\n\r\n0\r\n\r\n");
            assert!(resp(raw.as_bytes()).is_err(), "accepted {headers:?}");
        }
    }

    #[test]
    fn regression_skips_interim_responses() {
        let raw = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: </a>\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nab";
        let response = resp(raw).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"ab");
        assert_eq!(response.header_lines, ["Content-Length: 2"]);
        assert!(resp(b"HTTP/1.1 101 Switching Protocols\r\n\r\n").is_err());
    }

    #[test]
    fn regression_bodyless_statuses_ignore_advertised_length() {
        for status in [204, 304] {
            let raw = format!("HTTP/1.1 {status} No Body\r\nContent-Length: 999\r\n\r\n");
            assert!(resp_with_cap(raw.as_bytes(), 1).unwrap().body.is_empty());
        }
    }

    #[test]
    fn regression_bounded_headers_chunks_and_trailers() {
        let long = "a".repeat(MAX_HEADER_BLOCK);
        for raw in [
            format!("HTTP/1.1 200 OK\r\nX: {long}\r\n\r\n"),
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1;{long}\r\na\r\n0\r\n\r\n"
            ),
            format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX: {long}\r\n\r\n"),
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX: unfinished".to_string(),
        ] {
            assert!(
                resp(raw.as_bytes()).is_err(),
                "accepted unbounded or truncated metadata"
            );
        }
    }

    #[test]
    fn head_does_not_read_a_body_even_with_chunked_framing() {
        let mut stream = NoExtraRead(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n");
        assert!(read_response(&mut stream, "HEAD", 0)
            .unwrap()
            .body
            .is_empty());
    }

    #[test]
    fn caps_zero_exact_and_cumulative_chunks() {
        for raw in [
            &b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"[..],
            &b"HTTP/1.0 200 OK\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"[..],
        ] {
            assert!(resp_with_cap(raw, 0).unwrap().body.is_empty());
        }
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n1\r\nb\r\n0\r\nX-Trailer: fine\r\n\r\n";
        assert_eq!(resp_with_cap(raw, 2).unwrap().body, b"ab");
        assert!(matches!(resp_with_cap(raw, 1), Err(ReadError::TooLarge(_))));
        let mut early =
            NoExtraRead(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n1\r\n");
        assert!(matches!(
            read_response(&mut early, "GET", 1),
            Err(ReadError::TooLarge(_))
        ));
        assert!(ensure_body_room(usize::MAX, 1, usize::MAX).is_err());
        assert!(ensure_body_room(0, usize::MAX, usize::MAX).is_ok());
    }

    #[test]
    fn eof_rejects_zero_cap_and_late_overflow() {
        assert!(matches!(
            resp_with_cap(b"HTTP/1.0 200 OK\r\n\r\na", 0),
            Err(ReadError::TooLarge(_))
        ));
        let raw = format!("HTTP/1.0 200 OK\r\n\r\n{}", "a".repeat(5000));
        assert_eq!(
            resp_with_cap(raw.as_bytes(), 5000).unwrap().body.len(),
            5000
        );
        assert!(matches!(
            resp_with_cap(raw.as_bytes(), 4999),
            Err(ReadError::TooLarge(_))
        ));
    }

    #[test]
    fn header_limit_includes_delimiters_and_interim_heads() {
        let prefix = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX: ";
        let suffix = "\r\n\r\n";
        let raw = format!(
            "{prefix}{}{suffix}",
            "a".repeat(MAX_HEADER_BLOCK - prefix.len() - suffix.len())
        );
        assert!(resp(raw.as_bytes()).is_ok());
        let oversized = raw.replacen("X: ", "X: aa", 1);
        assert!(matches!(
            resp(oversized.as_bytes()),
            Err(ReadError::TooLarge(_))
        ));
        let interim = format!("HTTP/1.1 100 Continue\r\n\r\n{raw}");
        assert!(matches!(
            resp(interim.as_bytes()),
            Err(ReadError::TooLarge(_))
        ));
    }

    #[test]
    fn chunk_metadata_has_line_and_cumulative_limits() {
        let prefix = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        let exact_line = format!("{prefix}1;{}\r\na\r\n0\r\n\r\n", "x".repeat(1020));
        assert_eq!(resp(exact_line.as_bytes()).unwrap().body, b"a");
        let oversized = exact_line.replacen("1;", "1;x", 1);
        assert!(matches!(
            resp(oversized.as_bytes()),
            Err(ReadError::TooLarge(_))
        ));
        let many_chunks = format!(
            "{prefix}{}0\r\n\r\n",
            "1\r\na\r\n".repeat(MAX_HEADER_BLOCK / 5 + 1)
        );
        assert!(matches!(
            resp(many_chunks.as_bytes()),
            Err(ReadError::TooLarge(_))
        ));
        let trailer_prefix = format!("{prefix}0\r\nX: ");
        let trailer = format!(
            "{trailer_prefix}{}\r\n\r\n",
            "a".repeat(MAX_HEADER_BLOCK - 10)
        );
        assert!(resp(trailer.as_bytes()).is_ok());
        let too_long = trailer.replacen("X: ", "X: aa", 1);
        assert!(matches!(
            resp(too_long.as_bytes()),
            Err(ReadError::TooLarge(_))
        ));
    }

    #[test]
    fn rejects_malformed_chunk_lines_and_trailers() {
        for body in [
            "+1\r\na\r\n0\r\n\r\n",
            " 1\r\na\r\n0\r\n\r\n",
            "g\r\n",
            "1\r\naXX0\r\n\r\n",
            "0\r\nX: bad\0value\r\n\r\n",
            "0\r\nContent-Length: 3\r\n\r\n",
            "0\r\nTransfer-Encoding: chunked\r\n\r\n",
            "0\r\n",
            "0\r\n folded trailer\r\n\r\n",
        ] {
            let raw = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{body}");
            assert!(resp(raw.as_bytes()).is_err(), "accepted {body:?}");
        }
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffffffffffffffffffff\r\n";
        assert!(matches!(resp(raw), Err(ReadError::TooLarge(_))));
    }

    #[test]
    fn fragmented_response_and_io_error_kind_preserved() {
        struct Fragmented<'a>(&'a [u8]);
        impl Read for Fragmented<'_> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                let want = out.len().min(1);
                self.0.read(&mut out[..want])
            }
        }
        let mut raw = Fragmented(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nTransfer-Encoding: ChUnKeD\r\n\r\n2\r\nab\r\n0\r\nX: value\r\n\r\n");
        assert_eq!(read_response(&mut raw, "GET", 2).unwrap().body, b"ab");
        struct Fail;
        impl Read for Fail {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from_raw_os_error(libc::EAGAIN))
            }
        }
        let error = read_response(&mut Fail, "GET", 1).unwrap_err();
        assert!(matches!(error, ReadError::Io(e) if e.raw_os_error() == Some(libc::EAGAIN)));
    }

    #[test]
    fn header_tokens_and_status_controls_are_validated() {
        for header in [
            "X Name: value",
            "X@: value",
            "X : value",
            " folded: value",
            "No-Colon",
        ] {
            let raw = format!("HTTP/1.1 200 OK\r\n{header}\r\n\r\n");
            assert!(resp(raw.as_bytes()).is_err());
        }
        assert!(resp(b"HTTP/1.1 200 O\0K\r\n\r\n").is_err());
        assert!(resp(b"HTTP/1.1 200 OK\r\nX: tab\tvalue\r\nContent-Length: 0\r\n\r\n").is_ok());
        assert!(resp(b"HTTP/1.1 100 Continue\r\n\r\n").is_err());
        assert!(resp(b"HTTP/1.1 101 Switching Protocols\r\n\r\n").is_err());
    }

    #[test]
    fn rejects_bad_status_line() {
        assert!(resp(b"NOT-HTTP\r\n\r\n").is_err());
        assert!(resp(b"HTTP/2 200 OK\r\n\r\n").is_err());
    }
}
