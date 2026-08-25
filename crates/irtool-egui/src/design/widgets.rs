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

// ── 工具栏组件（P5 视觉返工：移植 demo ui/mod.rs 的胶囊按钮组语言）────────

use super::icon::{self, Icon};
use eframe::egui::{Order, Pos2, Stroke};

/// 胶囊按钮（spec §4 Button）：30px 高、圆角 6、可选图标 + 文字。
/// `rest` 常态底色（主按钮传 accent、危险按钮传 danger.fg、描边按钮传 surface），
/// `outline` 描边色（描边按钮传 border，实底按钮传 None）。
pub fn flat_button(
    ui: &mut egui::Ui,
    icon: Option<Icon>,
    label: &str,
    rest: Color32,
    hovered: Color32,
    fg: Color32,
    outline: Option<Color32>,
    enabled: bool,
) -> egui::Response {
    let font = FontId::proportional(12.5);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, fg);
    let icon_w = if icon.is_some() { 14.0 + 6.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + icon_w + 24.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let p = ui.painter();
    let fill = if !enabled {
        rest.gamma_multiply(0.45)
    } else if resp.hovered() {
        hovered
    } else {
        rest
    };
    p.rect_filled(rect, CornerRadius::same(6), fill);
    if let Some(oc) = outline {
        p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, oc), StrokeKind::Inside);
    }

    let mut tx = rect.left() + 12.0;
    if let Some(ic) = icon {
        icon::draw(
            ui,
            ic,
            Pos2::new(tx + 7.0, rect.center().y),
            14.0,
            if enabled { fg } else { fg.gamma_multiply(0.5) },
        );
        tx += icon_w;
    }
    ui.painter()
        .galley(Pos2::new(tx, rect.center().y - galley.size().y / 2.0), galley, fg);
    // 自绘控件注册无障碍 label（kittest/读屏按 label 寻址）。
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// 自绘下拉（demo dropdown）：30px 描边胶囊 + ChevronDown，弹层 Area + 阴影。
/// `slot` 为互斥开合槽（同屏只开一个下拉），调用方持有 `open: &mut Option<u8>`。
pub fn dropdown<S: AsRef<str>>(
    ui: &mut egui::Ui,
    id: &'static str,
    slot: u8,
    open: &mut Option<u8>,
    model: &[S],
    current: usize,
    width: f32,
    pal: &Palette,
) -> Option<usize> {
    let font = FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(model[current].as_ref().to_string(), font, pal.fg_primary);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    let is_open = *open == Some(slot);

    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(6), pal.bg_elev1);
    let bc = if resp.hovered() || is_open { pal.border_strong } else { pal.border };
    p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, bc), StrokeKind::Inside);
    p.galley(
        Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        pal.fg_primary,
    );
    icon::draw(
        ui,
        Icon::ChevronDown,
        Pos2::new(rect.right() - 16.5, rect.center().y),
        13.0,
        if resp.hovered() { pal.fg_primary } else { pal.fg_tertiary },
    );

    let clicked = resp.clicked();
    if clicked {
        *open = if is_open { None } else { Some(slot) };
    }

    let mut selected = None;
    if *open == Some(slot) {
        selected = show_menu(ui.ctx(), id, rect, model, current, pal);
        if selected.is_some() {
            *open = None;
        } else if !clicked && ui.ctx().input(|i| i.pointer.any_click()) {
            *open = None;
        }
    }
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, model[current].as_ref().to_string())
    });
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    selected
}

fn show_menu<S: AsRef<str>>(
    ctx: &egui::Context,
    id: &'static str,
    btn: egui::Rect,
    model: &[S],
    current: usize,
    pal: &Palette,
) -> Option<usize> {
    let mut selected = None;
    egui::Area::new(egui::Id::new(id))
        .order(Order::Foreground)
        .fixed_pos(Pos2::new(btn.left(), btn.bottom() + 7.0))
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(pal.bg_elev1)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(5))
                .shadow(egui::Shadow {
                    offset: [0, 4],
                    blur: 14,
                    spread: 0,
                    color: Color32::from_black_alpha(40),
                })
                .show(ui, |ui| {
                    ui.set_min_width(btn.width() + 20.0);
                    for (i, item) in model.iter().enumerate() {
                        let sel = i == current;
                        let (row, rresp) =
                            ui.allocate_exact_size(Vec2::new(ui.available_width(), 27.0), Sense::click());
                        let pr = ui.painter_at(row);
                        if rresp.hovered() {
                            pr.rect_filled(row, CornerRadius::same(5), pal.hover);
                        }
                        let fg = if sel { pal.accent } else { pal.fg_primary };
                        let galley =
                            pr.layout_no_wrap(item.as_ref().to_string(), FontId::proportional(12.0), fg);
                        pr.galley(
                            Pos2::new(row.left() + 9.0, row.center().y - galley.size().y / 2.0),
                            galley,
                            fg,
                        );
                        if sel {
                            icon::draw(
                                ui,
                                Icon::Check,
                                Pos2::new(row.right() - 15.0, row.center().y),
                                12.0,
                                pal.accent,
                            );
                        }
                        if rresp.clicked() {
                            selected = Some(i);
                        }
                    }
                });
        });
    selected
}

/// 图标钮（30×30，radius 7，hover 软底，tooltip + 手型光标）。
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tip: &str, active: bool, pal: &Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let p = ui.painter();
    if active || (resp.hovered() && resp.enabled()) {
        p.rect_filled(rect, CornerRadius::same(7), if active { pal.selected.bg } else { pal.hover });
    }
    let col = if active {
        pal.accent
    } else if resp.hovered() {
        pal.fg_primary
    } else {
        pal.fg_secondary
    };
    icon::draw(ui, icon, rect.center(), 15.0, col);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// 竖分隔线（工具栏按钮组分隔）。
pub fn vsep(ui: &mut egui::Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 20.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, pal.border);
}
