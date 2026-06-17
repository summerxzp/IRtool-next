use irtool_core::IrError;
use irtool_monitor::{Alert, EventPage, EventQuery, MonitorConfig, MonitorEvent, RuntimeTelemetry};
use irtool_pcap::{AdapterInfo, PcapConfig, PcapCountersSnapshot};
use irtool_service::context::AppContext;
use irtool_service::services::monitor::MonitorService;
use irtool_service::services::pcap::PcapService;
use tauri::State;

// --- Monitor commands ---

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_config(ctx: State<'_, AppContext>) -> Result<MonitorConfig, IrError> {
    MonitorService { ctx: &ctx }.get_config().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_update_config(ctx: State<'_, AppContext>, config: MonitorConfig) -> Result<(), IrError> {
    MonitorService { ctx: &ctx }.update_config(config).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_enter_background(ctx: State<'_, AppContext>) -> Result<(), IrError> {
    MonitorService { ctx: &ctx }.enter_background().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_exit_background(ctx: State<'_, AppContext>) -> Result<(), IrError> {
    MonitorService { ctx: &ctx }.exit_background().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_alerts(ctx: State<'_, AppContext>, limit: u32) -> Result<Vec<Alert>, IrError> {
    MonitorService { ctx: &ctx }.get_alerts(limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_is_background(ctx: State<'_, AppContext>) -> Result<bool, IrError> {
    MonitorService { ctx: &ctx }.is_background().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_clear_alerts(ctx: State<'_, AppContext>) -> Result<u64, IrError> {
    MonitorService { ctx: &ctx }.clear_alerts().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_events(
    ctx: State<'_, AppContext>,
    limit: u32,
) -> Result<Vec<MonitorEvent>, IrError> {
    MonitorService { ctx: &ctx }.get_events(limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_event_count(ctx: State<'_, AppContext>) -> Result<u64, IrError> {
    MonitorService { ctx: &ctx }.get_event_count().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_clear_events(ctx: State<'_, AppContext>) -> Result<u64, IrError> {
    MonitorService { ctx: &ctx }.clear_events().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_event_type_counts(ctx: State<'_, AppContext>) -> Result<Vec<(String, u64)>, IrError> {
    MonitorService { ctx: &ctx }.event_type_counts().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_db_size(ctx: State<'_, AppContext>) -> Result<u64, IrError> {
    MonitorService { ctx: &ctx }.get_db_size().await
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn cmd_monitor_search_events(
    ctx: State<'_, AppContext>,
    source: Option<String>,
    event_type: Option<String>,
    process_name: Option<String>,
    key_field: Option<String>,
    search_text: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<MonitorEvent>, IrError> {
    MonitorService { ctx: &ctx }
        .search_events(source, event_type, process_name, key_field, search_text, limit, offset)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_search_event_page(
    ctx: State<'_, AppContext>,
    query: EventQuery,
) -> Result<EventPage, IrError> {
    MonitorService { ctx: &ctx }.search_event_page(query).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_telemetry(ctx: State<'_, AppContext>) -> Result<RuntimeTelemetry, IrError> {
    MonitorService { ctx: &ctx }.get_telemetry().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_test_feishu(webhook_url: String) -> Result<(), IrError> {
    MonitorService::test_feishu(webhook_url).await
}

// --- Pcap commands ---

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_is_available() -> Result<bool, IrError> {
    Ok(PcapService::is_available())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_start(
    ctx: State<'_, AppContext>,
    config: PcapConfig,
) -> Result<(), IrError> {
    PcapService { ctx: &ctx }.start(config).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_stop(ctx: State<'_, AppContext>) -> Result<(), IrError> {
    PcapService { ctx: &ctx }.stop().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_is_running(ctx: State<'_, AppContext>) -> Result<bool, IrError> {
    PcapService { ctx: &ctx }.is_running().await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_list_adapters() -> Result<Vec<AdapterInfo>, IrError> {
    PcapService::list_adapters()
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_get_counters(ctx: State<'_, AppContext>) -> Result<PcapCountersSnapshot, IrError> {
    PcapService { ctx: &ctx }.get_counters().await
}

// --- Alert Popup Window (Tauri-specific, stays here) ---

use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{Emitter, Manager};

static POPUP_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(serde::Deserialize, specta::Type)]
pub struct AlertPopupParams {
    pub rule_name: String,
    pub key_field: String,
    pub event_type: String,
    pub process_name: String,
    pub protocol: String,
    pub timestamp: i64,
    pub source_addr: Option<String>,
    pub remote_addr: Option<String>,
    pub process_chain: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_show_alert_popup(
    app: tauri::AppHandle,
    ctx: State<'_, AppContext>,
    params: AlertPopupParams,
) -> Result<(), String> {
    let popup_duration = ctx
        .monitor_engine
        .lock()
        .await
        .get_config()
        .notify_config
        .popup_duration_secs;
    show_alert_popup_window(
        &app,
        &params.rule_name,
        &params.key_field,
        &params.event_type,
        &params.process_name,
        &params.protocol,
        params.timestamp,
        params.source_addr.as_deref().unwrap_or(""),
        params.remote_addr.as_deref().unwrap_or(""),
        params.process_chain.as_deref().unwrap_or(""),
        popup_duration,
    )
    .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn show_alert_popup_window(
    app: &tauri::AppHandle,
    rule_name: &str,
    key_field: &str,
    event_type: &str,
    process_name: &str,
    protocol: &str,
    timestamp: i64,
    source_addr: &str,
    remote_addr: &str,
    process_chain: &str,
    popup_duration_secs: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let label = format!("alert-popup-{millis}");

    let width = 380.0_f64;
    let height = 160.0_f64;
    let margin = 20.0_f64;
    let gap = 8.0_f64;

    let popup_index = POPUP_COUNT.fetch_add(1, Ordering::SeqCst);

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("alert-popup.html".into()))
        .title("IRtool Alert")
        .inner_size(width, height)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;

    if let Some(monitor) = window.primary_monitor().ok().flatten() {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let wa_x = work_area.position.x as f64 / scale;
        let wa_y = work_area.position.y as f64 / scale;
        let wa_w = work_area.size.width as f64 / scale;
        let wa_h = work_area.size.height as f64 / scale;
        let x = wa_x + wa_w - width - margin;
        let y = wa_y + wa_h - height - margin - (popup_index as f64 * (height + gap));
        window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)))?;
    }

    let label_clone = label.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            POPUP_COUNT.fetch_sub(1, Ordering::SeqCst);
            tracing::debug!("alert popup window destroyed: {label_clone}");
        }
    });

    let alert_key = format!("{event_type}-{timestamp}");
    let app_emit = app.clone();
    let label_emit = label.clone();
    let rule_name = rule_name.to_string();
    let key_field = key_field.to_string();
    let event_type = event_type.to_string();
    let process_name = process_name.to_string();
    let protocol = protocol.to_string();
    let source_addr = source_addr.to_string();
    let remote_addr = remote_addr.to_string();
    let process_chain = process_chain.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = app_emit.emit_to(
            &label_emit,
            "evt_show_alert_popup",
            serde_json::json!({
                "alert_key": alert_key,
                "rule_name": rule_name,
                "key_field": key_field,
                "event_type": event_type,
                "process_name": process_name,
                "protocol": protocol,
                "timestamp": timestamp,
                "source_addr": source_addr,
                "remote_addr": remote_addr,
                "process_chain": process_chain,
                "duration_secs": popup_duration_secs,
            }),
        );
    });

    if popup_duration_secs > 0 {
        let app_clone = app.clone();
        let label_clone = label.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(popup_duration_secs as u64));
            let _ = app_clone.emit_to(&label_clone, "evt_close_alert_popup", ());
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Some(win) = app_clone.get_webview_window(&label_clone) {
                let _ = win.close();
            }
        });
    }

    Ok(())
}
