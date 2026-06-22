use irtool_autoruns::AutorunItem;
use irtool_core::IrError;
use irtool_net_monitor::NetConn;
use irtool_service::context::AppContext;
use irtool_service::services::process::ProcessService;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cmd_process_snapshot(ctx: State<'_, AppContext>) -> Result<irtool_process::ProcessSnapshot, IrError> {
    ProcessService { ctx: &ctx }.snapshot().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_process_chain(ctx: State<'_, AppContext>, pid: u32) -> Result<irtool_process::ProcessChain, IrError> {
    ProcessService { ctx: &ctx }.chain(pid).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_query_by_pid(ctx: State<'_, AppContext>, pid: u32) -> Result<Vec<NetConn>, IrError> {
    let history = ctx.net_history.clone();
    tokio::task::spawn_blocking(move || history.query_by_pid(pid))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_query_by_path(
    ctx: State<'_, AppContext>,
    exe_path: String,
) -> Result<Vec<AutorunItem>, IrError> {
    let store = ctx.autoruns_store.clone();
    tokio::task::spawn_blocking(move || store.query_by_path(&exe_path))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))
}
