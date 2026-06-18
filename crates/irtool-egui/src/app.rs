use std::time::Duration;

use eframe::egui;
use irtool_service::context::AppContext;
use irtool_service::event_bus::AppEvent;

use crate::event_bridge::EventBridge;
use crate::nav::Page;
use crate::pages::{network::NetworkPageState, placeholder};
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
}

impl IrtoolApp {
    pub fn new(
        ctx: AppContext,
        event_bridge: EventBridge,
        rt: tokio::runtime::Runtime,
        mode: StartupMode,
    ) -> Self {
        let is_admin = is_running_as_admin();

        Self {
            ctx,
            rt,
            event_bridge,
            current_page: Page::Network,
            is_admin,
            is_fallback: mode == StartupMode::Fallback,
            theme_applied: false,
            network: NetworkPageState::default(),
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
            AppEvent::AutorunsProgress(_)
            | AppEvent::AutorunsSignatureProgress(_)
            | AppEvent::AutorunsHashProgress(_)
            | AppEvent::AutorunsScanComplete { .. }
            | AppEvent::AutorunsScanCancelled(_)
            | AppEvent::AutorunsScanFailed { .. } => {
                // TODO: handle in Autoruns page
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
        }

        // 1. Drain events from EventBus
        for event in self.event_bridge.drain() {
            self.handle_event(event);
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
        }

        // 5. Content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_page {
                Page::Network => {
                    self.network.render(ui, &self.ctx, self.rt.handle());
                }
                other => {
                    placeholder::render_placeholder(ui, other);
                }
            }
        });

        // 6. Request periodic repaint for polling updates
        ctx.request_repaint_after(Duration::from_millis(100));
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
