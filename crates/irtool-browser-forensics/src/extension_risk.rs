//! 扩展风险标注 + IOC 精确匹配

use crate::extension_inventory::ExtensionInfo;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::OnceLock;
use tracing::warn;

/// IOC 匹配结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IocMatch {
    pub ioc_type: String,
    pub value: String,
    pub severity: String,
}

/// IOC 条目（来自 JSON 文件）
#[derive(Debug, Clone, Deserialize)]
pub struct IocEntry {
    pub ioc_type: String, // "extension_id", "update_url", "name"
    pub value: String,
    pub severity: String, // "high", "medium", "low"
    pub description: String,
}

/// 扩展权限风险权重表（可配置，参考设计方案）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionWeights {
    pub all_urls: u32,             // <all_urls>，最危险
    pub web_request: u32,          // webRequest
    pub web_request_blocking: u32, // webRequestBlocking
    pub cookies: u32,              // cookies
    pub tabs: u32,                 // tabs
    pub scripting: u32,            // scripting
    pub content_scripts: u32,      // 声明 content_scripts
    pub native_messaging: u32,     // nativeMessaging
    pub file_system: u32,          // fileSystem
}

impl Default for PermissionWeights {
    fn default() -> Self {
        Self {
            all_urls: 40,
            web_request: 20,
            web_request_blocking: 20,
            cookies: 15,
            tabs: 10,
            scripting: 15,
            content_scripts: 10,
            native_messaging: 15,
            file_system: 15,
        }
    }
}

/// 扩展风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    /// 根据数值评分推断风险等级
    /// - >= 70: High
    /// - >= 40: Medium
    /// - 其他: Low
    pub fn from_score(score: u32) -> Self {
        match score {
            s if s >= 70 => RiskLevel::High,
            s if s >= 40 => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }
}

/// 官方商店域名
const OFFICIAL_STORE_DOMAINS: &[&str] = &["google.com", "microsoft.com"];

/// 风险标注规则
///
/// 基于权限组合和元数据对扩展进行风险标注，不做数值评分。
pub fn compute_risk_flags(ext: &ExtensionInfo) -> Vec<String> {
    let mut flags = Vec::new();

    // high_privilege_combo: 同时拥有 webRequest + tabs + cookies + <all_urls>
    let has_webrequest = ext.permissions.iter().any(|p| p == "webRequest");
    let has_tabs = ext.permissions.iter().any(|p| p == "tabs");
    let has_cookies = ext.permissions.iter().any(|p| p == "cookies");
    let has_all_urls =
        ext.host_permissions.iter().any(|p| p == "<all_urls>") || ext.permissions.iter().any(|p| p == "<all_urls>");
    if has_webrequest && has_tabs && has_cookies && has_all_urls {
        flags.push("high_privilege_combo".to_string());
    }

    // broad_host_access: host_permissions 包含 <all_urls>
    if ext.host_permissions.iter().any(|p| p == "<all_urls>") {
        flags.push("broad_host_access".to_string());
    }

    // content_script_inject: 声明 content_scripts
    if ext.has_content_scripts {
        flags.push("content_script_inject".to_string());
    }

    // side_loaded: install_source 为 side_loaded
    if ext.install_source.as_deref() == Some("side_loaded") {
        flags.push("side_loaded".to_string());
    }

    // unknown_update_url: update_url 不属于官方商店
    if let Some(ref url) = ext.update_url {
        if !is_official_store_url(url) {
            flags.push("unknown_update_url".to_string());
        }
    }

    // preferences_tampered: Secure Preferences HMAC 校验失败
    if ext.preferences_tampered {
        flags.push("preferences_tampered".to_string());
    }

    // recently_installed: 安装时间 < 7 天
    if let Some(ref install_time) = ext.install_time {
        if is_recently_installed(install_time) {
            flags.push("recently_installed".to_string());
        }
    }

    flags
}

/// 计算扩展的数值风险评分（0-100）
///
/// 遍历 permissions + host_permissions + content_scripts 声明，
/// 按权重表累加，封顶 100。IOC 命中直接 100。
pub fn compute_risk_score(ext: &ExtensionInfo) -> u32 {
    compute_risk_score_with_weights(ext, &PermissionWeights::default())
}

