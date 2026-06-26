//! URL/域名匹配工具模块。
//! 解决旧实现 `lower_url.contains(domain)` 的误报问题：
//! - `domain="evil.com"` 不应匹配 `notevil.com`
//! - `domain="evil.com"` 不应匹配 `evil.com.attacker.net`

use url::Url;

/// 判断 url 的 host 是否等于 domain 或为其子域名。
///
/// 精确匹配：`host == domain`
/// 子域名后缀匹配：`host.ends_with(".{domain}")`（必须带点前缀，避免 evil.com 匹配 notevil.com）
pub fn domain_matches_url(domain: &str, url: &str) -> bool {
    let domain_lower = domain.to_lowercase();
    let host = match Url::parse(url) {
        Ok(u) => u.host_str().map(|h| h.to_lowercase()),
        Err(_) => return false,
    };
    let host = match host {
        Some(h) => h,
        None => return false,
    };
    host == domain_lower || host.ends_with(&format!(".{}", domain_lower))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_domain_match() {
        assert!(domain_matches_url("evil.com", "https://evil.com/path"));
        assert!(domain_matches_url("evil.com", "http://evil.com"));
    }

    #[test]
    fn subdomain_match() {
        assert!(domain_matches_url("evil.com", "https://a.evil.com/x"));
        assert!(domain_matches_url("evil.com", "https://sub.a.evil.com/x"));
    }

    #[test]
    fn not_match_different_domain_with_substring() {
        // 关键反例：notevil.com 不应匹配 evil.com
        assert!(!domain_matches_url("evil.com", "https://notevil.com/path"));
        // evil.com.attacker.net 不应匹配 evil.com
        assert!(!domain_matches_url("evil.com", "https://evil.com.attacker.net/x"));
    }

    #[test]
    fn case_insensitive() {
        assert!(domain_matches_url("Evil.COM", "https://EVIL.com/x"));
        assert!(domain_matches_url("evil.com", "HTTPS://Sub.Evil.COM/x"));
    }

    #[test]
    fn no_scheme_returns_false() {
        // 无 scheme 的 url 解析失败，返回 false（保守处理）
        assert!(!domain_matches_url("evil.com", "evil.com"));
        assert!(!domain_matches_url("evil.com", "notevil.com"));
    }

    #[test]
    fn ipv4_host() {
        assert!(domain_matches_url("127.0.0.1", "http://127.0.0.1:8080/x"));
    }

    #[test]
    fn with_port() {
        assert!(domain_matches_url("evil.com", "https://evil.com:8443/x"));
    }
}
