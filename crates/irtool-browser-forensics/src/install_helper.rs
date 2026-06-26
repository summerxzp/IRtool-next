//! Helper Extension 安装辅助
//!
//! 提供 Native Messaging Host 注册功能，包括：
//! - 写入 Native Messaging Host JSON 配置文件
//! - 通过 `reg add` 注册到浏览器注册表

use crate::core::BrowserKind;
use std::io;
use std::path::PathBuf;

/// Native Messaging Host 名称
const NM_HOST_NAME: &str = "irtool_attribution";

/// 获取 Native Messaging Host JSON 配置文件的写入路径
fn nm_host_json_path(browser: BrowserKind) -> PathBuf {
    let app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    match browser {
        BrowserKind::Chrome => PathBuf::from(app_data)
            .join("Google")
            .join("Chrome")
            .join("NativeMessagingHosts"),
        BrowserKind::Edge => PathBuf::from(app_data)
            .join("Microsoft")
            .join("Edge")
            .join("NativeMessagingHosts"),
        BrowserKind::Brave => PathBuf::from(app_data)
            .join("BraveSoftware")
            .join("Brave-Browser")
            .join("NativeMessagingHosts"),
        BrowserKind::Vivaldi => PathBuf::from(app_data).join("Vivaldi").join("NativeMessagingHosts"),
    }
}

/// 获取浏览器注册表中 Native Messaging Host 的注册表路径
fn nm_host_reg_path(browser: BrowserKind) -> String {
    match browser {
        BrowserKind::Chrome => r"SOFTWARE\Google\Chrome\NativeMessagingHosts".to_string(),
        BrowserKind::Edge => r"SOFTWARE\Microsoft\Edge\NativeMessagingHosts".to_string(),
        BrowserKind::Brave => r"SOFTWARE\BraveSoftware\Brave-Browser\NativeMessagingHosts".to_string(),
        BrowserKind::Vivaldi => r"SOFTWARE\Vivaldi\NativeMessagingHosts".to_string(),
    }
}

/// 获取 IRtool 可执行文件路径
fn irtool_exe_path() -> io::Result<PathBuf> {
    std::env::current_exe()
}

/// 安装 Native Messaging Host
///
/// 1. 生成 NMH JSON 配置文件并写入浏览器目录
/// 2. 通过 `reg add` 注册到浏览器注册表
pub fn install_native_messaging_host(browser: BrowserKind) -> Result<String, String> {
    let exe_path = irtool_exe_path().map_err(|e| format!("failed to get exe path: {}", e))?;
    let exe_str = exe_path
        .to_str()
        .ok_or_else(|| "exe path is not valid unicode".to_string())?;

    // 1. 生成 JSON 配置
    let json_content = format!(
        r#"{{
  "name": "{nm_host_name}",
  "description": "IRtool Attribution Helper",
  "path": "{exe_str}",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://*/"
  ]
}}"#,
        nm_host_name = NM_HOST_NAME,
        exe_str = exe_str.replace('\\', "\\\\"),
    );

    // 2. 写入 JSON 文件
    let nm_dir = nm_host_json_path(browser);
    std::fs::create_dir_all(&nm_dir).map_err(|e| format!("failed to create NM dir: {}", e))?;
    let json_path = nm_dir.join(format!("{}.json", NM_HOST_NAME));
    std::fs::write(&json_path, &json_content).map_err(|e| format!("failed to write NM json: {}", e))?;

    // 3. 注册到注册表（使用 reg add 避免提权问题）
    let reg_path = nm_host_reg_path(browser);
    let reg_key = format!(r"{}\{}", reg_path, NM_HOST_NAME);
    let json_path_str = json_path
        .to_str()
        .ok_or_else(|| "json path is not valid unicode".to_string())?;

    let output = std::process::Command::new("reg")
        .args(["add", &reg_key, "/ve", "/d", json_path_str, "/f"])
        .output()
        .map_err(|e| format!("failed to run reg add: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("reg add failed: {}", stderr));
    }

    Ok(format!(
        "Native Messaging Host '{}' installed for {} at {}",
        NM_HOST_NAME,
        browser.display_name(),
        json_path_str,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nm_host_json_path_format() {
        // 验证路径生成不 panic
        let _ = nm_host_json_path(BrowserKind::Chrome);
        let _ = nm_host_json_path(BrowserKind::Edge);
        let _ = nm_host_json_path(BrowserKind::Brave);
        let _ = nm_host_json_path(BrowserKind::Vivaldi);
    }

    #[test]
    fn nm_host_reg_path_format() {
        assert!(nm_host_reg_path(BrowserKind::Chrome).contains("Google"));
        assert!(nm_host_reg_path(BrowserKind::Edge).contains("Microsoft"));
        assert!(nm_host_reg_path(BrowserKind::Brave).contains("BraveSoftware"));
        assert!(nm_host_reg_path(BrowserKind::Vivaldi).contains("Vivaldi"));
    }

    #[test]
    fn json_content_valid() {
        let json = format!(
            r#"{{
  "name": "{}",
  "description": "IRtool Attribution Helper",
  "path": "C:\\\\test\\\\irtool.exe",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://*/"
  ]
}}"#,
            NM_HOST_NAME,
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("json should be valid");
        assert_eq!(parsed["name"], NM_HOST_NAME);
        assert_eq!(parsed["type"], "stdio");
    }
}
