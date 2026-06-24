//! 权限匹配：将目标域名与扩展的 host_permissions 做匹配
//!
//! 实现设计文档 §4.2.4 / §9.2 中定义的域名→扩展匹配能力。
//! 将 Chromium match pattern 转换为正则表达式进行匹配，
//! 同时识别敏感权限组合（如 webRequest + <all_urls>）。

use crate::extension_inventory::ExtensionInfo;
use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::debug;

/// 权限匹配结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PermissionMatchResult {
    /// 匹配的扩展列表，按风险等级排序
    pub matching_extensions: Vec<MatchedExtension>,
}

/// 匹配到的扩展
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MatchedExtension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub risk_flags: Vec<String>,
    /// 匹配到的 host_permissions 模式
    pub matched_patterns: Vec<String>,
    /// 是否拥有敏感权限组合（如 webRequest）
    pub has_sensitive_permissions: bool,
}

/// 将目标域名与扩展的 host_permissions 做匹配
pub fn match_domain_to_extensions(domain: &str, extensions: &[ExtensionInfo]) -> PermissionMatchResult {
    let mut matched: Vec<MatchedExtension> = extensions
        .iter()
        .filter_map(|ext| match_extension(domain, ext))
        .collect();

    sort_matched_extensions(&mut matched);

    debug!("domain '{}' matched {} extension(s)", domain, matched.len());

    PermissionMatchResult {
        matching_extensions: matched,
    }
}

/// 检查单个扩展是否匹配目标域名
fn match_extension(domain: &str, ext: &ExtensionInfo) -> Option<MatchedExtension> {
    let mut matched_patterns: Vec<String> = Vec::new();

    // 检查 host_permissions 中的每个模式
    for pattern in &ext.host_permissions {
        if domain_matches_host_pattern(domain, pattern) {
            matched_patterns.push(pattern.clone());
        }
    }

    // 也检查 permissions 中的 <all_urls>（MV2 中可能放在 permissions 里）
    if ext.permissions.iter().any(|p| p == "<all_urls>") && !matched_patterns.iter().any(|p| p == "<all_urls>") {
        matched_patterns.push("<all_urls>".to_string());
    }

    // 敏感权限组合：webRequest + <all_urls>
    let has_webrequest = ext.permissions.iter().any(|p| p == "webRequest");
    let has_all_urls = ext
        .host_permissions
        .iter()
        .chain(ext.permissions.iter())
        .any(|p| p == "<all_urls>");
    let has_sensitive_permissions = has_webrequest && has_all_urls;

    // 即使 host_permissions 不匹配，拥有 webRequest + <all_urls> 的扩展也应包含
    if matched_patterns.is_empty() && !has_sensitive_permissions {
        return None;
    }

    Some(MatchedExtension {
        id: ext.id.clone(),
        name: ext.name.clone(),
        version: ext.version.clone(),
        risk_flags: ext.risk_flags.clone(),
        matched_patterns,
        has_sensitive_permissions,
    })
}

/// 检查域名是否匹配某个 host_permissions 模式
///
/// 输入为纯域名（如 `evil.com`），匹配时只比较 host 部分，
/// 忽略 pattern 中的 scheme 和 path。
fn domain_matches_host_pattern(domain: &str, pattern: &str) -> bool {
    // 特殊值 <all_urls> 匹配所有域名
    if pattern == "<all_urls>" {
        return true;
    }

    let Some(host_pattern) = extract_host_from_pattern(pattern) else {
        debug!("failed to parse match pattern: {}", pattern);
        return false;
    };

    match_host_pattern(domain, &host_pattern)
}

/// 从 Chromium match pattern 中提取 host 部分
///
/// 格式: `<scheme>://<host><path>`，提取 host 部分
fn extract_host_from_pattern(pattern: &str) -> Option<String> {
    let rest = pattern
        .strip_prefix("*://")
        .or_else(|| pattern.strip_prefix("http://"))
        .or_else(|| pattern.strip_prefix("https://"))?;

    // host 到第一个 / 为止
    let host = match rest.find('/') {
        Some(idx) => &rest[..idx],
        None => rest,
    };

    Some(host.to_string())
}