/// 用自定义权重计算风险评分
pub fn compute_risk_score_with_weights(ext: &ExtensionInfo, weights: &PermissionWeights) -> u32 {
    // IOC 命中直接满分
    if !ext.ioc_matches.is_empty() {
        return 100;
    }

    let mut score: u32 = 0;

    // 检查 permissions 数组
    for perm in &ext.permissions {
        match perm.as_str() {
            "<all_urls>" => score += weights.all_urls,
            "webRequest" => score += weights.web_request,
            "webRequestBlocking" => score += weights.web_request_blocking,
            "cookies" => score += weights.cookies,
            "tabs" => score += weights.tabs,
            "scripting" => score += weights.scripting,
            "nativeMessaging" => score += weights.native_messaging,
            "fileSystem" | "fileSystem.write" => score += weights.file_system,
            _ => {}
        }
    }

    // 检查 host_permissions（Manifest V3）
    for host in &ext.host_permissions {
        if host == "<all_urls>" {
            score += weights.all_urls;
        }
    }

    // content_scripts 声明加分
    if ext.has_content_scripts {
        score += weights.content_scripts;
    }

    score.min(100)
}

/// IOC 精确匹配
///
/// 匹配扩展信息与全局 IOC 数据库，返回所有命中的匹配结果。
pub fn match_ioc(ext: &ExtensionInfo) -> Vec<IocMatch> {
    let ioc_db = get_or_load_ioc_database();
    let mut matches = Vec::new();
    for entry in ioc_db {
        let matched = match entry.ioc_type.as_str() {
            "extension_id" => ext.id == entry.value,
            "update_url" => ext.update_url.as_deref() == Some(&entry.value),
            "name" => ext.name.to_lowercase().contains(&entry.value.to_lowercase()),
            _ => false,
        };
        if matched {
            matches.push(IocMatch {
                ioc_type: entry.ioc_type.clone(),
                value: entry.value.clone(),
                severity: entry.severity.clone(),
            });
        }
    }
    matches
}

/// 从本地 JSON 文件加载 IOC 列表
///
/// 文件路径: %APPDATA%/irtool/browser-forensics/ioc.json
/// 如果文件不存在或解析失败，返回空列表。
fn load_ioc_database(app_dirs: &irtool_core::AppDirs) -> Vec<IocEntry> {
    let path = app_dirs.data_dir().join("browser-forensics").join("ioc.json");
    if !path.exists() {
        return vec![];
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            warn!("failed to parse IOC database: {}", e);
            vec![]
        }),
        Err(e) => {
            warn!("failed to read IOC database: {}", e);
            vec![]
        }
    }
}

/// 延迟加载 / 缓存的 IOC 数据库（进程级别）
fn get_or_load_ioc_database() -> &'static Vec<IocEntry> {
    static IOC_DB: OnceLock<Vec<IocEntry>> = OnceLock::new();
    IOC_DB.get_or_init(|| {
        let app_dirs = irtool_core::AppDirs::detect();
        load_ioc_database(&app_dirs)
    })
}

/// 检查 update_url 是否属于官方商店
fn is_official_store_url(url: &str) -> bool {
    OFFICIAL_STORE_DOMAINS.iter().any(|domain| url.contains(domain))
}

/// 检查安装时间是否在 7 天以内
fn is_recently_installed(install_time: &str) -> bool {
    let Some(dt) = dt_from_rfc3339(install_time) else {
        return false;
    };
    let now = Utc::now();
    let seven_days = chrono::Duration::days(7);
    now.signed_duration_since(dt) < seven_days
}

