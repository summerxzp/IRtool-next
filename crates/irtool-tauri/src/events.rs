//! Event bridge: maps [`AppEvent`] variants to Tauri event names.

use irtool_service::AppEvent;
use tauri::{Emitter, Manager};

/// Map from `AppEvent` variant to the Tauri event name used by the frontend.
const EVT_NETWORK_SNAPSHOT: &str = "evt_network_snapshot";
const EVT_NETWORK_ERROR: &str = "evt_network_error";
const EVT_NETWORK_ENRICHMENT: &str = "evt_network_enrichment";
const EVT_AUTORUNS_PROGRESS: &str = "evt_autoruns_progress";
const EVT_AUTORUNS_SIGNATURE_PROGRESS: &str = "evt_autoruns_signature_progress";
const EVT_AUTORUNS_HASH_PROGRESS: &str = "evt_autoruns_hash_progress";
const EVT_TASK_CANCELLED: &str = "evt_task_cancelled";
const EVT_TASK_FAILED: &str = "evt_task_failed";
const EVT_SYSMON_EVENT: &str = "evt_sysmon_event";
const EVT_MONITOR_ALERT: &str = "evt_monitor_alert";
const EVT_BROWSER_MALICIOUS_CONNECTION: &str = "evt_browser_malicious_connection";
const EVT_EXTENSION_ATTRIBUTION: &str = "evt_extension_attribution";
const EVT_PCAP_EVENT: &str = "evt_pcap_event";
const EVT_CLOSE_REQUESTED: &str = "evt_close_requested";
const EVT_TOOLS_DOWNLOAD_PROGRESS: &str = "evt_tools_download_progress";
const EVT_TOOLS_DOWNLOAD_ERROR: &str = "evt_tools_download_error";
const EVT_TOOLS_DOWNLOAD_COMPLETE: &str = "evt_tools_download_complete";

/// Spawn a task that reads [`AppEvent`]s from the bus and bridges them to Tauri events.
pub fn start_event_bridge(ctx: &irtool_service::AppContext, app: tauri::AppHandle) {
    let mut rx = ctx.event_bus.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => dispatch_event(&app, event),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("event bridge lagged, skipped {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("event bus closed, bridge exiting");
                    break;
                }
            }
        }
    });
}

fn dispatch_event(app: &tauri::AppHandle, event: AppEvent) {
    match event {
        AppEvent::NetworkSnapshot(p) => {
            let _ = app.emit(EVT_NETWORK_SNAPSHOT, &p);
        }
        AppEvent::NetworkError(e) => {
            let _ = app.emit(EVT_NETWORK_ERROR, &e);
        }
        AppEvent::NetworkEnrichment(p) => {
            let _ = app.emit(EVT_NETWORK_ENRICHMENT, &p);
        }
        AppEvent::AutorunsProgress(p) => {
            let _ = app.emit(EVT_AUTORUNS_PROGRESS, &p);
        }
        AppEvent::AutorunsSignatureProgress(p) => {
            let _ = app.emit(EVT_AUTORUNS_SIGNATURE_PROGRESS, &p);
        }
        AppEvent::AutorunsHashProgress(p) => {
            let _ = app.emit(EVT_AUTORUNS_HASH_PROGRESS, &p);
        }
        AppEvent::AutorunsScanComplete { count: _ } => {
            // Re-apply window icon after autorunsc64.exe finishes (icon cache flush side-effect).
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                match app_clone.get_webview_window("main") {
                    Some(window) => match app_clone.default_window_icon() {
                        Some(icon) => {
                            tracing::info!("autoruns re-applying window icon ({}x{})", icon.width(), icon.height());
                            if let Err(e) = window.set_icon(icon.clone()) {
                                tracing::warn!("autoruns set_icon failed: {}", e);
                            }
                        }
                        None => tracing::warn!("autoruns default_window_icon() returned None"),
                    },
                    None => tracing::warn!("autoruns get_webview_window(\"main\") returned None"),
                }
            });
        }
        AppEvent::AutorunsScanCancelled(task_id) => {
            let _ = app.emit(EVT_TASK_CANCELLED, task_id);
        }
        AppEvent::AutorunsScanFailed { task_id, error } => {
            let _ = app.emit(EVT_TASK_FAILED, serde_json::json!({"task_id": task_id, "error": error}));
        }
        AppEvent::SysmonEvent(e) => {
            let _ = app.emit(EVT_SYSMON_EVENT, &*e);
        }
        AppEvent::MonitorAlert(a) => {
            let _ = app.emit(EVT_MONITOR_ALERT, &a);
        }
        AppEvent::BrowserMaliciousConnection(p) => {
            let _ = app.emit(EVT_BROWSER_MALICIOUS_CONNECTION, &p);
        }
        AppEvent::ExtensionAttribution(p) => {
            let _ = app.emit(EVT_EXTENSION_ATTRIBUTION, &p);
        }
        AppEvent::PcapEvent(e) => {
            let _ = app.emit(EVT_PCAP_EVENT, &e);
        }
        AppEvent::CloseRequested => {
            let _ = app.emit(EVT_CLOSE_REQUESTED, ());
        }
        AppEvent::ToolsDownloadProgress {
            tool_id,
            downloaded,
            total,
        } => {
            let _ = app.emit(
                EVT_TOOLS_DOWNLOAD_PROGRESS,
                serde_json::json!({"tool_id": tool_id, "downloaded": downloaded, "total": total}),
            );
        }
        AppEvent::ToolsDownloadError { tool_id, error } => {
            let _ = app.emit(
                EVT_TOOLS_DOWNLOAD_ERROR,
                serde_json::json!({"tool_id": tool_id, "error": error}),
            );
        }
        AppEvent::ToolsDownloadComplete { errors } => {
            let _ = app.emit(EVT_TOOLS_DOWNLOAD_COMPLETE, serde_json::json!({"errors": errors}));
        }
    }
}
