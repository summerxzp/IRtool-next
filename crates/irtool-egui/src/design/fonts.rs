//! 字体排印（spec §3）：字族兜底链 + 字号阶梯。
//!
//! ## 字号阶梯的 egui 取舍（spec §3.2）
//!
//! egui 0.36 的 `TextStyle` 是封闭枚举（Heading/Body/Button/Small/Monospace + `Name(Arc<str>)`），
//! 内建仅 5 档，装不下 7 级阶梯；且 `FontId` 无 weight 字段（上游 TODO，msyh.ttc 内的
//! semibold/medium 档位加载不到）。取舍：
//!
//! 1. 5 个内建 TextStyle 承担最常用档：`Heading`=display(20)、`Body`=body(13)、
//!    `Button`=control(12.5)、`Small`=caption(11.5)、`Monospace`=mono table(12)——
//!    这样 egui 内建 widget（按钮/提示/菜单）自动落进阶梯。
//! 2. 阶梯多出的 title(16)/section(14)/table(12) 以 `TextStyle::Name` 注册进
//!    `style.text_styles`，配套 [`Self::title_style`]/[`Self::section_style`]/[`Self::table_style`]
//!    语义访问器（`.text_style(...)` 或 `.font(...)` 均可）。
//! 3. 字重：egui 仅 normal/strong 两态，spec 的 medium/semibold 一律降级为 `.strong()`，
//!    与 React 版的观感差异记录在 spec 回填建议中。
//! 4. 字号两主题一致；`dark` 参数仅为 API 对称保留（预留给深色微调），当前不参与计算。

use eframe::egui::{Context, FontDefinitions, FontId, TextStyle};

/// 字号阶梯数值（spec §3.2，单位 pt）。
pub mod size {
    pub const DISPLAY: f32 = 20.0;
    pub const TITLE: f32 = 16.0;
    pub const SECTION: f32 = 14.0;
    pub const BODY: f32 = 13.0;
    pub const CONTROL: f32 = 12.5;
    pub const TABLE: f32 = 12.0;
    pub const CAPTION: f32 = 11.5;
}

/// 语义 FontId 构造器（`RichText::font`/painter 直接可用，等宽同名的见 `mono_*`）。
pub const fn display() -> FontId {
    FontId::proportional(size::DISPLAY)
}
pub const fn title() -> FontId {
    FontId::proportional(size::TITLE)
}
pub const fn section() -> FontId {
    FontId::proportional(size::SECTION)
}
pub const fn body() -> FontId {
    FontId::proportional(size::BODY)
}
pub const fn control() -> FontId {
    FontId::proportional(size::CONTROL)
}
pub const fn table() -> FontId {
    FontId::proportional(size::TABLE)
}
pub const fn caption() -> FontId {
    FontId::proportional(size::CAPTION)
}

/// 等宽对应档（时间/IP:端口/PID/路径/哈希，用途清单见 spec §3.1）。
pub const fn mono_table() -> FontId {
    FontId::monospace(size::TABLE)
}
pub const fn mono_caption() -> FontId {
    FontId::monospace(size::CAPTION)
}

/// `TextStyle::Name` 语义访问器（配合 [`apply_text_styles`] 注册后可用于 `.text_style()`）。
pub fn title_style() -> TextStyle {
    TextStyle::Name("design-title".into())
}
pub fn section_style() -> TextStyle {
    TextStyle::Name("design-section".into())
}
pub fn table_style() -> TextStyle {
    TextStyle::Name("design-table".into())
}

/// 安装字族兜底链（spec §3.1，逻辑自 demo main.rs 验证版搬运）：
/// - Proportional：msyh.ttc 置首，内置 Ubuntu-Light 留链尾兜底；不引 simsun/simhei。
/// - Monospace：consola.ttf 置首，缺字回落雅黑。
pub fn install_fonts(ctx: &Context) {
    let mut defs = FontDefinitions::default();

    if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\msyh.ttc") {
        defs.font_data.insert(
            "yahei".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        defs.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "yahei".into());
    }

    if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\consola.ttf") {
        defs.font_data.insert(
            "consolas".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        defs.families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "consolas".into());
    }
    defs.families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("yahei".into());

    ctx.set_fonts(defs);
}

/// 把字号阶梯写入当前 Style 的 `text_styles`（不动 visuals，可安全叠加在既有 style 上）。
/// egui 0.36 的 style 按主题分存（Light/Dark 各一套），此处两套都写入以保证一致。
pub fn apply_text_styles(ctx: &Context, dark: bool) {
    let _ = dark; // 阶梯两主题一致；参数保留给深色微调
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut style = (*ctx.style_of(theme)).clone();
        let t = &mut style.text_styles;
        t.insert(TextStyle::Heading, display());
        t.insert(TextStyle::Body, body());
        t.insert(TextStyle::Button, control());
        t.insert(TextStyle::Small, caption());
        t.insert(TextStyle::Monospace, mono_table());
        t.insert(title_style(), title());
        t.insert(section_style(), section());
        t.insert(table_style(), table());
        ctx.set_style_of(theme, style);
    }
}
