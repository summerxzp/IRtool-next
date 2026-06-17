use irtool_core::IrError;
use irtool_service::services::workspace::WorkspaceService;

#[tauri::command]
#[specta::specta]
pub async fn cmd_workspace_run_command(program: String, args: String) -> Result<String, IrError> {
    WorkspaceService::run_command(program, args).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_workspace_unhide_path(path: String) -> Result<String, IrError> {
    WorkspaceService::unhide_path(path)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_workspace_take_ownership(path: String) -> Result<String, IrError> {
    WorkspaceService::take_ownership(path)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_workspace_sample_path(path: String, output_dir: String, password: String) -> Result<String, IrError> {
    WorkspaceService::sample_path(path, output_dir, password)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_workspace_open_path(path: String) -> Result<String, IrError> {
    WorkspaceService::open_path(path)
}
