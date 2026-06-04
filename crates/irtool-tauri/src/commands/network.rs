use crate::events::{EVT_NETWORK_ERROR, EVT_NETWORK_SNAPSHOT};
use crate::state::AppState;
use irtool_core::IrError;
use irtool_net_monitor::{kill_process, NetCollector, NetConn, RetentionPolicy};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::Duration;
use tauri::{Emitter, State};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkSnapshotPayload {
    pub items: Vec<NetConn>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicyDto {
    None,
    Seconds(u64),
    Forever,
}

impl From<RetentionPolicyDto> for RetentionPolicy {
    fn from(value: RetentionPolicyDto) -> Self {
        match value {
            RetentionPolicyDto::None => RetentionPolicy::None,
            RetentionPolicyDto::Seconds(s) => RetentionPolicy::Seconds(s),
            RetentionPolicyDto::Forever => RetentionPolicy::Forever,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkPollingControl {
    pub interval_ms: Option<u64>,
    pub paused: Option<bool>,
    pub retention: Option<RetentionPolicyDto>,
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_snapshot(
    state: State<'_, AppState>,
) -> Result<NetworkSnapshotPayload, IrError> {
    let collector = state.net_collector.clone();
    let history = state.net_history.clone();
    let retention = state.net_polling.lock().retention;
    let snap = tokio::task::spawn_blocking(move || collector.snapshot())
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;
    let merged = history.merge(snap, retention);
    Ok(NetworkSnapshotPayload {
        items: merged,
        timestamp: irtool_net_monitor::types::now_epoch_secs(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_kill_process(pid: u32) -> Result<(), IrError> {
    info!("kill process requested: pid={}", pid);
    let result = tokio::task::spawn_blocking(move || kill_process(pid))
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?;
    match &result {
        Ok(()) => info!("kill process result: success, pid={}", pid),
        Err(e) => error!("kill process result: failed, pid={}, error={}", pid, e),
    }
    result
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_set_polling(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    control: NetworkPollingControl,
) -> Result<(), IrError> {
    let mut polling = state.net_polling.lock();
    if let Some(interval) = control.interval_ms {
        polling.interval_ms = interval.clamp(500, 60_000);
    }
    if let Some(paused) = control.paused {
        polling.paused = paused;
    }
    if let Some(retention) = control.retention {
        polling.retention = retention.into();
    }
    let new_interval = polling.interval_ms;
    let paused = polling.paused;

    info!(
        "network polling config changed: interval_ms={}, paused={}, retention={:?}",
        new_interval, paused, polling.retention
    );

    if let Some(token) = polling.cancel.take() {
        token.cancel();
    }
    if !paused {
        let token = CancellationToken::new();
        polling.cancel = Some(token.clone());
        drop(polling);

        let collector = state.net_collector.clone();
        let history = state.net_history.clone();
        let retention_now = state.net_polling.lock().retention;
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            run_polling_loop(collector, history, retention_now, app_clone, new_interval, token)
                .await;
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_clear_history(state: State<'_, AppState>) -> Result<(), IrError> {
    info!("network history cleared");
    state.net_history.clear_history();
    Ok(())
}

async fn run_polling_loop(
    collector: std::sync::Arc<irtool_net_monitor::WindowsNetCollector>,
    history: std::sync::Arc<irtool_net_monitor::HistoryStore>,
    retention: RetentionPolicy,
    app: tauri::AppHandle,
    interval_ms: u64,
    cancel: CancellationToken,
) {
    info!(interval_ms, "network polling loop starting");
    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("network polling loop cancelled");
                break;
            }
            _ = ticker.tick() => {
                let collector_clone = collector.clone();
                let snap = tokio::task::spawn_blocking(move || collector_clone.snapshot()).await;
                match snap {
                    Ok(Ok(items)) => {
                        let merged = history.merge(items, retention);
                        let payload = NetworkSnapshotPayload {
                            items: merged,
                            timestamp: irtool_net_monitor::types::now_epoch_secs(),
                        };
                        if let Err(e) = app.emit(EVT_NETWORK_SNAPSHOT, &payload) {
                            error!("emit snapshot failed: {}", e);
                        }
                    }
                    Ok(Err(e)) => {
                        let _ = app.emit(EVT_NETWORK_ERROR, e.to_string());
                    }
                    Err(e) => {
                        let _ = app.emit(EVT_NETWORK_ERROR, format!("join error: {}", e));
                    }
                }
            }
        }
    }
}

pub fn start_default_polling(state: &AppState, app: &tauri::AppHandle) {
    let token = CancellationToken::new();
    state.net_polling.lock().cancel = Some(token.clone());
    let collector = state.net_collector.clone();
    let history = state.net_history.clone();
    let retention = state.net_polling.lock().retention;
    let interval = state.net_polling.lock().interval_ms;
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        run_polling_loop(collector, history, retention, app_clone, interval, token).await;
    });
}
