use eframe::egui::{self, Color32, Vec2};

use crate::theme;

/// Semantic badge variant.
pub enum BadgeVariant {
    Success,
    Info,
    Warning,
    Danger,
    Default,
}

impl BadgeVariant {
    fn color(&self) -> Color32 {
        match self {
            BadgeVariant::Success => theme::semantic_success(),
            BadgeVariant::Info => theme::semantic_info(),
            BadgeVariant::Warning => theme::semantic_warning(),
            BadgeVariant::Danger => theme::semantic_danger(),
            BadgeVariant::Default => theme::semantic_default(),
        }
    }
}

/// Render a small colored badge with text.
pub fn badge(ui: &mut egui::Ui, text: &str, variant: BadgeVariant) {
    let bg = variant.color();
    let fg = Color32::WHITE;
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(text.to_uppercase(), font, fg));
    let size = galley.size() + Vec2::new(8.0, 4.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, bg);
    ui.painter().galley(rect.min + Vec2::new(4.0, 2.0), galley, fg);
}
