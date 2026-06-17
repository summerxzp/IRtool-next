use irtool_core::IrError;
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
