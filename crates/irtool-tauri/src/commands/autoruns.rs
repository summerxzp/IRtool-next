use crate::events::{EVT_AUTORUNS_PROGRESS, EVT_AUTORUNS_SIGNATURE_PROGRESS, EVT_TASK_CANCELLED, EVT_TASK_FAILED};
use crate::state::AppState;
use irtool_autoruns::{
    AutorunItem, DeleteResult, ScanOptions, ScanPhase, ScanProgress, SignatureProgress,
};
use irtool_core::{IrError, TaskId};
use std::path::PathBuf;
use tauri::{Emitter, State};

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_scan(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    options: ScanOptions,
) -> Result<TaskId, IrError> {
    let scanner = state.autoruns_scanner.clone();
    if scanner.is_none() {
        tracing::error!("autoruns scan requested but scanner is not available");
        return Err(IrError::FeatureDisabled("autoruns scanner not available".into()));
    }

    let store = state.autoruns_store.clone();
    let tasks = state.tasks.clone();
    let (id, token) = tasks.register();
    let app_for_progress = app.clone();
    let app_for_result = app.clone();

    tracing::info!("autoruns scan started, task_id={}", id);

    tauri::async_runtime::spawn(async move {
        let task_id = id;
        let scanner = scanner;
        let progress = move |mut p: ScanProgress| {
            p.task_id = task_id;
            let _ = app_for_progress.emit(EVT_AUTORUNS_PROGRESS, &p);
        };

        let result = match scanner.as_ref() {
            Some(s) => s.scan(options, progress, token).await,
            None => Err(IrError::FeatureDisabled("autoruns scanner not available".into())),
        };

        match result {
            Ok(items) => {
                let count = items.len();
                store.clear_and_put(items);
                let _ = app_for_result.emit(
                    EVT_AUTORUNS_PROGRESS,
                    &ScanProgress {
                        task_id,
                        phase: ScanPhase::Complete,
                        current: count,
                        total: count,
                        message: format!("扫描完成，共 {} 项", count),
                    },
                );
                tasks.finish(id);
            }
            Err(IrError::Cancelled) => {
                tracing::info!("autoruns scan cancelled, task_id={}", id);
                let _ = app_for_result.emit(EVT_TASK_CANCELLED, id);
            }
            Err(e) => {
                tracing::error!("autoruns scan failed, task_id={}: {}", id, e);
                let _ = app_for_result.emit(
                    EVT_TASK_FAILED,
                    serde_json::json!({"task_id": id, "error": e}),
                );
            }
        }
        tasks.finish(id);
    });

    Ok(id)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_get_result(
    state: State<'_, AppState>,
) -> Result<Vec<AutorunItem>, IrError> {
    Ok(state.autoruns_store.get_all())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_verify_signatures(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<TaskId, IrError> {
    let scanner = state.autoruns_scanner.clone();
    if scanner.is_none() {
        return Err(IrError::FeatureDisabled("autoruns scanner not available".into()));
    }

    let store = state.autoruns_store.clone();
    let tasks = state.tasks.clone();
    let (id, _token) = tasks.register();
    let app_clone = app.clone();

    tokio::task::spawn_blocking(move || {
        let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let task_id = id;

        if let Some(s) = scanner.as_ref() {
            let results = s.verify_signatures_batch(&path_bufs, |current, total| {
                let _ = app_clone.emit(
                    EVT_AUTORUNS_SIGNATURE_PROGRESS,
                    &SignatureProgress {
                        task_id,
                        current,
                        total,
                    },
                );
            });

            for (path, status) in results {
                store.update_signature(&path.to_string_lossy(), status);
            }
        }

        tasks.finish(id);
    });

    Ok(id)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_delete_entry(
    state: State<'_, AppState>,
    entry_id: u64,
) -> Result<DeleteResult, IrError> {
    let scanner = state.autoruns_scanner.clone();
    let scanner_ref = scanner.as_ref().as_ref()
        .ok_or_else(|| IrError::FeatureDisabled("autoruns scanner not available".into()))?;

    let item = state
        .autoruns_store
        .get(entry_id)
        .ok_or_else(|| IrError::Internal("entry not found".into()))?;

    let result = scanner_ref.delete_entry(&item)?;

    if result.success {
        state.autoruns_store.remove(entry_id);
    }

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_cancel_scan(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> Result<(), IrError> {
    state.tasks.cancel(task_id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_calculate_hash(
    state: State<'_, AppState>,
    entry_id: u64,
) -> Result<AutorunItem, IrError> {
    let item = state.autoruns_store.get(entry_id)
        .ok_or_else(|| IrError::Internal("entry not found".into()))?;
    let path_str = item.image_path.as_deref()
        .ok_or_else(|| IrError::Io("no image path".into()))?;
    let path = PathBuf::from(path_str);
    let (md5, sha256) = irtool_autoruns::AutorunsScanner::calculate_hash(&path)?;
    state.autoruns_store.update_hash(entry_id, md5, sha256);
    state.autoruns_store.get(entry_id)
        .ok_or_else(|| IrError::Internal("entry not found after update".into()))
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_autoruns_sigcheck(
    image_path: String,
) -> Result<String, IrError> {
    let sigcheck_path = irtool_autoruns::find_sigcheck()?;
    let file_path = PathBuf::from(&image_path);
    irtool_autoruns::AutorunsScanner::sigcheck_file(&sigcheck_path, &file_path)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_open_explorer(
    path: String,
) -> Result<(), IrError> {
    irtool_autoruns::open_in_explorer(&path)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_open_regedit(
    registry_path: String,
) -> Result<(), IrError> {
    irtool_autoruns::open_regedit(&registry_path)
}

#[tauri::command]
#[specta::specta]
pub fn cmd_autoruns_open_services() -> Result<(), IrError> {
    irtool_autoruns::open_services_msc()
}
