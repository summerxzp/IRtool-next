use irtool_core::IrError;
use irtool_process::{get_process_chain, take_snapshot, ProcessChain, ProcessSnapshot};

#[tauri::command]
#[specta::specta]
pub async fn cmd_process_snapshot() -> Result<ProcessSnapshot, IrError> {
    tokio::task::spawn_blocking(|| take_snapshot())
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_process_chain(pid: u32) -> Result<ProcessChain, IrError> {
    tokio::task::spawn_blocking(move || get_process_chain(pid))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}
