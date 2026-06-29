//! CDP 归因融合纯函数。
//!
//! 当 `irtool-net-monitor` 检测到恶意连接时，查询 CDP 会话中匹配的 request，
//! 命中则生成 `CdpMatchResult`（由 service 层转换为 `TabAttribution`）。
//!
//! ## 匹配条件
//!
//! - URL hostname 匹配恶意 domain（精确匹配或子域名后缀匹配）
//! - 时间窗内（默认 ±5s，CDP timestamp 与恶意连接时间比较）
//!
//! ## 关键约束
//!
//! - hostname 匹配不能用 `contains`（避免 notevil.com 误判 evil.com）
//! - 命中后 confidence 为 Confirmed（CDP 是铁证）

use crate::events::CdpRequest;

/// CDP 归因匹配结果（独立类型，service 层转换为 TabAttribution）
#[derive(Debug, Clone, PartialEq)]
pub struct CdpMatchResult {
    /// 归因置信度（CDP 命中恒为 Confirmed）
    pub confidence: CdpConfidence,
    /// documentURL（顶层文档 URL，归因铁证）
    pub url: String,
    /// 匹配的 CDP request id（便于追溯）
    pub request_id: String,
    /// 匹配的 CDP request url（恶意请求本身）
    pub matched_url: String,
    /// 来自 target url 的扩展 ID（仅当 target 是 chrome-extension:// 时存在）
    pub extension_id: Option<String>,
    /// target 类型：page / service_worker / background_page 等
    pub target_type: Option<String>,
}

/// CDP 归因置信度（独立枚举，避免跨 crate 依赖）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpConfidence {
    /// CDP 命中 → 铁证
    Confirmed,
}

/// 从 URL 提取 hostname，失败返回 None
fn url_to_hostname(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .map(|u| u.host_str().unwrap_or("").to_string())
        .filter(|h| !h.is_empty())
}

/// 判断 hostname 是否匹配 target domain（精确匹配或子域名后缀匹配）
///
/// 例如 target="evil.com" 匹配 hostname="evil.com" 或 "sub.evil.com"，
/// **不匹配** "notevil.com"（避免子串陷阱）
fn hostname_matches_domain(hostname: &str, target: &str) -> bool {
    if hostname == target {
        return true;
    }
    let suffix = format!(".{}", target);
    hostname.ends_with(&suffix)
}

/// 判断 CDP request 是否匹配恶意连接（domain + 时间窗）
///
/// - `cdp_req`: CDP 抓取的请求
/// - `malicious_domain`: 恶意连接的域名
/// - `malicious_timestamp_secs`: 恶意连接时间戳（秒，epoch）
/// - `time_window_secs`: 时间窗（秒，默认 5.0）
///
/// 匹配条件：request URL hostname 匹配恶意 domain 且 |cdp.timestamp - malicious_timestamp| <= time_window
pub fn match_cdp_to_malicious(
    cdp_req: &CdpRequest,
    malicious_domain: &str,
    malicious_timestamp_secs: f64,
    time_window_secs: f64,
) -> bool {
    let hostname = match url_to_hostname(&cdp_req.url) {
        Some(h) => h,
        None => return false,
    };
    if !hostname_matches_domain(&hostname, malicious_domain) {
        return false;
    }
    let time_diff = (cdp_req.timestamp - malicious_timestamp_secs).abs();
    time_diff <= time_window_secs
}

/// 从 target url 提取扩展 ID。
/// target url 形如 `chrome-extension://<32位id>/_generated_background_page.html`
/// 返回 `None` 如果不是扩展 target。
///
/// Chrome 扩展 ID 为 32 个字符（a-p 字母），本函数按长度校验。
pub fn extract_extension_id(target_url: &str) -> Option<String> {
    const PREFIX: &str = "chrome-extension://";
    let rest = target_url.strip_prefix(PREFIX)?;
    // 扩展 ID 之后是 `/`（如 `chrome-extension://<id>/path`），或 url 仅含 ID
    let id_end = rest.find('/').unwrap_or(rest.len());
    let id = &rest[..id_end];
    if id.len() == 32 {
        Some(id.to_string())
    } else {
        None
    }
}

