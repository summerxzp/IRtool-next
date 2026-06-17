use irtool_autoruns::{AutorunItem, DeleteResult, ScanOptions};
use irtool_core::{IrError, TaskId};
use irtool_service::context::AppContext;
use irtool_service::services::autoruns::AutorunsService;
use std::sync::atomic::Ordering;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_scan(
    ctx: State<'_, AppContext>,
    options: ScanOptions,
) -> Result<TaskId, IrError> {
    AutorunsService { ctx: &ctx }.scan(options).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_get_result(ctx: State<'_, AppContext>) -> Result<Vec<AutorunItem>, IrError> {
    AutorunsService { ctx: &ctx }.get_result().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_verify_signatures(
    ctx: State<'_, AppContext>,
    paths: Vec<String>,
) -> Result<TaskId, IrError> {
    AutorunsService { ctx: &ctx }.verify_signatures(paths).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_delete_entry(ctx: State<'_, AppContext>, entry_id: u64) -> Result<DeleteResult, IrError> {
    AutorunsService { ctx: &ctx }.delete_entry(entry_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_cancel_scan(ctx: State<'_, AppContext>, task_id: TaskId) -> Result<(), IrError> {
    AutorunsService { ctx: &ctx }.cancel_scan(task_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_calculate_hash(ctx: State<'_, AppContext>, entry_id: u64) -> Result<AutorunItem, IrError> {
    AutorunsService { ctx: &ctx }.calculate_hash(entry_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_batch_calculate_hash(
    ctx: State<'_, AppContext>,
    entry_ids: Vec<u64>,
) -> Result<TaskId, IrError> {
    AutorunsService { ctx: &ctx }.batch_calculate_hash(entry_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_sigcheck(image_path: String) -> Result<String, IrError> {
    AutorunsService::sigcheck(image_path).await
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_open_explorer(path: String) -> Result<(), IrError> {
    AutorunsService::open_explorer(path)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_open_regedit(registry_path: String) -> Result<(), IrError> {
    AutorunsService::open_regedit(registry_path)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_open_services() -> Result<(), IrError> {
    AutorunsService::open_services()
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_extract_icon(image_path: String) -> Result<Option<String>, IrError> {
    AutorunsService::extract_icon(image_path)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_batch_extract_icons(paths: Vec<String>) -> Result<Vec<(String, Option<String>)>, IrError> {
    AutorunsService::batch_extract_icons(paths)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_is_scanning(ctx: State<'_, AppContext>) -> Result<bool, IrError> {
    Ok(ctx.autoruns_scanning.load(Ordering::SeqCst))
}
