//! 主题运行时（P3 接线）：Light / Dark / System 三态 + Windows 系统偏好 + 全局状态。
//!
//! 状态管理：全局 [`OnceLock<RwLock<ThemeState>>`]（读多写少，页面每帧经
//! [`palette()`] 取一份 `Palette` 拷贝，uncontended 读锁开销可忽略），壳与页面
//! 均无感切换。mode 持久化为 `config/ui-state.json`（serde_json，跟随
//! portable.flag 机制的数据目录，由调用方传入目录）。
//!
//! System 判定：注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\
//! Personalize` 的 `AppsUseLightTheme`（REG_DWORD，0=深色）。已登记限制：运行期
//! 不监听系统主题变化（WM_SETTINGCHANGE），仅启动与切回 System 时重读。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use eframe::egui::style::WidgetVisuals;
use eframe::egui::{self, Color32, Context, CornerRadius, Stroke};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::fonts;
use super::tokens::Palette;

/// 三态主题（语义对齐 React `ui/src/stores/theme-store.ts` 的 `Theme`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

impl ThemeMode {
    /// 顶栏切换按钮的循环顺序：浅色 → 深色 → 跟随系统。
    pub fn next(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
            ThemeMode::System => ThemeMode::Light,
        }
    }
}

/// 界面语言（P4）：对齐 React 侧 supportedLngs（ui/src/lib/i18n.ts）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

impl Language {
    /// 全部支持的语言（settings 页语言控件枚举用）。
    pub const ALL: [Language; 2] = [Language::ZhCn, Language::EnUs];

    /// rust-i18n locale 代码（= ui-state.json 落盘值）。
    pub fn code(self) -> &'static str {
        match self {
            Language::ZhCn => "zh-CN",
            Language::EnUs => "en-US",
        }
    }

    /// 控件选项文案：各语言自称，不随当前语言翻译（React 惯例）。
    pub fn native_label(self) -> &'static str {
        match self {
            Language::ZhCn => "简体中文",
            Language::EnUs => "English",
        }
    }
}

/// ui-state.json 的落盘结构（theme + language + tables）。
///
/// `tables` 是各表格（design/table.rs）的持久化段：table_id → 组件自定义 JSON
/// （schema 由 table.rs 自管，本模块只负责存取，保持解耦）。缺字段由
/// `#[serde(default)]` 兜底，旧文件向后兼容。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiState {
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub language: Language,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub tables: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            theme: ThemeMode::default(),
            language: Language::default(),
            tables: std::collections::BTreeMap::new(),
        }
    }
}

/// Windows 系统应用主题是否偏好深色（AppsUseLightTheme=0 → 深色）。
#[cfg(windows)]
fn system_prefers_dark() -> bool {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_DWORD,
    };

    let mut key = HKEY::default();
    let opened =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"), None, KEY_READ, &mut key) };
    if opened.is_err() {
        return false;
    }

    let mut data: u32 = 0;
    let mut size: u32 = std::mem::size_of::<u32>() as u32;
    let queried = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }

    queried.is_ok() && data == 0
}

#[cfg(not(windows))]
fn system_prefers_dark() -> bool {
    false
}

struct ThemeState {
    mode: ThemeMode,
    dark: bool,
    pal: Palette,
}

static STATE: OnceLock<RwLock<ThemeState>> = OnceLock::new();

/// 界面语言运行时（与主题同模式：OnceLock + RwLock，读多写少）。
static LANG: OnceLock<RwLock<Language>> = OnceLock::new();

fn lang_state() -> &'static RwLock<Language> {
    LANG.get_or_init(|| RwLock::new(Language::default()))
}

/// 当前界面语言。
pub fn language() -> Language {
    *lang_state().read()
}

/// 设置界面语言并同步 rust-i18n locale（切换即时生效；重绘由调用方 request_repaint）。
pub fn set_language(lang: Language) {
    *lang_state().write() = lang;
    rust_i18n::set_locale(lang.code());
}

fn state() -> &'static RwLock<ThemeState> {
    STATE.get_or_init(|| {
        let mode = ThemeMode::System;
        RwLock::new(ThemeState {
            mode,
            dark: system_prefers_dark(),
            pal: resolved_palette(mode),
        })
    })
}

fn resolved_palette(mode: ThemeMode) -> Palette {
    match mode {
        ThemeMode::Light => Palette::light(),
        ThemeMode::Dark => Palette::dark(),
        ThemeMode::System => {
            if system_prefers_dark() {
                Palette::dark()
            } else {
                Palette::light()
            }
        }
    }
}

/// 初始化运行时（进程启动一次；重复调用仅改写 mode）。返回解析后的 mode。
pub fn init(mode: ThemeMode) {
    let pal = resolved_palette(mode);
    let mut s = state().write();
    s.mode = mode;
    s.dark = pal.dark;
    s.pal = pal;
}

/// 当前模式。
pub fn mode() -> ThemeMode {
    state().read().mode
}

/// 当前解析结果是否深色。
pub fn is_dark() -> bool {
    state().read().dark
}

/// 当前 resolved 调色板（每帧调用；返回小结构体拷贝）。
pub fn palette() -> Palette {
    state().read().pal
}

/// 切换模式：更新全局状态并立即应用到 ctx。切换到 System 时重读注册表。
pub fn set_mode(mode: ThemeMode, ctx: &Context) {
    init(mode);
    apply(ctx);
}

/// 顶栏切换按钮：Light → Dark → System 循环。
pub fn cycle(ctx: &Context) -> ThemeMode {
    let next = mode().next();
    set_mode(next, ctx);
    next
}

// ── egui 应用 ──────────────────────────────────────────────

