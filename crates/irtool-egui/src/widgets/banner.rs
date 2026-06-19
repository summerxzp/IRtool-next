use eframe::egui;

use crate::theme;

/// Render an error banner. Returns true if the close button was clicked.
///
/// Usage:
/// ```ignore
/// if let Some(ref err) = self.last_error {
///     let err = err.clone();
///     if crate::widgets::banner::error_banner(ui, &err) {
///         self.last_error = None;
///     }
/// }
/// ui.add_space(4.0);
/// ```
pub fn error_banner(ui: &mut egui::Ui, error: &str) -> bool {
    let mut close_clicked = false;
    egui::Frame::new()
        .fill(theme::SEMANTIC_DANGER.linear_multiply(0.2))
        .inner_margin(theme::ELEMENT_GAP)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("错误: {}", error))
                        .color(theme::SEMANTIC_DANGER)
                        .size(12.0),
                );
                ui.add_space((ui.available_width() - 20.0).max(0.0));
                if ui.small_button("×").clicked() {
                    close_clicked = true;
                }
            });
        });
    close_clicked
}
