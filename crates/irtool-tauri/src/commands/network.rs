use crate::events::{EVT_MONITOR_ALERT, EVT_NETWORK_ENRICHMENT, EVT_NETWORK_ERROR, EVT_NETWORK_SNAPSHOT};
use crate::state::AppState;
use irtool_core::IrError;
use irtool_monitor::{EventSource, MonitorEvent};
use irtool_net_monitor::{
    kill_process, CmdlineEnricher, CmdlineResult, CmdlineStatus, NetCollector, NetConn, RetentionPolicy,
};
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkEnrichmentPayload {
    pub pid: u32,
    pub cmdline_status: CmdlineStatus,
    pub process_cmdline: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_snapshot(state: State<'_, AppState>) -> Result<NetworkSnapshotPayload, IrError> {
    let collector = state.net_collector.clone();
    let history = state.net_history.clone();
    let enricher = state.net_enricher.clone();
    let retention = state.net_polling.lock().retention;
    let collector_for_enrich = collector.clone();
    let snap = tokio::task::spawn_blocking(move || collector.snapshot())
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;
    let mut merged = history.merge(snap, retention);
    // Apply cached cmdline results
    collector_for_enrich.enrich_cmdlines(&mut merged, &enricher);
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
        let enricher = state.net_enricher.clone();
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            run_polling_loop(
                collector,
                history,
                enricher,
                shared_retention,
                app_clone,
                new_interval,
                token,
                monitor_engine,
            )
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

#[tauri::command]
#[specta::specta]
pub async fn cmd_network_refresh_cmdline(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    pid: u32,
) -> Result<(), IrError> {
    info!("manual cmdline refresh requested: pid={}", pid);

    // Perform the query immediately (do NOT clear cache — that would cause the
    // next polling cycle to reset the status to Pending, creating a flicker).
    let enricher = state.net_enricher.clone();
    let app_clone = app.clone();

    tokio::task::spawn_blocking(move || {
        info!("manual cmdline refresh: starting WMI query for pid={}", pid);
        let result = irtool_net_monitor::process_info::targeted_query_cmdlines(&[pid]);
        let now = std::time::Instant::now();
        info!(
            "manual cmdline refresh: WMI query completed for pid={}, result={:?}",
            pid,
            result.as_ref().map(|r| (
                &r.cmdlines,
                &r.exited_pids,
                &r.failed_pids,
                &r.no_cmdline_pids,
                &r.query_failed
            ))
        );

        match result {
            Some(query_result) => {
                if query_result.query_failed {
                    // WMI connection failed
                    info!(
                        "manual cmdline refresh: pid={} WMI query failed (connection error)",
                        pid
                    );
                    enricher.update(
                        pid,
                        CmdlineResult {
                            cmdline: None,
                            status: CmdlineStatus::Failed,
                            cached_at: now,
                        },
                    );
                    let payload = NetworkEnrichmentPayload {
                        pid,
                        cmdline_status: CmdlineStatus::Failed,
                        process_cmdline: None,
                    };
                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                } else if query_result.failed_pids.contains(&pid) {
                    // This specific PID query timed out
                    info!("manual cmdline refresh: pid={} WMI query timed out", pid);
                    enricher.update(
                        pid,
                        CmdlineResult {
                            cmdline: None,
                            status: CmdlineStatus::Failed,
                            cached_at: now,
                        },
                    );
                    let payload = NetworkEnrichmentPayload {
                        pid,
                        cmdline_status: CmdlineStatus::Failed,
                        process_cmdline: None,
                    };
                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                } else if query_result.exited_pids.contains(&pid) {
                    // PID not in WMI results — process exited
                    info!("manual cmdline refresh: pid={} process exited", pid);
                    enricher.update(
                        pid,
                        CmdlineResult {
                            cmdline: None,
                            status: CmdlineStatus::Exited,
                            cached_at: now,
                        },
                    );
                    let payload = NetworkEnrichmentPayload {
                        pid,
                        cmdline_status: CmdlineStatus::Exited,
                        process_cmdline: None,
                    };
                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                } else if let Some(cmdline) = query_result.cmdlines.get(&pid) {
                    // Success
                    info!("manual cmdline refresh: pid={} cmdline found", pid);
                    enricher.update(
                        pid,
                        CmdlineResult {
                            cmdline: Some(cmdline.clone()),
                            status: CmdlineStatus::Ready,
                            cached_at: now,
                        },
                    );
                    let payload = NetworkEnrichmentPayload {
                        pid,
                        cmdline_status: CmdlineStatus::Ready,
                        process_cmdline: Some(cmdline.clone()),
                    };
                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                } else if query_result.no_cmdline_pids.contains(&pid) {
                    // PID found in WMI but CommandLine is None (protected process)
                    info!("manual cmdline refresh: pid={} found but no cmdline (protected)", pid);
                    enricher.update(
                        pid,
                        CmdlineResult {
                            cmdline: None,
                            status: CmdlineStatus::Denied,
                            cached_at: now,
                        },
                    );
                    let payload = NetworkEnrichmentPayload {
                        pid,
                        cmdline_status: CmdlineStatus::Denied,
                        process_cmdline: None,
                    };
                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                } else {
                    // PID found in WMI but no CommandLine (system process, etc.)
                    // This is still "Ready" status, just no cmdline
                    info!("manual cmdline refresh: pid={} found but no cmdline", pid);
                    enricher.update(
                        pid,
                        CmdlineResult {
                            cmdline: None,
                            status: CmdlineStatus::Ready,
                            cached_at: now,
                        },
                    );
                    let payload = NetworkEnrichmentPayload {
                        pid,
                        cmdline_status: CmdlineStatus::Ready,
                        process_cmdline: None,
                    };
                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                }
            }
            None => {
                // WMI query returned None (shouldn't happen with new impl, but handle it)
                info!("manual cmdline refresh: pid={} WMI query returned None", pid);
                enricher.update(
                    pid,
                    CmdlineResult {
                        cmdline: None,
                        status: CmdlineStatus::Failed,
                        cached_at: now,
                    },
                );
                let payload = NetworkEnrichmentPayload {
                    pid,
                    cmdline_status: CmdlineStatus::Failed,
                    process_cmdline: None,
                };
                let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
            }
        }
    });

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_polling_loop(
    collector: std::sync::Arc<irtool_net_monitor::WindowsNetCollector>,
    history: std::sync::Arc<irtool_net_monitor::HistoryStore>,
    enricher: std::sync::Arc<CmdlineEnricher>,
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
                let enricher_clone = enricher.clone();
                let snap = tokio::task::spawn_blocking(move || {
                    let mut conns = collector_clone.snapshot()?;
                    // Apply cached cmdline results and enqueue PIDs needing enrichment
                    collector_clone.enrich_cmdlines(&mut conns, &enricher_clone);
                    Ok::<Vec<NetConn>, IrError>(conns)
                }).await;
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

                        // Background cmdline enrichment: drain pending PIDs, query WMI, update cache
                        let pending = enricher.drain_pending(100);
                        if !pending.is_empty() {
                            let enricher_clone = enricher.clone();
                            let app_clone = app.clone();
                            let pids = pending.clone();
                            tokio::task::spawn_blocking(move || {
                                let result = irtool_net_monitor::process_info::targeted_query_cmdlines(&pids);
                                let now = std::time::Instant::now();
                                let mut enrichment_events: Vec<NetworkEnrichmentPayload> = Vec::new();

                                match result {
                                    Some(query_result) => {
                                        // Handle failed PIDs (timeout or WMI error)
                                        for &pid in &query_result.failed_pids {
                                            enricher_clone.update(pid, CmdlineResult {
                                                cmdline: None,
                                                status: CmdlineStatus::Failed,
                                                cached_at: now,
                                            });
                                            enrichment_events.push(NetworkEnrichmentPayload {
                                                pid,
                                                cmdline_status: CmdlineStatus::Failed,
                                                process_cmdline: None,
                                            });
                                        }
                                        // Handle exited PIDs (not found in WMI)
                                        for &pid in &query_result.exited_pids {
                                            enricher_clone.update(pid, CmdlineResult {
                                                cmdline: None,
                                                status: CmdlineStatus::Exited,
                                                cached_at: now,
                                            });
                                            enrichment_events.push(NetworkEnrichmentPayload {
                                                pid,
                                                cmdline_status: CmdlineStatus::Exited,
                                                process_cmdline: None,
                                            });
                                        }
                                        // Handle successful results
                                        for (&pid, cmdline) in &query_result.cmdlines {
                                            enricher_clone.update(pid, CmdlineResult {
                                                cmdline: Some(cmdline.clone()),
                                                status: CmdlineStatus::Ready,
                                                cached_at: now,
                                            });
                                            enrichment_events.push(NetworkEnrichmentPayload {
                                                pid,
                                                cmdline_status: CmdlineStatus::Ready,
                                                process_cmdline: Some(cmdline.clone()),
                                            });
                                        }
                                        // Handle PIDs found in WMI but CommandLine is None
                                        // (protected processes like AV) - mark as Denied
                                        for &pid in &query_result.no_cmdline_pids {
                                            enricher_clone.update(pid, CmdlineResult {
                                                cmdline: None,
                                                status: CmdlineStatus::Denied,
                                                cached_at: now,
                                            });
                                            enrichment_events.push(NetworkEnrichmentPayload {
                                                pid,
                                                cmdline_status: CmdlineStatus::Denied,
                                                process_cmdline: None,
                                            });
                                        }
                                    }
                                    None => {
                                        // WMI query returned None (unsupported platform or fatal error)
                                        for &pid in &pids {
                                            enricher_clone.update(pid, CmdlineResult {
                                                cmdline: None,
                                                status: CmdlineStatus::Failed,
                                                cached_at: now,
                                            });
                                            enrichment_events.push(NetworkEnrichmentPayload {
                                                pid,
                                                cmdline_status: CmdlineStatus::Failed,
                                                process_cmdline: None,
                                            });
                                        }
                                    }
                                }

                                // Emit enrichment events
                                for payload in enrichment_events {
                                    let _ = app_clone.emit(EVT_NETWORK_ENRICHMENT, &payload);
                                }
                            });
                        }

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
    let enricher = state.net_enricher.clone();
    let shared_retention = Arc::new(Mutex::new(retention));
    let monitor_engine = state.monitor_engine.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        run_polling_loop(
            collector,
            history,
            enricher,
            shared_retention,
            app_clone,
            interval,
            token,
            monitor_engine,
        )
        .await;
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
