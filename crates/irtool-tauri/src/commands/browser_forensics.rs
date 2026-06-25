use irtool_browser_forensics::extension_attribution::ExtensionAttribution;
use irtool_browser_forensics::*;
use irtool_core::IrError;
use irtool_service::context::AppContext;
use tauri::State;

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
        Ok(attribute_history(&profile, target_time))
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
) -> Result<HistoryList, IrError> {
    let limit = limit.unwrap_or(500);
    tokio::task::spawn_blocking(move || {
        let profiles = enumerate_profiles(browser);
        let profile = profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
        Ok(irtool_browser_forensics::scan_history(&profile, limit))
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
) -> Result<BrowserContext, IrError> {
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
