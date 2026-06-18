use eframe::egui;

use crate::app::IrtoolApp;
use crate::nav::Page;
use crate::theme;

impl IrtoolApp {
    /// Non-selectable label helper.
    fn ui_label(&self, ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
        ui.add(egui::Label::new(text).selectable(false))
    }

    /// Render the top bar with app info and global status.
    pub fn render_topbar(&mut self, ui: &mut egui::Ui) {
        ui.set_height(theme::TOPBAR_HEIGHT);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            // Logo + version
            self.ui_label(ui,
                egui::RichText::new("IRtool")
                    .strong()
                    .size(16.0)
                    .color(theme::ACCENT),
            );
            self.ui_label(ui,
                egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(11.0)
                    .color(theme::FG_TERTIARY),
            );

            ui.separator();

            // Admin badge
            let admin_text = if self.is_admin { "管理员" } else { "普通用户" };
            let admin_color = if self.is_admin {
                theme::SEMANTIC_SUCCESS
            } else {
                theme::SEMANTIC_WARNING
            };
            self.ui_label(ui,
                egui::RichText::new(admin_text)
                    .size(11.0)
                    .color(admin_color),
            );

            ui.separator();

            // Fallback mode badge
            if self.is_fallback {
                crate::widgets::badge::badge(ui, "降级模式", crate::widgets::badge::BadgeVariant::Warning);
                ui.separator();
            }

            // Polling status — clickable to toggle pause/resume
            let polling_status = if self.network.paused {
                "⏸ 已暂停"
            } else {
                "▶ 轮询中"
            };
            let polling_color = if self.network.paused {
                theme::SEMANTIC_WARNING
            } else {
                theme::SEMANTIC_SUCCESS
            };
            let polling_resp = ui.add(
                egui::Label::new(
                    egui::RichText::new(polling_status)
                        .size(11.0)
                        .color(polling_color),
                )
                .sense(egui::Sense::click()),
            );
            if polling_resp.clicked() {
                self.network.paused = !self.network.paused;
                // Also update backend
                let svc_ctx = self.ctx.clone();
                let paused = self.network.paused;
                let interval = self.network.interval_ms;
                self.rt.spawn(async move {
                    let svc = irtool_service::services::network::NetworkService { ctx: &svc_ctx };
                    let _ = svc
                        .set_polling(irtool_service::dto::network::NetworkPollingControl {
                            interval_ms: Some(interval),
                            paused: Some(paused),
                            retention: None,
                        })
                        .await;
                });
            }

            // Connection count (right-aligned)
            if let Some(ref snap) = self.network.snapshot {
                let total = snap.items.len();
                let history_count = snap.items.iter().filter(|c| !c.is_current).count();
                let count_text = if history_count > 0 {
                    format!("{} 连接 (📜 {} 历史)", total, history_count)
                } else {
                    format!("{} 连接", total)
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.ui_label(ui,
                        egui::RichText::new(count_text)
                            .size(11.0)
                            .color(theme::FG_TERTIARY),
                    );
                });
            }
        });
    }

    /// Render the sidebar with navigation items.
    pub fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.set_width(theme::SIDEBAR_WIDTH);
        ui.add_space(8.0);

        for page in Page::ALL {
            let is_active = self.current_page == page;
            let bg = if is_active {
                theme::BG_ELEVATED
            } else {
                egui::Color32::TRANSPARENT
            };
            let fg = if is_active {
                theme::ACCENT
            } else {
                theme::FG_SECONDARY
            };

            let resp = ui
                .allocate_ui(egui::vec2(ui.available_width(), 32.0), |ui| {
                    let frame = egui::Frame::new()
                        .fill(bg)
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(8, 4));
                    frame.show(ui, |ui| {
                        let label = format!("{} {}", page.icon(), page.label());
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(label).size(13.0).color(fg),
                            )
                            .sense(egui::Sense::click()),
                        );
                        resp
                    })
                })
                .inner
                .inner;

            if resp.clicked() {
                self.current_page = page;
            }

            // Hover effect
            if resp.hovered() && !is_active {
                // Could add a repaint request here if needed
            }

            ui.add_space(2.0);
        }
    }
}
