//! Profile 枚举：扫描浏览器用户数据目录下的 Profile 目录

use crate::core::BrowserKind;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// 浏览器 Profile 信息
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BrowserProfile {
    pub browser: BrowserKind,
    /// Profile 目录名（如 "Default", "Profile 1"）
    pub name: String,
    /// Profile 目录完整路径
    pub path: PathBuf,
}

/// 扫描指定浏览器下的所有 Profile 目录
///
/// Chromium 浏览器的 Profile 目录特征：
/// - 包含 `Preferences` 文件
/// - 目录名格式为 "Default" 或 "Profile N"
pub fn enumerate_profiles(browser: BrowserKind) -> Vec<BrowserProfile> {
    let user_data_dir = match browser.user_data_dir() {
        Some(d) => d,
        None => return vec![],
    };

    if !user_data_dir.exists() {
        return vec![];
    }

    let mut profiles = Vec::new();

    // 检查 Default 目录
    let default_path = user_data_dir.join("Default");
    if is_valid_profile_dir(&default_path) {
        profiles.push(BrowserProfile {
            browser,
            name: "Default".to_string(),
            path: default_path,
        });
    }

    // 检查 Profile N 目录
    if let Ok(entries) = std::fs::read_dir(&user_data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("Profile ") && is_valid_profile_dir(&entry.path()) {
                profiles.push(BrowserProfile {
                    browser,
                    name: name_str.to_string(),
                    path: entry.path(),
                });
            }
        }
    }

    profiles
}

/// 扫描所有已安装浏览器的 Profile
pub fn enumerate_all_profiles() -> Vec<BrowserProfile> {
    BrowserKind::all().iter().flat_map(|b| enumerate_profiles(*b)).collect()
}

/// 判断目录是否为有效的 Chromium Profile 目录
fn is_valid_profile_dir(path: &std::path::Path) -> bool {
    path.is_dir() && path.join("Preferences").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_profiles_nonexistent_browser() {
        // 如果 Chrome 未安装，应返回空列表而非 panic
        let profiles = enumerate_profiles(BrowserKind::Chrome);
        // 不做断言，只确保不 panic
        let _ = profiles.len();
    }

    #[test]
    fn enumerate_all_profiles_no_panic() {
        let _ = enumerate_all_profiles();
    }
}
