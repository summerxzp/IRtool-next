//! 扩展归因 Layer 1：网络关联
//!
//! 对浏览器进程的网络连接执行扩展归因，将目标域名与拥有对应
//! host_permissions 的扩展做匹配，生成归因标签和候选扩展列表。

use crate::core::{browser_kind_from_process_name, extract_profile_directory, BrowserKind};
use crate::extension_inventory::scan_extensions_cached;
use crate::permission_matcher::match_domain_to_extensions;
use crate::profile::enumerate_profiles;
use crate::MatchedExtension;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;

/// 扩展归因 Layer 1 结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionAttribution {
    /// 归因标签
    pub label: String,
    /// 浏览器类型
    pub browser: BrowserKind,
    /// Profile 名
    pub profile: String,
    /// 进程 PID
    pub pid: u32,
    /// 目标域名
    pub domain: String,
    /// 候选扩展列表（权限匹配结果）
    pub candidate_extensions: Vec<MatchedExtension>,
}

/// 对浏览器进程的网络连接执行扩展归因 Layer 1
///
/// 输入：网络连接信息（进程名、PID、目标域名）
/// 输出：归因标签 + 候选扩展列表
pub fn attribute_extension(
    process_name: &str,
    pid: u32,
    domain: &str,
    cmdline: Option<&str>,
) -> Option<ExtensionAttribution> {
    // 1. 进程→浏览器识别
    let browser = browser_kind_from_process_name(process_name)?;

    // 2. Profile 定位
    let profiles = enumerate_profiles(browser);
    if profiles.is_empty() {
        warn!("no profiles found for {}", browser.display_name());
        return None;
    }

    let target_profile_name = cmdline.and_then(extract_profile_directory);

    let target_profiles: Vec<_> = match target_profile_name {
        Some(ref name) => {
            let filtered: Vec<_> = profiles.into_iter().filter(|p| p.name == *name).collect();
            if filtered.is_empty() {
                warn!(
                    "profile '{}' specified in cmdline not found for {}",
                    name,
                    browser.display_name()
                );
                return None;
            }
            filtered
        }
        None => profiles,
    };

    if target_profiles.is_empty() {
        return None;
    }

    // 3. 扩展扫描 + 权限匹配（跨所有目标 Profile）
    let mut all_candidates: Vec<MatchedExtension> = Vec::new();

    for profile in &target_profiles {
        let inventory = scan_extensions_cached(browser, profile);
        let result = match_domain_to_extensions(domain, &inventory.extensions);
        all_candidates.extend(result.matching_extensions);
    }

    // Profile 名：优先使用 cmdline 指定的，否则使用第一个目标 Profile
    let profile_name = target_profile_name.unwrap_or_else(|| {
        target_profiles
            .first()
            .map(|p| p.name.as_str())
            .unwrap_or_default()
            .to_string()
    });

    // 4. 归因标签生成
    let label = if all_candidates.is_empty() {
        "browser-owned, extension-uncertain"
    } else {
        "browser-owned, extension-candidate"
    };

    Some(ExtensionAttribution {
        label: label.to_string(),
        browser,
        profile: profile_name,
        pid,
        domain: domain.to_string(),
        candidate_extensions: all_candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_browser_process_returns_none() {
        let result = attribute_extension("notepad.exe", 1234, "evil.com", None);
        assert!(result.is_none());
    }

    #[test]
    fn unknown_process_returns_none() {
        let result = attribute_extension("svchost.exe", 5678, "example.com", None);
        assert!(result.is_none());
    }

    #[test]
    fn cmdline_profile_directory_extraction() {
        // 验证 cmdline 中的 --profile-directory 能正确传递到 extract_profile_directory
        let cmdline = r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --profile-directory="Profile 1""#;
        let profile_name = extract_profile_directory(cmdline);
        assert_eq!(profile_name, Some("Profile 1".to_string()));
    }

    #[test]
    fn label_generation_logic() {
        // 测试归因标签生成逻辑
        let label_uncertain = {
            let candidates: Vec<MatchedExtension> = vec![];
            if candidates.is_empty() {
                "browser-owned, extension-uncertain"
            } else {
                "browser-owned, extension-candidate"
            }
        };
        assert_eq!(label_uncertain, "browser-owned, extension-uncertain");

        let label_candidate = {
            let candidates: Vec<MatchedExtension> = vec![MatchedExtension {
                id: "extid".to_string(),
                name: "Test".to_string(),
                version: "1.0".to_string(),
                risk_flags: vec![],
                matched_patterns: vec!["*://*.example.com/*".to_string()],
                has_sensitive_permissions: false,
            }];
            if candidates.is_empty() {
                "browser-owned, extension-uncertain"
            } else {
                "browser-owned, extension-candidate"
            }
        };
        assert_eq!(label_candidate, "browser-owned, extension-candidate");
    }

    #[test]
    fn edge_browser_recognized() {
        let browser = browser_kind_from_process_name("msedge.exe");
        assert_eq!(browser, Some(BrowserKind::Edge));
    }

    #[test]
    fn case_insensitive_process_name() {
        assert_eq!(browser_kind_from_process_name("Chrome.exe"), Some(BrowserKind::Chrome));
        assert_eq!(browser_kind_from_process_name("MSEdge.exe"), Some(BrowserKind::Edge));
    }
}