/// 解析 RFC3339 时间字符串
fn dt_from_rfc3339(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.to_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
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
            risk_score: 0,
            risk_level: RiskLevel::Low,
        };
        overrides(&mut ext);
        ext
    }

    #[test]
    fn no_risk_flags_for_benign_extension() {
        let ext = make_ext(|_| {});
        let flags = compute_risk_flags(&ext);
        assert!(flags.is_empty());
    }

    #[test]
    fn high_privilege_combo() {
        let ext = make_ext(|e| {
            e.permissions = vec!["webRequest".to_string(), "tabs".to_string(), "cookies".to_string()];
            e.host_permissions = vec!["<all_urls>".to_string()];
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"high_privilege_combo".to_string()));
        assert!(flags.contains(&"broad_host_access".to_string()));
    }

    #[test]
    fn high_privilege_combo_missing_cookie() {
        let ext = make_ext(|e| {
            e.permissions = vec!["webRequest".to_string(), "tabs".to_string()];
            e.host_permissions = vec!["<all_urls>".to_string()];
        });
        let flags = compute_risk_flags(&ext);
        assert!(!flags.contains(&"high_privilege_combo".to_string()));
    }

    #[test]
    fn broad_host_access() {
        let ext = make_ext(|e| {
            e.host_permissions = vec!["<all_urls>".to_string()];
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"broad_host_access".to_string()));
    }

    #[test]
    fn content_script_inject() {
        let ext = make_ext(|e| {
            e.has_content_scripts = true;
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"content_script_inject".to_string()));
    }

    #[test]
    fn side_loaded() {
        let ext = make_ext(|e| {
            e.install_source = Some("side_loaded".to_string());
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"side_loaded".to_string()));
    }

    #[test]
    fn unknown_update_url() {
        let ext = make_ext(|e| {
            e.update_url = Some("https://evil.com/update.crx".to_string());
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"unknown_update_url".to_string()));
    }

    #[test]
    fn official_google_update_url() {
        let ext = make_ext(|e| {
            e.update_url = Some("https://clients2.google.com/service/update2/crx".to_string());
        });
        let flags = compute_risk_flags(&ext);
        assert!(!flags.contains(&"unknown_update_url".to_string()));
    }

    #[test]
    fn official_microsoft_update_url() {
        let ext = make_ext(|e| {
            e.update_url = Some("https://edge.microsoft.com/extension/update".to_string());
        });
        let flags = compute_risk_flags(&ext);
        assert!(!flags.contains(&"unknown_update_url".to_string()));
    }

    #[test]
    fn preferences_tampered() {
        let ext = make_ext(|e| {
            e.preferences_tampered = true;
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"preferences_tampered".to_string()));
    }

    #[test]
    fn recently_installed() {
        let now = Utc::now().to_rfc3339();
        let ext = make_ext(|e| {
            e.install_time = Some(now);
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"recently_installed".to_string()));
    }

    #[test]
    fn not_recently_installed() {
        let old = chrono::Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap().to_rfc3339();
        let ext = make_ext(|e| {
            e.install_time = Some(old);
        });
        let flags = compute_risk_flags(&ext);
        assert!(!flags.contains(&"recently_installed".to_string()));
    }

    #[test]
    fn match_ioc_returns_empty() {
        let ext = make_ext(|_| {});
        assert!(match_ioc(&ext).is_empty());
    }

    #[test]
    fn is_official_store_url_google() {
        assert!(is_official_store_url("https://clients2.google.com/service/update2/crx"));
    }

    #[test]
    fn is_official_store_url_microsoft() {
        assert!(is_official_store_url("https://edge.microsoft.com/extension/update"));
    }

    #[test]
    fn is_official_store_url_evil() {
        assert!(!is_official_store_url("https://evil.com/update.crx"));
    }

    #[test]
    fn high_privilege_combo_all_urls_in_permissions() {
        // <all_urls> 可能在 permissions 而非 host_permissions 中
        let ext = make_ext(|e| {
            e.permissions = vec![
                "webRequest".to_string(),
                "tabs".to_string(),
                "cookies".to_string(),
                "<all_urls>".to_string(),
            ];
        });
        let flags = compute_risk_flags(&ext);
        assert!(flags.contains(&"high_privilege_combo".to_string()));
    }

    // === P0.6 数值风险评分测试 ===

    #[test]
    fn compute_risk_score_all_urls_only() {
        // 仅 <all_urls> 权限 → score=40, level=Medium（40 >= 40 阈值）
        let ext = make_ext(|e| {
            e.permissions = vec!["<all_urls>".to_string()];
        });
        let score = compute_risk_score(&ext);
        assert_eq!(score, 40);
        assert_eq!(RiskLevel::from_score(score), RiskLevel::Medium);
    }

    #[test]
    fn compute_risk_score_high_privilege_combo() {
        // webRequest(20) + tabs(10) + cookies(15) + <all_urls>(40) = 85, level=High
        let ext = make_ext(|e| {
            e.permissions = vec![
                "webRequest".to_string(),
                "tabs".to_string(),
                "cookies".to_string(),
                "<all_urls>".to_string(),
            ];
        });
        let score = compute_risk_score(&ext);
        assert_eq!(score, 85);
        assert_eq!(RiskLevel::from_score(score), RiskLevel::High);
    }

    #[test]
    fn compute_risk_score_no_permissions() {
        // 无权限 → score=0, level=Low
        let ext = make_ext(|_| {});
        let score = compute_risk_score(&ext);
        assert_eq!(score, 0);
        assert_eq!(RiskLevel::from_score(score), RiskLevel::Low);
    }

    #[test]
    fn compute_risk_score_ioc_match_forces_100() {
        // IOC 命中直接 100 分
        let ext = make_ext(|e| {
            e.ioc_matches = vec![IocMatch {
                ioc_type: "extension_id".to_string(),
                value: "testid".to_string(),
                severity: "high".to_string(),
            }];
        });
        let score = compute_risk_score(&ext);
        assert_eq!(score, 100);
        assert_eq!(RiskLevel::from_score(score), RiskLevel::High);
    }

    #[test]
    fn compute_risk_score_capped_at_100() {
        // 多权限累加超过 100 → 封顶 100
        // <all_urls>(40) + webRequest(20) + webRequestBlocking(20) + cookies(15) + tabs(10) + scripting(15) = 120
        let ext = make_ext(|e| {
            e.permissions = vec![
                "<all_urls>".to_string(),
                "webRequest".to_string(),
                "webRequestBlocking".to_string(),
                "cookies".to_string(),
                "tabs".to_string(),
                "scripting".to_string(),
            ];
        });
        let score = compute_risk_score(&ext);
        assert_eq!(score, 100);
        assert_eq!(RiskLevel::from_score(score), RiskLevel::High);
    }

    #[test]
    fn compute_risk_score_content_scripts_adds_10() {
        // has_content_scripts 加 10 分
        let ext = make_ext(|e| {
            e.has_content_scripts = true;
        });
        let score = compute_risk_score(&ext);
        assert_eq!(score, 10);
    }

    #[test]
    fn risk_level_thresholds() {
        // 边界测试：39=Low, 40=Medium, 69=Medium, 70=High
        assert_eq!(RiskLevel::from_score(0), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(39), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(40), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(69), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(70), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(100), RiskLevel::High);
    }

    #[test]
    fn permission_weights_default_values() {
        // 验证默认权重值
        let w = PermissionWeights::default();
        assert_eq!(w.all_urls, 40);
        assert_eq!(w.web_request, 20);
        assert_eq!(w.web_request_blocking, 20);
        assert_eq!(w.cookies, 15);
        assert_eq!(w.tabs, 10);
        assert_eq!(w.scripting, 15);
        assert_eq!(w.content_scripts, 10);
        assert_eq!(w.native_messaging, 15);
        assert_eq!(w.file_system, 15);
    }

    #[test]
    fn compute_risk_score_host_permissions_all_urls() {
        // <all_urls> 在 host_permissions（MV3）也加分
        let ext = make_ext(|e| {
            e.host_permissions = vec!["<all_urls>".to_string()];
        });
        let score = compute_risk_score(&ext);
        assert_eq!(score, 40);
    }

    #[test]
    fn compute_risk_score_with_custom_weights() {
        // 自定义权重计算
        let ext = make_ext(|e| {
            e.permissions = vec!["tabs".to_string()];
        });
        let weights = PermissionWeights {
            all_urls: 40,
            web_request: 20,
            web_request_blocking: 20,
            cookies: 15,
            tabs: 50, // 自定义高权重
            scripting: 15,
            content_scripts: 10,
            native_messaging: 15,
            file_system: 15,
        };
        let score = compute_risk_score_with_weights(&ext, &weights);
        assert_eq!(score, 50);
    }
}
