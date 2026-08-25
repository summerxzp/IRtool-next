use std::time::Duration;

use eframe::egui;
use irtool_service::context::AppContext;
use irtool_service::event_bus::AppEvent;
use irtool_service::services::autoruns::AutorunsService;
use irtool_service::services::monitor::MonitorService;
use irtool_service::services::tools::ToolsService;
use irtool_service::types::AutorunItem;
use irtool_service::types::ToolStatus;

use crate::design::theme as dtheme;
use crate::event_bridge::EventBridge;
use crate::nav::Page;
use crate::pages::{
    autoruns::AutorunsPageState,
    browser_forensics::BrowserForensicsPageState,
    database::{DatabasePageState, DbRefresh},
    monitor::{MonitorPageState, MonitorRefresh},
    network::{DetailDock, NetworkPageState},
    process::ProcessPageState,
    settings::{SettingsPageState, SettingsRefresh},
    sysmon::{SysmonPageState, SysmonRefresh},
    workspace::{WorkspacePageState, WorkspaceRefresh},
};
use crate::theme;
use crate::StartupMode;

/// WebView2 下载页地址（微软官方）。
const WEBVIEW2_DOWNLOAD_URL: &str =
    "https://developer.microsoft.com/zh-cn/microsoft-edge/webview2?form=MA13LH#download";

/// The main egui application struct.
pub struct IrtoolApp {
    // Infrastructure
    pub ctx: AppContext,
    pub rt: tokio::runtime::Runtime,
    pub event_bridge: EventBridge,

    // Navigation
    pub current_page: Page,
    /// 侧栏展开态（图标+文字 / 纯图标；session 内记忆）。
    pub sidebar_expanded: bool,

    // Global state
    pub is_admin: bool,
    pub is_fallback: bool,
    theme_applied: bool,

    // Startup dialogs
    fallback_notice_open: bool,
    tools_check_open: bool,
    tools_status: Vec<ToolStatus>,
    tools_downloading: bool,
    tools_download_progress: std::collections::HashMap<String, u8>,
    tools_download_error: Option<String>,
    tools_download_done: bool,
    tools_refresh_tx: std::sync::mpsc::Sender<Vec<ToolStatus>>,
    tools_refresh_rx: std::sync::mpsc::Receiver<Vec<ToolStatus>>,

    // Per-page state
    pub network: NetworkPageState,
    pub process: ProcessPageState,
    pub autoruns: AutorunsPageState,
    pub browser_forensics: BrowserForensicsPageState,
    pub sysmon: SysmonPageState,
    pub monitor: MonitorPageState,
    pub database: DatabasePageState,
    pub workspace: WorkspacePageState,
    pub settings: SettingsPageState,

    // Async data refresh channels
    autoruns_refresh_tx: std::sync::mpsc::Sender<Vec<AutorunItem>>,
    autoruns_refresh_rx: std::sync::mpsc::Receiver<Vec<AutorunItem>>,
    sysmon_refresh_rx: std::sync::mpsc::Receiver<SysmonRefresh>,
    monitor_refresh_rx: std::sync::mpsc::Receiver<MonitorRefresh>,
    database_refresh_rx: std::sync::mpsc::Receiver<DbRefresh>,
    workspace_refresh_rx: std::sync::mpsc::Receiver<WorkspaceRefresh>,
    settings_refresh_rx: std::sync::mpsc::Receiver<SettingsRefresh>,

    // Exit / background mode
    exit_check_pending: bool,
    is_background_mode: bool,
    exit_check_tx: std::sync::mpsc::Sender<bool>,
    exit_check_rx: std::sync::mpsc::Receiver<bool>,
    /// 为 true 时允许窗口关闭（跳过 CancelClose 拦截）。
    force_exit: bool,
    /// 后台监控运行中关闭窗口的确认对话框是否打开。
    exit_confirm_open: bool,

    // System tray
    #[allow(dead_code)]
    tray_icon: Option<tray_icon::TrayIcon>,
    tray_show_id: muda::MenuId,
    tray_quit_id: muda::MenuId,
    /// 托盘"退出"菜单设置的原子标志。窗口隐藏后 update() 不被调用，
    /// 需要通过原子标志在 update() 恢复执行时传递 force_exit 状态。
    force_exit_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 标记 set_event_handler 是否已设置。
    tray_handler_set: bool,
    /// 日志 non_blocking guard，进程退出时 drop 以 flush 缓冲区。
    #[allow(dead_code)]
    log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    /// 单实例互斥锁 guard，进程退出前持有以防止多实例。
    #[allow(dead_code)]
    single_instance_guard: Option<crate::single_instance::SingleInstanceGuard>,
}

