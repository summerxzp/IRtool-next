use eframe::egui;

use crate::theme;

/// Sort direction.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn toggle(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            SortDir::Asc => " ▲",
            SortDir::Desc => " ▼",
        }
    }
}

/// A sortable column header for `egui_extras::TableBuilder`.
///
/// Returns `Some(column_id)` if the user clicked the header (to toggle sort).
pub fn sortable_header(ui: &mut egui::Ui, label: &str, is_sorted: bool, dir: SortDir) -> bool {
    let text = if is_sorted {
        format!("{}{}", label, dir.arrow())
    } else {
        label.to_string()
    };

    let font = if is_sorted {
        egui::FontId::proportional(13.0)
    } else {
        egui::FontId::proportional(12.0)
    };

    let color = if is_sorted { theme::ACCENT } else { theme::FG_SECONDARY };

    let response =
        ui.add(egui::Label::new(egui::RichText::new(text).font(font).color(color)).sense(egui::Sense::click()));

    response.clicked()
}