/// 将域名与 host pattern 做匹配
///
/// - `*` 匹配任意主机
/// - `*.example.com` 匹配 example.com 及其子域名
/// - `example.com` 精确匹配，不匹配子域名
fn match_host_pattern(domain: &str, host_pattern: &str) -> bool {
    if host_pattern == "*" {
        return true;
    }

    if let Some(suffix) = host_pattern.strip_prefix("*.") {
        // *.example.com 匹配 example.com 及其子域名
        domain == suffix || domain.ends_with(&format!(".{}", suffix))
    } else {
        // 精确匹配
        domain == host_pattern
    }
}

/// 将 Chromium match pattern 转换为正则表达式
///
/// Chromium match pattern 格式：`<scheme>://<host><path>`
/// - scheme: `*` 或 `http` 或 `https`
/// - host: `*` 或 `*.example.com` 或 `example.com`
/// - path: `/*` 或 `/path/*`
/// - 特殊值 `<all_urls>` 匹配所有
///
/// 注意：域名匹配场景下使用 `domain_matches_host_pattern` 即可，
/// 此函数用于需要完整 URL 匹配的场景。
pub fn match_pattern_to_regex(pattern: &str) -> Option<Regex> {
    // 特殊值
    if pattern == "<all_urls>" {
        return Regex::new(r"(?i)^.+$").ok();
    }

    // 解析 scheme://host/path
    let rest = pattern
        .strip_prefix("*://")
        .map(|r| ("*", r))
        .or_else(|| pattern.strip_prefix("http://").map(|r| ("http", r)))
        .or_else(|| pattern.strip_prefix("https://").map(|r| ("https", r)));

    let (scheme, rest) = rest?;

    // 分离 host 和 path：host 到第一个 / 为止
    let (host_part, path_part) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    // 构建 scheme 正则
    let scheme_re = match scheme {
        "*" => "(?:https?|ftp)",
        "http" => "http",
        "https" => "https",
        _ => return None,
    };

    // 构建 host 正则
    let host_re = if host_part == "*" {
        r"[^/]+".to_string()
    } else if let Some(suffix) = host_part.strip_prefix("*.") {
        let escaped = regex::escape(suffix);
        format!(r"(?:.*\.)?{}", escaped)
    } else {
        regex::escape(host_part)
    };

    // 构建 path 正则
    let path_re = regex::escape(path_part).replace("\\*", ".*");

    let regex_str = format!(r"(?i)^{}://{}{}$", scheme_re, host_re, path_re);

    Regex::new(&regex_str).ok()
}

/// 风险标志的排序优先级
fn risk_flag_priority(flag: &str) -> u32 {
    match flag {
        "high_privilege_combo" => 0,
        "broad_host_access" => 1,
        "content_script_inject" => 2,
        _ => 3,
    }
}