impl IrtoolApp {
    pub fn new(
        ctx: AppContext,
        event_bridge: EventBridge,
        rt: tokio::runtime::Runtime,
        mode: StartupMode,
        log_guard: tracing_appender::non_blocking::WorkerGuard,
        single_instance_guard: crate::single_instance::SingleInstanceGuard,
    ) -> Self {
        let is_admin = is_running_as_admin();

        // 主题/语言运行时初始化：读取 config/ui-state.json（无文件 → System / zh-CN），
        // System 按注册表 AppsUseLightTheme 解析；语言同路加载并应用 rust-i18n locale
        // （须在 create_tray 之前，保证托盘菜单文本使用启动语言）。
        let ui_state = dtheme::load_state(&ctx.app_dirs.config_dir());
        dtheme::init(ui_state.theme);
        dtheme::set_language(ui_state.language);

        let (autoruns_refresh_tx, autoruns_refresh_rx) = std::sync::mpsc::channel::<Vec<AutorunItem>>();
        let (sysmon_refresh_tx, sysmon_refresh_rx) = std::sync::mpsc::channel::<SysmonRefresh>();
        let (monitor_refresh_tx, monitor_refresh_rx) = std::sync::mpsc::channel::<MonitorRefresh>();
        let (database_refresh_tx, database_refresh_rx) = std::sync::mpsc::channel::<DbRefresh>();
        let (workspace_refresh_tx, workspace_refresh_rx) = std::sync::mpsc::channel::<WorkspaceRefresh>();
        let (settings_refresh_tx, settings_refresh_rx) = std::sync::mpsc::channel::<SettingsRefresh>();
        let (exit_check_tx, exit_check_rx) = std::sync::mpsc::channel::<bool>();

        let mut autoruns = AutorunsPageState::default();
        autoruns.refresh_tx = Some(autoruns_refresh_tx.clone());

        let mut sysmon = SysmonPageState::default();
        sysmon.refresh_tx = Some(sysmon_refresh_tx);
        // Kick off initial status / config fetch.
        sysmon.refresh_status(&ctx, rt.handle());

        let mut monitor = MonitorPageState::default();
        monitor.refresh_tx = Some(monitor_refresh_tx);
        monitor.trigger_config_load(&ctx, rt.handle());

        // 进程页：首次加载快照
        let mut process = ProcessPageState::default();
        process.trigger_refresh(&ctx, rt.handle());

        let database = DatabasePageState {
            refresh_tx: Some(database_refresh_tx),
            ..Default::default()
        };
        database.trigger_initial_load(&ctx, rt.handle());

        let workspace = WorkspacePageState {
            refresh_tx: Some(workspace_refresh_tx),
            ..Default::default()
        };

        let settings = SettingsPageState {
            refresh_tx: Some(settings_refresh_tx),
            ..Default::default()
        };

        // network 页（P5 范式页）：启动时恢复表格持久化状态（列宽/密度/排序）
        let mut network = NetworkPageState::default();
        network.load_table_state(&ctx.app_dirs.config_dir());

        // 启动时检测外部工具是否缺失（参考主 UI AppShell 的逻辑）
        let (tools_refresh_tx, tools_refresh_rx) = std::sync::mpsc::channel::<Vec<ToolStatus>>();
        let ctx_for_tools = ctx.clone();
        let tools_tx_for_spawn = tools_refresh_tx.clone();
        rt.handle().spawn(async move {
            let svc = ToolsService { ctx: &ctx_for_tools };
            let result = svc.check().await.unwrap_or_default();
            let _ = tools_tx_for_spawn.send(result);
        });

        // 创建系统托盘图标（用于后台模式恢复窗口）
        let (tray_icon, tray_show_id, tray_quit_id) = create_tray();

        let force_exit_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        Self {
            ctx,
            rt,
            event_bridge,
            current_page: Page::Network,
            sidebar_expanded: false,
            is_admin,
            is_fallback: mode == StartupMode::Fallback,
            theme_applied: false,
            fallback_notice_open: mode == StartupMode::Fallback,
            tools_check_open: false,
            tools_status: Vec::new(),
            tools_downloading: false,
            tools_download_progress: std::collections::HashMap::new(),
            tools_download_error: None,
            tools_download_done: false,
            tools_refresh_tx,
            tools_refresh_rx,
            network,
            process,
            autoruns,
            browser_forensics: BrowserForensicsPageState::default(),
            sysmon,
            monitor,
            database,
            workspace,
            settings,
            autoruns_refresh_tx,
            autoruns_refresh_rx,
            sysmon_refresh_rx,
            monitor_refresh_rx,
            database_refresh_rx,
            workspace_refresh_rx,
            settings_refresh_rx,
            exit_check_pending: false,
            is_background_mode: false,
            exit_check_tx,
            exit_check_rx,
            force_exit: false,
            exit_confirm_open: false,
            tray_icon,
            tray_show_id,
            tray_quit_id,
            force_exit_flag,
            tray_handler_set: false,
            log_guard: Some(log_guard),
            single_instance_guard: Some(single_instance_guard),
        }
    }

