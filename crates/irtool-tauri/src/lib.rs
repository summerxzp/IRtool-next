mod commands;
mod events;
mod logger;
mod single_instance;
mod state;
mod tray;
#[allow(dead_code)]
mod types;

use crate::commands::autoruns::*;
use crate::commands::monitor::*;
use crate::commands::network::*;
use crate::commands::process::*;
use crate::commands::sysmon::*;
use crate::commands::tools::*;
use crate::commands::workspace::*;
use crate::state::AppState;
use irtool_core::{AppDirs, IrError};
use serde::Serialize;
use specta::Type;
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use tauri::{Emitter, Manager, State};
use tauri_specta::{collect_commands, Builder};
use tracing::info;

#[derive(Serialize, Type)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub is_admin: bool,
}

#[tauri::command]
#[specta::specta]
fn cmd_app_info() -> AppInfo {
    AppInfo {
        name: "IRtool".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        build: "alpha".into(),
        is_admin: is_running_as_admin(),
    }
}

#[tauri::command]
#[specta::specta]
fn cmd_log_frontend(message: String) {
    tracing::warn!("[frontend] {}", message);
}

#[tauri::command]
#[specta::specta]
async fn cmd_app_force_quit(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), IrError> {
    info!("force quit requested, exiting background mode first");
    // Exit background mode before quitting so persisted config is reset.
    // This ensures the app starts in foreground mode on next launch.
    let _ = state.monitor_engine.lock().await.exit_background_mode();
    app.exit(0);
    Ok(())
}

#[cfg(windows)]
fn is_running_as_admin() -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok()
            && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn is_running_as_admin() -> bool {
    false
}

#[cfg(windows)]
fn elevate_and_restart() -> Result<(), Box<dyn std::error::Error>> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_NORMAL;

    let exe = std::env::current_exe()?;
    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = exe
        .to_string_lossy()
        .as_ref()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_NORMAL,
        );
        // ShellExecuteW returns a value > 32 on success
        if result.0 as isize <= 32 {
            return Err(format!("ShellExecuteW 'runas' failed with code {}", result.0 as isize).into());
        }
    }

    Ok(())
}

