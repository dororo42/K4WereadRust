//! URL 解析（只接受 https）与目标结构。
//! 非法状态不可表示：解析失败即返回 None，调用方无法用半解析的目标发请求。

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    /// 始终以 / 开头（含 query）
    pub path: String,
}

pub fn parse_https_url(url: &str) -> Option<Target> {
    let rest = url.strip_prefix("https://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            if h.is_empty() || p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            (h, p.parse::<u16>().ok()?)
        }
        None => (authority, 443),
    };
    if host.contains('@') || host.starts_with('[') || host.is_empty() {
        return None; // userinfo/IPv6 字面量：K4 无 IPv6 栈，明确拒绝
    }
    Some(Target {
        host: host.to_ascii_lowercase(),
        port,
        path: path.to_string(),
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
    fn rejects_non_https_and_garbage() {
        assert!(parse_https_url("http://weread.qq.com/").is_none());
        assert!(parse_https_url("https://").is_none());
        assert!(parse_https_url("https://user@weread.qq.com/").is_none());
        assert!(parse_https_url("https://[::1]:443/").is_none());
        assert!(parse_https_url("https://weread.qq.com:/x").is_none());
    }
}