    fn handle_event(&mut self, event: AppEvent, ctx: &egui::Context) {
        match event {
            AppEvent::NetworkSnapshot(payload) => {
                self.network.handle_snapshot(payload);
            }
            AppEvent::NetworkError(err) => {
                self.network.handle_error(err);
            }
            AppEvent::NetworkEnrichment(enrichment) => {
                self.network.handle_enrichment(enrichment);
            }
            AppEvent::MonitorAlert(alert) => {
                tracing::info!(
                    "monitor alert: rule={}, event={}, key={}",
                    alert.rule_name,
                    alert.event_type,
                    alert.key_field
                );
            }
            AppEvent::BrowserMaliciousConnection(payload) => {
                tracing::info!(
                    "browser malicious connection: process={}, pid={}, domain={}, ip={}",
                    payload.process_name,
                    payload.pid,
                    payload.domain,
                    payload.ip,
                );
            }
            AppEvent::ExtensionAttribution(payload) => {
                tracing::info!(
                    "extension attribution: {} {} → {} (ext={:?})",
                    payload.method,
                    payload.url,
                    payload.attribution_status,
                    payload.extension_name,
                );
            }
            AppEvent::AutorunsProgress(p) => {
                self.autoruns.handle_scan_progress(p);
            }
            AppEvent::AutorunsSignatureProgress(_) => {
                // Signature progress not shown in toolbar; could be added later
            }
            AppEvent::AutorunsHashProgress(_) => {
                // Hash progress not shown in toolbar; could be added later
            }
            AppEvent::AutorunsScanComplete { count } => {
                self.autoruns.handle_scan_complete(count);
                // Spawn async data refresh; result arrives via channel
                let ctx_clone = self.ctx.clone();
                let tx = self.autoruns_refresh_tx.clone();
                // Also refresh workspace's autoruns data so the workspace tab stays in sync
                let ws_tx = self.workspace.refresh_tx.clone();
                self.rt.handle().spawn(async move {
                    let items = AutorunsService { ctx: &ctx_clone }
                        .get_result()
                        .await
                        .unwrap_or_default();
                    let _ = tx.send(items.clone());
                    if let Some(ws_tx) = ws_tx {
                        let _ = ws_tx.send(WorkspaceRefresh {
                            autorun_items: Some(items),
                            ..Default::default()
                        });
                    }
                });
            }
            AppEvent::AutorunsScanCancelled(task_id) => {
                self.autoruns.handle_scan_cancelled(task_id);
            }
            AppEvent::AutorunsScanFailed { task_id, error } => {
                self.autoruns.handle_scan_failed(task_id, error);
            }
            AppEvent::SysmonEvent(ev) => {
                self.sysmon.handle_sysmon_event(*ev);
            }
            AppEvent::PcapEvent(ev) => {
                self.sysmon.handle_pcap_event(ev);
            }
            AppEvent::ToolsDownloadProgress {
                tool_id,
                downloaded,
                total,
            } => {
                if total > 0 {
                    let pct = ((downloaded as f64 / total as f64) * 100.0) as u8;
                    self.tools_download_progress.insert(tool_id, pct);
                }
            }
            AppEvent::ToolsDownloadError { tool_id, error } => {
                let label = tool_label(&tool_id);
                let entry = format!("{}: {}", label, error);
                self.tools_download_error = Some(match self.tools_download_error.take() {
                    Some(prev) => format!("{}\n{}", prev, entry),
                    None => entry,
                });
            }
            AppEvent::ToolsDownloadComplete { errors } => {
                self.tools_downloading = false;
                self.tools_download_done = true;
                if errors == 0 {
                    self.tools_download_error = None;
                }
                // 下载完成后重新检测工具状态
                let ctx_clone = self.ctx.clone();
                let tx = self.tools_refresh_tx.clone();
                self.rt.handle().spawn(async move {
                    let svc = ToolsService { ctx: &ctx_clone };
                    let result = svc.check().await.unwrap_or_default();
                    let _ = tx.send(result);
                });
            }
            AppEvent::CloseRequested => {
                tracing::info!("close requested, shutting down");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Fallback 通知弹窗：告知用户因 WebView2 缺失而降级到备用界面。
    fn render_fallback_notice(&mut self, ctx: &egui::Context) {
        if !self.fallback_notice_open {
            return;
        }
        let mut open = self.fallback_notice_open;
        let mut know_clicked = false;
        let heading = rust_i18n::t!("shell.fallback.heading");
        let desc = rust_i18n::t!("shell.fallback.desc");
        let install_hint = rust_i18n::t!("shell.fallback.install-hint");
        let rec_title = rust_i18n::t!("shell.fallback.rec-title");
        let rec_detail = rust_i18n::t!("shell.fallback.rec-detail");
        let open_dl = rust_i18n::t!("shell.fallback.open-download-page");
        egui::Window::new(rust_i18n::t!("shell.fallback.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(420.0)
            .max_width(520.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("ℹ").size(20.0).color(theme::semantic_warning()));
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(heading.as_ref()).strong().size(14.0));
                    });
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new(desc.as_ref()).size(12.0).color(theme::fg_secondary()));
                    ui.add_space(10.0);

                    ui.label(
                        egui::RichText::new(install_hint.as_ref())
                            .size(12.0)
                            .color(theme::fg_primary()),
                    );
                    ui.add_space(4.0);

                    // 推荐选项说明
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(rec_title.as_ref())
                                .size(11.0)
                                .color(theme::fg_secondary()),
                        );
                        ui.label(
                            egui::RichText::new(rec_detail.as_ref())
                                .size(11.0)
                                .color(theme::fg_tertiary()),
                        );
                    });
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button(open_dl.as_ref()).clicked() {
                            let _ = webbrowser::open(WEBVIEW2_DOWNLOAD_URL);
                        }
                        ui.add_space((ui.available_width() - 60.0).max(0.0));
                        if ui.button(rust_i18n::t!("shell.fallback.ok").as_ref()).clicked() {
                            know_clicked = true;
                        }
                    });
                });
            });
        if know_clicked {
            open = false;
        }
        self.fallback_notice_open = open;
    }

    /// 外部工具检测弹窗：参考主 UI ToolsCheckDialog 实现。
    fn render_tools_check(&mut self, ctx: &egui::Context) {
        if !self.tools_check_open {
            return;
        }
        let mut open = self.tools_check_open;
        let mut close_clicked = false;
        let mut recheck_clicked = false;
        let mut download_clicked = false;
        let mut relaunch_clicked = false;
        let intro = rust_i18n::t!("shell.tools.intro");

        egui::Window::new(rust_i18n::t!("shell.tools.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(440.0)
            .max_width(560.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(intro.as_ref()).size(11.0).color(theme::fg_tertiary()));
                    ui.add_space(6.0);

                    if self.tools_status.is_empty() {
                        let checking = rust_i18n::t!("shell.tools.checking");
                        ui.label(egui::RichText::new(checking.as_ref()).size(12.0).color(theme::fg_tertiary()));
                    } else {
                        for tool in &self.tools_status {
                            ui.horizontal(|ui| {
                                let (icon, color) = if tool.installed {
                                    ("√", theme::semantic_success())
                                } else {
                                    ("!", theme::semantic_warning())
                                };
                                ui.label(egui::RichText::new(icon).size(14.0).color(color));
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(tool_label(&tool.id)).strong().size(12.0));
                                        if tool.optional {
                                            let optional = rust_i18n::t!("shell.tools.optional");
                                            ui.label(
                                                egui::RichText::new(optional.as_ref())
                                                    .size(10.0)
                                                    .color(theme::fg_tertiary()),
                                            );
                                        }
                                    });
                                    let missing = rust_i18n::t!("shell.tools.missing");
                                    let detail = if tool.installed {
                                        format!(
                                            "v{} — {}",
                                            tool.version.as_deref().unwrap_or("?"),
                                            tool.files.join(", ")
                                        )
                                    } else {
                                        format!("{}: {}", missing.as_ref(), tool.missing_files.join(", "))
                                    };
                                    ui.label(egui::RichText::new(detail).size(10.0).color(theme::fg_tertiary()));
                                });
                            });
                            ui.add_space(2.0);
                        }
                    }

                    // 下载进度
                    if self.tools_downloading && !self.tools_download_progress.is_empty() {
                        ui.add_space(4.0);
                        let entries: Vec<u8> = self.tools_download_progress.values().copied().collect();
                        let overall = if entries.is_empty() {
                            0
                        } else {
                            entries.iter().map(|&v| v as u32).sum::<u32>() / entries.len() as u32
                        };
                        ui.label(
                            egui::RichText::new(rust_i18n::t!("shell.tools.downloading", percent = overall).as_ref())
                                .size(11.0)
                                .color(theme::fg_secondary()),
                        );
                        let progress = overall as f32 / 100.0;
                        ui.add(egui::ProgressBar::new(progress).desired_width(400.0));
                    }

                    // 下载完成提示
                    if self.tools_download_done && !self.tools_downloading {
                        ui.add_space(4.0);
                        if self.tools_download_error.is_none() {
                            let done = rust_i18n::t!("shell.tools.download-done");
                            egui::Frame::new()
                                .fill(theme::semantic_success().linear_multiply(0.15))
                                .inner_margin(6.0)
                                .corner_radius(4.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(done.as_ref())
                                            .size(11.0)
                                            .color(theme::semantic_success()),
                                    );
                                });
                        } else {
                            let partial = rust_i18n::t!("shell.tools.partial-failed");
                            let msg = format!(
                                "{}\n{}",
                                partial.as_ref(),
                                self.tools_download_error.as_deref().unwrap_or("")
                            );
                            egui::Frame::new()
                                .fill(theme::semantic_danger().linear_multiply(0.15))
                                .inner_margin(6.0)
                                .corner_radius(4.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(msg)
                                            .size(11.0)
                                            .color(theme::semantic_danger()),
                                    );
                                });
                        }
                    }

                    // 错误信息（下载中）
                    if self.tools_downloading {
                        if let Some(ref err) = self.tools_download_error {
                            ui.add_space(4.0);
                            let failed = rust_i18n::t!("shell.tools.download-failed");
                            let msg = format!("{}\n{}", failed.as_ref(), err);
                            egui::Frame::new()
                                .fill(theme::semantic_danger().linear_multiply(0.15))
                                .inner_margin(6.0)
                                .corner_radius(4.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(msg)
                                            .size(11.0)
                                            .color(theme::semantic_danger()),
                                    );
                                });
                        }
                    }

                    ui.add_space(6.0);
                    let source = rust_i18n::t!("shell.tools.source");
                    let relaunch = rust_i18n::t!("shell.tools.relaunch");
                    let recheck = rust_i18n::t!("shell.tools.recheck");
                    let download_missing = rust_i18n::t!("shell.tools.download-missing");
                    let close = rust_i18n::t!("shell.tools.close");
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(source.as_ref())
                                .size(10.0)
                                .color(theme::fg_tertiary()),
                        );
                        ui.add_space((ui.available_width() - 200.0).max(0.0));
                        if self.tools_download_done
                            && !self.tools_downloading
                            && self.tools_download_error.is_none()
                            && ui.button(relaunch.as_ref()).clicked()
                        {
                            relaunch_clicked = true;
                        }
                        if ui.button(recheck.as_ref()).clicked() {
                            recheck_clicked = true;
                        }
                        let missing = self.tools_status.iter().any(|t| !t.installed);
                        if missing
                            && !self.tools_downloading
                            && !self.tools_download_done
                            && ui.button(download_missing.as_ref()).clicked()
                        {
                            download_clicked = true;
                        }
                        if ui.button(close.as_ref()).clicked() {
                            close_clicked = true;
                        }
                    });
                });
            });

        if close_clicked {
            self.tools_download_done = false;
            open = false;
        }

        if recheck_clicked {
            self.tools_download_done = false;
            let ctx_clone = self.ctx.clone();
            let tx = self.tools_refresh_tx.clone();
            self.tools_download_error = None;
            self.rt.handle().spawn(async move {
                let svc = ToolsService { ctx: &ctx_clone };
                let result = svc.check().await.unwrap_or_default();
                let _ = tx.send(result);
            });
        }

        if download_clicked {
            let missing: Vec<String> = self
                .tools_status
                .iter()
                .filter(|t| !t.installed)
                .map(|t| t.id.clone())
                .collect();
            if !missing.is_empty() {
                self.tools_downloading = true;
                self.tools_download_done = false;
                self.tools_download_progress.clear();
                self.tools_download_error = None;
                let ctx_clone = self.ctx.clone();
                self.rt.handle().spawn(async move {
                    let svc = ToolsService { ctx: &ctx_clone };
                    if let Err(e) = svc.download(missing).await {
                        tracing::error!("下载工具失败: {}", e);
                    }
                });
            }
        }

        if relaunch_clicked {
            // 退出后台模式
            let ctx_clone = self.ctx.clone();
            self.rt.handle().spawn(async move {
                let _ = irtool_service::services::app::AppService { ctx: &ctx_clone }
                    .force_quit()
                    .await;
            });

            // 释放单实例锁，通过 ShellExecuteW Win32 API 启动新实例
            // ShellExecuteW 原生 UTF-16 编码，完美支持中文/Unicode 路径
            self.single_instance_guard = None;
            let current_exe = std::env::current_exe().ok();

            #[cfg(windows)]
            {
                use windows::core::PCWSTR;
                use windows::Win32::UI::Shell::ShellExecuteW;
                use windows::Win32::UI::WindowsAndMessaging::SW_NORMAL;

                if let Some(exe_path) = current_exe {
                    let verb: Vec<u16> = "open\0".encode_utf16().collect();
                    let file: Vec<u16> = exe_path
                        .to_string_lossy()
                        .as_ref()
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    unsafe {
                        let _ = ShellExecuteW(
                            None,
                            PCWSTR(verb.as_ptr()),
                            PCWSTR(file.as_ptr()),
                            PCWSTR::null(),
                            PCWSTR::null(),
                            SW_NORMAL,
                        );
                    }
                }
            }

            #[cfg(not(windows))]
            {
                if let Some(exe_path) = current_exe {
                    let _ = std::process::Command::new(&exe_path).spawn();
                }
            }

            self.force_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        self.tools_check_open = open;
    }

    /// 请求退出：异步检查后台监控模式，根据结果决定隐藏到托盘或直接退出。
    pub fn request_exit(&mut self) {
        if self.exit_check_pending {
            return;
        }
        self.exit_check_pending = true;
        let ctx_clone = self.ctx.clone();
        let tx = self.exit_check_tx.clone();
        self.rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx_clone };
            let is_bg = svc.is_background().await.unwrap_or(false);
            let _ = tx.send(is_bg);
        });
    }
}

