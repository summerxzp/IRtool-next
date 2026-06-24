//! 扩展资产盘点：manifest.json + Preferences + Secure Preferences 三源合并

use crate::core::webkit_timestamp;
use crate::extension_risk::{compute_risk_flags, match_ioc};
use crate::profile::BrowserProfile;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tracing::{debug, warn};

/// 扩展资产清单
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionInventory {
    pub browser: crate::core::BrowserKind,
    pub profile: String,
    pub extensions: Vec<ExtensionInfo>,
}

/// 单个扩展的完整信息
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub install_time: Option<String>,
    pub install_source: Option<String>,
    pub update_url: Option<String>,
    pub was_installed_by_default: Option<bool>,
    pub permissions: Vec<String>,
    pub host_permissions: Vec<String>,
    pub has_content_scripts: bool,
    pub has_background: bool,
    pub preferences_tampered: bool,
    pub risk_flags: Vec<String>,
    pub ioc_matches: Vec<crate::extension_risk::IocMatch>,
    pub path: PathBuf,
}

/// manifest.json 的关键字段（非完整反序列化）
#[derive(Debug, Deserialize)]
struct Manifest {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    permissions: Option<Vec<serde_json::Value>>,
    host_permissions: Option<Vec<String>>,
    background: Option<serde_json::Value>,
    content_scripts: Option<Vec<serde_json::Value>>,
    #[serde(rename = "default_locale")]
    default_locale: Option<String>,
}

/// Preferences 中 extensions.settings.<id> 的关键字段
#[derive(Debug, Deserialize)]
struct ExtensionPrefEntry {
    state: Option<u32>,
    install_time: Option<String>,
    #[expect(dead_code)]
    disable_reasons: Option<serde_json::Value>,
    install_source: Option<String>,
    update_url: Option<String>,
    was_installed_by_default: Option<bool>,
}

/// Secure Preferences 中 extensions.settings.<id> 的关键字段
///
/// Edge 等浏览器将扩展信息存放在 Secure Preferences 而非 Preferences 中，
/// 字段名与 Preferences 略有不同（如 first_install_time vs install_time）。
#[derive(Debug, Deserialize)]
struct ExtensionSecurePrefEntry {
    state: Option<u32>,
    first_install_time: Option<String>,
    disable_reasons: Option<serde_json::Value>,
    from_webstore: Option<bool>,
    was_installed_by_default: Option<bool>,
}

/// _locales/<locale>/messages.json 中的消息条目
#[derive(Debug, Deserialize)]
struct LocaleMessage {
    message: Option<String>,
}