/// 对匹配到的扩展排序
///
/// 排序规则：
/// 1. 有 risk_flags 的扩展优先
/// 2. high_privilege_combo > broad_host_access > content_script_inject > 其他
/// 3. 同级别按名称排序
fn sort_matched_extensions(extensions: &mut [MatchedExtension]) {
    extensions.sort_by(|a, b| {
        // 有 risk_flags 的优先
        let a_has_flags = !a.risk_flags.is_empty();
        let b_has_flags = !b.risk_flags.is_empty();
        match b_has_flags.cmp(&a_has_flags) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        // 按最高优先级 risk_flag 排序
        let a_min = a
            .risk_flags
            .iter()
            .map(|f| risk_flag_priority(f))
            .min()
            .unwrap_or(u32::MAX);
        let b_min = b
            .risk_flags
            .iter()
            .map(|f| risk_flag_priority(f))
            .min()
            .unwrap_or(u32::MAX);
        match a_min.cmp(&b_min) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        // 同级别按名称排序
        a.name.cmp(&b.name)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_ext(overrides: impl FnOnce(&mut ExtensionInfo)) -> ExtensionInfo {
        let mut ext = ExtensionInfo {
            id: "testid".to_string(),
            name: "Test".to_string(),
            version: "1.0".to_string(),
            description: None,
            enabled: true,
            install_time: None,
            install_source: None,
            update_url: None,
            was_installed_by_default: None,
            permissions: vec![],
            host_permissions: vec![],
            has_content_scripts: false,
            has_background: false,
            preferences_tampered: false,
            risk_flags: vec![],
            ioc_matches: vec![],
            path: PathBuf::from("/tmp/test"),
        };
        overrides(&mut ext);
        ext
    }

    // === match_pattern_to_regex 单元测试 ===

    #[test]
    fn all_urls_matches_any_domain() {
        assert!(domain_matches_host_pattern("evil.com", "<all_urls>"));
        assert!(domain_matches_host_pattern("sub.evil.com", "<all_urls>"));
        assert!(domain_matches_host_pattern("anything.example.org", "<all_urls>"));
    }

    #[test]
    fn wildcard_scheme_and_subdomain() {
        // *://*.example.com/* 匹配子域名和自身
        assert!(domain_matches_host_pattern("sub.example.com", "*://*.example.com/*"));
        assert!(domain_matches_host_pattern("example.com", "*://*.example.com/*"));
        assert!(domain_matches_host_pattern("a.b.example.com", "*://*.example.com/*"));
        // 不匹配其他域名
        assert!(!domain_matches_host_pattern("notexample.com", "*://*.example.com/*"));
    }

    #[test]
    fn exact_host_no_subdomain() {
        // *://example.com/* 匹配 example.com 但不匹配子域名
        assert!(domain_matches_host_pattern("example.com", "*://example.com/*"));
        assert!(!domain_matches_host_pattern("sub.example.com", "*://example.com/*"));
    }

    #[test]
    fn https_scheme_pattern() {
        // https://*.example.com/* 匹配域名（域名匹配忽略 scheme）
        assert!(domain_matches_host_pattern(
            "sub.example.com",
            "https://*.example.com/*"
        ));
        assert!(domain_matches_host_pattern("example.com", "https://*.example.com/*"));
        // 精确匹配
        assert!(domain_matches_host_pattern("example.com", "https://example.com/*"));
        assert!(!domain_matches_host_pattern("sub.example.com", "https://example.com/*"));
    }

    #[test]
    fn http_scheme_pattern() {
        assert!(domain_matches_host_pattern("example.com", "http://example.com/*"));
        assert!(!domain_matches_host_pattern("sub.example.com", "http://example.com/*"));
    }

    #[test]
    fn wildcard_host_matches_any() {
        assert!(domain_matches_host_pattern("anything.com", "*://*/*"));
    }

    #[test]
    fn path_pattern_ignored_for_domain_match() {
        // 域名匹配忽略 path 部分
        assert!(domain_matches_host_pattern("example.com", "*://example.com/api/*"));
        assert!(!domain_matches_host_pattern("sub.example.com", "*://example.com/api/*"));
    }

    // === 敏感权限组合测试 ===

    #[test]
    fn webrequest_sensitive_permissions() {
        let ext = make_ext(|e| {
            e.permissions = vec!["webRequest".to_string()];
            e.host_permissions = vec!["<all_urls>".to_string()];
        });
        let result = match_extension("unrelated.com", &ext);
        assert!(result.is_some());
        let matched = result.unwrap();
        assert!(matched.has_sensitive_permissions);
        // <all_urls> 匹配所有域名
        assert!(matched.matched_patterns.contains(&"<all_urls>".to_string()));
    }

    #[test]
    fn webrequest_without_all_urls_not_sensitive() {
        let ext = make_ext(|e| {
            e.permissions = vec!["webRequest".to_string()];
            e.host_permissions = vec!["*://specific.com/*".to_string()];
        });
        // 不匹配 unrelated.com，且不是敏感权限组合
        let result = match_extension("unrelated.com", &ext);
        assert!(result.is_none());
    }

    #[test]
    fn webrequest_all_urls_in_permissions() {
        let ext = make_ext(|e| {
            e.permissions = vec!["webRequest".to_string(), "<all_urls>".to_string()];
            e.host_permissions = vec![];
        });
        let result = match_extension("any.com", &ext);
        assert!(result.is_some());
        let matched = result.unwrap();
        assert!(matched.has_sensitive_permissions);
    }

    // === match_domain_to_extensions 集成测试 ===

    #[test]
    fn match_domain_full_integration() {
        let extensions = vec![
            // 高风险扩展：webRequest + <all_urls>
            make_ext(|e| {
                e.id = "ext1".to_string();
                e.name = "Ad Blocker Pro".to_string();
                e.version = "2.0".to_string();
                e.permissions = vec!["webRequest".to_string(), "tabs".to_string(), "cookies".to_string()];
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["high_privilege_combo".to_string(), "broad_host_access".to_string()];
            }),
            // 中等风险：特定域名匹配
            make_ext(|e| {
                e.id = "ext2".to_string();
                e.name = "Site Helper".to_string();
                e.version = "1.0".to_string();
                e.host_permissions = vec!["*://*.evil.com/*".to_string()];
                e.risk_flags = vec!["content_script_inject".to_string()];
                e.has_content_scripts = true;
            }),
            // 无风险：不匹配
            make_ext(|e| {
                e.id = "ext3".to_string();
                e.name = "Safe Extension".to_string();
                e.version = "1.0".to_string();
                e.host_permissions = vec!["*://*.safe.com/*".to_string()];
                e.risk_flags = vec![];
            }),
        ];

        let result = match_domain_to_extensions("evil.com", &extensions);

        // 应匹配 ext1 (<all_urls>) 和 ext2 (*.evil.com)
        assert_eq!(result.matching_extensions.len(), 2);

        // ext1 应排在前面（high_privilege_combo 优先级最高）
        assert_eq!(result.matching_extensions[0].id, "ext1");
        assert!(result.matching_extensions[0].has_sensitive_permissions);
        assert!(result.matching_extensions[0]
            .matched_patterns
            .contains(&"<all_urls>".to_string()));

        // ext2 排在第二
        assert_eq!(result.matching_extensions[1].id, "ext2");
        assert!(!result.matching_extensions[1].has_sensitive_permissions);
        assert!(result.matching_extensions[1]
            .matched_patterns
            .contains(&"*://*.evil.com/*".to_string()));
    }

    #[test]
    fn no_matching_extensions() {
        let extensions = vec![make_ext(|e| {
            e.host_permissions = vec!["*://*.safe.com/*".to_string()];
        })];
        let result = match_domain_to_extensions("evil.com", &extensions);
        assert!(result.matching_extensions.is_empty());
    }

    #[test]
    fn sorting_by_risk_flag_priority() {
        let extensions = vec![
            // content_script_inject (priority 2)
            make_ext(|e| {
                e.id = "ext_cs".to_string();
                e.name = "Content Script Ext".to_string();
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["content_script_inject".to_string()];
            }),
            // high_privilege_combo (priority 0)
            make_ext(|e| {
                e.id = "ext_hp".to_string();
                e.name = "High Privilege Ext".to_string();
                e.permissions = vec!["webRequest".to_string()];
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["high_privilege_combo".to_string()];
            }),
            // broad_host_access (priority 1)
            make_ext(|e| {
                e.id = "ext_broad".to_string();
                e.name = "Broad Access Ext".to_string();
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["broad_host_access".to_string()];
            }),
        ];

        let result = match_domain_to_extensions("any.com", &extensions);
        assert_eq!(result.matching_extensions.len(), 3);

        // 排序：high_privilege_combo > broad_host_access > content_script_inject
        assert_eq!(result.matching_extensions[0].id, "ext_hp");
        assert_eq!(result.matching_extensions[1].id, "ext_broad");
        assert_eq!(result.matching_extensions[2].id, "ext_cs");
    }

    #[test]
    fn sorting_same_risk_level_by_name() {
        let extensions = vec![
            make_ext(|e| {
                e.id = "ext_b".to_string();
                e.name = "Bravo".to_string();
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["broad_host_access".to_string()];
            }),
            make_ext(|e| {
                e.id = "ext_a".to_string();
                e.name = "Alpha".to_string();
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["broad_host_access".to_string()];
            }),
        ];

        let result = match_domain_to_extensions("any.com", &extensions);
        assert_eq!(result.matching_extensions[0].name, "Alpha");
        assert_eq!(result.matching_extensions[1].name, "Bravo");
    }

    #[test]
    fn extensions_with_risk_flags_prioritized() {
        let extensions = vec![
            // 无 risk_flags
            make_ext(|e| {
                e.id = "ext_no_risk".to_string();
                e.name = "No Risk".to_string();
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec![];
            }),
            // 有 risk_flags
            make_ext(|e| {
                e.id = "ext_risk".to_string();
                e.name = "With Risk".to_string();
                e.host_permissions = vec!["<all_urls>".to_string()];
                e.risk_flags = vec!["broad_host_access".to_string()];
            }),
        ];

        let result = match_domain_to_extensions("any.com", &extensions);
        assert_eq!(result.matching_extensions[0].id, "ext_risk");
        assert_eq!(result.matching_extensions[1].id, "ext_no_risk");
    }

    #[test]
    fn subdomain_matching_with_wildcard() {
        // *://*.evil.com/* 应匹配 sub.evil.com
        let ext = make_ext(|e| {
            e.host_permissions = vec!["*://*.evil.com/*".to_string()];
        });
        let result = match_extension("sub.evil.com", &ext);
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .matched_patterns
            .contains(&"*://*.evil.com/*".to_string()));
    }

    #[test]
    fn exact_host_rejects_subdomain() {
        // *://evil.com/* 不应匹配 sub.evil.com
        let ext = make_ext(|e| {
            e.host_permissions = vec!["*://evil.com/*".to_string()];
        });
        let result = match_extension("sub.evil.com", &ext);
        assert!(result.is_none());
    }

    #[test]
    fn invalid_pattern_returns_false() {
        assert!(!domain_matches_host_pattern("example.com", "not-a-valid-pattern"));
    }

    #[test]
    fn multiple_patterns_matched() {
        let ext = make_ext(|e| {
            e.host_permissions = vec![
                "<all_urls>".to_string(),
                "*://*.evil.com/*".to_string(),
                "*://safe.com/*".to_string(),
            ];
        });
        let result = match_extension("evil.com", &ext);
        assert!(result.is_some());
        let matched = result.unwrap();
        // <all_urls> 和 *://*.evil.com/* 都匹配
        assert!(matched.matched_patterns.contains(&"<all_urls>".to_string()));
        assert!(matched.matched_patterns.contains(&"*://*.evil.com/*".to_string()));
        // *://safe.com/* 不匹配
        assert!(!matched.matched_patterns.contains(&"*://safe.com/*".to_string()));
    }

    // === match_pattern_to_regex 单元测试（完整 URL 匹配）===

    #[test]
    fn regex_all_urls() {
        let re = match_pattern_to_regex("<all_urls>").unwrap();
        assert!(re.is_match("https://evil.com/path"));
        assert!(re.is_match("http://evil.com/"));
    }

    #[test]
    fn regex_wildcard_scheme() {
        let re = match_pattern_to_regex("*://example.com/*").unwrap();
        assert!(re.is_match("http://example.com/"));
        assert!(re.is_match("https://example.com/"));
        assert!(re.is_match("https://example.com/path/to/page"));
        assert!(!re.is_match("https://sub.example.com/"));
    }

    #[test]
    fn regex_https_only() {
        let re = match_pattern_to_regex("https://example.com/*").unwrap();
        assert!(re.is_match("https://example.com/"));
        assert!(!re.is_match("http://example.com/"));
    }

    #[test]
    fn regex_subdomain_wildcard() {
        let re = match_pattern_to_regex("*://*.example.com/*").unwrap();
        assert!(re.is_match("https://sub.example.com/"));
        assert!(re.is_match("http://a.b.example.com/path"));
        assert!(re.is_match("https://example.com/"));
        assert!(!re.is_match("https://notexample.com/"));
    }
}
