use irtool_core::IrError;
use irtool_service::context::AppContext;
use irtool_service::services::tools::ToolsService;
use irtool_tools::ToolStatus;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cmd_tools_check(ctx: State<'_, AppContext>) -> Result<Vec<ToolStatus>, IrError> {
    ToolsService { ctx: &ctx }.check().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_tools_download(ctx: State<'_, AppContext>, tool_ids: Vec<String>) -> Result<(), IrError> {
    ToolsService { ctx: &ctx }.download(tool_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_tools_import_zip(
    ctx: State<'_, AppContext>,
    tool_id: String,
    zip_path: String,
) -> Result<(), IrError> {
    ToolsService { ctx: &ctx }.import_zip(tool_id, zip_path).await
}
