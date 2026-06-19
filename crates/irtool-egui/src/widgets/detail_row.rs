use eframe::egui;

use crate::theme;

/// Detail row with click-to-copy value (DESIGN.md 4.4).
pub fn detail_row(ui: &mut egui::Ui, label: &str, value: Option<&str>, mono: bool) {
    let Some(value) = value else { return };
    if value.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.set_min_width(70.0);
        ui.label(egui::RichText::new(label).color(theme::FG_TERTIARY).size(11.0));
    });
    ui.horizontal(|ui| {
        let text = egui::RichText::new(value).size(11.0).color(theme::FG_PRIMARY);
        let text = if mono {
            text.font(egui::FontId::monospace(11.0))
        } else {
            text
        };
        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
        let hovered = resp.hovered();
        let clicked = resp.clicked();
        if hovered {
            resp.on_hover_text("点击复制");
        }
        if clicked {
            ui.ctx().copy_text(value.to_string());
        }
        if hovered {
            ui.label(egui::RichText::new("复制").size(10.0).color(theme::FG_TERTIARY));
        }
    });
    ui.add_space(2.0);
}
