//! URL 解析（只接受 https）与目标结构。
//! 非法状态不可表示：解析失败即返回 None，调用方无法用半解析的目标发请求。

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    /// 始终以 / 开头（含 query，不含 fragment）
    pub path: String,
}

pub fn parse_https_url(url: &str) -> Option<Target> {
    let rest = url.strip_prefix("https://")?;
    // authority 在第一个 / ? # 处结束；后两者同时决定 query/fragment 的归属。
    let cut = rest
        .find(['/', '?', '#'])
        .unwrap_or(rest.len());
    let authority = &rest[..cut];
    let mut rest_path = &rest[cut..];
    if let Some(i) = rest_path.find('#') {
        rest_path = &rest_path[..i];
    }
    let path = if rest_path.is_empty() {
        "/".to_string()
    } else if rest_path.starts_with('?') {
        format!("/{rest_path}")
    } else {
        rest_path.to_string()
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            if h.is_empty() || p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let port = p.parse::<u16>().ok()?;
            if port == 0 {
                return None;
            }
            (h, port)
        }
        None => (authority, 443),
    };
    // 入口处收紧 host：CR/LF、空格、userinfo、IPv6 字面量、非 ASCII 一律拒绝，
    // 不把 Host 头拼接与 SNI 的合法性寄希望于下游的间接校验。
    if host.is_empty()
        || host.len() > 253
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
    {
        return None;
    }
    if path.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return None;
    }
    Some(Target {
        host: host.to_ascii_lowercase(),
        port,
        path,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Budgets {
    pub block_timeout: Duration,
    pub total_deadline: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic() {
        let t = parse_https_url("https://weread.qq.com/web/book/read?a=1").unwrap();
        assert_eq!(t.host, "weread.qq.com");
        assert_eq!(t.port, 443);
        assert_eq!(t.path, "/web/book/read?a=1");
    }

    #[test]
    fn parses_explicit_port_and_bare_host() {
        let t = parse_https_url("https://127.0.0.1:8443").unwrap();
        assert_eq!(t.port, 8443);
        assert_eq!(t.path, "/");
        let t = parse_https_url("https://weread.qq.com").unwrap();
        assert_eq!(t.port, 443);
    }

    #[test]
    fn separates_query_and_strips_fragment() {
        let t = parse_https_url("https://weread.qq.com?a=1").unwrap();
        assert_eq!(t.host, "weread.qq.com");
        assert_eq!(t.path, "/?a=1");
        let t = parse_https_url("https://weread.qq.com#frag").unwrap();
        assert_eq!(t.path, "/");
        let t = parse_https_url("https://weread.qq.com/web?a=1#frag").unwrap();
        assert_eq!(t.path, "/web?a=1");
        let t = parse_https_url("https://127.0.0.1:8443?x=1").unwrap();
        assert_eq!(t.port, 8443);
        assert_eq!(t.path, "/?x=1");
    }

    #[test]
    fn rejects_unsafe_hosts_and_paths() {
        for url in [
            "https://weread.qq.com\r\nX: 1/",
            "https://host name/",
            "https://host/\r\n",
            "https://host:0/",
            "https://hosté/",
        ] {
            assert!(parse_https_url(url).is_none(), "accepted {url:?}");
        }
    }

    #[test]
    fn rejects_non_https_and_garbage() {
        assert!(parse_https_url("http://weread.qq.com/").is_none());
        assert!(parse_https_url("https://").is_none());
        assert!(parse_https_url("https://user@weread.qq.com/").is_none());
        assert!(parse_https_url("https://[::1]:443/").is_none());
        assert!(parse_https_url("https://weread.qq.com:/x").is_none());
    }
}
