use eframe::egui::{self, Color32};

// ── Time formatting (UTC+8, DESIGN.md 4.8) ──────────────────

/// Format an epoch (seconds) as UTC+8 string: `2026/06/19,06:21:47`.
pub fn fmt_time(epoch: u64) -> String {
    if epoch == 0 {
        return "-".to_string();
    }
    let cst = chrono::FixedOffset::east_opt(8 * 3600).unwrap(); // UTC+8
    chrono::DateTime::from_timestamp(epoch as i64, 0)
        .map(|dt| dt.with_timezone(&cst).format("%Y/%m/%d,%H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string())
}

/// Format epoch milliseconds as UTC+8 string: `2026/06/19,06:21:47`.
pub fn fmt_time_millis(millis: i64) -> String {
    let secs = millis / 1000;
    let cst = chrono::FixedOffset::east_opt(8 * 3600).unwrap(); // UTC+8
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.with_timezone(&cst).format("%Y/%m/%d,%H:%M:%S").to_string())
        .unwrap_or_else(|| millis.to_string())
}

/// Format bytes as human-readable string: `1.5 GB`.
pub fn fmt_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let i = (bytes.ilog2() / 10) as usize; // 2^10 = 1024
    let i = i.min(UNITS.len() - 1);
    let val = bytes as f64 / (1024_u64.pow(i as u32) as f64);
    let val_str = if val >= 100.0 {
        format!("{:.0}", val)
    } else {
        format!("{:.1}", val)
    };
    format!("{} {}", val_str, UNITS[i])
}

/// Format uptime from epoch-millis start time: `1天2时`, `3分4秒`, etc.
pub fn fmt_uptime(started_at_millis: Option<i64>) -> String {
    let Some(started) = started_at_millis else {
        return "-".to_string();
    };
    if started <= 0 {
        return "-".to_string();
    }
    let now = chrono::Utc::now().timestamp_millis();
    let elapsed = (now - started).max(0) / 1000;
    let days = elapsed / 86400;
    let hours = (elapsed % 86400) / 3600;
    let mins = (elapsed % 3600) / 60;
    let secs = elapsed % 60;
    if days > 0 {
        format!("{}天{}时", days, hours)
    } else if hours > 0 {
        format!("{}时{}分", hours, mins)
    } else if mins > 0 {
        format!("{}分{}秒", mins, secs)
    } else {
        format!("{}秒", secs)
    }
}

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
pub const DETAIL_PANEL_HEIGHT: f32 = 220.0;
pub const PANEL_PADDING: f32 = 12.0;
pub const ELEMENT_GAP: f32 = 8.0;
pub const TABLE_ROW_HEIGHT: f32 = 22.0;
pub const TABLE_HEADER_HEIGHT: f32 = 24.0;

/// Apply the light theme and CJK font to an egui context.
pub fn apply_light_theme(ctx: &egui::Context) {
    // ── Scale up for high-DPI displays ──
    // egui defaults to 1.0x which looks tiny on high-resolution screens.
    // Use 1.15x as a comfortable baseline; users can Ctrl+/- to adjust.
    let scale = ctx.pixels_per_point();
    if scale <= 1.0 {
        ctx.set_pixels_per_point(1.15);
    }

    // ── Load CJK font ──
    // Try multiple candidate paths so Chinese renders on N-edition Windows
    // or systems where msyh.ttc is unavailable.
    let font_candidates = [
        r"C:\Windows\Fonts\msyh.ttc",     // Microsoft YaHei
        r"C:\Windows\Fonts\msyhbd.ttc",   // Microsoft YaHei Bold
        r"C:\Windows\Fonts\simsun.ttc",   // SimSun
        r"C:\Windows\Fonts\msjh.ttc",     // Microsoft JhengHei
        r"C:\Windows\Fonts\msgothic.ttc", // MS Gothic
    ];
    let font_data = font_candidates
        .iter()
        .find_map(|path| std::fs::read(path).ok().map(|data| (path, data)));
    if let Some((_font_path, font_data)) = font_data {
        let mut fonts = egui::FontDefinitions::default();

        fonts.font_data.insert(
            "msyh".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(font_data).tweak(egui::FontTweak {
                scale: 1.0,
                ..Default::default()
            })),
        );

        // Insert msyh at the front of Proportional so that symbol characters
        // (✓ ✕ ⚠ etc.) are rendered by msyh instead of Hack, which may lack
        // proper glyphs for these codepoints and show tofu boxes.
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "msyh".to_owned());

        // Add as fallback for monospace text (keep Hack primary for code)
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("msyh".to_owned());

        ctx.set_fonts(fonts);
    } else {
        tracing::warn!(
            "No CJK font found in candidates {:?}, Chinese text may not render correctly",
            font_candidates
        );
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

    // ── Disable selectable labels globally (DESIGN.md 4.3 Label Convention) ──
    // All ui.label() calls should not enter text-selection mode.
    ctx.options_mut(|o| {
        std::sync::Arc::make_mut(&mut o.dark_style)
            .interaction
            .selectable_labels = false;
        std::sync::Arc::make_mut(&mut o.light_style)
            .interaction
            .selectable_labels = false;
    });
}