/// 从匹配的 CDP request 构建归因结果
///
/// 命中后 confidence 恒为 Confirmed，url 为 documentURL（顶层文档）。
/// 若 documentURL 为空则回退到 request url。
///
/// - `target_url`/`target_type`：来自 `SessionManager` 的 `TargetInfo`，
///   用于填充 `extension_id` 和 `target_type` 字段。
/// - 当 `target_type` 为 `page`/`service_worker`/`background_page` 时 confidence 保持 Confirmed
///   （page 请求与扩展 SW 请求均为铁证）。
pub fn build_match_result(cdp_req: &CdpRequest, target_url: &str, target_type: &str) -> CdpMatchResult {
    let url = if cdp_req.document_url.is_empty() {
        cdp_req.url.clone()
    } else {
        cdp_req.document_url.clone()
    };
    CdpMatchResult {
        confidence: CdpConfidence::Confirmed,
        url,
        request_id: cdp_req.request_id.clone(),
        matched_url: cdp_req.url.clone(),
        extension_id: extract_extension_id(target_url),
        target_type: Some(target_type.to_string()),
    }
}

/// 在一批 CDP requests 中查找匹配恶意连接的第一个，返回归因结果
///
/// 时间复杂度 O(n)，命中即返回（第一个匹配项）。
///
/// - `target_url`/`target_type`：透传给 `build_match_result` 用于扩展识别。
pub fn find_match(
    cdp_requests: &[CdpRequest],
    malicious_domain: &str,
    malicious_timestamp_secs: f64,
    time_window_secs: f64,
    target_url: &str,
    target_type: &str,
) -> Option<CdpMatchResult> {
    cdp_requests
        .iter()
        .find(|req| match_cdp_to_malicious(req, malicious_domain, malicious_timestamp_secs, time_window_secs))
        .map(|req| build_match_result(req, target_url, target_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CdpInitiator;

    fn make_cdp_request(url: &str, document_url: &str, timestamp: f64) -> CdpRequest {
        CdpRequest {
            request_id: "req-1".to_string(),
            url: url.to_string(),
            method: "GET".to_string(),
            document_url: document_url.to_string(),
            initiator: CdpInitiator {
                init_type: "parser".to_string(),
                url: None,
                stack: None,
            },
            frame_id: "frame-1".to_string(),
            loader_id: "loader-1".to_string(),
            timestamp,
        }
    }

    #[test]
    fn url_to_hostname_normal() {
        assert_eq!(url_to_hostname("https://evil.com/path"), Some("evil.com".to_string()));
        assert_eq!(
            url_to_hostname("http://sub.example.com:8080/x"),
            Some("sub.example.com".to_string())
        );
    }

    #[test]
    fn url_to_hostname_invalid() {
        assert_eq!(url_to_hostname("not-a-url"), None);
        assert_eq!(url_to_hostname(""), None);
    }

    #[test]
    fn hostname_matches_domain_exact() {
        assert!(hostname_matches_domain("evil.com", "evil.com"));
    }

    #[test]
    fn hostname_matches_domain_subdomain() {
        assert!(hostname_matches_domain("sub.evil.com", "evil.com"));
        assert!(hostname_matches_domain("a.b.evil.com", "evil.com"));
    }

    #[test]
    fn hostname_matches_domain_not_substring() {
        // notevil.com 不应匹配 evil.com（子串陷阱）
        assert!(!hostname_matches_domain("notevil.com", "evil.com"));
        assert!(!hostname_matches_domain("evil.com.evil.attacker.com", "evil.com"));
    }

    #[test]
    fn hostname_matches_domain_different() {
        assert!(!hostname_matches_domain("example.com", "evil.com"));
        assert!(!hostname_matches_domain("", "evil.com"));
    }

    #[test]
    fn match_cdp_to_malicious_domain_and_time_match() {
        let req = make_cdp_request("https://evil.com/payload", "https://evil.com/landing", 1000.0);
        assert!(match_cdp_to_malicious(&req, "evil.com", 1000.0, 5.0));
        assert!(match_cdp_to_malicious(&req, "evil.com", 1003.0, 5.0));
        assert!(match_cdp_to_malicious(&req, "evil.com", 1005.0, 5.0));
    }

    #[test]
    fn match_cdp_to_malicious_time_out_of_window() {
        let req = make_cdp_request("https://evil.com/payload", "https://evil.com/landing", 1000.0);
        assert!(!match_cdp_to_malicious(&req, "evil.com", 1006.0, 5.0));
        assert!(!match_cdp_to_malicious(&req, "evil.com", 994.0, 5.0));
    }

    #[test]
    fn match_cdp_to_malicious_domain_mismatch() {
        let req = make_cdp_request("https://example.com/x", "https://example.com/", 1000.0);
        assert!(!match_cdp_to_malicious(&req, "evil.com", 1000.0, 5.0));
    }

    #[test]
    fn match_cdp_to_malicious_subdomain_match() {
        let req = make_cdp_request("https://sub.evil.com/x", "https://sub.evil.com/", 1000.0);
        assert!(match_cdp_to_malicious(&req, "evil.com", 1000.0, 5.0));
    }

    #[test]
    fn match_cdp_to_malicious_not_substring() {
        let req = make_cdp_request("https://notevil.com/x", "https://notevil.com/", 1000.0);
        assert!(!match_cdp_to_malicious(&req, "evil.com", 1000.0, 5.0));
    }

    #[test]
    fn match_cdp_to_malicious_invalid_url() {
        let req = make_cdp_request("not-a-url", "doc", 1000.0);
        assert!(!match_cdp_to_malicious(&req, "evil.com", 1000.0, 5.0));
    }

    #[test]
    fn build_match_result_uses_document_url() {
        let req = make_cdp_request("https://evil.com/payload", "https://evil.com/landing.html", 1000.0);
        let result = build_match_result(&req, "https://evil.com/landing.html", "page");
        assert_eq!(result.confidence, CdpConfidence::Confirmed);
        assert_eq!(result.url, "https://evil.com/landing.html");
        assert_eq!(result.request_id, "req-1");
        assert_eq!(result.matched_url, "https://evil.com/payload");
        // 非 chrome-extension target → extension_id 为 None
        assert_eq!(result.extension_id, None);
        assert_eq!(result.target_type.as_deref(), Some("page"));
    }

    #[test]
    fn build_match_result_fallback_to_request_url_when_document_empty() {
        let mut req = make_cdp_request("https://evil.com/payload", "https://evil.com/landing", 1000.0);
        req.document_url = "".to_string();
        let result = build_match_result(&req, "https://evil.com/landing", "page");
        assert_eq!(result.url, "https://evil.com/payload");
    }

    #[test]
    fn build_match_result_with_extension_service_worker() {
        // 扩展 SW target：target_url 形如 chrome-extension://<32位id>/...
        let req = make_cdp_request(
            "https://evil.com/beacon",
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/_generated_background_page.html",
            1000.0,
        );
        let target_url = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/_generated_background_page.html";
        let result = build_match_result(&req, target_url, "service_worker");
        assert_eq!(result.confidence, CdpConfidence::Confirmed);
        assert_eq!(result.extension_id.as_deref(), Some("abcdefghijklmnopabcdefghijklmnop"));
        assert_eq!(result.target_type.as_deref(), Some("service_worker"));
    }

    #[test]
    fn build_match_result_with_extension_background_page() {
        // 旧式 background_page target（Manifest V2）
        let req = make_cdp_request(
            "https://evil.com/bg",
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/background.html",
            1000.0,
        );
        let target_url = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/background.html";
        let result = build_match_result(&req, target_url, "background_page");
        assert_eq!(result.confidence, CdpConfidence::Confirmed);
        assert_eq!(result.extension_id.as_deref(), Some("abcdefghijklmnopabcdefghijklmnop"));
        assert_eq!(result.target_type.as_deref(), Some("background_page"));
    }

    #[test]
    fn build_match_result_page_target_no_extension() {
        // 普通 page target：url 不是 chrome-extension:// → extension_id 为 None
        let req = make_cdp_request("https://evil.com/x", "https://evil.com/landing", 1000.0);
        let result = build_match_result(&req, "https://evil.com/landing", "page");
        assert_eq!(result.extension_id, None);
        assert_eq!(result.target_type.as_deref(), Some("page"));
        assert_eq!(result.confidence, CdpConfidence::Confirmed);
    }

    #[test]
    fn find_match_returns_first_match() {
        let reqs = vec![
            make_cdp_request("https://example.com/x", "doc1", 1000.0),
            make_cdp_request("https://evil.com/payload", "https://evil.com/landing", 1000.0),
            make_cdp_request("https://evil.com/another", "doc2", 1000.0),
        ];
        let result = find_match(&reqs, "evil.com", 1000.0, 5.0, "https://evil.com/landing", "page");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.matched_url, "https://evil.com/payload");
        assert_eq!(result.url, "https://evil.com/landing");
        assert_eq!(result.target_type.as_deref(), Some("page"));
        assert_eq!(result.extension_id, None);
    }

    #[test]
    fn find_match_no_match_returns_none() {
        let reqs = vec![
            make_cdp_request("https://example.com/x", "doc1", 1000.0),
            make_cdp_request("https://notevil.com/y", "doc2", 1000.0),
        ];
        let result = find_match(&reqs, "evil.com", 1000.0, 5.0, "https://example.com", "page");
        assert!(result.is_none());
    }

    #[test]
    fn find_match_extension_service_worker_target() {
        // 命中扩展 SW 请求：extension_id 应被填充
        let ext_id = "abcdefghijklmnopabcdefghijklmnop";
        let target_url = format!("chrome-extension://{}/_generated_background_page.html", ext_id);
        let reqs = vec![make_cdp_request("https://evil.com/exfil", &target_url, 1000.0)];
        let result = find_match(&reqs, "evil.com", 1000.0, 5.0, &target_url, "service_worker");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.confidence, CdpConfidence::Confirmed);
        assert_eq!(result.extension_id.as_deref(), Some(ext_id));
        assert_eq!(result.target_type.as_deref(), Some("service_worker"));
    }

    // ── extract_extension_id 测试 ──────────────────────────────────

    #[test]
    fn extract_extension_id_normal_with_path() {
        let url = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/_generated_background_page.html";
        assert_eq!(
            extract_extension_id(url),
            Some("abcdefghijklmnopabcdefghijklmnop".to_string())
        );
    }

    #[test]
    fn extract_extension_id_no_path() {
        // url 仅含扩展 ID（无尾随路径）
        let url = "chrome-extension://abcdefghijklmnopabcdefghijklmnop";
        assert_eq!(
            extract_extension_id(url),
            Some("abcdefghijklmnopabcdefghijklmnop".to_string())
        );
    }

    #[test]
    fn extract_extension_id_wrong_length_returns_none() {
        // 非 32 位 ID → None
        let url = "chrome-extension://short/background.html";
        assert_eq!(extract_extension_id(url), None);
    }

    #[test]
    fn extract_extension_id_not_extension_scheme() {
        // 非 chrome-extension:// 协议 → None
        assert_eq!(extract_extension_id("https://example.com/page"), None);
        assert_eq!(extract_extension_id("http://evil.com/x"), None);
        assert_eq!(extract_extension_id("about:blank"), None);
    }

    #[test]
    fn extract_extension_id_empty_string() {
        assert_eq!(extract_extension_id(""), None);
    }
}
