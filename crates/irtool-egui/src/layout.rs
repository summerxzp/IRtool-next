use eframe::egui::{self, Align2, Color32, Pos2, Sense, Vec2};

use crate::app::IrtoolApp;
use crate::design::fonts;
use crate::design::icon::{self as design_icon, Icon};
use crate::design::theme as dtheme;
use crate::design::tokens::Palette;
use crate::nav::Page;
use crate::theme;

/// 导航钮边长（rail_btn / rail_centered 共用）。
const RAIL_BTN: f32 = 42.0;

impl IrtoolApp {
    /// Non-selectable label helper.
    fn ui_label(&self, ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
        ui.add(egui::Label::new(text).selectable(false))
    }

    /// 把当前主题 mode 落盘（ui-state.json，跟随 portable 数据目录）。
    fn persist_theme(&self) {
        dtheme::store_mode(dtheme::mode(), &self.ctx.app_dirs.config_dir());
    }

    /// Render the top bar with app info and global status.
    /// 视觉基准：React TopBar.tsx + demo toolbar（surface 底 / chip / 右侧动作区）。
    pub fn render_topbar(&mut self, ui: &mut egui::Ui) {
        let pal = dtheme::palette();
        ui.set_min_height(theme::TOPBAR_HEIGHT);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;

            // Logo + version
            self.ui_label(
                ui,
                egui::RichText::new("IRtool")
                    .font(fonts::section())
                    .strong()
                    .color(pal.fg_primary),
            );
            self.ui_label(
                ui,
                egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .font(fonts::caption())
                    .color(pal.fg_tertiary),
            );

            ui.separator();

            // Admin chip（demo env_chip：状态点 + 次级文字）
            let admin_text = if self.is_admin {
                rust_i18n::t!("status.admin")
            } else {
                rust_i18n::t!("status.non-admin")
            };
            let (admin_text_ref, admin_dot) = (&admin_text, if self.is_admin { pal.success.fg } else { pal.warning.fg });
            Self::dot_chip(ui, &pal, admin_text_ref, admin_dot);

            // Fallback mode badge
            if self.is_fallback {
                let fallback_badge = rust_i18n::t!("shell.topbar.fallback-badge");
                crate::widgets::badge::badge(ui, &fallback_badge, crate::widgets::badge::BadgeVariant::Warning);
            }

            // Right actions: exit + theme toggle（右到左布局，先加的在最右）
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let exit_label = rust_i18n::t!("shell.topbar.exit");
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(exit_label.as_ref())
                                .font(fonts::control())
                                .color(pal.fg_secondary),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    self.request_exit();
                }
                if self.theme_toggle(ui).clicked() {
                    // Light → Dark → System 循环（图标/文字标当前态），立即应用并落盘
                    dtheme::cycle(ui.ctx());
                    self.persist_theme();
                }
            });
        });
    }

    /// 状态点 + 文字 chip（demo status_bars env_chip：7px 圆点 + fg-secondary 文字）。
    fn dot_chip(ui: &mut egui::Ui, pal: &Palette, label: &str, dot: Color32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 3.5, dot);
        ui.label(
            egui::RichText::new(label)
                .font(fonts::table())
                .color(pal.fg_secondary),
        );
        ui.add_space(6.0);
    }

    /// 主题切换按钮：图标标当前态（☀/🌙/🖥）+ 文字（浅色/深色/跟随系统），
    /// 点击循环 Light → Dark → System（React 版当前为亮暗两态切换，三态为本侧超集）。
    fn theme_toggle(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let pal = dtheme::palette();
        let mode = dtheme::mode();
        let (icon, key) = match mode {
            dtheme::ThemeMode::Light => (Icon::Sun, "shell.theme.light"),
            dtheme::ThemeMode::Dark => (Icon::Moon, "shell.theme.dark"),
            dtheme::ThemeMode::System => (Icon::Monitor, "shell.theme.system"),
        };
        let label = rust_i18n::t!(key);
        let tip = rust_i18n::t!("shell.theme.toggle-tip", mode = label.as_ref());

        let galley = ui
            .painter()
            .layout_no_wrap(label.as_ref().to_string(), fonts::control(), pal.fg_secondary);
        let h = 26.0;
        let w = 8.0 + 16.0 + 5.0 + galley.size().x + 8.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        {
            let p = ui.painter();
            if resp.hovered() || resp.contains_pointer() {
                p.rect_filled(rect, egui::CornerRadius::same(6), pal.hover);
            }
        }
        design_icon::draw(
            ui,
            icon,
            Pos2::new(rect.left() + 8.0 + 8.0, rect.center().y),
            16.0,
            pal.fg_secondary,
        );
        ui.painter().galley(
            Pos2::new(rect.left() + 8.0 + 16.0 + 5.0, rect.center().y - galley.size().y / 2.0),
            galley,
            pal.fg_secondary,
        );
        resp.on_hover_text(tip)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Render the navigation rail (P3 起替代旧 160px 文字侧栏).
    /// 视觉基准：demo side_rail —— rail 底色 / 38px 图标钮 / hover 软底 /
    /// 当前页 selected.bg + accent 左缘指示条 / 设置项沉底。
    pub fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        let pal = dtheme::palette();
        ui.add_space(14.0);

        // Logo tile（demo：34px accent 圆角块 + "IR"）
        let (logo_rect, _) = ui.allocate_exact_size(Vec2::new(theme::RAIL_WIDTH, 36.0), Sense::hover());
        let lp = ui.painter();
        let tile = egui::Rect::from_center_size(logo_rect.center(), Vec2::splat(34.0));
        lp.rect_filled(tile, 9.0, pal.accent);
        lp.text(tile.center(), Align2::CENTER_CENTER, "IR", fonts::body(), pal.on_accent);

        ui.add_space(12.0);

        for page in Page::MAIN {
            Self::rail_centered(ui);
            if Self::rail_btn(ui, &pal, page.icon(), &page.label(), self.current_page == page).clicked() {
                self.current_page = page;
            }
        }

        // 设置项沉底 + 与主导航之间的分隔线（demo 布局）
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(14.0);
            Self::rail_centered(ui);
            if Self::rail_btn(ui, &pal, Page::Settings.icon(), &Page::Settings.label(), self.current_page == Page::Settings)
                .clicked()
            {
                self.current_page = Page::Settings;
            }
            ui.add_space(10.0);
            Self::rail_centered(ui);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(30.0, 1.0), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, pal.border);
        });
    }

    /// 水平居中垫片（demo centered()：把 38px 内容在 rail 内居中）。
    fn rail_centered(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let pad = ((ui.available_width() - RAIL_BTN) / 2.0).max(0.0);
            ui.add_space(pad);
        });
    }

    /// 导航钮：38×38 / glyph 18 / radius 10，active 左缘竖指示条，hover 提示文案。
    fn rail_btn(ui: &mut egui::Ui, pal: &Palette, icon: Icon, tip: &str, active: bool) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(RAIL_BTN), Sense::click());
        let p = ui.painter();
        if active {
            p.rect_filled(rect, 10.0, pal.selected.bg);
        } else if resp.hovered() {
            p.rect_filled(rect, 10.0, pal.hover);
        }
        if active {
            p.rect_filled(
                egui::Rect::from_min_size(
                    Pos2::new(rect.left() - 9.0, rect.center().y - 10.0),
                    Vec2::new(3.0, 20.0),
                ),
                2.0,
                pal.accent,
            );
        }
        let col = if active {
            pal.accent
        } else if resp.hovered() {
            pal.fg_primary
        } else {
            pal.fg_secondary
        };
        design_icon::draw(ui, icon, rect.center(), 20.0, col);
        resp.on_hover_text(tip)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}
