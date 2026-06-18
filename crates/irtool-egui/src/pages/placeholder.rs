use eframe::egui;

use crate::nav::Page;
use crate::theme;

/// Placeholder page for features not yet implemented.
pub fn render_placeholder(ui: &mut egui::Ui, page: Page) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.label(
            egui::RichText::new(page.icon())
                .size(48.0),
        );
        ui.add_space(16.0);
        ui.label(
            egui::RichText::new(page.label())
                .size(24.0)
                .color(theme::FG_PRIMARY),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("即将推出")
                .size(16.0)
                .color(theme::FG_SECONDARY),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("此页面尚未在 egui 前端中实现")
                .size(12.0)
                .color(theme::FG_TERTIARY),
        );
    });
}
