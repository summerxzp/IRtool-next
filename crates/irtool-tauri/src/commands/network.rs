use irtool_core::IrError;
use irtool_service::context::AppContext;
use irtool_service::dto::network::{NetworkPollingControl, NetworkSnapshotPayload};
use irtool_service::services::network::NetworkService;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_snapshot(ctx: State<'_, AppContext>) -> Result<NetworkSnapshotPayload, IrError> {
    NetworkService { ctx: &ctx }.snapshot().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_kill_process(ctx: State<'_, AppContext>, pid: u32) -> Result<(), IrError> {
    NetworkService { ctx: &ctx }.kill_process(pid).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_set_polling(
    ctx: State<'_, AppContext>,
    control: NetworkPollingControl,
) -> Result<(), IrError> {
    NetworkService { ctx: &ctx }.set_polling(control).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_clear_history(ctx: State<'_, AppContext>) -> Result<(), IrError> {
    NetworkService { ctx: &ctx }.clear_history().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_refresh_cmdline(ctx: State<'_, AppContext>, pid: u32) -> Result<(), IrError> {
    NetworkService { ctx: &ctx }.refresh_cmdline(pid).await
}
