use irtool_browser_forensics::extension_attribution::ExtensionAttribution;
use irtool_browser_forensics::*;
use irtool_cdp::discovery::{discover_targets, is_port_listening};
use irtool_core::IrError;
use irtool_service::context::AppContext;
use irtool_service::services::cdp_capture::CdpCaptureService;
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
pub async fn cmd_browser_forensics_get_helper_extension_path(app: tauri::AppHandle) -> Result<String, IrError> {
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
        resource_dir.join("helper-extension").to_string_lossy().to_string()
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

/// 下发自我卸载信号给扩展（手动清理）。
///
/// 在 config.json 中写入 `selfUninstall: <timestamp>`，NMH 检测到 mtime 变化后
/// 透传给扩展，扩展调用 `chrome.management.uninstallSelf()` 立即卸载。
///
/// 用途：应急响应场景，用户点击"清理扩展"按钮时调用。
/// 注意：此操作不可逆，扩展会被立即卸载。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_self_uninstall() -> Result<(), IrError> {
    irtool_service::services::browser_forensics::send_self_uninstall_signal();
    Ok(())
}

/// 设置扩展自我清理超时时间（分钟）。
///
/// 写入 config.json 的 `selfCleanupTimeoutMin` 字段：
/// - 0 = 禁用自动清理
/// - >0 = 启用，IRtool 离线超过该时长后扩展自动卸载（默认 60）
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_set_self_cleanup_timeout(timeout_min: u32) -> Result<(), IrError> {
    irtool_service::services::browser_forensics::set_self_cleanup_timeout(timeout_min);
    Ok(())
}

/// 读取当前 selfCleanupTimeoutMin 配置。
///
/// 返回 None 时 UI 应显示默认值 60。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_get_self_cleanup_timeout() -> Result<Option<u32>, IrError> {
    Ok(irtool_service::services::browser_forensics::get_self_cleanup_timeout())
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
    tracing::info!(killed_processes = killed, "reconnect: killed NMH processes");

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

// ── CDP 远程调试抓包 ───────────────────────────────────────────

/// CDP 抓包服务状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CdpCaptureStatus {
    /// 抓包服务是否运行
    pub running: bool,
    /// 已连接的浏览器类型（running=true 时有值）
    pub browser_kind: Option<String>,
    /// 调试端口（running=true 时有值）
    pub port: Option<u16>,
}

/// 探测浏览器调试端口（不启动抓包服务，仅检查 9222/9223/9229 是否有浏览器监听）。
///
/// 用于 UI 判断：若探测到端口 → 显示"启动抓包"按钮；
/// 若未探测到 → 显示"启动调试浏览器"按钮（自动启动带调试端口的浏览器）。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_cdp_probe() -> Result<Option<CdpCaptureStatus>, IrError> {
    let targets = discover_targets().await;
    Ok(targets.into_iter().next().map(|t| CdpCaptureStatus {
        running: false,
        browser_kind: Some(format!("{:?}", t.browser).to_lowercase()),
        port: Some(t.port),
    }))
}

/// 启动 CDP 抓包服务。
///
/// 内部流程：
/// 1. discover_targets 探测调试端口
/// 2. CdpCaptureService::start 启动后台抓包 task
/// 3. 句柄存入 AppContext.cdp_capture 供后续 stop 使用
///
/// 抓包事件通过 EventBus → evt_extension_attribution 推送到前端，
/// 复用现有 ExtensionEventsView 展示。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_cdp_capture_start(ctx: State<'_, AppContext>) -> Result<CdpCaptureStatus, IrError> {
    // 探测端口（用于状态返回）
    let targets = discover_targets().await;
    let cdp_target = targets.into_iter().next().ok_or_else(|| {
        IrError::Internal("no CDP target discovered (browser not running with --remote-debugging-port?)".to_string())
    })?;

    // 先获取锁，再启动 service，避免并发启动产生孤儿 task
    // （否则两个并发请求会在锁前各自启动 service，被覆盖的 service 无句柄可停止）
    let mut guard = ctx.cdp_capture.lock().await;
    if let Some(old) = guard.take() {
        tracing::warn!("cdp capture: previous service still running, stopping before start");
        let _ = old.stop().await;
    }

    // 持锁期间启动 service
    let service = CdpCaptureService::start(ctx.event_bus.clone()).await?;
    *guard = Some(service);
    drop(guard); // 释放锁，避免后续日志记录期间长期持锁

    tracing::info!(
        port = cdp_target.port,
        browser = ?cdp_target.browser,
        "cdp capture started"
    );

    Ok(CdpCaptureStatus {
        running: true,
        browser_kind: Some(format!("{:?}", cdp_target.browser).to_lowercase()),
        port: Some(cdp_target.port),
    })
}