/// 工具 ID 到中文显示名称的映射（与主 UI TOOL_LABELS 一致）。
fn tool_label(id: &str) -> &str {
    match id {
        "autoruns" => "Autoruns",
        "sigcheck" => "Sigcheck",
        "sysmon" => "Sysmon",
        _ => id,
    }
}

impl eframe::App for IrtoolApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.36: App::update(ctx) -> App::ui(&mut Ui)，Context 从 Ui 取。
        let ctx = ui.ctx().clone();

        // 拦截窗口关闭请求：检查后台模式后决定隐藏到托盘或退出
        if ctx.input(|i| i.viewport().close_requested()) && !self.force_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_exit();
        }

        // 检查托盘"退出"菜单设置的原子标志
        if self.force_exit_flag.load(std::sync::atomic::Ordering::SeqCst) {
            self.force_exit = true;
            self.force_exit_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }

        // 首次 update：设置托盘事件处理器，直接用 Win32 API 操作窗口
        // （窗口被 Visible(false) 隐藏后 update() 不被调用，不能依赖缓冲区方式）
        if !self.tray_handler_set {
            self.tray_handler_set = true;
            let force_exit_flag = self.force_exit_flag.clone();
            let show_id = self.tray_show_id.clone();
            let quit_id = self.tray_quit_id.clone();

            // 托盘图标事件（双击恢复窗口）
            // 注意：set_event_handler 是进程级全局静态函数，后续调用会静默替换之前的处理器。
            // tray_handler_set 标志仅防止同一实例重复设置，无法阻止外部代码覆盖。
            tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
                if let tray_icon::TrayIconEvent::DoubleClick {
                    button: tray_icon::MouseButton::Left,
                    ..
                } = event
                {
                    show_and_activate_window();
                }
            }));

            // 托盘菜单事件（显示窗口 / 退出）
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                if event.id == show_id {
                    show_and_activate_window();
                } else if event.id == quit_id {
                    force_exit_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    show_and_activate_window();
                    post_close_to_window();
                }
            }));
        }

        // Apply theme on first update (when we have the real egui context)
        if !self.theme_applied {
            theme::apply(&ctx);
            self.theme_applied = true;
            self.event_bridge.attach_context(ctx.clone());
        }

        // 0. 全屏底层填充：防止 Panel 之间因 DPI 缩放/对齐产生缝隙，露出 eframe 默认深色背景。
        let screen = ctx.viewport_rect();
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(screen, 0.0, theme::bg_primary());

        // 1. Drain events from EventBus
        for event in self.event_bridge.drain() {
            self.handle_event(event, &ctx);
        }

        // 1b. Drain autoruns data refresh results
        while let Ok(items) = self.autoruns_refresh_rx.try_recv() {
            self.autoruns.items = items;
            // Keep selected_item in sync with refreshed data (e.g. after hash calculation)
            if let Some(id) = self.autoruns.selected_id {
                self.autoruns.selected_item = self.autoruns.items.iter().find(|i| i.id == id).cloned();
            }
            self.autoruns.mark_cache_dirty();
        }

        // 1c. Drain sysmon async refresh results
        while let Ok(r) = self.sysmon_refresh_rx.try_recv() {
            self.sysmon.apply_refresh(r);
        }

        // 1d. Drain monitor async refresh results
        while let Ok(r) = self.monitor_refresh_rx.try_recv() {
            self.monitor.apply_refresh(r);
        }

        // 1e. Drain database async refresh results
        while let Ok(r) = self.database_refresh_rx.try_recv() {
            self.database.apply_refresh(r);
        }

        // 1f. Drain workspace async refresh results
        while let Ok(r) = self.workspace_refresh_rx.try_recv() {
            self.workspace.apply_refresh(r);
        }

        // 1g. Drain settings async refresh results
        while let Ok(r) = self.settings_refresh_rx.try_recv() {
            self.settings.apply_refresh(r);
        }

        // 1h. Drain tools check results — 缺失则弹窗
        while let Ok(statuses) = self.tools_refresh_rx.try_recv() {
            let was_downloading = self.tools_downloading;
            self.tools_status = statuses.clone();
            if was_downloading {
                // 下载完成后的重新检测，不自动关闭弹窗，让用户看到结果
            } else if statuses.iter().any(|t| !t.installed) {
                self.tools_check_open = true;
            }
        }

        // 1i. Drain exit background-mode check results
        while let Ok(is_bg) = self.exit_check_rx.try_recv() {
            self.exit_check_pending = false;
            self.is_background_mode = is_bg;
            if is_bg {
                // 后台监控运行中 → 弹确认框，不直接隐藏
                self.exit_confirm_open = true;
            } else {
                // 无后台监控 → 直接退出
                self.force_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // 2. Top bar（壳：elev-1 底 + 底缘 border 分隔线，demo toolbar 基准）
        egui::Panel::top("topbar")
            .frame(dtheme::shell_panel_frame(egui::Margin::symmetric(12, 6)))
            .show_separator_line(true)
            .show(ui, |ui| {
                self.render_topbar(ui);
            });

        // 3. Navigation rail（壳：rail 底色，demo side_rail 基准）
        let rail_w = if self.sidebar_expanded { 168.0 } else { theme::RAIL_WIDTH };
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(rail_w)
            .frame(
                egui::Frame::new()
                    .fill(dtheme::palette().rail)
                    .inner_margin(egui::Margin::ZERO)
                    .outer_margin(egui::Margin::ZERO)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(0.0)
                    .shadow(egui::Shadow::NONE),
            )
            .show_separator_line(false)
            .show(ui, |ui| {
                self.render_sidebar(ui);
            });

        // 4. Stats bar (bottom, always at the very bottom；壳：elev-1 底 + 顶缘分隔线)
        if matches!(
            self.current_page,
            Page::Network | Page::Autoruns | Page::Sysmon | Page::Database | Page::Workspace
        ) {
            egui::Panel::bottom("stats_bar")
                .frame(dtheme::shell_panel_frame(egui::Margin::symmetric(16, 6)))
                .show_separator_line(true)
                .show(ui, |ui| match self.current_page {
                    Page::Network => {
                        // 计数靠左侧导航栏（用户反馈：工具栏右侧与按钮重叠）
                        let (total, hist) = self.network.row_counts();
                        let text = if hist > 0 {
                            rust_i18n::t!(
                                "network.stats.count-history",
                                total = total.to_string(),
                                history = hist.to_string()
                            )
                        } else {
                            rust_i18n::t!("network.stats.count", total = total.to_string())
                        };
                        ui.label(
                            egui::RichText::new(text.as_ref())
                                .font(crate::design::fonts::caption())
                                .color(dtheme::palette().fg_tertiary),
                        );
                    }
                    Page::Autoruns => self.autoruns.render_stats_bar(ui),
                    Page::Sysmon => self.sysmon.render_stats_bar(ui),
                    Page::Database => self.database.render_stats_bar(ui),
                    Page::Workspace => self.workspace.render_stats_bar(ui),
                    _ => {
                        ui.label("");
                    }
                });
        }

        // 4b. Detail panel (bottom, above stats bar — declared after so it stacks above)
        if self.current_page == Page::Network && self.network.detail_visible {
            // 详情可切右/底（header 内切换钮）；egui panel 尺寸 session 内记忆，每页独立
            let frame = theme::panel_frame(egui::Margin::symmetric(8, 4));
            match self.network.detail_dock {
                DetailDock::Right => {
                    egui::Panel::right("detail_panel_right")
                        .default_size(420.0)
                        .size_range(320.0..=680.0)
                        .resizable(true)
                        .frame(frame)
                        .show(ui, |ui| {
                            self.network.render_detail_panel(ui, &self.ctx, self.rt.handle());
                        });
                }
                DetailDock::Bottom => {
                    egui::Panel::bottom("detail_panel_bottom")
                        .default_size(280.0)
                        .size_range(160.0..=520.0)
                        .resizable(true)
                        .frame(frame)
                        .show(ui, |ui| {
                            self.network.render_detail_panel(ui, &self.ctx, self.rt.handle());
                        });
                }
            }
        } else if self.current_page == Page::Process
            && self.process.detail_visible
            && self.process.selected_pid.is_some()
        {
            egui::Panel::bottom("process_detail_panel")
                .default_size(theme::DETAIL_PANEL_HEIGHT)
                .resizable(true)
                .frame(theme::panel_frame(egui::Margin::symmetric(8, 4)))
                .show(ui, |ui| {
                    self.process
                        .render_detail_panel(ui, &self.ctx, self.rt.handle(), &self.sysmon.events);
                });
        } else if self.current_page == Page::Autoruns
            && self.autoruns.detail_visible
            && self.autoruns.selected_id.is_some()
        {
            egui::Panel::bottom("autoruns_detail_panel")
                .default_size(theme::DETAIL_PANEL_HEIGHT)
                .resizable(true)
                .frame(theme::panel_frame(egui::Margin::symmetric(8, 4)))
                .show(ui, |ui| {
                    self.autoruns.render_detail_panel(ui, &self.ctx, self.rt.handle());
                });
        } else if self.current_page == Page::Sysmon && self.sysmon.detail_visible {
            egui::Panel::bottom("sysmon_detail_panel")
                .default_size(theme::DETAIL_PANEL_HEIGHT)
                .resizable(true)
                .frame(theme::panel_frame(egui::Margin::symmetric(8, 4)))
                .show(ui, |ui| {
                    self.sysmon.render_detail_panel(ui, &self.ctx, self.rt.handle());
                });
        } else if self.current_page == Page::Database && self.database.detail_visible {
            egui::Panel::bottom("database_detail_panel")
                .default_size(theme::DETAIL_PANEL_HEIGHT)
                .resizable(true)
                .frame(theme::panel_frame(egui::Margin::symmetric(8, 4)))
                .show(ui, |ui| {
                    self.database.render_detail_panel(ui, &self.ctx, self.rt.handle());
                });
        } else if self.current_page == Page::Workspace && self.workspace.detail_visible {
            egui::Panel::bottom("workspace_detail_panel")
                .default_size(theme::DETAIL_PANEL_HEIGHT)
                .resizable(true)
                .frame(theme::panel_frame(egui::Margin::symmetric(8, 4)))
                .show(ui, |ui| {
                    self.workspace.render_detail_panel(ui, &self.ctx, self.rt.handle());
                });
        }

        // 5. Content area
        egui::CentralPanel::default()
            .frame(theme::panel_frame(egui::Margin::ZERO))
            .show(ui, |ui| match self.current_page {
                Page::Network => {
                    self.network.render(ui, &self.ctx, self.rt.handle());
                }
                Page::Process => {
                    self.process
                        .render(ui, &self.ctx, self.rt.handle(), &self.sysmon.events);
                }
                Page::Autoruns => {
                    self.autoruns.render(ui, &self.ctx, self.rt.handle());
                }
                Page::BrowserForensics => {
                    self.browser_forensics.show(ui, &self.ctx, self.rt.handle());
                }
                Page::Sysmon => {
                    self.sysmon.render(ui, &self.ctx, self.rt.handle());
                }
                Page::Monitor => {
                    self.monitor.render(ui, &self.ctx, self.rt.handle());
                }
                Page::Database => {
                    self.database.render(ui, &self.ctx, self.rt.handle());
                }
                Page::Workspace => {
                    self.workspace.render(ui, &self.ctx, self.rt.handle());
                }
                Page::Settings => {
                    self.settings.render(ui, &self.ctx, self.rt.handle());
                }
            });

        // 6. Startup dialogs (fallback notice + tools check)
        self.render_fallback_notice(&ctx);
        self.render_tools_check(&ctx);

        // 6b. Exit confirmation dialog (background monitoring running)
        if self.exit_confirm_open {
            let msg1 = rust_i18n::t!("shell.exit-confirm.msg1");
            let msg2 = rust_i18n::t!("shell.exit-confirm.msg2");
            egui::Window::new(rust_i18n::t!("shell.exit-confirm.title"))
                .collapsible(false)
                .resizable(false)
                .min_width(320.0)
                .show(&ctx, |ui| {
                    ui.label(msg1.as_ref());
                    ui.label(msg2.as_ref());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(rust_i18n::t!("shell.exit-confirm.background").as_ref()).clicked() {
                            self.exit_confirm_open = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }
                        if ui.button(rust_i18n::t!("shell.exit-confirm.quit").as_ref()).clicked() {
                            self.exit_confirm_open = false;
                            self.force_exit = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button(rust_i18n::t!("common.cancel").as_ref()).clicked() {
                            self.exit_confirm_open = false;
                        }
                    });
                });
        }

        // 7. Request periodic repaint for polling updates
        ctx.request_repaint_after(Duration::from_millis(1000));
    }

    fn on_exit(&mut self) {
        tracing::info!("egui frontend exiting");
        // 持久化 network 表格状态（列宽/密度/排序；P5 范式）
        self.network
            .save_table_state(&self.ctx.app_dirs.config_dir());
        // 重置后台模式，防止下次启动时状态不一致
        let ctx = self.ctx.clone();
        self.rt.block_on(async move {
            let svc = MonitorService { ctx: &ctx };
            let _ = svc.exit_background().await;
        });
    }
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
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok()
            && elevation.TokenIsElevated != 0;
        let _ = windows::Win32::Foundation::CloseHandle(token);
        result
    }
}