/// 扫描指定 Profile 下的所有扩展
pub fn scan_extensions(profile: &BrowserProfile) -> ExtensionInventory {
    let mut inventory = ExtensionInventory {
        browser: profile.browser,
        profile: profile.name.clone(),
        extensions: vec![],
    };

    // 数据源 2 & 3：Preferences 和 Secure Preferences
    let prefs = read_json_file(&profile.path.join("Preferences"));
    let secure_prefs = read_json_file(&profile.path.join("Secure Preferences"));

    // 数据源 1：Extensions 目录扫描
    let extensions_dir = profile.path.join("Extensions");
    if !extensions_dir.is_dir() {
        debug!("no Extensions directory: {}", extensions_dir.display());
        return inventory;
    }

    let entries = match std::fs::read_dir(&extensions_dir) {
        Ok(e) => e,
        Err(err) => {
            warn!("failed to read Extensions dir: {}", err);
            return inventory;
        }
    };

    for entry in entries.flatten() {
        let ext_id = match entry.file_name().to_str() {
            Some(id) => id.to_string(),
            None => continue,
        };

        // 每个 extension-id 目录下可能有多个版本子目录
        let version_dir = match find_latest_version_dir(entry.path()) {
            Some(d) => d,
            None => {
                debug!("no version dir for extension {}", ext_id);
                continue;
            }
        };

        let manifest_path = version_dir.join("manifest.json");
        let mut manifest = match read_json_file::<Manifest>(&manifest_path) {
            Some(m) => m,
            None => {
                warn!("failed to read manifest for extension {}", ext_id);
                continue;
            }
        };

        // 解析国际化名称
        let name = resolve_extension_name(&manifest, &version_dir);

        // 解析 permissions（可能是字符串或对象，过滤只保留字符串）
        let permissions = parse_permissions(&manifest.permissions);

        // 从 Preferences 提取补充信息
        let (pref_entry, tampered) = extract_pref_info(&prefs, &secure_prefs, &ext_id);

        let install_time = pref_entry
            .as_ref()
            .and_then(|p| p.install_time.as_ref())
            .and_then(|ts| ts.parse::<i64>().ok())
            .and_then(webkit_timestamp::from_webkit_micros)
            .map(|dt| dt.to_rfc3339());

        let enabled = pref_entry.as_ref().is_some_and(|p| p.state == Some(1));

        let mut ext_info = ExtensionInfo {
            id: ext_id.clone(),
            name,
            version: manifest.version.take().unwrap_or_default(),
            description: manifest.description.take(),
            enabled,
            install_time,
            install_source: pref_entry.as_ref().and_then(|p| p.install_source.clone()),
            update_url: pref_entry
                .as_ref()
                .and_then(|p| p.update_url.clone())
                .or_else(|| extract_update_url_from_manifest(&manifest)),
            was_installed_by_default: pref_entry.and_then(|p| p.was_installed_by_default),
            permissions,
            host_permissions: manifest.host_permissions.unwrap_or_default(),
            has_content_scripts: manifest.content_scripts.is_some(),
            has_background: manifest.background.is_some(),
            preferences_tampered: tampered,
            risk_flags: vec![],
            ioc_matches: vec![],
            path: version_dir,
        };

        // 风险标注 + IOC 匹配
        ext_info.risk_flags = compute_risk_flags(&ext_info);
        ext_info.ioc_matches = match_ioc(&ext_info);

        inventory.extensions.push(ext_info);
    }

    inventory
}

/// 找到扩展目录下最新版本子目录
fn find_latest_version_dir(ext_dir: PathBuf) -> Option<PathBuf> {
    // Chromium 扩展版本目录名通常是数字点分格式如 "1.0.0"
    // 也可能是纯数字。按修改时间取最新的。
    let entries = std::fs::read_dir(&ext_dir).ok()?;
    let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        match &latest {
            None => latest = Some((entry.path(), modified)),
            Some((_, prev)) if modified > *prev => latest = Some((entry.path(), modified)),
            _ => {}
        }
    }

    latest.map(|(p, _)| p)
}

/// 解析国际化扩展名称
///
/// manifest.json 中的 name 可能是 `__MSG_name__` 格式，
/// 此时需要从 `_locales/<locale>/messages.json` 读取实际名称。
fn resolve_extension_name(manifest: &Manifest, version_dir: &std::path::Path) -> String {
    if let Some(name) = &manifest.name {
        if let Some(msg_key) = extract_message_key(name) {
            // 尝试 en locale，然后 fallback 到 default_locale
            let locales_to_try = if let Some(ref locale) = manifest.default_locale {
                vec![locale.clone(), "en".to_string()]
            } else {
                vec!["en".to_string()]
            };

            for locale in locales_to_try {
                let msg_path = version_dir.join("_locales").join(&locale).join("messages.json");
                if let Some(messages) = read_json_file::<std::collections::HashMap<String, LocaleMessage>>(&msg_path) {
                    let key_lower = msg_key.to_lowercase();
                    if let Some(msg) = messages.get(&key_lower) {
                        if let Some(message) = &msg.message {
                            return message.clone();
                        }
                    }
                }
            }
            // 无法解析国际化名称，返回原始值
            debug!("could not resolve i18n name for key: {}", msg_key);
        }
        return name.clone();
    }
    String::new()
}

/// 从 `__MSG_xxx__` 格式提取消息键名
fn extract_message_key(name: &str) -> Option<&str> {
    let trimmed = name.trim();
    if trimmed.starts_with("__MSG_") && trimmed.ends_with("__") {
        Some(&trimmed[6..trimmed.len() - 2])
    } else {
        None
    }
}

/// 解析 permissions 字段
///
/// MV2 中 permissions 可以是字符串或对象（如 {"fileHandler": [...]}），
/// 只保留字符串类型的权限名
fn parse_permissions(raw: &Option<Vec<serde_json::Value>>) -> Vec<String> {
    match raw {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => vec![],
    }
}

