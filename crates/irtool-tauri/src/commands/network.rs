use crate::events::{EVT_MONITOR_ALERT, EVT_NETWORK_ERROR, EVT_NETWORK_SNAPSHOT};
use crate::state::AppState;
use irtool_core::IrError;
use irtool_monitor::{EventSource, MonitorEvent};
use irtool_net_monitor::{kill_process, NetCollector, NetConn, RetentionPolicy};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
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
pub async fn cmd_network_snapshot(state: State<'_, AppState>) -> Result<NetworkSnapshotPayload, IrError> {
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
        let shared_retention = Arc::new(Mutex::new(polling.retention));
        let monitor_engine = state.monitor_engine.clone();
        drop(polling);

        let collector = state.net_collector.clone();
        let history = state.net_history.clone();
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            run_polling_loop(collector, history, shared_retention, app_clone, new_interval, token, monitor_engine).await;
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
    retention: Arc<Mutex<RetentionPolicy>>,
    app: tauri::AppHandle,
    interval_ms: u64,
    cancel: CancellationToken,
    monitor_engine: Arc<tokio::sync::Mutex<irtool_monitor::MonitorEngine>>,
) {
    info!(interval_ms, "network polling loop starting");
    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                info!("network polling loop cancelled");
                break;
            }
            _ = ticker.tick() => {
                let collector_clone = collector.clone();
                let snap = tokio::task::spawn_blocking(move || collector_clone.snapshot()).await;
                // 如果在 snapshot 期间已取消，跳过处理直接退出
                if cancel.is_cancelled() {
                    info!("network polling loop cancelled during snapshot");
                    break;
                }
                match snap {
                    Ok(Ok(items)) => {
                        let ret = *retention.lock();
                        let now_secs = irtool_net_monitor::types::now_epoch_secs();
                        let merged = history.merge(items, ret);
                        // 将新增连接（first_seen == now）转发到告警引擎
                        for conn in &merged {
                            if conn.first_seen == now_secs {
                                let monitor_event = netconn_to_monitor_event(conn);
                                let alerts = monitor_engine.lock().await.process_monitor_event(&monitor_event).await;
                                for alert in &alerts {
                                    let _ = app.emit(EVT_MONITOR_ALERT, alert);
                                }
                            }
                        }
                        let payload = NetworkSnapshotPayload {
                            items: merged,
                            timestamp: now_secs,
                        };
                        // 只在非后台模式时 emit network 事件到前端
                        let is_background = monitor_engine.lock().await.is_background_mode();
                        if !is_background {
                            if let Err(e) = app.emit(EVT_NETWORK_SNAPSHOT, &payload) {
                                error!("emit snapshot failed: {}", e);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        // 错误事件始终 emit，这样如果用户从托盘恢复窗口能看到问题
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
    let (retention, interval, paused) = {
        let mut polling = state.net_polling.lock();
        polling.cancel = Some(token.clone());
        (polling.retention, polling.interval_ms, polling.paused)
    };
    if paused {
        token.cancel();
        return;
    }
    let collector = state.net_collector.clone();
    let history = state.net_history.clone();
    let shared_retention = Arc::new(Mutex::new(retention));
    let monitor_engine = state.monitor_engine.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        run_polling_loop(collector, history, shared_retention, app_clone, interval, token, monitor_engine).await;
    });
}

/// NetConn → MonitorEvent 转换
fn netconn_to_monitor_event(conn: &NetConn) -> MonitorEvent {
    MonitorEvent {
        id: 0,
        timestamp: (conn.first_seen as i64) * 1000,
        source: EventSource::NetMonitor,
        event_type: "network_monitor".to_string(),
        process_name: format!("{} ({})", conn.process_name.clone().unwrap_or_default(), conn.pid),
        key_field: format!("{}:{}", conn.remote.addr, conn.remote.port),
        raw_json: serde_json::to_string(&conn).unwrap_or_default(),
    }
}
