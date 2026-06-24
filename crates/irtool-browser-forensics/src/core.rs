//! 共享基础：BrowserKind、路径映射、WebKit 时间戳转换

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Chromium 系浏览器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Chrome,
    Edge,
}

impl BrowserKind {
    /// 所有支持的浏览器
    pub fn all() -> &'static [BrowserKind] {
        &[BrowserKind::Chrome, BrowserKind::Edge]
    }

    /// 浏览器用户数据目录
    pub fn user_data_dir(&self) -> Option<PathBuf> {
        let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
        let sub = match self {
            BrowserKind::Chrome => r"Google\Chrome\User Data",
            BrowserKind::Edge => r"Microsoft\Edge\User Data",
        };
        Some(PathBuf::from(local_app_data).join(sub))
    }

    /// 浏览器主进程名
    pub fn process_name(&self) -> &'static str {
        match self {
            BrowserKind::Chrome => "chrome.exe",
            BrowserKind::Edge => "msedge.exe",
        }
    }

    /// 扩展策略注册表路径（HKLM 下）
    #[cfg(windows)]
    pub fn extension_policy_reg_path(&self) -> &'static str {
        match self {
            BrowserKind::Chrome => r"SOFTWARE\Policies\Google\Chrome\ExtensionInstallForcelist",
            BrowserKind::Edge => r"SOFTWARE\Policies\Microsoft\Edge\ExtensionInstallForcelist",
        }
    }

    /// 显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Chrome",
            BrowserKind::Edge => "Edge",
        }
    }
}

impl std::fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// WebKit 时间戳转换
///
/// Chromium 使用 WebKit 时间戳：从 1601-01-01 00:00:00 UTC 起的微秒数。
/// 参考：https://docs.microsoft.com/en-us/windows/win32/sysinfo/converting-a-time-t-value-to-a-filetime
pub mod webkit_timestamp {
    use chrono::{DateTime, TimeZone, Utc};

    /// WebKit 纪元与 Unix 纪元之间的微秒差
    /// 1601-01-01 到 1970-01-01 之间的 100 纳秒间隔数 = 116444736000000000
    /// 转为微秒 = 11644473600_000_000
    const WEBKIT_UNIX_DELTA_MICROS: i64 = 11_644_473_600_000_000;

    /// 将 WebKit 微秒时间戳转换为 DateTime<Utc>
    pub fn from_webkit_micros(micros: i64) -> Option<DateTime<Utc>> {
        let unix_micros = micros.checked_sub(WEBKIT_UNIX_DELTA_MICROS)?;
        let secs = unix_micros.div_euclid(1_000_000);
        let nanos = (unix_micros.rem_euclid(1_000_000) * 1_000) as u32;
        Utc.timestamp_opt(secs, nanos).single()
    }

    /// 将 DateTime<Utc> 转换为 WebKit 微秒时间戳
    pub fn to_webkit_micros(dt: &DateTime<Utc>) -> i64 {
        let unix_micros = dt.timestamp_micros();
        unix_micros + WEBKIT_UNIX_DELTA_MICROS
    }
}

/// 从进程命令行提取 `--profile-directory` 参数值
pub fn extract_profile_directory(cmdline: &str) -> Option<String> {
    // 查找 --profile-directory= 的位置
    let prefix = "--profile-directory=";
    let start = cmdline.find(prefix)?;
    let value_start = start + prefix.len();
    let rest = &cmdline[value_start..];

    // 处理引号包裹的值
    if let Some(stripped) = rest.strip_prefix('"') {
        // 双引号包裹：找到结束引号
        if let Some(end) = stripped.find('"') {
            let value = &stripped[..end];
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    } else if let Some(stripped) = rest.strip_prefix('\'') {
        // 单引号包裹
        if let Some(end) = stripped.find('\'') {
            let value = &stripped[..end];
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    } else {
        // 无引号：取到下一个空格或行尾
        let value = rest.split_whitespace().next()?;
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// 根据进程名推断 BrowserKind
pub fn browser_kind_from_process_name(name: &str) -> Option<BrowserKind> {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "chrome.exe" => Some(BrowserKind::Chrome),
        "msedge.exe" => Some(BrowserKind::Edge),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    #[test]
    fn browser_kind_user_data_dir() {
        for kind in BrowserKind::all() {
            let dir = kind.user_data_dir();
            // 在 CI 环境中 LOCALAPPDATA 可能不存在，但不应 panic
            assert!(dir.is_some() || std::env::var("LOCALAPPDATA").is_err());
        }
    }

    #[test]
    fn browser_kind_process_name() {
        assert_eq!(BrowserKind::Chrome.process_name(), "chrome.exe");
        assert_eq!(BrowserKind::Edge.process_name(), "msedge.exe");
    }

    #[test]
    fn browser_kind_display() {
        assert_eq!(BrowserKind::Chrome.to_string(), "Chrome");
        assert_eq!(BrowserKind::Edge.to_string(), "Edge");
    }

    #[test]
    fn webkit_timestamp_conversion() {
        // 2024-01-01 00:00:00 UTC
        let dt = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let micros = webkit_timestamp::to_webkit_micros(&dt);
        let roundtrip = webkit_timestamp::from_webkit_micros(micros).unwrap();
        assert_eq!(dt, roundtrip);
    }

    #[test]
    fn webkit_timestamp_epoch() {
        // Unix epoch (1970-01-01) 应对应 WebKit 差值
        let unix_epoch = chrono::Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
        let micros = webkit_timestamp::to_webkit_micros(&unix_epoch);
        assert_eq!(micros, 11_644_473_600_000_000);
    }

    #[test]
    fn webkit_timestamp_zero() {
        // WebKit 0 = 1601-01-01，远在 Unix epoch 之前，转换结果应早于 1970
        let result = webkit_timestamp::from_webkit_micros(0);
        assert!(result.is_none() || result.unwrap().year() < 1970);
    }

    #[test]
    fn extract_profile_directory_equals() {
        let cmdline = r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --profile-directory="Profile 1""#;
        assert_eq!(extract_profile_directory(cmdline), Some("Profile 1".to_string()));
    }

    #[test]
    fn extract_profile_directory_default() {
        let cmdline = r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --profile-directory=Default"#;
        assert_eq!(extract_profile_directory(cmdline), Some("Default".to_string()));
    }

    #[test]
    fn extract_profile_directory_missing() {
        let cmdline = r#""C:\Program Files\Google\Chrome\Application\chrome.exe""#;
        assert_eq!(extract_profile_directory(cmdline), None);
    }

    #[test]
    fn browser_kind_from_process() {
        assert_eq!(browser_kind_from_process_name("chrome.exe"), Some(BrowserKind::Chrome));
        assert_eq!(browser_kind_from_process_name("MSEdge.exe"), Some(BrowserKind::Edge));
        assert_eq!(browser_kind_from_process_name("notepad.exe"), None);
    }
}
