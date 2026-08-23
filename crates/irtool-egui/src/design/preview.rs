//! 设计样板（design board）：渲染全部令牌与组件示例，用于两仓库对照验证与将来回归。
//! 不接入任何业务页面；demo 仓库有视觉结构相同的镜像实现（egui-demo/src/board.rs）。

use super::fonts;
use super::tokens::{Palette, RoleColors};
use super::widgets;
use eframe::egui::{self, Sense, Vec2};

/// 渲染设计样板：中性色板 / 角色三件套 / 组件示例 / 字号阶梯。
pub fn design_board(ui: &mut egui::Ui, pal: &Palette) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.add_space(18.0);
            // 标题
            ui.label(
                egui::RichText::new("IRtool Design Board")
                    .font(fonts::display())
                    .strong()
                    .color(pal.fg_primary),
            );
            ui.label(
                egui::RichText::new(if pal.dark {
                    "theme: dark · 基准 ui/src/styles/tokens.css（默认块）"
                } else {
                    "theme: light · 基准 ui/src/styles/tokens.css（data-theme=light 块）"
                })
                .font(fonts::mono_caption())
                .color(pal.fg_tertiary),
            );
            ui.add_space(14.0);

            neutral_swatches(ui, pal);
            ui.add_space(12.0);
            role_swatches(ui, pal);
            ui.add_space(12.0);
            widget_samples(ui, pal);
            ui.add_space(12.0);
            type_ladder(ui, pal);
            ui.add_space(24.0);
        });
}

fn section_title(ui: &mut egui::Ui, pal: &Palette, text: &str, note: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(text)
                .font(fonts::section())
                .strong()
                .color(pal.fg_primary),
        );
        ui.label(egui::RichText::new(note).font(fonts::caption()).color(pal.fg_tertiary));
    });
    ui.add_space(8.0);
}

/// 色块亮暗判断（用于 swatch 内标注色的选择；两色均取自 Palette，非硬编码）。
fn pick_label_col(pal: &Palette, c: egui::Color32) -> egui::Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 140.0 {
        pal.fg_primary
    } else {
        pal.on_accent
    }
}

fn swatch(ui: &mut egui::Ui, pal: &Palette, name: &str, color: egui::Color32, ext: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(104.0, 48.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, color);
    p.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, pal.border), egui::StrokeKind::Inside);
    let col = pick_label_col(pal, color);
    let hex = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
    let tag = if ext { "·ext" } else { "" };
    p.text(
        rect.left_bottom() + egui::Vec2::new(5.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        hex,
        fonts::mono_caption(),
        col,
    );
    p.text(
        rect.left_top() + egui::Vec2::new(5.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!("{name}{tag}"),
        fonts::caption(),
        col,
    );
    ui.add_space(6.0);
}

fn neutral_swatches(ui: &mut egui::Ui, pal: &Palette) {
    widgets::panel_frame(pal, 16).show(ui, |ui| {
        section_title(ui, pal, "中性阶梯", "tokens.css · ext=demo 扩展保留值");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            swatch(ui, pal, "bg-base", pal.bg_base, false);
            swatch(ui, pal, "bg-elev1", pal.bg_elev1, false);
            swatch(ui, pal, "bg-elev2", pal.bg_elev2, false);
            swatch(ui, pal, "border", pal.border, false);
            swatch(ui, pal, "fg-primary", pal.fg_primary, false);
            swatch(ui, pal, "fg-secondary", pal.fg_secondary, false);
            swatch(ui, pal, "fg-tertiary", pal.fg_tertiary, false);
            swatch(ui, pal, "accent", pal.accent, false);
            swatch(ui, pal, "rail", pal.rail, true);
            swatch(ui, pal, "hover", pal.hover, true);
            swatch(ui, pal, "border-strong", pal.border_strong, true);
            swatch(ui, pal, "row-line", pal.row_line, true);
        });
    });
}

fn role_rows(ui: &mut egui::Ui, pal: &Palette, name: &str, r: &RoleColors, sample: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(86.0, 18.0), Sense::hover());
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            name,
            fonts::table(),
            pal.fg_secondary,
        );
        mini_swatch(ui, pal, "fg", r.fg);
        mini_swatch(ui, pal, "bg", r.bg);
        mini_swatch(ui, pal, "br", r.border);
        ui.add_space(10.0);
        widgets::badge(ui, sample, r.fg, r.bg, r.border);
        if name == "critical" || name == "danger" {
            ui.add_space(6.0);
            widgets::badge_solid(ui, sample, r.fg, pal.on_accent);
        }
    });
    ui.add_space(4.0);
}

