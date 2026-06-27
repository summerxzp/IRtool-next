use irtool_browser_forensics::extension_attribution::ExtensionAttribution;
use irtool_browser_forensics::*;
use irtool_core::IrError;
use irtool_service::context::AppContext;
use irtool_service::services::extension_connection::ExtensionConnectionStatus;
use tauri::{Manager, State};

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_list_profiles(_ctx: State<'_, AppContext>) -> Result<Vec<BrowserProfile>, IrError> {
    tokio::task::spawn_blocking(|| Ok(enumerate_all_profiles()))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_scan_extensions(
    _ctx: State<'_, AppContext>,
    browser: BrowserKind,
    profile_name: String,
) -> Result<ExtensionInventory, IrError> {
    tokio::task::spawn_blocking(move || {
        let profiles = enumerate_profiles(browser);
        let profile = profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
        Ok(scan_extensions_cached(browser, &profile))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_scan_all_extensions(
    _ctx: State<'_, AppContext>,
    browser: BrowserKind,
) -> Result<Vec<ExtensionInventory>, IrError> {
    tokio::task::spawn_blocking(move || {
        let profiles = enumerate_profiles(browser);
        Ok(profiles.iter().map(|p| scan_extensions_cached(browser, p)).collect())
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_scan_downloads(
    _ctx: State<'_, AppContext>,
    browser: BrowserKind,
    profile_name: String,
) -> Result<DownloadAttribution, IrError> {
    tokio::task::spawn_blocking(move || {
        let profiles = enumerate_profiles(browser);
        let profile = profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
        Ok(irtool_browser_forensics::scan_downloads(&profile))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_recover_tabs(
    _ctx: State<'_, AppContext>,
    browser: BrowserKind,
    profile_name: String,
) -> Result<SessionRecoveryResult, IrError> {
    tokio::task::spawn_blocking(move || {
        let profiles = enumerate_profiles(browser);
        let profile = profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
        Ok(recover_tabs(&profile))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_attribute_history(
    _ctx: State<'_, AppContext>,
    browser: BrowserKind,
    profile_name: String,
    target_time: String,
) -> Result<HistoryAttribution, IrError> {
    tokio::task::spawn_blocking(move || {
        let target_time = chrono::DateTime::parse_from_rfc3339(&target_time)
            .map_err(|e| IrError::Internal(format!("invalid timestamp: {}", e)))?
            .to_utc();
        let profiles = enumerate_profiles(browser);
        let profile = profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
        Ok(attribute_history(&profile, target_time, ""))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_scan_history(
    _ctx: State<'_, AppContext>,
    browser: BrowserKind,
    profile_name: String,
    limit: Option<i64>,
    since: Option<i64>,
) -> Result<HistoryList, IrError> {
    let limit = limit.unwrap_or(500);
    tokio::task::spawn_blocking(move || {
        let profiles = enumerate_profiles(browser);
        let profile = profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
        Ok(irtool_browser_forensics::scan_history(&profile, limit, since))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_context_attribution(
    _ctx: State<'_, AppContext>,
    domain: String,
    ip: Option<String>,
    process_name: String,
    pid: u32,
    timestamp: String,
    cmdline: Option<String>,
) -> Result<EvidenceObject, IrError> {
    let ts = chrono::DateTime::parse_from_rfc3339(&timestamp)
        .map_err(|e| IrError::Internal(format!("invalid timestamp: {}", e)))?
        .to_utc();
    tokio::task::spawn_blocking(move || {
        Ok(attribute_browser_context(
            &domain,
            ip.as_deref(),
            &process_name,
            pid,
            ts,
            cmdline.as_deref(),
        ))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_attribute_extension(
    _ctx: State<'_, AppContext>,
    process_name: String,
    pid: u32,
    domain: String,
    cmdline: Option<String>,
) -> Result<ExtensionAttribution, IrError> {
    tokio::task::spawn_blocking(move || {
        irtool_browser_forensics::extension_attribution::attribute_extension(
            &process_name,
            pid,
            &domain,
            cmdline.as_deref(),
        )
        .ok_or_else(|| IrError::Internal("failed to attribute extension".to_string()))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// 注册 Helper Extension 的 Native Messaging Host
///
/// 写入 Native Messaging Host JSON 配置文件并注册到浏览器注册表。
/// 支持 Chrome / Edge。
///
/// 扩展 ID 默认由 manifest.json 的 `key` 字段固定，无需前端传入。
/// `extension_id_override` 用于兜底场景（高级选项），用户可手动输入扩展 ID 覆盖。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_install_native_messaging_host(
    browser: BrowserKind,
    extension_id_override: Option<String>,
) -> Result<String, IrError> {
    tokio::task::spawn_blocking(move || {
        irtool_browser_forensics::install_helper::install_native_messaging_host(
            browser,
            extension_id_override.as_deref(),
        )
        .map_err(|e| IrError::Internal(format!("failed to install native messaging host: {}", e)))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// 获取 Helper Extension 目录的绝对路径。
///
/// dev 模式：返回 workspace 根下的 `helper-extension/` 目录（基于 CARGO_MANIFEST_DIR 解析）。
/// release 模式：返回 Tauri resource_dir 下的 `helper-extension/`。
///
/// 前端用此路径：1) 复制到剪贴板供用户 Load unpacked 时粘贴；2) 判断扩展目录是否存在。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_get_helper_extension_path(
    app: tauri::AppHandle,
) -> Result<String, IrError> {
    let path = if cfg!(debug_assertions) {
        // dev 模式：crates/irtool-tauri/ → 上溯两级到 workspace 根 → helper-extension/
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        std::path::Path::new(manifest_dir)
            .join("..")
            .join("..")
            .join("helper-extension")
            .canonicalize()
            .map_err(|e| IrError::Internal(format!("failed to resolve helper-extension path: {}", e)))?
            .to_string_lossy()
            .to_string()
    } else {
        // release 模式：Tauri resource_dir 下的 helper-extension/
        let resource_dir = app
            .path()
            .resource_dir()
            .map_err(|e| IrError::Internal(format!("failed to get resource_dir: {}", e)))?;
        resource_dir
            .join("helper-extension")
            .to_string_lossy()
            .to_string()
    };

    // Windows canonicalize() 会加 `\\?\` 前缀（扩展长度路径语法），
    // Chrome 的"加载已解压的扩展程序"文件夹选择器不识别该前缀，需 strip。
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path).to_string();
    Ok(path)
}

/// 打开浏览器的扩展管理页。
///
/// Chrome → `chrome://extensions`
/// Edge   → `edge://extensions`
///
/// 通过启动浏览器 exe 并传递 URL 参数实现（`chrome://` 不是标准 URL scheme，
/// 无法用 shell open 直接打开）。浏览器 exe 路径从注册表或 Program Files 查找。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_open_extensions_page(browser: BrowserKind) -> Result<(), IrError> {
    tokio::task::spawn_blocking(move || {
        let (exe_candidates, url) = match browser {
            BrowserKind::Chrome => (
                vec![
                    r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
                    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe".to_string(),
                    env_local_app_data_chrome(),
                ],
                "chrome://extensions",
            ),
            BrowserKind::Edge => (
                vec![
                    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string(),
                    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe".to_string(),
                ],
                "edge://extensions",
            ),
        };

        for exe in &exe_candidates {
            if std::path::Path::new(exe).exists() {
                std::process::Command::new(exe)
                    .arg(url)
                    .spawn()
                    .map_err(|e| IrError::Internal(format!("failed to launch {}: {}", exe, e)))?;
                return Ok(());
            }
        }
        Err(IrError::Internal(format!(
            "{} executable not found in standard locations",
            browser.display_name()
        )))
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

fn env_local_app_data_chrome() -> String {
    std::env::var("LOCALAPPDATA")
        .map(|d| format!(r"{}\Google\Chrome\Application\chrome.exe", d))
        .unwrap_or_default()
}

/// 基于域名/IP 的归因：查找所有与目标相关的浏览器痕迹
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_attribute_by_domain(
    _ctx: State<'_, AppContext>,
    target: String,
    browser: BrowserKind,
) -> Result<Vec<DomainAttribution>, IrError> {
    tokio::task::spawn_blocking(move || Ok(attribute_by_domain(&target, browser)))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// 向 Helper Extension 下发域名过滤规则（config 下行通道）。
///
/// 写入 `%TEMP%\irtool\config.json`，NMH host 检测到文件变更后
/// 通过 stdout 向扩展转发 `{"type":"config","filterDomains":[...]}`。
///
/// 传递空数组可清除过滤规则。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_send_config(filter_domains: Vec<String>) -> Result<(), IrError> {
    irtool_service::services::browser_forensics::send_config(&filter_domains)
        .map_err(|e| IrError::Internal(format!("send_config failed: {}", e)))
}

/// 读取当前已下发的 filterDomains（用于 UI 启动时同步显示）。
///
/// 从 `%TEMP%\irtool\config.json` 读取。文件不存在时返回空数组。
/// 用途：IRtool 重启后 UI store 清空，但磁盘 config 仍有效，扩展继续按旧配置过滤；
/// 此命令让 UI 能把磁盘上的已下发域名同步回界面，避免"已下发但 UI 看不到"的不一致。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_get_config() -> Result<Vec<String>, IrError> {
    Ok(irtool_service::services::browser_forensics::get_native_config_filter_domains())
}

/// 查询 Helper Extension 连接状态
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_extension_status(
    ctx: State<'_, AppContext>,
) -> Result<ExtensionConnectionStatus, IrError> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(ctx.extension_connection.status(now_ms))
}

/// 重新连接诊断结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ReconnectDiagnostics {
    /// 被 kill 的 NMH 进程数
    pub killed_processes: usize,
    /// NMH 二进制路径（从 install_helper 推算）
    pub nmh_exe_path: String,
    /// NMH 二进制是否存在
    pub nmh_exe_exists: bool,
    /// 当前连接状态
    pub connection: ExtensionConnectionStatus,
}

/// 重新连接 Helper Extension
///
/// 1. kill 所有卡死的 NMH 进程（irtool-native-messaging-host.exe）
/// 2. 下发 reconnectSignal 给扩展（通过 config.json → NMH → 扩展）
///    扩展收到后立即重置退避计数器并尝试重连，省去等待指数退避的时间
/// 3. 返回诊断信息（NMH 二进制路径/存在性、连接状态）
///
/// 注意：reconnectSignal 只有在 NMH 进程存活且扩展 port 已断开时才有效。
/// 如果 NMH 进程被 kill，扩展的 onDisconnect 会触发自动重连（无需 signal）。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_reconnect_extension(
    ctx: State<'_, AppContext>,
) -> Result<ReconnectDiagnostics, IrError> {
    let started_at = std::time::Instant::now();

    // 1. kill 所有 NMH 进程（用 taskkill，忽略"未找到进程"错误）
    let killed = kill_nmh_processes();
    tracing::info!(
        killed_processes = killed,
        "reconnect: killed NMH processes"
    );

    // 2. 推算 NMH 二进制路径（与 install_helper::nmh_exe_path 一致）
    let nmh_exe_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .map(|d| {
            if cfg!(windows) {
                d.join("irtool-native-messaging-host.exe")
            } else {
                d.join("irtool-native-messaging-host")
            }
        })
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let nmh_exe_exists = !nmh_exe_path.is_empty() && std::path::Path::new(&nmh_exe_path).exists();
    tracing::info!(
        nmh_exe_path = %nmh_exe_path,
        nmh_exe_exists = nmh_exe_exists,
        "reconnect: NMH exe status"
    );

    // 3. 下发 reconnectSignal（即使 killed=0，扩展 port 可能已断开但 NMH 进程存活，
    //    此时 signal 能让扩展立即重连而非等待指数退避）
    irtool_service::services::browser_forensics::send_reconnect_signal();

    // 4. 获取当前连接状态
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let connection = ctx.extension_connection.status(now_ms);

    tracing::info!(
        killed_processes = killed,
        nmh_exe_exists = nmh_exe_exists,
        connected = connection.connected,
        last_heartbeat_ms = connection.last_heartbeat_ms,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "reconnect: completed"
    );

    Ok(ReconnectDiagnostics {
        killed_processes: killed,
        nmh_exe_path,
        nmh_exe_exists,
        connection,
    })
}

/// kill 所有 irtool-native-messaging-host 进程
fn kill_nmh_processes() -> usize {
    let output = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "irtool-native-messaging-host.exe"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match output {
        Ok(o) if o.status.success() => {
            // taskkill 成功，解析输出获取进程数
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().filter(|l| l.contains("PID")).count().max(1)
        }
        _ => 0,
    }
}
