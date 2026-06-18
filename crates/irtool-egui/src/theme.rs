use eframe::egui::{self, Color32};

// ── Background (Light Theme) ────────────────────────────────
pub const BG_PRIMARY: Color32 = Color32::WHITE;
pub const BG_SECONDARY: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf5);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(0xea, 0xea, 0xea);

// ── Foreground ──────────────────────────────────────────────
pub const FG_PRIMARY: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x2e);
pub const FG_SECONDARY: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
pub const FG_TERTIARY: Color32 = Color32::from_rgb(0x99, 0x99, 0x99);

// ── Accent ──────────────────────────────────────────────────
pub const ACCENT: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x1d, 0x4e, 0xd8);

// ── Semantic ────────────────────────────────────────────────
pub const SEMANTIC_SUCCESS: Color32 = Color32::from_rgb(0x16, 0xa3, 0x4a);
pub const SEMANTIC_INFO: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
pub const SEMANTIC_WARNING: Color32 = Color32::from_rgb(0xca, 0x8a, 0x04);
pub const SEMANTIC_DANGER: Color32 = Color32::from_rgb(0xdc, 0x26, 0x26);
pub const SEMANTIC_DEFAULT: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);

// ── Table ───────────────────────────────────────────────────
#[allow(dead_code)]
pub const TABLE_HEADER_BG: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf0);
pub const TABLE_ROW_SELECTED: Color32 = Color32::from_rgba_premultiplied(0x25, 0x63, 0xeb, 30);

// ── Layout ──────────────────────────────────────────────────
pub const TOPBAR_HEIGHT: f32 = 32.0;
pub const SIDEBAR_WIDTH: f32 = 180.0;
pub const DETAIL_PANEL_WIDTH: f32 = 300.0;
#[allow(dead_code)]
pub const PANEL_PADDING: f32 = 12.0;
#[allow(dead_code)]
pub const ELEMENT_GAP: f32 = 8.0;
pub const TABLE_ROW_HEIGHT: f32 = 22.0;
pub const TABLE_HEADER_HEIGHT: f32 = 24.0;

/// Apply the light theme and CJK font to an egui context.
pub fn apply_light_theme(ctx: &egui::Context) {
    // ── Load CJK font (Microsoft YaHei) ──
    let font_path = std::path::Path::new(r"C:\Windows\Fonts\msyh.ttc");
    if let Ok(font_data) = std::fs::read(font_path) {
        let mut fonts = egui::FontDefinitions::default();

        fonts.font_data.insert(
            "msyh".to_owned(),
            std::sync::Arc::new(
                egui::FontData::from_owned(font_data).tweak(egui::FontTweak {
                    scale: 0.88,
                    ..Default::default()
                }),
            ),
        );

        // Add as fallback for proportional text
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("msyh".to_owned());

        // Add as fallback for monospace text
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("msyh".to_owned());

        ctx.set_fonts(fonts);
    } else {
        tracing::warn!("CJK font msyh.ttc not found, Chinese text may not render correctly");
    }

    // ── Light visuals ──
    let mut visuals = egui::Visuals::light();

    visuals.override_text_color = Some(FG_PRIMARY);
    visuals.widgets.noninteractive.bg_fill = BG_PRIMARY;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, FG_PRIMARY);
    visuals.widgets.inactive.bg_fill = BG_SECONDARY;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, FG_PRIMARY);
    visuals.widgets.hovered.bg_fill = BG_ELEVATED;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, ACCENT_HOVER);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);

    visuals.panel_fill = BG_PRIMARY;
    visuals.window_fill = BG_PRIMARY;
    visuals.extreme_bg_color = BG_SECONDARY;
    visuals.faint_bg_color = Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 3);

    visuals.selection.bg_fill = TABLE_ROW_SELECTED;

    ctx.set_visuals(visuals);
}
