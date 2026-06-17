use irtool_core::IrError;
use irtool_service::context::AppContext;
use irtool_service::services::sysmon::SysmonService;
use irtool_sysmon::{EventConfigEntry, SysmonEvent, SysmonStatus};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_status(ctx: State<'_, AppContext>) -> Result<SysmonStatus, IrError> {
    SysmonService { ctx: &ctx }.status().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_is_channel_available(ctx: State<'_, AppContext>) -> Result<bool, IrError> {
    SysmonService { ctx: &ctx }.is_channel_available().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_install(ctx: State<'_, AppContext>, accept_eula: bool) -> Result<(bool, String), IrError> {
    SysmonService { ctx: &ctx }.install(accept_eula).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_uninstall(ctx: State<'_, AppContext>) -> Result<(bool, String), IrError> {
    SysmonService { ctx: &ctx }.uninstall().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_update_config(ctx: State<'_, AppContext>) -> Result<(bool, String), IrError> {
    SysmonService { ctx: &ctx }.update_config().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_get_existing_events(
    ctx: State<'_, AppContext>,
    limit: u32,
    enabled_event_ids: Vec<u32>,
) -> Result<Vec<SysmonEvent>, IrError> {
    SysmonService { ctx: &ctx }.get_existing_events(limit, enabled_event_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_default_event_configs() -> Result<Vec<EventConfigEntry>, IrError> {
    SysmonService::default_event_configs().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_generate_config(
    ctx: State<'_, AppContext>,
    enabled_events: Vec<String>,
) -> Result<String, IrError> {
    SysmonService { ctx: &ctx }.generate_config(enabled_events).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_start_subscription(
    ctx: State<'_, AppContext>,
    enabled_event_ids: Vec<u32>,
    poll_interval_ms: Option<u64>,
) -> Result<(), IrError> {
    SysmonService { ctx: &ctx }
        .start_subscription(enabled_event_ids, poll_interval_ms)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_stop_subscription(ctx: State<'_, AppContext>) -> Result<(), IrError> {
    SysmonService { ctx: &ctx }.stop_subscription().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_is_subscribing(ctx: State<'_, AppContext>) -> Result<bool, IrError> {
    Ok(SysmonService { ctx: &ctx }.is_subscribing())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_get_event_count(
    ctx: State<'_, AppContext>,
    enabled_event_ids: Vec<u32>,
) -> Result<u64, IrError> {
    SysmonService { ctx: &ctx }.get_event_count(enabled_event_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_get_log_max_size(ctx: State<'_, AppContext>) -> Result<u64, IrError> {
    SysmonService { ctx: &ctx }.get_log_max_size().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_set_log_max_size(ctx: State<'_, AppContext>, size_mb: u64) -> Result<(), IrError> {
    SysmonService { ctx: &ctx }.set_log_max_size(size_mb).await
}
