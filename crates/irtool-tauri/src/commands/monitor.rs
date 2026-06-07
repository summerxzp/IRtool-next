use crate::state::AppState;
use irtool_core::IrError;
use irtool_monitor::{Alert, MonitorConfig};
use irtool_pcap::PcapConfig;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_config(state: State<'_, AppState>) -> Result<MonitorConfig, IrError> {
    Ok(state.monitor_engine.lock().await.get_config())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_update_config(
    state: State<'_, AppState>,
    config: MonitorConfig,
) -> Result<(), IrError> {
    state.monitor_engine.lock().await.update_config(config)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_enter_background(state: State<'_, AppState>) -> Result<(), IrError> {
    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    state.monitor_engine.lock().await.enter_background_mode(&app_dir)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_exit_background(state: State<'_, AppState>) -> Result<(), IrError> {
    state.monitor_engine.lock().await.exit_background_mode()
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_get_alerts(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<Alert>, IrError> {
    state.monitor_engine.lock().await.get_recent_alerts(limit)
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_is_background(state: State<'_, AppState>) -> Result<bool, IrError> {
    Ok(state.monitor_engine.lock().await.is_background_mode())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_clear_alerts(state: State<'_, AppState>) -> Result<u64, IrError> {
    state.monitor_engine.lock().await.clear_alerts()
}

// --- P6 新增 ---

#[tauri::command]
#[specta::specta]
pub async fn cmd_monitor_test_feishu(webhook_url: String) -> Result<(), IrError> {
    irtool_monitor::notify::test_feishu_webhook(&webhook_url).await
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_is_available() -> Result<bool, IrError> {
    Ok(irtool_pcap::PcapCollector::is_available())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_start(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    config: PcapConfig,
) -> Result<(), IrError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<irtool_pcap::PcapEvent>();

    {
        let mut collector = state.pcap_collector.lock().await;
        collector.start(config, tx)?;
    }

    // Forward pcap events: rule engine + frontend emit
    let monitor_engine = state.monitor_engine.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            // 规则引擎处理
            let alerts = monitor_engine.lock().await.process_pcap_event(&event).await;
            for alert in &alerts {
                let _ = app.emit(crate::events::EVT_MONITOR_ALERT, alert);
            }
            // 始终 emit pcap 事件到前端
            let _ = app.emit(crate::events::EVT_PCAP_EVENT, &event);
        }
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_stop(state: State<'_, AppState>) -> Result<(), IrError> {
    let mut collector = state.pcap_collector.lock().await;
    collector.stop();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cmd_pcap_is_running(state: State<'_, AppState>) -> Result<bool, IrError> {
    let collector = state.pcap_collector.lock().await;
    Ok(collector.is_running())
}

// --- Alert Popup Window ---

use std::sync::atomic::{AtomicU32, Ordering};

static POPUP_COUNT: AtomicU32 = AtomicU32::new(0);

#[tauri::command]
#[specta::specta]
pub async fn cmd_show_alert_popup(
    app: tauri::AppHandle,
    rule_name: String,
    key_field: String,
    event_type: String,
    process_name: String,
    protocol: String,
    timestamp: i64,
    source_addr: Option<String>,
    process_chain: Option<String>,
) -> Result<(), String> {
    show_alert_popup_window(
        &app,
        &rule_name,
        &key_field,
        &event_type,
        &process_name,
        &protocol,
        timestamp,
        source_addr.as_deref().unwrap_or(""),
        process_chain.as_deref().unwrap_or(""),
    )
    .map_err(|e| e.to_string())
}

fn show_alert_popup_window(
    app: &tauri::AppHandle,
    rule_name: &str,
    key_field: &str,
    event_type: &str,
    process_name: &str,
    protocol: &str,
    timestamp: i64,
    source_addr: &str,
    process_chain: &str,
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

    // Atomically get current popup count for stacking, then increment
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

    // Position in bottom-right corner of work area (excludes taskbar)
    if let Some(monitor) = window.primary_monitor().ok().flatten() {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let wa_x = work_area.position.x as f64 / scale;
        let wa_y = work_area.position.y as f64 / scale;
        let wa_w = work_area.size.width as f64 / scale;
        let wa_h = work_area.size.height as f64 / scale;
        let x = wa_x + wa_w - width - margin;
        let y = wa_y + wa_h - height - margin - (popup_index as f64 * (height + gap));
        window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
            x, y,
        )))?;
    }

    // Don't show window yet — the frontend will show it after receiving popup data.
    // This avoids a white flash from the empty React shell being visible before content arrives.

    // Decrement popup counter when this window is destroyed
    let label_clone = label.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            POPUP_COUNT.fetch_sub(1, Ordering::SeqCst);
            tracing::debug!("alert popup window destroyed: {label_clone}");
        }
    });

    // Emit event to the popup window with alert data.
    // IMPORTANT: delay 500ms to give React time to mount and register its event listener.
    // Without this, emit_to fires before the WebView has loaded the JS, and the event is lost.
    let alert_key = format!("{event_type}-{timestamp}");
    let app_emit = app.clone();
    let label_emit = label.clone();
    let rule_name = rule_name.to_string();
    let key_field = key_field.to_string();
    let event_type = event_type.to_string();
    let process_name = process_name.to_string();
    let protocol = protocol.to_string();
    let source_addr = source_addr.to_string();
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
                "process_chain": process_chain,
            }),
        );
    });

    // Auto-close after 30 seconds
    let app_clone = app.clone();
    let label_clone = label.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30));
        // Signal the popup to animate out
        let _ = app_clone.emit_to(&label_clone, "evt_close_alert_popup", ());
        // Give animation time, then close the window
        std::thread::sleep(std::time::Duration::from_millis(500));
        if let Some(win) = app_clone.get_webview_window(&label_clone) {
            let _ = win.close();
        }
    });

    Ok(())
}
