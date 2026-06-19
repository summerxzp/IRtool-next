use std::time::Duration;

use eframe::egui;
use irtool_service::context::AppContext;
use irtool_service::event_bus::AppEvent;
use irtool_service::services::autoruns::AutorunsService;
use irtool_service::types::AutorunItem;

use crate::event_bridge::EventBridge;
use crate::nav::Page;
use crate::pages::{autoruns::AutorunsPageState, network::NetworkPageState, placeholder};
use crate::theme;
use crate::StartupMode;

/// The main egui application struct.
pub struct IrtoolApp {
    // Infrastructure
    pub ctx: AppContext,
    pub rt: tokio::runtime::Runtime,
    pub event_bridge: EventBridge,

    // Navigation
    pub current_page: Page,

    // Global state
    pub is_admin: bool,
    pub is_fallback: bool,
    theme_applied: bool,

    // Per-page state
    pub network: NetworkPageState,
    pub autoruns: AutorunsPageState,

    // Async data refresh channels
    autoruns_refresh_tx: std::sync::mpsc::Sender<Vec<AutorunItem>>,
    autoruns_refresh_rx: std::sync::mpsc::Receiver<Vec<AutorunItem>>,
}

impl IrtoolApp {
    pub fn new(
        ctx: AppContext,
        event_bridge: EventBridge,
        rt: tokio::runtime::Runtime,
        mode: StartupMode,
    ) -> Self {
        let is_admin = is_running_as_admin();
        let (autoruns_refresh_tx, autoruns_refresh_rx) = std::sync::mpsc::channel::<Vec<AutorunItem>>();

        Self {
            ctx,
            rt,
            event_bridge,
            current_page: Page::Network,
            is_admin,
            is_fallback: mode == StartupMode::Fallback,
            theme_applied: false,
            network: NetworkPageState::default(),
            autoruns: AutorunsPageState::default(),
            autoruns_refresh_tx,
            autoruns_refresh_rx,
        }
    }

    fn handle_event(&mut self, event: AppEvent) {
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
            AppEvent::MonitorAlert(_) => {
                // TODO: handle in Monitor page
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
                self.rt.handle().spawn(async move {
                    let items = AutorunsService { ctx: &ctx_clone }
                        .get_result()
                        .await
                        .unwrap_or_default();
                    let _ = tx.send(items);
                });
            }
            AppEvent::AutorunsScanCancelled(task_id) => {
                self.autoruns.handle_scan_cancelled(task_id);
            }
            AppEvent::AutorunsScanFailed { task_id, error } => {
                self.autoruns.handle_scan_failed(task_id, error);
            }
            AppEvent::SysmonEvent(_) => {
                // TODO: handle in Sysmon page
            }
            AppEvent::PcapEvent(_) => {
                // TODO: handle in Pcap page
            }
            AppEvent::ToolsDownloadProgress { .. }
            | AppEvent::ToolsDownloadError { .. }
            | AppEvent::ToolsDownloadComplete { .. } => {
                // TODO: handle in Tools page
            }
            AppEvent::CloseRequested => {
                // TODO: handle close
            }
        }
    }
}

impl eframe::App for IrtoolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme on first update (when we have the real egui context)
        if !self.theme_applied {
            theme::apply_light_theme(ctx);
            self.theme_applied = true;
            self.event_bridge.attach_context(ctx.clone());
        }

        // 1. Drain events from EventBus
        for event in self.event_bridge.drain() {
            self.handle_event(event);
        }

        // 1b. Drain autoruns data refresh results
        while let Ok(items) = self.autoruns_refresh_rx.try_recv() {
            self.autoruns.items = items;
            self.autoruns.mark_cache_dirty();
        }

        // 2. Top bar
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            self.render_topbar(ui);
        });

        // 3. Sidebar
        egui::SidePanel::left("sidebar").show(ctx, |ui| {
            self.render_sidebar(ui);
        });

        // 4. Detail panel (right side, must be before CentralPanel)
        if self.current_page == Page::Network
            && self.network.detail_visible
            && self.network.selected_pid.is_some()
        {
            egui::SidePanel::right("detail_panel")
                .default_width(theme::DETAIL_PANEL_WIDTH)
                .resizable(true)
                .show(ctx, |ui| {
                    self.network.render_detail_panel(ui, &self.ctx, self.rt.handle());
                });
        } else if self.current_page == Page::Autoruns
            && self.autoruns.detail_visible
            && self.autoruns.selected_id.is_some()
        {
            egui::SidePanel::right("autoruns_detail_panel")
                .default_width(theme::DETAIL_PANEL_WIDTH)
                .resizable(true)
                .show(ctx, |ui| {
                    self.autoruns.render_detail_panel(ui, &self.ctx, self.rt.handle());
                });
        }

        // 4b. Stats bar (bottom) for pages that have one
        if matches!(self.current_page, Page::Network | Page::Autoruns) {
            egui::TopBottomPanel::bottom("stats_bar").show(ctx, |ui| {
                match self.current_page {
                    Page::Autoruns => self.autoruns.render_stats_bar(ui),
                    _ => { ui.label(""); }
                }
            });
        }

        // 5. Content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_page {
                Page::Network => {
                    self.network.render(ui, &self.ctx, self.rt.handle());
                }
                Page::Autoruns => {
                    self.autoruns.render(ui, &self.ctx, self.rt.handle());
                }
                other => {
                    placeholder::render_placeholder(ui, other);
                }
            }
        });

        // 6. Request periodic repaint for polling updates
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        tracing::info!("egui frontend exiting");
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