#[cfg(not(windows))]
fn is_running_as_admin() -> bool {
    false
}

/// 通过 FindWindowW 查找 IRtool 窗口并显示/激活。
/// 窗口被 Visible(false) 隐藏后，egui 的 update() 不被调用，
/// 因此托盘菜单事件需要直接用 Win32 API 操作窗口。
#[cfg(windows)]
fn show_and_activate_window() {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_SHOW};
    unsafe {
        if let Ok(hwnd) = FindWindowW(w!("winit-window-class-name"), w!("IRtool")) {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
fn show_and_activate_window() {}

/// 向 IRtool 窗口发送 WM_CLOSE 消息，触发 winit 关闭流程。
/// WM_CLOSE 是窗口消息，winit 会处理并触发 update()。
#[cfg(windows)]
fn post_close_to_window() {
    use windows::core::w;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE};
    unsafe {
        if let Ok(hwnd) = FindWindowW(w!("winit-window-class-name"), w!("IRtool")) {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM::default(), LPARAM::default());
        }
    }
}

#[cfg(not(windows))]
fn post_close_to_window() {}

/// 创建系统托盘图标和菜单。
/// 返回 (TrayIcon, 显示窗口菜单项ID, 退出菜单项ID)。
/// 注意：菜单文本取启动时语言（托盘菜单无法逐帧刷新）。
fn create_tray() -> (Option<tray_icon::TrayIcon>, muda::MenuId, muda::MenuId) {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let menu = Menu::new();
    let show_item = MenuItem::new(rust_i18n::t!("shell.tray.show").to_string(), true, None);
    let quit_item = MenuItem::new(rust_i18n::t!("shell.tray.quit").to_string(), true, None);
    let sep = PredefinedMenuItem::separator();
    let _ = menu.append(&show_item);
    let _ = menu.append(&sep);
    let _ = menu.append(&quit_item);

    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    // 加载图标（与窗口图标相同的 PNG）
    let icon_bytes = include_bytes!("../../irtool-tauri/icons/icon.png");
    let icon = image::load_from_memory(icon_bytes).ok().and_then(|img| {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Icon::from_rgba(rgba.to_vec(), w, h).ok()
    });

    let tooltip = format!("{} - {}", rust_i18n::t!("app.name"), rust_i18n::t!("app.tagline"));
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(tooltip);

    if let Some(icon) = icon {
        builder = builder.with_icon(icon);
    }

    let tray = builder.build().ok();

    (tray, show_id, quit_id)
}