static FONTS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// 壳面板（顶栏/状态栏）frame：elev-1 底、无边框无阴影；与其他区域的分隔
/// 由 Panel 的 `show_separator_line(true)` 提供（线色 = noninteractive bg_stroke = border）。
pub fn shell_panel_frame(inner_margin: egui::Margin) -> egui::Frame {
    egui::Frame::new()
        .fill(palette().bg_elev1)
        .inner_margin(inner_margin)
        .outer_margin(egui::Margin::ZERO)
        .stroke(egui::Stroke::NONE)
        .corner_radius(0.0)
        .shadow(egui::Shadow::NONE)
}

fn widget_visuals(w: &mut WidgetVisuals, bg: Color32, fg: Color32, stroke: Stroke) {
    w.bg_fill = bg;
    w.weak_bg_fill = bg;
    w.fg_stroke = Stroke::new(1.0, fg);
    w.bg_stroke = stroke;
    w.corner_radius = CornerRadius::same(6);
    w.expansion = 0.0;
}

/// 由 Palette 生成一套完整 egui Style（视觉决策搬运自 demo theme::apply，P2 定稿基准）。
fn style_from(pal: &Palette) -> egui::Style {
    let mut style = egui::Style::default();
    let v = &mut style.visuals;
    v.dark_mode = pal.dark;
    v.panel_fill = pal.bg_elev1;
    v.window_fill = pal.bg_elev1;
    v.extreme_bg_color = pal.bg_base;
    v.faint_bg_color = pal.hover;
    v.hyperlink_color = pal.accent;
    // 选中高亮 = selected.bg（accent 12% alpha，React 版 accent-soft 的对齐实现）
    v.selection.bg_fill = pal.selected.bg;
    v.selection.stroke = Stroke::new(1.0, pal.accent);
    v.window_stroke = Stroke::new(1.0, pal.border);
    // 与 demo 的差异登记：demo 的 noninteractive 前景为 fg-secondary，此处取
    // fg-primary——主项目页面存在大量无显式配色的 ui.label，旧版主题以
    // override_text_color=FG_PRIMARY 兜底，保持正文主色可读性。
    widget_visuals(&mut v.widgets.noninteractive, pal.bg_elev1, pal.fg_primary, Stroke::new(1.0, pal.border));
    widget_visuals(&mut v.widgets.inactive, Color32::TRANSPARENT, pal.fg_primary, Stroke::NONE);
    widget_visuals(&mut v.widgets.hovered, pal.hover, pal.fg_primary, Stroke::NONE);
    widget_visuals(&mut v.widgets.active, pal.hover, pal.accent, Stroke::NONE);
    widget_visuals(&mut v.widgets.open, Color32::TRANSPARENT, pal.fg_primary, Stroke::NONE);
    style
}

/// 把当前 resolved 主题应用到 egui Context：
/// 字族/字号阶梯 + 双主题 Style（egui 0.36 按主题分存，spec §1 注意）+ set_theme。
/// 每帧调用安全，但建议仅在初始化与切换时调用。
pub fn apply(ctx: &Context) {
    let dark = is_dark();

    // 一次性安装（字族兜底链 + DPI 基线 + 全局 label 不可选中）
    if !FONTS_INSTALLED.swap(true, Ordering::Relaxed) {
        fonts::install_fonts(ctx);
        let scale = ctx.pixels_per_point();
        if scale <= 1.0 {
            ctx.set_pixels_per_point(1.15);
        }
        ctx.options_mut(|o| {
            std::sync::Arc::make_mut(&mut o.dark_style).interaction.selectable_labels = false;
            std::sync::Arc::make_mut(&mut o.light_style).interaction.selectable_labels = false;
        });
    }

    // 两套主题都写入，保证 egui 内部按 Theme 分存的 style 一致（spec §1）
    ctx.set_style_of(egui::Theme::Light, style_from(&Palette::light()));
    ctx.set_style_of(egui::Theme::Dark, style_from(&Palette::dark()));
    fonts::apply_text_styles(ctx, dark);

    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

// ── 持久化（ui-state.json，目录由调用方传入 = AppDirs::config_dir）──

/// 从 `config_dir/ui-state.json` 读取完整 UI 状态（theme/language/tables）；
/// 无文件/解析失败 → 默认值（System / zh-CN / 空 tables）。旧文件缺字段时
/// 由 `#[serde(default)]` 兜底。
pub fn load_state(config_dir: &std::path::Path) -> UiState {
    let path = config_dir.join("ui-state.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<UiState>(&text).ok())
        .unwrap_or_default()
}

/// 读改写 `config_dir/ui-state.json`：读出现有内容（缺文件/坏 JSON → 默认值）
/// 交给 `f` 修改后原子写回（先 tmp 再 rename）。各持久化段落（主题/语言/表格
/// 状态等）统一走这里做读改写，保证互相不覆盖。
pub fn update_state(config_dir: &std::path::Path, f: impl FnOnce(&mut UiState)) {
    let path = config_dir.join("ui-state.json");
    let mut state = load_state(config_dir);
    f(&mut state);
    let tmp = config_dir.join("ui-state.json.tmp");
    let body = serde_json::to_string_pretty(&state).unwrap_or_default();
    if std::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
        tracing::warn!("failed to persist ui-state.json at {}", path.display());
    }
}

/// 把主题 mode 写入 `config_dir/ui-state.json`（经 [`update_state`] 读改写，
/// 不动 tables 等其他段）；language 取运行时当前值一并落盘。
pub fn store_mode(mode: ThemeMode, config_dir: &std::path::Path) {
    update_state(config_dir, |s| {
        s.theme = mode;
        s.language = language();
    });
}
