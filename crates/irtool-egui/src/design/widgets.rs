//! 组件公共样式（spec §4）——吃令牌的通用 widget 集。
//!
//! 约束：函数体内禁止出现任何硬编码色值（spec §6-1），全部颜色经参数或
//! [`Palette`](super::tokens::Palette) 传入；尺寸/圆角常量不属于色值，允许内置。

use super::fonts;
use super::tokens::Palette;
use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, Sense, StrokeKind, Vec2};

/// 软色徽章（默认 API，spec §4 Badge）：bg 软底 + fg 文字 + border 软边，
/// 圆角 5 / 高 20 / caption 字号。颜色取 `role.{x}` 三件套。
pub fn badge(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32, border: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        fonts::caption(),
        fg,
    );
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 16.0, 20.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(5), bg);
    p.rect_stroke(rect, CornerRadius::same(5), egui::Stroke::new(1.0, border), StrokeKind::Inside);
    p.galley(
        egui::Pos2::new(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    resp
}

/// 实色徽章（强调变体，spec §2.2 用法约定）：fill（= role.fg）底 + on 色文字，
/// 仅用于 critical/danger 的强提醒。
pub fn badge_solid(ui: &mut egui::Ui, text: &str, fill: Color32, on: Color32) -> egui::Response {
    badge(ui, text, on, fill, Color32::TRANSPARENT)
}

/// Chip（状态点 + 文字，spec §4 Chip）：7px 圆点（tint=role.fg）+ label（fg_secondary 档，
/// 显式传入）。value 非空时以 tint 色附后（状态栏统计场景）。
pub fn chip(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    tint: Color32,
    label_col: Color32,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 3.5, tint);
        ui.label(egui::RichText::new(label).size(fonts::size::TABLE).color(label_col));
        if !value.is_empty() {
            ui.label(
                egui::RichText::new(value)
                    .size(fonts::size::TABLE)
                    .strong()
                    .color(tint),
            );
        }
        ui.add_space(6.0);
        resp
    }).inner
}

/// 统计条 count chip（同 demo status_bars）：label + 强调 value（tint），5px 固定间距。
pub fn count_chip(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    tint: Color32,
    label_col: Color32,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(egui::RichText::new(label).size(fonts::size::TABLE).color(label_col));
        ui.label(
            egui::RichText::new(value)
                .size(fonts::size::TABLE)
                .strong()
                .color(tint),
        );
    });
    ui.add_space(8.0);
}

/// 空态（spec §4 EmptyState）：44px 图标位（icon_text 字符充当，fg_tertiary 60%）
/// + 主文案（section/strong/fg_secondary）+ 引导文案（body/fg_tertiary），
/// 回答"为什么空 + 下一步"。
pub fn empty_state(ui: &mut egui::Ui, pal: &Palette, icon_text: &str, title: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
        let icon_col = pal.fg_tertiary.gamma_multiply(0.6);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon_text,
            FontId::proportional(36.0),
            icon_col,
        );
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(title)
                .font(fonts::section())
                .strong()
                .color(pal.fg_secondary),
        );
        ui.label(
            egui::RichText::new(hint)
                .font(fonts::body())
                .color(pal.fg_tertiary),
        );
    });
}

/// 面板 Frame（spec §4 各弹层/面板公共底）：elev-1 底 + border 描边 + 圆角 6。
pub fn panel_frame(pal: &Palette, margin: impl Into<Margin>) -> egui::Frame {
    egui::Frame::default()
        .fill(pal.bg_elev1)
        .stroke(egui::Stroke::new(1.0, pal.border))
        .corner_radius(6.0)
        .inner_margin(margin)
}

/// 全宽 1px 分隔线。
pub fn separator(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
}