fn mini_swatch(ui: &mut egui::Ui, pal: &Palette, tag: &str, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(52.0, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, color);
    p.rect_stroke(rect, 3.0, egui::Stroke::new(1.0, pal.border), egui::StrokeKind::Inside);
    p.text(
        rect.right_center() + egui::Vec2::new(-3.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        tag,
        fonts::caption(),
        pick_label_col(pal, color),
    );
    ui.add_space(4.0);
}

fn role_swatches(ui: &mut egui::Ui, pal: &Palette) {
    widgets::panel_frame(pal, 16).show(ui, |ui| {
        section_title(ui, pal, "角色三件套", "fg · bg=12% · border=25% · 软/实色徽章");
        role_rows(ui, pal, "critical", &pal.critical, "CRITICAL");
        role_rows(ui, pal, "danger", &pal.danger, "CLOSE_WAIT");
        role_rows(ui, pal, "warning", &pal.warning, "TIME_WAIT");
        role_rows(ui, pal, "success", &pal.success, "ESTABLISHED");
        role_rows(ui, pal, "info", &pal.info, "LISTEN");
        role_rows(ui, pal, "neutral", &pal.neutral, "UDP -");
        role_rows(ui, pal, "dim", &pal.dim, "已结束");
        role_rows(ui, pal, "selected", &pal.selected, "SELECTED");
    });
}

fn widget_samples(ui: &mut egui::Ui, pal: &Palette) {
    widgets::panel_frame(pal, 16).show(ui, |ui| {
        section_title(ui, pal, "组件", "badge 两态 / chip / count_chip / empty_state");

        ui.label(
            egui::RichText::new("badge · 软色（默认）")
                .font(fonts::caption())
                .color(pal.fg_tertiary),
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            widgets::badge(ui, "CRITICAL", pal.critical.fg, pal.critical.bg, pal.critical.border);
            widgets::badge(ui, "CLOSE_WAIT", pal.danger.fg, pal.danger.bg, pal.danger.border);
            widgets::badge(ui, "TIME_WAIT", pal.warning.fg, pal.warning.bg, pal.warning.border);
            widgets::badge(ui, "ESTABLISHED", pal.success.fg, pal.success.bg, pal.success.border);
            widgets::badge(ui, "LISTEN", pal.info.fg, pal.info.bg, pal.info.border);
            widgets::badge(ui, "UDP -", pal.neutral.fg, pal.neutral.bg, pal.neutral.border);
            widgets::badge(ui, "已结束", pal.dim.fg, pal.dim.bg, pal.dim.border);
            widgets::badge(ui, "SELECTED", pal.selected.fg, pal.selected.bg, pal.selected.border);
        });
        ui.add_space(6.0);

        ui.label(
            egui::RichText::new("badge · 实色（仅 critical/danger）")
                .font(fonts::caption())
                .color(pal.fg_tertiary),
        );
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            widgets::badge_solid(ui, "高危告警", pal.critical.fg, pal.on_accent);
            widgets::badge_solid(ui, "进程已终止", pal.danger.fg, pal.on_accent);
        });
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("chip / count_chip")
                .font(fonts::caption())
                .color(pal.fg_tertiary),
        );
        ui.horizontal(|ui| {
            widgets::chip(ui, "网络监测中", "", pal.success.fg, pal.fg_secondary);
            widgets::chip(ui, "监测已暂停", "", pal.warning.fg, pal.fg_secondary);
            widgets::chip(ui, "持久化未扫描", "", pal.warning.fg, pal.fg_secondary);
        });
        ui.horizontal(|ui| {
            widgets::count_chip(ui, "端点", "128", pal.fg_primary, pal.fg_secondary);
            widgets::count_chip(ui, "已建立", "64", pal.success.fg, pal.fg_secondary);
            widgets::count_chip(ui, "监听", "23", pal.info.fg, pal.fg_secondary);
            widgets::count_chip(ui, "TimeWait", "5", pal.warning.fg, pal.fg_secondary);
            widgets::count_chip(ui, "CloseWait", "2", pal.danger.fg, pal.fg_secondary);
        });
        ui.add_space(10.0);

        widgets::separator(ui, pal.border);
        ui.add_space(8.0);
        let w = ui.available_width();
        ui.allocate_ui(Vec2::new(w, 130.0), |ui| {
            widgets::empty_state(
                ui,
                pal,
                "◎",
                "暂无连接数据",
                "调整过滤条件，或点击刷新 / 等待自动采集",
            );
        });
    });
}

fn type_ladder(ui: &mut egui::Ui, pal: &Palette) {
    widgets::panel_frame(pal, 16).show(ui, |ui| {
        section_title(ui, pal, "字号阶梯", "spec §3.2 · mono 同级对应");
        let rows: [(&str, f32); 7] = [
            ("display", fonts::size::DISPLAY),
            ("title", fonts::size::TITLE),
            ("section", fonts::size::SECTION),
            ("body", fonts::size::BODY),
            ("control", fonts::size::CONTROL),
            ("table", fonts::size::TABLE),
            ("caption", fonts::size::CAPTION),
        ];
        for (name, size) in rows {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 18.0), Sense::hover());
                ui.painter().text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    name,
                    fonts::mono_caption(),
                    pal.fg_tertiary,
                );
                ui.label(
                    egui::RichText::new(format!("网络监测 Network {size}"))
                        .font(egui::FontId::proportional(size))
                        .strong()
                        .color(pal.fg_primary),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("14:32:05 192.168.1.1:443")
                        .font(egui::FontId::monospace(size))
                        .color(pal.fg_secondary),
                );
            });
        }
    });
}

// ── 独立预览入口（examples/design_board.rs 调用，不接任何页面）─────────────

struct DesignBoardApp {
    pal: Palette,
}

impl eframe::App for DesignBoardApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(self.pal.bg_base))
            .show(ui, |ui| design_board(ui, &self.pal));
    }
}

/// 启动独立样板窗口（1100x800）。入口：`cargo run -p irtool-egui --example design_board [-- --dark]`。
pub fn run_preview(dark: bool) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("IRtool Design Board")
            .with_inner_size([1100.0, 800.0])
            .with_min_inner_size([900.0, 640.0]),
        ..Default::default()
    };
    let pal = if dark { Palette::dark() } else { Palette::light() };
    eframe::run_native(
        "irtool-design-board",
        options,
        Box::new(move |cc| {
            let ctx = &cc.egui_ctx;
            fonts::install_fonts(ctx);
            fonts::apply_text_styles(ctx, dark);
            ctx.set_theme(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            Ok(Box::new(DesignBoardApp { pal }))
        }),
    )
}
