//! Helper Extension 安装辅助
//!
//! 提供 Native Messaging Host 注册功能，包括：
//! - 写入 Native Messaging Host JSON 配置文件
//! - 通过 `reg add` 注册到浏览器注册表

use crate::core::BrowserKind;
use std::io;
use std::path::PathBuf;

/// Native Messaging Host 名称
///
/// 必须与 helper-extension/service_worker.js 中的 NATIVE_HOST 常量保持一致。
/// 采用 `com.domain.product` 格式，符合 Chrome Native Messaging 命名约定。
const NM_HOST_NAME: &str = "com.irtool.attribution";

/// Helper Extension 的固定 ID
///
/// 通过在 manifest.json 中写入 `key` 字段（RSA 2048 公钥，base64 编码的 DER
/// SubjectPublicKeyInfo）固定扩展 ID，使 load unpacked 时 Chrome 分配的 ID 永远一致。
///
/// 扩展 ID 算法：SHA-256(public_key_der)[0..16]，每 nibble 映射到 a-p。
/// 私钥保存在 `.tmp_helper_ext_privkey.pem`（仅用于以后打包 .crx，不提交 git）。
///
/// 这样设计的好处：
/// 1. 用户无需手动输入扩展 ID（实现细节不暴露给用户）
/// 2. NMH JSON 的 allowed_origins 可硬编码，无需运行时传入 ID
/// 3. 跨浏览器（Chrome/Edge/Brave）ID 一致
const HELPER_EXTENSION_ID: &str = "mbgpppibaejglkaklcbceckkicbhkalp";

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
    }
}

/// 获取浏览器注册表中 Native Messaging Host 的注册表路径
///
/// 写入 HKCU（当前用户）而非 HKLM，避免需要管理员权限。
/// Chrome/Edge 的 Native Messaging Host 查找顺序为 HKCU → HKLM。
fn nm_host_reg_path(browser: BrowserKind) -> String {
    match browser {
        BrowserKind::Chrome => r"HKCU\SOFTWARE\Google\Chrome\NativeMessagingHosts".to_string(),
        BrowserKind::Edge => r"HKCU\SOFTWARE\Microsoft\Edge\NativeMessagingHosts".to_string(),
    }
}

/// 获取 Native Messaging Host 可执行文件路径
///
/// 必须使用独立二进制 `irtool-native-messaging-host.exe`，而非主 exe。
/// 原因：主 exe 的 app-manifest.xml 声明了 `requireAdministrator`，
/// Chrome 是非提升进程，无法启动要求 admin 的 NMH 进程。
/// 独立二进制默认 asInvoker，可被 Chrome 正常启动。
///
/// dev 模式：`target/debug/irtool-native-messaging-host.exe`
/// release 模式：主 exe 同目录下的 `irtool-native-messaging-host.exe`
fn nmh_exe_path() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| io::Error::other("no parent dir"))?;
    let nmh_exe = if cfg!(windows) {
        dir.join("irtool-native-messaging-host.exe")
    } else {
        dir.join("irtool-native-messaging-host")
    };
    Ok(nmh_exe)
}

