use std::sync::Arc;
use std::time::Duration;

use irtool_core::IrError;
use irtool_monitor::{EventSource, MonitorEvent};
use irtool_net_monitor::{
    kill_process, CmdlineEnricher, CmdlineResult, CmdlineStatus, HistoryStore, NetCollector, NetConn, RetentionPolicy,
    WindowsNetCollector,
};
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::context::AppContext;
use crate::dto::browser_forensics::BrowserMaliciousConnectionPayload;
use crate::dto::network::{NetworkEnrichmentPayload, NetworkPollingControl, NetworkSnapshotPayload};
use crate::event_bus::AppEvent;
use irtool_browser_forensics::browser_kind_from_process_name;

pub struct NetworkService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> NetworkService<'a> {
    pub async fn snapshot(&self) -> Result<NetworkSnapshotPayload, IrError> {
        let collector = self.ctx.net_collector.clone();
        let history = self.ctx.net_history.clone();
        let enricher = self.ctx.net_enricher.clone();
        let retention = self.ctx.net_polling.lock().retention;
        let collector_for_enrich = collector.clone();
        let snap = tokio::task::spawn_blocking(move || collector.snapshot())
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;
        let mut merged = history.merge(snap, retention);
        collector_for_enrich.enrich_cmdlines(&mut merged, &enricher);
        Ok(NetworkSnapshotPayload {
            items: merged,
            timestamp: irtool_net_monitor::types::now_epoch_secs(),
        })
    }

    pub async fn kill_process(&self, pid: u32) -> Result<(), IrError> {
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

    pub async fn set_polling(&self, control: NetworkPollingControl) -> Result<(), IrError> {
        let mut polling = self.ctx.net_polling.lock();
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
            let monitor_engine = self.ctx.monitor_engine.clone();
            let event_bus = self.ctx.event_bus.clone();
            let browser_ip_index = self.ctx.browser_ip_index.clone();
            drop(polling);

            let collector = self.ctx.net_collector.clone();
            let history = self.ctx.net_history.clone();
            let enricher = self.ctx.net_enricher.clone();
            tokio::spawn(async move {
                run_polling_loop(
                    collector,
                    history,
                    enricher,
                    shared_retention,
                    event_bus,
                    new_interval,
                    token,
                    monitor_engine,
                    browser_ip_index,
                )
                .await;
            });
        }
        Ok(())
    }

    pub async fn clear_history(&self) -> Result<(), IrError> {
        info!("network history cleared");
        self.ctx.net_history.clear_history();
        Ok(())
    }

    pub async fn refresh_cmdline(&self, pid: u32) -> Result<(), IrError> {
        info!("manual cmdline refresh requested: pid={}", pid);

        let enricher = self.ctx.net_enricher.clone();
        let event_bus = self.ctx.event_bus.clone();

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

            let emit = |status: CmdlineStatus, cmdline: Option<String>| {
                enricher.update(
                    pid,
                    CmdlineResult {
                        cmdline: cmdline.clone(),
                        status,
                        cached_at: now,
                    },
                );
                event_bus.publish(AppEvent::NetworkEnrichment(NetworkEnrichmentPayload {
                    pid,
                    cmdline_status: status,
                    process_cmdline: cmdline,
                }));
            };

            match result {
                Some(query_result) => {
                    if query_result.query_failed {
                        info!(
                            "manual cmdline refresh: pid={} WMI query failed (connection error)",
                            pid
                        );
                        emit(CmdlineStatus::Failed, None);
                    } else if query_result.failed_pids.contains(&pid) {
                        info!("manual cmdline refresh: pid={} WMI query timed out", pid);
                        emit(CmdlineStatus::Failed, None);
                    } else if query_result.exited_pids.contains(&pid) {
                        info!("manual cmdline refresh: pid={} process exited", pid);
                        emit(CmdlineStatus::Exited, None);
                    } else if let Some(cmdline) = query_result.cmdlines.get(&pid) {
                        info!("manual cmdline refresh: pid={} cmdline found", pid);
                        emit(CmdlineStatus::Ready, Some(cmdline.clone()));
                    } else if query_result.no_cmdline_pids.contains(&pid) {
                        info!("manual cmdline refresh: pid={} found but no cmdline (protected)", pid);
                        emit(CmdlineStatus::Denied, None);
                    } else {
                        info!("manual cmdline refresh: pid={} found but no cmdline", pid);
                        emit(CmdlineStatus::Ready, None);
                    }
                }
                None => {
                    info!("manual cmdline refresh: pid={} WMI query returned None", pid);
                    emit(CmdlineStatus::Failed, None);
                }
            }
        });

        Ok(())
    }

    /// Start the default polling loop (called from frontend setup).
    pub fn start_default_polling(&self, handle: tokio::runtime::Handle) {
        let token = CancellationToken::new();
        let (retention, interval, paused) = {
            let mut polling = self.ctx.net_polling.lock();
            polling.cancel = Some(token.clone());
            (polling.retention, polling.interval_ms, polling.paused)
        };
        if paused {
            token.cancel();
            return;
        }
        let collector = self.ctx.net_collector.clone();
        let history = self.ctx.net_history.clone();
        let enricher = self.ctx.net_enricher.clone();
        let shared_retention = Arc::new(Mutex::new(retention));
        let monitor_engine = self.ctx.monitor_engine.clone();
        let event_bus = self.ctx.event_bus.clone();
        let browser_ip_index = self.ctx.browser_ip_index.clone();
        handle.spawn(async move {
            run_polling_loop(
                collector,
                history,
                enricher,
                shared_retention,
                event_bus,
                interval,
                token,
                monitor_engine,
                browser_ip_index,
            )
            .await;
        });
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_polling_loop(
    collector: Arc<WindowsNetCollector>,
    history: Arc<HistoryStore>,
    enricher: Arc<CmdlineEnricher>,
    retention: Arc<Mutex<RetentionPolicy>>,
    event_bus: crate::EventBus,
    interval_ms: u64,
    cancel: CancellationToken,
    monitor_engine: Arc<tokio::sync::Mutex<irtool_monitor::MonitorEngine>>,
    browser_ip_index: crate::services::browser_ip_index::SharedBrowserIpIndex,
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
                    collector_clone.enrich_cmdlines(&mut conns, &enricher_clone);
                    Ok::<Vec<NetConn>, IrError>(conns)
                }).await;
                if cancel.is_cancelled() {
                    info!("network polling loop cancelled during snapshot");
                    break;
                }
                match snap {
                    Ok(Ok(items)) => {
                        let ret = *retention.lock();
                        let now_secs = irtool_net_monitor::types::now_epoch_secs();
                        let merged = history.merge(items, ret);

                        // Background cmdline enrichment
                        let pending = enricher.drain_pending(100);
                        if !pending.is_empty() {
                            let enricher_clone = enricher.clone();
                            let event_bus_clone = event_bus.clone();
                            let pids = pending.clone();
                            tokio::task::spawn_blocking(move || {
                                let result = irtool_net_monitor::process_info::targeted_query_cmdlines(&pids);
                                let now = std::time::Instant::now();
                                let mut events: Vec<AppEvent> = Vec::new();

                                let mut emit = |pid: u32, status: CmdlineStatus, cmdline: Option<String>| {
                                    enricher_clone.update(pid, CmdlineResult {
                                        cmdline: cmdline.clone(),
                                        status,
                                        cached_at: now,
                                    });
                                    events.push(AppEvent::NetworkEnrichment(NetworkEnrichmentPayload {
                                        pid,
                                        cmdline_status: status,
                                        process_cmdline: cmdline,
                                    }));
                                };

                                match result {
                                    Some(query_result) => {
                                        for &pid in &query_result.failed_pids {
                                            emit(pid, CmdlineStatus::Failed, None);
                                        }
                                        for &pid in &query_result.exited_pids {
                                            emit(pid, CmdlineStatus::Exited, None);
                                        }
                                        for (&pid, cmdline) in &query_result.cmdlines {
                                            emit(pid, CmdlineStatus::Ready, Some(cmdline.clone()));
                                        }
                                        for &pid in &query_result.no_cmdline_pids {
                                            emit(pid, CmdlineStatus::Denied, None);
                                        }
                                    }
                                    None => {
                                        for &pid in &pids {
                                            emit(pid, CmdlineStatus::Failed, None);
                                        }
                                    }
                                }

                                for event in events {
                                    event_bus_clone.publish(event);
                                }
                            });
                        }

                        // Forward new connections to alert engine
                        for conn in &merged {
                            if conn.first_seen == now_secs {
                                let monitor_event = netconn_to_monitor_event(conn);
                                let alerts = monitor_engine.lock().await.process_monitor_event(&monitor_event).await;
                                for alert in &alerts {
                                    event_bus.publish(AppEvent::MonitorAlert(alert.clone()));

                                    // 检查是否为浏览器进程的恶意连接（覆盖 Chrome/Edge）
                                    let proc_name = conn.process_name.as_deref().unwrap_or("");
                                    if browser_kind_from_process_name(proc_name).is_some() {
                                        // T3: 写入 IP→浏览器进程索引，供 pcap 域名事件反查
                                        let now_ms = (now_secs as i64) * 1000;
                                        browser_ip_index
                                            .lock()
                                            .await
                                            .insert(&conn.remote.addr, conn.pid, proc_name, now_ms);

                                        let payload = BrowserMaliciousConnectionPayload {
                                            // net-monitor 只有 IP（NetConn.remote.addr），无域名信息；
                                            // domain 留空，前端按 domain 非空判断是否为域名告警。
                                            // 域名→IP 关联由 T3（pcap/sysmon 域名反查）或前端手动输入处理。
                                            domain: String::new(),
                                            ip: conn.remote.addr.clone(),
                                            process_name: conn.process_name.clone().unwrap_or_default(),
                                            pid: conn.pid,
                                            cmdline: conn.process_cmdline.clone(),
                                            alert_id: alert.id.to_string(),
                                        };
                                        event_bus.publish(AppEvent::BrowserMaliciousConnection(payload));
                                    }
                                }
                            }
                        }
                        let payload = NetworkSnapshotPayload {
                            items: merged,
                            timestamp: now_secs,
                        };
                        // Only emit network snapshot when not in background mode
                        let is_background = monitor_engine.lock().await.is_background_mode();
                        if !is_background {
                            event_bus.publish(AppEvent::NetworkSnapshot(payload));
                        }
                    }
                    Ok(Err(e)) => {
                        event_bus.publish(AppEvent::NetworkError(e.to_string()));
                    }
                    Err(e) => {
                        event_bus.publish(AppEvent::NetworkError(format!("join error: {}", e)));
                    }
                }
            }
        }
    }
}

/// NetConn → MonitorEvent conversion
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