pub fn run() {
    let app_dirs = AppDirs::detect();

    let _logger_guard = logger::init_logger(app_dirs.logs_dir());

    info!("============================================");
    info!("IRtool v{} starting", env!("CARGO_PKG_VERSION"));
    info!("Admin: {}", is_running_as_admin());
    info!("Portable: {}", app_dirs.is_portable());
    info!("Root: {}", app_dirs.root().display());
    info!("============================================");

    if !is_running_as_admin() {
        info!("Not running as admin, requesting elevation...");
        match elevate_and_restart() {
            Ok(()) => {
                info!("Elevated instance launched, exiting current instance");
                std::process::exit(0);
            }
            Err(e) => {
                tracing::warn!("Elevation failed: {}, continuing in limited mode", e);
            }
        }
    }

    let builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        cmd_app_info,
        cmd_log_frontend,
        cmd_app_force_quit,
        cmd_network_snapshot,
        cmd_network_kill_process,
        cmd_network_set_polling,
        cmd_network_clear_history,
        cmd_network_refresh_cmdline,
        // --- P2 新增 ---
        cmd_autoruns_scan,
        cmd_autoruns_get_result,
        cmd_autoruns_verify_signatures,
        cmd_autoruns_delete_entry,
        cmd_autoruns_cancel_scan,
        cmd_autoruns_calculate_hash,
        cmd_autoruns_batch_calculate_hash,
        cmd_autoruns_sigcheck,
        cmd_autoruns_open_explorer,
        cmd_autoruns_open_regedit,
        cmd_autoruns_open_services,
        cmd_autoruns_extract_icon,
        cmd_autoruns_batch_extract_icons,
        cmd_autoruns_is_scanning,
        // --- P3 新增 ---
        cmd_process_snapshot,
        cmd_process_chain,
        // --- P4 新增 ---
        cmd_sysmon_status,
        cmd_sysmon_is_channel_available,
        cmd_sysmon_install,
        cmd_sysmon_uninstall,
        cmd_sysmon_update_config,
        cmd_sysmon_get_existing_events,
        cmd_sysmon_default_event_configs,
        cmd_sysmon_generate_config,
        cmd_sysmon_start_subscription,
        cmd_sysmon_stop_subscription,
        cmd_sysmon_is_subscribing,
        cmd_sysmon_get_event_count,
        cmd_sysmon_get_log_max_size,
        cmd_sysmon_set_log_max_size,
        // --- P5 新增 ---
        cmd_monitor_get_config,
        cmd_monitor_update_config,
        cmd_monitor_enter_background,
        cmd_monitor_exit_background,
        cmd_monitor_get_alerts,
        cmd_monitor_is_background,
        cmd_monitor_clear_alerts,
        cmd_monitor_get_events,
        cmd_monitor_get_event_count,
        cmd_monitor_search_events,
        cmd_monitor_search_event_page,
        cmd_monitor_get_telemetry,
        cmd_monitor_test_feishu,
        cmd_monitor_clear_events,
        cmd_monitor_event_type_counts,
        cmd_monitor_get_db_size,
        // --- P3 工作台 ---
        cmd_workspace_run_command,
        cmd_workspace_unhide_path,
        cmd_workspace_take_ownership,
        cmd_workspace_sample_path,
        cmd_workspace_open_path,
        // --- P6 新增 ---
        cmd_pcap_is_available,
        cmd_pcap_start,
        cmd_pcap_stop,
        cmd_pcap_is_running,
        cmd_pcap_list_adapters,
        cmd_pcap_get_counters,
        // --- Alert Popup ---
        cmd_show_alert_popup,
        // --- Tools Manager ---
        cmd_tools_check,
        cmd_tools_download,
        cmd_tools_import_zip,
    ]);

    #[cfg(debug_assertions)]
    {
        builder
            .export(
                Typescript::default()
                    .bigint(specta_typescript::BigIntExportBehavior::Number)
                    .header("// @ts-nocheck\n// auto-generated by tauri-specta — DO NOT EDIT\n"),
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/lib/bindings.ts"),
            )
            .expect("failed to export bindings.ts");
    }

    let app_state = AppState::new(app_dirs.clone());

    tauri::Builder::default()
        .manage(app_state.clone())
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            single_instance::handle_second_instance(app, args, cwd);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            commands::network::start_default_polling(&app_state, app.handle());

            // 创建系统托盘
            crate::tray::create_tray(app.handle())?;

            // 设置窗口图标（确保任务栏显示自定义图标，尤其 decorations=false 时）
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
            }

            // 拦截窗口关闭：后台模式时阻止关闭并通知前端弹窗确认
            if let Some(window) = app.get_webview_window("main") {
                let win_clone = window.clone();
                let engine = app_state.monitor_engine.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let is_background = engine.try_lock().map(|e| e.is_background_mode()).unwrap_or(false);
                        if is_background {
                            api.prevent_close();
                            let _ = win_clone.emit(crate::events::EVT_CLOSE_REQUESTED, ());
                        }
                        // else: let the window close normally, app exits
                    }
                });
            }

            // Re-apply window icon on focus gain (desktop refresh from tools like autorunsc can reset taskbar icon)
            let icon_data = app
                .default_window_icon()
                .map(|icon| (icon.rgba().to_vec(), icon.width(), icon.height()));
            if let Some(window) = app.get_webview_window("main") {
                if let Some((rgba, width, height)) = icon_data {
                    let win = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Focused(true) = event {
                            let _ = win.set_icon(tauri::image::Image::new(&rgba, width, height));
                        }
                    });
                }
            }

            info!("main window setup; default polling started");
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("Application exited with error: {}", e);
            std::process::exit(1);
        });
}