/// 安装 Native Messaging Host
///
/// 1. 生成 NMH JSON 配置文件并写入浏览器目录
/// 2. 通过 `reg add` 注册到浏览器注册表
///
/// 扩展 ID 默认由 manifest.json 的 `key` 字段固定（见 `HELPER_EXTENSION_ID`），
/// 用户无需手动输入。`extension_id_override` 用于兜底场景：当固定 ID 因某些原因
/// 失效（如 manifest.json 的 key 被删除、某些 Chromium 衍生版不尊重 key）时，
/// 用户可在"高级选项"中手动输入扩展 ID 覆盖。
///
/// Chrome 不支持 `allowed_origins` 使用通配符，必须指定具体扩展 ID。
pub fn install_native_messaging_host(
    browser: BrowserKind,
    extension_id_override: Option<&str>,
) -> Result<String, String> {
    // 确定使用的扩展 ID：优先用 override（兜底），否则用固定 ID
    let extension_id = match extension_id_override {
        Some(id) => {
            let id = id.trim();
            if id.len() != 32 || !id.chars().all(|c| ('a'..='p').contains(&c)) {
                let msg = format!(
                    "invalid extension id '{}': must be 32 lowercase ascii letters (a-p)",
                    id
                );
                tracing::error!(error = %msg, "install NMH failed: invalid override extension id");
                return Err(msg);
            }
            tracing::warn!(
                override_id = %id,
                default_id = %HELPER_EXTENSION_ID,
                "install NMH with user-provided extension ID override (fallback mode)"
            );
            id
        }
        None => HELPER_EXTENSION_ID,
    };

    tracing::info!(
        browser = %browser.display_name(),
        extension_id = %extension_id,
        "installing native messaging host"
    );

    let exe_path = nmh_exe_path().map_err(|e| {
        let msg = format!("failed to get NMH exe path: {}", e);
        tracing::error!(error = %msg, "install NMH failed");
        msg
    })?;

    // 检查 NMH 二进制是否存在
    if !exe_path.exists() {
        let msg = format!(
            "irtool-native-messaging-host not found at {}. Please build it with: cargo build -p irtool-native-messaging",
            exe_path.display()
        );
        tracing::error!(
            nmh_exe_path = %exe_path.display(),
            "install NMH failed: NMH binary not found"
        );
        return Err(msg);
    }

    let exe_str = exe_path
        .to_str()
        .ok_or_else(|| "exe path is not valid unicode".to_string())?;

    // 1. 生成 JSON 配置（allowed_origins 使用确定的扩展 ID）
    let origin = format!("chrome-extension://{}/", extension_id);
    let json_content = format!(
        r#"{{
  "name": "{nm_host_name}",
  "description": "IRtool Attribution Helper",
  "path": "{exe_str}",
  "type": "stdio",
  "allowed_origins": [
    "{origin}"
  ]
}}"#,
        nm_host_name = NM_HOST_NAME,
        exe_str = exe_str.replace('\\', "\\\\"),
        origin = origin,
    );

    // 2. 写入 JSON 文件
    let nm_dir = nm_host_json_path(browser);
    std::fs::create_dir_all(&nm_dir).map_err(|e| {
        let msg = format!("failed to create NM dir: {}", e);
        tracing::error!(error = %msg, nm_dir = %nm_dir.display(), "install NMH failed");
        msg
    })?;
    let json_path = nm_dir.join(format!("{}.json", NM_HOST_NAME));
    std::fs::write(&json_path, &json_content).map_err(|e| {
        let msg = format!("failed to write NM json: {}", e);
        tracing::error!(error = %msg, json_path = %json_path.display(), "install NMH failed");
        msg
    })?;
    tracing::info!(
        json_path = %json_path.display(),
        nmh_exe_path = %exe_path.display(),
        "NMH JSON config written"
    );

    // 3. 注册到注册表（使用 reg add 避免提权问题）
    let reg_path = nm_host_reg_path(browser);
    let reg_key = format!(r"{}\{}", reg_path, NM_HOST_NAME);
    let json_path_str = json_path
        .to_str()
        .ok_or_else(|| "json path is not valid unicode".to_string())?;

    let output = std::process::Command::new("reg")
        .args(["add", &reg_key, "/ve", "/d", json_path_str, "/f"])
        .output()
        .map_err(|e| {
            let msg = format!("failed to run reg add: {}", e);
            tracing::error!(error = %msg, reg_key = %reg_key, "install NMH failed");
            msg
        })?;

    if !output.status.success() {
        // reg.exe 在中文 Windows 上以 GBK（CP936）编码输出错误信息，
        // 用 encoding_rs 解码避免乱码（参考 irtool-autoruns/src/delete.rs）。
        let (stderr, _, _) = encoding_rs::GBK.decode(&output.stderr);
        let msg = format!("reg add failed: {}", stderr.trim());
        tracing::error!(
            error = %msg,
            reg_key = %reg_key,
            exit_code = ?output.status.code(),
            "install NMH failed: reg add error"
        );
        return Err(msg);
    }

    tracing::info!(
        browser = %browser.display_name(),
        json_path = %json_path_str,
        extension_id = %extension_id,
        "native messaging host installed successfully"
    );

    Ok(format!(
        "Native Messaging Host '{}' installed for {} at {} (extension: {})",
        NM_HOST_NAME,
        browser.display_name(),
        json_path_str,
        extension_id,
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
    }

    #[test]
    fn nm_host_reg_path_format() {
        // 必须以 HKCU\ 前缀，否则 reg add 会报参数错误
        assert!(nm_host_reg_path(BrowserKind::Chrome).starts_with("HKCU\\"));
        assert!(nm_host_reg_path(BrowserKind::Chrome).contains("Google"));
        assert!(nm_host_reg_path(BrowserKind::Edge).starts_with("HKCU\\"));
        assert!(nm_host_reg_path(BrowserKind::Edge).contains("Microsoft"));
    }

    #[test]
    fn json_content_valid() {
        // 使用固定的 Helper Extension ID 生成 NMH JSON
        let origin = format!("chrome-extension://{}/", HELPER_EXTENSION_ID);
        let json = format!(
            r#"{{
  "name": "{}",
  "description": "IRtool Attribution Helper",
  "path": "C:\\\\test\\\\irtool.exe",
  "type": "stdio",
  "allowed_origins": [
    "{}"
  ]
}}"#,
            NM_HOST_NAME, origin,
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("json should be valid");
        assert_eq!(parsed["name"], NM_HOST_NAME);
        assert_eq!(parsed["type"], "stdio");
        assert_eq!(parsed["allowed_origins"][0], origin);
    }

    #[test]
    fn helper_extension_id_format_valid() {
        // 固定扩展 ID 必须是 32 位小写字母 (a-p)
        assert_eq!(HELPER_EXTENSION_ID.len(), 32);
        assert!(
            HELPER_EXTENSION_ID.chars().all(|c| ('a'..='p').contains(&c)),
            "extension id must only contain chars a-p, got: {}",
            HELPER_EXTENSION_ID
        );
    }

    #[test]
    fn install_rejects_extension_id_outside_a_to_p() {
        // 32 chars but contains 'q' (outside the a-p alphabet Chrome uses)
        let bad_id = "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";
        let result = install_native_messaging_host(BrowserKind::Chrome, Some(bad_id));
        assert!(result.is_err(), "extension id containing 'q' should be rejected");
    }
}
