use crate::state::AppState;
use irtool_core::IrError;
use irtool_monitor::{Alert, MonitorConfig};
use irtool_pcap::PcapConfig;
use tauri::Emitter;
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
