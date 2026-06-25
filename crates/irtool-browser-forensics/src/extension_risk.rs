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
}
