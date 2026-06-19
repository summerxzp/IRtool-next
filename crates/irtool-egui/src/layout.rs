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
            ui.spacing_mut().item_spacing.x = theme::PANEL_PADDING;

            // Logo + version
            self.ui_label(
                ui,
                egui::RichText::new("IRtool").strong().size(16.0).color(theme::ACCENT),
            );
            self.ui_label(
                ui,
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
            self.ui_label(ui, egui::RichText::new(admin_text).size(11.0).color(admin_color));

            ui.separator();

            // Fallback mode badge
            if self.is_fallback {
                crate::widgets::badge::badge(ui, "降级模式", crate::widgets::badge::BadgeVariant::Warning);
                ui.separator();
            }

            // Exit button (right-aligned)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("× 退出").size(11.0).color(theme::FG_SECONDARY),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    self.request_exit();
                }
            });

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
            let fg = if is_active { theme::ACCENT } else { theme::FG_SECONDARY };

            let resp = ui
                .allocate_ui(egui::vec2(ui.available_width(), 32.0), |ui| {
                    let frame = egui::Frame::new()
                        .fill(bg)
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(8, 4));
                    frame.show(ui, |ui| {
                        let label = format!("{} {}", page.icon(), page.label())
                            .trim_start()
                            .to_string();

                        ui.add(
                            egui::Label::new(egui::RichText::new(label).size(13.0).color(fg))
                                .sense(egui::Sense::click()),
                        )
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