/// 停止 CDP 抓包服务。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_cdp_capture_stop(ctx: State<'_, AppContext>) -> Result<(), IrError> {
    let mut guard = ctx.cdp_capture.lock().await;
    if let Some(service) = guard.take() {
        service.stop().await?;
        tracing::info!("cdp capture stopped");
    }
    Ok(())
}

/// 查询 CDP 抓包服务状态。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_cdp_capture_status(ctx: State<'_, AppContext>) -> Result<CdpCaptureStatus, IrError> {
    let guard = ctx.cdp_capture.lock().await;
    let running = guard.is_some();
    // 若运行中，再探测一次端口用于状态返回（端口可能在运行期间关闭）
    let port_browser = if running {
        let targets = discover_targets().await;
        targets
            .into_iter()
            .next()
            .map(|t| (t.port, format!("{:?}", t.browser).to_lowercase()))
    } else {
        None
    };

    Ok(CdpCaptureStatus {
        running,
        browser_kind: port_browser.as_ref().map(|(_, b)| b.clone()),
        port: port_browser.map(|(p, _)| p),
    })
}

/// 启动带调试端口的浏览器（用于"一键启动调试浏览器"按钮）。
///
/// 用 `chrome.exe --remote-debugging-port=9222 --user-data-dir=<temp>` 启动独立实例。
/// 临时 profile 路径：`%TEMP%\irtool\debug-profile`，避免污染用户主 profile。
///
/// **关键限制**：Chrome/Edge 的单实例机制会拒绝在新进程启用调试端口，
/// 如果已有同名浏览器进程在运行（包括托盘后台进程），新进程会把参数转发给已有实例后立即退出。
/// 因此启动前必须先检测同名进程，若存在则直接返回错误，提示用户手动关闭。
///
/// spawn 后轮询 9222 端口是否监听（最多 8s），双重保险。
#[tauri::command]
#[specta::specta]
pub async fn cmd_browser_forensics_launch_browser_with_debug_port(browser: BrowserKind) -> Result<(), IrError> {
    let (exe_candidates, browser_name) = match browser {
        BrowserKind::Chrome => (
            vec![
                r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe".to_string(),
                env_local_app_data_chrome(),
            ],
            "Chrome",
        ),
        BrowserKind::Edge => (
            vec![
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string(),
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe".to_string(),
            ],
            "Edge",
        ),
    };

    let exe = exe_candidates
        .into_iter()
        .find(|p| !p.is_empty() && std::path::Path::new(p).exists())
        .ok_or_else(|| IrError::Internal(format!("{} executable not found", browser_name)))?;

    tracing::info!(exe = %exe, browser = %browser_name, "launching browser with debug port 9222");

    // spawn 前先检测端口是否已被占用（可能是之前的调试浏览器实例仍存活）
    if is_port_listening(9222).await {
        tracing::info!("port 9222 already listening before launch, skip spawning new browser");
        return Ok(());
    }

    // 临时 profile 目录
    let temp_dir = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Temp".to_string());
    let debug_profile = format!(r"{}\irtool\debug-profile", temp_dir);
    std::fs::create_dir_all(&debug_profile)
        .map_err(|e| IrError::Internal(format!("failed to create debug profile dir: {}", e)))?;

    // 清理可能残留的 SingletonLock（Chrome 异常退出后会残留，导致新进程误判已有实例）
    let singleton_lock = std::path::Path::new(&debug_profile).join("SingletonLock");
    if singleton_lock.exists() {
        tracing::info!(path = %singleton_lock.display(), "removing stale SingletonLock");
        let _ = std::fs::remove_file(&singleton_lock);
    }

    // 将 Chrome 的 stdout/stderr 重定向到日志文件，便于诊断退出原因
    let stderr_log = std::path::Path::new(&debug_profile).join("chrome-stderr.log");
    let stderr_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&stderr_log)
        .map_err(|e| IrError::Internal(format!("failed to open chrome stderr log: {}", e)))?;

    // 启动浏览器（独立进程，捕获 stderr 以便诊断）
    //
    // 关键：--user-data-dir 必须用等号连接路径（--user-data-dir=<path>），
    // 不能分成两个参数（"--user-data-dir", <path>）。
    // Chrome 的命令行解析会把分开的 --user-data-dir 当成无值开关，
    // 路径会被当成位置参数（URL），导致 Chrome 136+ 安全限制触发：
    // --remote-debugging-port 必须搭配非默认 user-data-dir，否则拒绝启用调试端口。
    let stdout_stdio = match stderr_file.try_clone() {
        Ok(f) => std::process::Stdio::from(f),
        Err(_) => std::process::Stdio::null(),
    };
    let user_data_dir_arg = format!("--user-data-dir={}", debug_profile);
    let mut child = std::process::Command::new(&exe)
        .args([
            "--remote-debugging-port=9222",
            &user_data_dir_arg,
            "--no-first-run",
            "--no-default-browser-check",
        ])
        .stdout(stdout_stdio)
        .stderr(std::process::Stdio::from(stderr_file))
        .spawn()
        .map_err(|e| IrError::Internal(format!("failed to launch {}: {}", exe, e)))?;

    let pid = child.id();
    tracing::info!(exe = %exe, profile = %debug_profile, pid, stderr_log = %stderr_log.display(), "browser process spawned, waiting for port 9222 to listen");

    // 轮询端口监听（最多 15s，每 500ms 一次）
    //
    // 关键：不依赖 child.try_wait() 判断 Chrome 是否退出。
    // Chrome 多进程架构下，spawn 拿到的主进程（launcher）启动真正的 browser process 后
    // 会立即退出（exit code 0），这是正常行为，不代表启动失败。
    // 特别是管理员权限启动时，主进程退出更快（1 秒内）。
    // 真正的 browser process 会继续运行并监听 9222 端口。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if is_port_listening(9222).await {
            tracing::info!(pid, "port 9222 is now listening, browser ready for CDP capture");
            return Ok(());
        }

        if std::time::Instant::now() >= deadline {
            tracing::error!(pid, "timed out waiting for port 9222 to listen (15s)");
            let stderr_content = std::fs::read_to_string(&stderr_log).unwrap_or_default();
            let stderr_preview: String = stderr_content.chars().take(2000).collect();
            tracing::error!(pid, stderr = %stderr_preview, "chrome stderr at timeout");
            // 尝试 kill 残留进程
            let _ = child.kill();
            return Err(IrError::Internal(format!(
                "{name} 已启动但 15 秒内端口 9222 未监听（spawn PID {pid}）。\n\n\
                 可能原因：\n\
                 1) 仍有 {name} 进程在运行（单实例转发，新进程参数被忽略）\n\
                 2) Chrome 136+ 安全限制：--remote-debugging-port 必须搭配非默认 user-data-dir\n\
                 3) {name} 启动缓慢或被安全软件拦截\n\n\
                 Chrome stderr 输出：\n{stderr}\n\n\
                 诊断文件：{log}",
                name = browser_name,
                pid = pid,
                stderr = if stderr_preview.is_empty() {
                    "(无输出)"
                } else {
                    &stderr_preview
                },
                log = stderr_log.display(),
            )));
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// 统计指定浏览器进程数量（用于单实例冲突检测）。
///
/// `process_name` 不带 `.exe` 后缀（如 "chrome"、"msedge"）。
/// 使用 ToolHelp32 进程快照枚举，比 WMI 更轻量。
#[allow(dead_code)]
fn count_browser_processes(process_name: &str) -> usize {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::*;

    let target = format!("{}.exe", process_name.to_lowercase());
    let mut count = 0usize;

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return 0,
        };

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
                );
                if name.to_lowercase() == target {
                    count += 1;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    count
}