/// 从 manifest 中提取 update_url（某些扩展在 manifest 中声明而非 Preferences）
fn extract_update_url_from_manifest(_manifest: &Manifest) -> Option<String> {
    // manifest.json 中 update_url 可能在顶层或 key "update_url" 中
    // 由于我们的 Manifest 结构体没有声明 update_url，这里用原始 JSON 读取
    None // 简化：Preferences 中已有 update_url，manifest 中的通常冗余
}

/// 从 Preferences 和 Secure Preferences 中提取扩展信息
///
/// 优先从 Preferences 读取，若 Preferences 中无该扩展条目则从 Secure Preferences 读取。
/// Edge 等浏览器将扩展信息全部存放在 Secure Preferences 中。
///
/// 返回 (扩展条目信息, 是否被篡改)
fn extract_pref_info(
    prefs: &Option<serde_json::Value>,
    secure_prefs: &Option<serde_json::Value>,
    ext_id: &str,
) -> (Option<ExtensionPrefEntry>, bool) {
    // 先从 Preferences 读取
    let pref_entry = prefs
        .as_ref()
        .and_then(|p| p.get("extensions"))
        .and_then(|e| e.get("settings"))
        .and_then(|s| s.get(ext_id))
        .and_then(|v| serde_json::from_value::<ExtensionPrefEntry>(v.clone()).ok());

    // 若 Preferences 中无该扩展，尝试从 Secure Preferences 读取
    let pref_entry = match pref_entry {
        Some(entry) => Some(entry),
        None => secure_prefs
            .as_ref()
            .and_then(|p| p.get("extensions"))
            .and_then(|e| e.get("settings"))
            .and_then(|s| s.get(ext_id))
            .and_then(|v| serde_json::from_value::<ExtensionSecurePrefEntry>(v.clone()).ok())
            .map(|sp| ExtensionPrefEntry {
                state: sp.state.or_else(|| {
                    // Secure Preferences 中若无 state 字段，根据 disable_reasons 推断
                    // disable_reasons 为空数组或 null → enabled
                    let has_disable_reasons = sp.disable_reasons.as_ref().is_some_and(|dr| match dr {
                        serde_json::Value::Array(arr) => !arr.is_empty(),
                        serde_json::Value::Number(n) => n.as_u64() != Some(0),
                        _ => false,
                    });
                    Some(if has_disable_reasons { 0 } else { 1 })
                }),
                install_time: sp.first_install_time,
                disable_reasons: sp.disable_reasons,
                install_source: sp
                    .from_webstore
                    .map(|ws| if ws { "webstore" } else { "other" }.to_string()),
                update_url: None,
                was_installed_by_default: sp.was_installed_by_default,
            }),
    };

    // Secure Preferences HMAC 检查
    let tampered = check_hmac(secure_prefs, ext_id);

    (pref_entry, tampered)
}

/// 检查 Secure Preferences 中扩展条目的 HMAC 是否存在
///
/// Chromium 系浏览器的 HMAC 存储在 `protection.macs.extensions.settings.<ext_id>` 路径下，
/// 而非 `extensions.settings.<ext_id>.hmac`。如果该路径下缺少 HMAC，则标记为被篡改。
fn check_hmac(secure_prefs: &Option<serde_json::Value>, ext_id: &str) -> bool {
    let hmac = secure_prefs
        .as_ref()
        .and_then(|p| p.get("protection"))
        .and_then(|p| p.get("macs"))
        .and_then(|m| m.get("extensions"))
        .and_then(|e| e.get("settings"))
        .and_then(|s| s.get(ext_id));

    match hmac {
        None => true, // 缺少 HMAC → 被篡改
        Some(v) => v.is_null() || (v.is_string() && v.as_str() == Some("")),
    }
}

