mod commands;
mod events;
mod logger;
mod single_instance;
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
use crate::events::start_event_bridge;
use irtool_core::{AppDirs, IrError};
use irtool_service::context::AppContext;
use irtool_service::dto::app::AppInfo;
use irtool_service::services::app::AppService;
use irtool_service::services::network::NetworkService;
#[cfg(debug_assertions)]
use specta_typescript::Typescript;
use tauri::{Emitter, Manager, State};
use tauri_specta::{collect_commands, Builder};
use tracing::info;

#[tauri::command]
#[specta::specta]
fn cmd_app_info() -> AppInfo {
    AppService::app_info(is_running_as_admin())
}

#[tauri::command]
#[specta::specta]
fn cmd_log_frontend(message: String) {
    AppService::log_frontend(message);
}

#[tauri::command]
#[specta::specta]
async fn cmd_app_force_quit(app: tauri::AppHandle, ctx: State<'_, AppContext>) -> Result<(), IrError> {
    AppService { ctx: &ctx }.force_quit().await?;
    app.exit(0);
    Ok(())
}

/// 自定义重启命令：解决便携版下 tauri-plugin-process 的 relaunch() 因单实例互斥锁
/// 导致新进程启动后立即退出的问题。
///
/// 原理：将批处理脚本写入临时 bat 文件，spawn 一个 detached cmd.exe 子进程，
/// 轮询等待当前进程退出后再启动新实例，然后当前进程调用 app.exit(0) 退出。
#[tauri::command]
#[specta::specta]
async fn cmd_relaunch(app: tauri::AppHandle, ctx: State<'_, AppContext>) -> Result<(), IrError> {
    // 先退出后台模式，确保状态干净
    let _ = AppService { ctx: &ctx }.force_quit().await;

    let current_exe = std::env::current_exe().map_err(|e| IrError::Internal(format!("获取当前 exe 路径失败: {e}")))?;
    let pid = std::process::id();
    let exe_path = current_exe.display().to_string();

    info!("[cmd_relaunch] pid={pid}, exe={exe_path}");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Stdio;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // 将批处理脚本写入临时文件，避免 cmd /c 多行字符串传递问题
        let bat_path = std::env::temp_dir().join(format!("irtool_relaunch_{pid}.bat"));
        let log_path = std::env::temp_dir().join(format!("irtool_relaunch_{pid}.log"));

        // 批处理脚本：等待当前 PID 退出后启动新实例
        // 每步都写入日志文件，方便诊断
        // 转义 exe_path 中的 % 字符，cmd.exe 中 %% 表示字面量 %
        let exe_escaped = exe_path.replace('%', "%%");
        let script = format!(
            "@echo off\r\n\
             echo [%date% %time%] relaunch script started, waiting for PID {pid} > \"{log}\"\r\n\
             :wait\r\n\
             tasklist /fi \"pid eq {pid}\" 2>nul | find \"{pid}\" >nul\r\n\
             if %errorlevel%==0 (\r\n\
                 ping -n 2 127.0.0.1 >nul\r\n\
                 goto wait\r\n\
             )\r\n\
             echo [%date% %time%] PID {pid} exited, starting {exe} >> \"{log}\"\r\n\
             start \"\" \"{exe}\"\r\n\
             echo [%date% %time%] start command issued >> \"{log}\"\r\n\
             del \"{log}\"\r\n\
             del \"%~f0\"\r\n",
            pid = pid,
            exe = exe_escaped,
            log = log_path.display(),
        );

        std::fs::write(&bat_path, &script).map_err(|e| IrError::Internal(format!("写入临时 bat 文件失败: {e}")))?;

        info!("[cmd_relaunch] bat file: {}", bat_path.display());

        std::process::Command::new("cmd")
            .args(["/c", bat_path.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| IrError::Internal(format!("启动重启辅助进程失败: {e}")))?;

        info!("[cmd_relaunch] helper process spawned, exiting app");
    }

    #[cfg(not(windows))]
    {
        let _ = pid;
        std::process::Command::new(&current_exe)
            .spawn()
            .map_err(|e| IrError::Internal(format!("启动新实例失败: {e}")))?;
    }

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

pub fn run() {
    // Acquire cross-process named mutex to ensure mutual exclusion with the egui
    // fallback frontend. tauri-plugin-single-instance only guards Tauri↔Tauri;
    // this also blocks startup when an egui instance already holds the mutex.
    #[cfg(windows)]
    {
        use windows::core::w;
        use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;

        let mutex_name = w!("Global\\IRtool-SingleInstance");
        match unsafe { CreateMutexW(None, true, mutex_name) } {
            Ok(handle) => {
                if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                    eprintln!("IRtool is already running.");
                    return;
                }
                // HANDLE is Copy with no Drop; the kernel mutex object stays
                // alive until process exit since we never call CloseHandle.
                let _ = handle;
            }
            Err(_) => {
                tracing::warn!("failed to create single-instance mutex, proceeding anyway");
            }
        }
    }

    let app_dirs = AppDirs::detect();

    let _logger_guard = logger::init_logger(app_dirs.logs_dir());

    info!("============================================");
    info!("IRtool v{} starting", env!("CARGO_PKG_VERSION"));
    info!("Admin: {}", is_running_as_admin());
    info!("Portable: {}", app_dirs.is_portable());
    info!("Root: {}", app_dirs.root().display());
    info!("============================================");

    if !is_running_as_admin() {
        tracing::warn!("Not running as admin, some features will be limited");
    }

    let builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        cmd_app_info,
        cmd_log_frontend,
        cmd_app_force_quit,
        cmd_relaunch,
        cmd_network_snapshot,
        cmd_network_kill_process,
        cmd_network_set_polling,
        cmd_network_clear_history,
        cmd_network_refresh_cmdline,
        // --- P2 ---
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
        // --- P3 ---
        cmd_process_snapshot,
        cmd_process_chain,
        // --- P4 ---
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
        // --- P5 ---
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
        // --- P3 workspace ---
        cmd_workspace_run_command,
        cmd_workspace_unhide_path,
        cmd_workspace_take_ownership,
        cmd_workspace_sample_path,
        cmd_workspace_open_path,
        // --- P6 ---
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

    let app_ctx = AppContext::new(app_dirs.clone());

    tauri::Builder::default()
        .manage(app_ctx.clone())
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

            // Start EventBus -> Tauri event bridge
            start_event_bridge(&app_ctx, app.handle().clone());

            // Start default network polling
            NetworkService { ctx: &app_ctx }.start_default_polling(tauri::async_runtime::handle().inner().clone());

            // Create system tray
            crate::tray::create_tray(app.handle())?;

            // Set window icon
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
            }

            // Intercept window close: in background mode, prevent close and emit event
            if let Some(window) = app.get_webview_window("main") {
                let win_clone = window.clone();
                let engine = app_ctx.monitor_engine.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let is_background = engine.try_lock().map(|e| e.is_background_mode()).unwrap_or(false);
                        if is_background {
                            api.prevent_close();
                            let _ = win_clone.emit("evt_close_requested", ());
                        }
                    }
                });
            }

            // Re-apply window icon on focus gain
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
