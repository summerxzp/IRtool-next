use crate::events::EVT_SYSMON_EVENT;
use crate::state::AppState;
use irtool_core::IrError;
use irtool_sysmon::{EventConfigEntry, SysmonEvent, SysmonStatus};
use tauri::{Emitter, State};

/// Get Sysmon installation/service status.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_status(state: State<'_, AppState>) -> Result<SysmonStatus, IrError> {
    Ok(state.sysmon_config.get_status_info())
}

/// Check if the Sysmon event channel is available.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_is_channel_available(state: State<'_, AppState>) -> Result<bool, IrError> {
    Ok(state.sysmon_reader.is_channel_available())
}

/// Install Sysmon with the given config.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_install(state: State<'_, AppState>, accept_eula: bool) -> Result<(bool, String), IrError> {
    let config = state.sysmon_config.clone();
    tokio::task::spawn_blocking(move || config.install(accept_eula))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// Uninstall Sysmon.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_uninstall(state: State<'_, AppState>) -> Result<(bool, String), IrError> {
    let config = state.sysmon_config.clone();
    tokio::task::spawn_blocking(move || config.uninstall())
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// Update Sysmon configuration.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_update_config(state: State<'_, AppState>) -> Result<(bool, String), IrError> {
    let config = state.sysmon_config.clone();
    tokio::task::spawn_blocking(move || config.update_config())
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// Get existing (historical) events from the Sysmon channel.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_get_existing_events(
    state: State<'_, AppState>,
    limit: u32,
    enabled_event_ids: Vec<u32>,
) -> Result<Vec<SysmonEvent>, IrError> {
    let reader = state.sysmon_reader.clone();
    tokio::task::spawn_blocking(move || reader.get_existing_events(limit, &enabled_event_ids))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
}

/// Get the default event configurations.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_default_event_configs() -> Result<Vec<EventConfigEntry>, IrError> {
    Ok(irtool_sysmon::default_event_configs())
}

/// Generate Sysmon XML config from enabled events list.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_generate_config(enabled_events: Vec<String>) -> Result<String, IrError> {
    Ok(irtool_sysmon::SysmonConfigManager::generate_config(&enabled_events))
}

/// Start real-time Sysmon event subscription.
/// Events are pushed to the frontend via `evt_sysmon_event` Tauri event.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_start_subscription(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    enabled_event_ids: Vec<u32>,
    poll_interval_ms: Option<u64>,
) -> Result<(), IrError> {
    let reader = state.sysmon_reader.clone();
    if reader.is_polling() {
        return Ok(()); // Already polling
    }

    // Init last_record_id to skip existing events
    let init_reader = reader.clone();
    let init_event_ids = enabled_event_ids.clone();
    tokio::task::spawn_blocking(move || {
        init_reader.init_last_record_id(&init_event_ids)
    })
    .await
    .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SysmonEvent>();

    let interval = poll_interval_ms.unwrap_or(500);
    reader.start_polling(enabled_event_ids, interval, tx);

    // Forward events to frontend via Tauri emit
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = app.emit(EVT_SYSMON_EVENT, &event);
        }
    });

    Ok(())
}

/// Stop real-time Sysmon event subscription.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_stop_subscription(state: State<'_, AppState>) -> Result<(), IrError> {
    state.sysmon_reader.stop_polling();
    Ok(())
}

/// Check if Sysmon subscription is currently active.
#[tauri::command]
#[specta::specta]
pub async fn cmd_sysmon_is_subscribing(state: State<'_, AppState>) -> Result<bool, IrError> {
    Ok(state.sysmon_reader.is_polling())
}
