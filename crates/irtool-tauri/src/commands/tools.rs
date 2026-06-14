use irtool_core::IrError;
use irtool_tools::{self as tools, ToolStatus};
use tauri::Emitter;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn cmd_tools_check(state: State<'_, AppState>) -> Result<Vec<ToolStatus>, IrError> {
    let tools_dir = state.app_dirs.tools_dir();
    if !tools_dir.exists() {
        let _ = std::fs::create_dir_all(&tools_dir);
    }
    Ok(tools::check_tools(&tools_dir))
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_tools_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    tool_ids: Vec<String>,
) -> Result<(), IrError> {
    let tools_dir = state.app_dirs.tools_dir();
    if !tools_dir.exists() {
        let _ = std::fs::create_dir_all(&tools_dir);
    }

    // Download all tools concurrently
    let mut handles = Vec::new();

    for tool_id in tool_ids {
        let app_clone = app.clone();
        let tools_dir_clone = tools_dir.clone();
        let id = tool_id.clone();

        let handle = tokio::spawn(async move {
            let app_ref = app_clone.clone();
            let id_for_progress = id.clone();
            let result = tools::download_tool(&id, &tools_dir_clone, move |downloaded, total| {
                let _ = app_ref.emit(
                    "evt_tools_download_progress",
                    serde_json::json!({
                        "tool_id": id_for_progress,
                        "downloaded": downloaded,
                        "total": total,
                    }),
                );
            })
            .await;

            // Accept EULA after successful download
            if result.is_ok() {
                if let Err(e) = tools::accept_eula(&id, &tools_dir_clone).await {
                    tracing::warn!("接受 EULA 失败 (非致命): {}", e);
                }
            }

            (id, result)
        });

        handles.push(handle);
    }

    let mut errors = Vec::new();

    for handle in handles {
        match handle.await {
            Ok((tool_id, Err(e))) => {
                tracing::error!("下载工具 {} 失败: {}", tool_id, e);
                let _ = app.emit(
                    "evt_tools_download_error",
                    serde_json::json!({
                        "tool_id": tool_id,
                        "error": e.to_string(),
                    }),
                );
                errors.push(e);
            }
            Ok((_, Ok(()))) => {}
            Err(e) => {
                tracing::error!("下载任务异常: {}", e);
            }
        }
    }

    let _ = app.emit(
        "evt_tools_download_complete",
        serde_json::json!({
            "errors": errors.len(),
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_tools_import_zip(
    state: State<'_, AppState>,
    tool_id: String,
    zip_path: String,
) -> Result<(), IrError> {
    let tools_dir = state.app_dirs.tools_dir();
    if !tools_dir.exists() {
        let _ = std::fs::create_dir_all(&tools_dir);
    }
    let zip = std::path::PathBuf::from(&zip_path);
    if !zip.exists() {
        return Err(IrError::Internal(format!("ZIP 文件不存在: {}", zip_path)));
    }
    tools::import_tool_zip(&tool_id, &tools_dir, &zip)?;

    // Accept EULA after import
    if let Err(e) = tools::accept_eula(&tool_id, &tools_dir).await {
        tracing::warn!("接受 EULA 失败 (非致命): {}", e);
    }

    Ok(())
}