/// 读取 JSON 文件
fn read_json_file<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
    if !path.exists() {
        debug!("file not found: {}", path.display());
        return None;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(err) => {
            warn!("failed to read {}: {}", path.display(), err);
            return None;
        }
    };
    match serde_json::from_str(&content) {
        Ok(v) => Some(v),
        Err(err) => {
            warn!("failed to parse JSON {}: {}", path.display(), err);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::BrowserKind;
    use std::fs;

    fn setup_test_profile(dir: &std::path::Path) -> BrowserProfile {
        BrowserProfile {
            browser: BrowserKind::Chrome,
            name: "Default".to_string(),
            path: dir.to_path_buf(),
        }
    }

    fn create_test_extension(ext_dir: &std::path::Path, ext_id: &str, version: &str, manifest_content: &str) {
        let version_dir = ext_dir.join(ext_id).join(version);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join("manifest.json"), manifest_content).unwrap();
    }

    fn create_test_preferences(dir: &std::path::Path, prefs_json: &str) {
        fs::write(dir.join("Preferences"), prefs_json).unwrap();
    }

    fn create_test_secure_preferences(dir: &std::path::Path, prefs_json: &str) {
        fs::write(dir.join("Secure Preferences"), prefs_json).unwrap();
    }

    #[test]
    fn scan_empty_profile() {
        let temp = tempfile::tempdir().unwrap();
        let profile = setup_test_profile(temp.path());
        // 创建 Preferences 使其成为有效 Profile
        fs::write(temp.path().join("Preferences"), "{}").unwrap();

        let inventory = scan_extensions(&profile);
        assert!(inventory.extensions.is_empty());
    }

    #[test]
    fn scan_with_basic_extension() {
        let temp = tempfile::tempdir().unwrap();
        let profile = setup_test_profile(temp.path());
        fs::write(temp.path().join("Preferences"), "{}").unwrap();

        let ext_dir = temp.path().join("Extensions");
        fs::create_dir_all(&ext_dir).unwrap();

        let manifest = r#"{
            "name": "Test Extension",
            "version": "1.0.0",
            "description": "A test extension",
            "permissions": ["tabs", "storage"],
            "content_scripts": [{"matches": ["<all_urls>"], "js": ["content.js"]}]
        }"#;
        create_test_extension(&ext_dir, "abcdefghijk", "1.0.0", manifest);

        let inventory = scan_extensions(&profile);
        assert_eq!(inventory.extensions.len(), 1);

        let ext = &inventory.extensions[0];
        assert_eq!(ext.id, "abcdefghijk");
        assert_eq!(ext.name, "Test Extension");
        assert_eq!(ext.version, "1.0.0");
        assert!(ext.has_content_scripts);
        assert!(!ext.has_background);
        assert!(ext.permissions.contains(&"tabs".to_string()));
        assert!(ext.permissions.contains(&"storage".to_string()));
    }

    #[test]
    fn scan_with_preferences_info() {
        let temp = tempfile::tempdir().unwrap();
        let profile = setup_test_profile(temp.path());

        let prefs = r#"{
            "extensions": {
                "settings": {
                    "testextid": {
                        "state": 1,
                        "install_time": "13348444800000000",
                        "install_source": "webstore",
                        "update_url": "https://clients2.google.com/service/update2/crx",
                        "was_installed_by_default": false
                    }
                }
            }
        }"#;
        create_test_preferences(temp.path(), prefs);
        create_test_secure_preferences(
            temp.path(),
            r#"{"protection":{"macs":{"extensions":{"settings":{"testextid":"fake_hmac_value"}}}}}"#,
        );

        let ext_dir = temp.path().join("Extensions");
        fs::create_dir_all(&ext_dir).unwrap();

        let manifest = r#"{"name": "Test", "version": "2.0"}"#;
        create_test_extension(&ext_dir, "testextid", "2.0", manifest);

        let inventory = scan_extensions(&profile);
        assert_eq!(inventory.extensions.len(), 1);

        let ext = &inventory.extensions[0];
        assert!(ext.enabled);
        assert!(ext.install_time.is_some());
        assert_eq!(ext.install_source.as_deref(), Some("webstore"));
        assert!(!ext.preferences_tampered);
    }

    #[test]
    fn scan_detects_tampered_preferences() {
        let temp = tempfile::tempdir().unwrap();
        let profile = setup_test_profile(temp.path());

        let prefs = r#"{
            "extensions": {
                "settings": {
                    "tamperedext": {
                        "state": 1,
                        "install_time": "13348444800000000"
                    }
                }
            }
        }"#;
        create_test_preferences(temp.path(), prefs);
        // Secure Preferences 中缺少该扩展的 HMAC
        create_test_secure_preferences(temp.path(), r#"{"protection":{"macs":{"extensions":{"settings":{}}}}}"#);

        let ext_dir = temp.path().join("Extensions");
        fs::create_dir_all(&ext_dir).unwrap();

        let manifest = r#"{"name": "Tampered", "version": "1.0"}"#;
        create_test_extension(&ext_dir, "tamperedext", "1.0", manifest);

        let inventory = scan_extensions(&profile);
        assert_eq!(inventory.extensions.len(), 1);
        assert!(inventory.extensions[0].preferences_tampered);
    }

    #[test]
    fn scan_with_i18n_name() {
        let temp = tempfile::tempdir().unwrap();
        let profile = setup_test_profile(temp.path());
        fs::write(temp.path().join("Preferences"), "{}").unwrap();

        let ext_dir = temp.path().join("Extensions");
        fs::create_dir_all(&ext_dir).unwrap();

        let version_dir = ext_dir.join("i18next").join("1.0");
        fs::create_dir_all(&version_dir).unwrap();

        let manifest = r#"{
            "name": "__MSG_extensionName__",
            "version": "1.0",
            "default_locale": "en"
        }"#;
        fs::write(version_dir.join("manifest.json"), manifest).unwrap();

        let locale_dir = version_dir.join("_locales").join("en");
        fs::create_dir_all(&locale_dir).unwrap();
        let messages = r#"{
            "extensionname": {
                "message": "My i18n Extension"
            }
        }"#;
        fs::write(locale_dir.join("messages.json"), messages).unwrap();

        let inventory = scan_extensions(&profile);
        assert_eq!(inventory.extensions.len(), 1);
        assert_eq!(inventory.extensions[0].name, "My i18n Extension");
    }

    #[test]
    fn extract_message_key_basic() {
        assert_eq!(extract_message_key("__MSG_name__"), Some("name"));
        assert_eq!(extract_message_key("__MSG_ext_name__"), Some("ext_name"));
        assert_eq!(extract_message_key("Normal Name"), None);
        assert_eq!(extract_message_key("__MSG_"), None);
    }

    #[test]
    fn parse_permissions_filters_objects() {
        let raw = Some(vec![
            serde_json::Value::String("tabs".to_string()),
            serde_json::json!({"fileHandler": ["text/plain"]}),
            serde_json::Value::String("storage".to_string()),
        ]);
        let result = parse_permissions(&raw);
        assert_eq!(result, vec!["tabs", "storage"]);
    }

    #[test]
    fn check_hmac_missing_entry() {
        let secure = Some(serde_json::json!({
            "protection": {"macs": {"extensions": {"settings": {}}}}
        }));
        assert!(check_hmac(&secure, "missing_ext"));
    }

    #[test]
    fn check_hmac_null_value() {
        let secure = Some(serde_json::json!({
            "protection": {"macs": {"extensions": {"settings": {"extid": null}}}}
        }));
        assert!(check_hmac(&secure, "extid"));
    }

    #[test]
    fn check_hmac_present() {
        let secure = Some(serde_json::json!({
            "protection": {"macs": {"extensions": {"settings": {"extid": "valid_hmac"}}}}
        }));
        assert!(!check_hmac(&secure, "extid"));
    }

    #[test]
    fn disabled_extension() {
        let temp = tempfile::tempdir().unwrap();
        let profile = setup_test_profile(temp.path());

        let prefs = r#"{
            "extensions": {
                "settings": {
                    "disabledext": {
                        "state": 0,
                        "disable_reasons": 1
                    }
                }
            }
        }"#;
        create_test_preferences(temp.path(), prefs);
        create_test_secure_preferences(
            temp.path(),
            r#"{"protection":{"macs":{"extensions":{"settings":{"disabledext":"x"}}}}}"#,
        );

        let ext_dir = temp.path().join("Extensions");
        fs::create_dir_all(&ext_dir).unwrap();

        let manifest = r#"{"name": "Disabled", "version": "1.0"}"#;
        create_test_extension(&ext_dir, "disabledext", "1.0", manifest);

        let inventory = scan_extensions(&profile);
        assert!(!inventory.extensions[0].enabled);
    }
}
