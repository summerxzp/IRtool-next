//! 图标（spec §4.1 定稿方案：SVG 内嵌 lucide + 光栅化缓存）。
//!
//! - 图源：lucide 线性图标集的 SVG 文件内嵌进二进制（`assets/icons/`，
//!   `include_bytes!`），无运行时文件依赖、无网络请求。
//! - 光栅化：复用 `egui_extras` "svg" feature 携带的 resvg/usvg 管线
//!   （`egui_extras::image::load_svg_bytes_with_size`），不新增直接依赖。
//! - 着色：SVG 的 `stroke="black"` 在加载时替换为白色，纹理以
//!   `painter.image(.., tint)` 乘法着色——图标色只能由调用方从
//!   role.fg / fg-primary/secondary/tertiary 取（spec §2.2-3）。
//! - 缓存：全局 `(Icon, 像素宽)` → `TextureHandle`，按 pixels_per_point
//!   取整分档；主题切换只改 tint 不重光栅化。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use eframe::egui::{Color32, Pos2, Rect, TextureHandle, Ui, Vec2};

/// 内嵌 lucide 图标集（导航 / 主题切换 / 通用动作）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Activity,
    Bell,
    Briefcase,
    Check,
    ChevronDown,
    Cpu,
    Database,
    Download,
    FileText,
    Monitor,
    Moon,
    PanelLeft,
    PanelLeftOpen,
    Pause,
    Play,
    Refresh,
    Search,
    Settings,
    Shield,
    Sliders,
    Sun,
    Trash,
    X,
}

impl Icon {
    /// 内嵌 SVG 字节（lucide，viewBox 24×24，stroke 已归一为 black）。
    pub fn svg_bytes(self) -> &'static [u8] {
        match self {
            Icon::Activity => include_bytes!("../../assets/icons/activity.svg"),
            Icon::Bell => include_bytes!("../../assets/icons/bell.svg"),
            Icon::Briefcase => include_bytes!("../../assets/icons/briefcase.svg"),
            Icon::Check => include_bytes!("../../assets/icons/check.svg"),
            Icon::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
            Icon::Cpu => include_bytes!("../../assets/icons/cpu.svg"),
            Icon::Database => include_bytes!("../../assets/icons/database.svg"),
            Icon::Download => include_bytes!("../../assets/icons/download.svg"),
            Icon::FileText => include_bytes!("../../assets/icons/file-text.svg"),
            Icon::Monitor => include_bytes!("../../assets/icons/monitor.svg"),
            Icon::Moon => include_bytes!("../../assets/icons/moon.svg"),
            Icon::PanelLeft => include_bytes!("../../assets/icons/panel-left.svg"),
            Icon::PanelLeftOpen => include_bytes!("../../assets/icons/panel-left-open.svg"),
            Icon::Pause => include_bytes!("../../assets/icons/pause.svg"),
            Icon::Play => include_bytes!("../../assets/icons/play.svg"),
            Icon::Refresh => include_bytes!("../../assets/icons/refresh.svg"),
            Icon::Search => include_bytes!("../../assets/icons/search.svg"),
            Icon::Settings => include_bytes!("../../assets/icons/settings.svg"),
            Icon::Shield => include_bytes!("../../assets/icons/shield.svg"),
            Icon::Sliders => include_bytes!("../../assets/icons/sliders.svg"),
            Icon::Sun => include_bytes!("../../assets/icons/sun.svg"),
            Icon::Trash => include_bytes!("../../assets/icons/trash.svg"),
            Icon::X => include_bytes!("../../assets/icons/x.svg"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Icon::Activity => "activity",
            Icon::Bell => "bell",
            Icon::Briefcase => "briefcase",
            Icon::Check => "check",
            Icon::ChevronDown => "chevron-down",
            Icon::Cpu => "cpu",
            Icon::Database => "database",
            Icon::Download => "download",
            Icon::FileText => "file-text",
            Icon::Monitor => "monitor",
            Icon::Moon => "moon",
            Icon::PanelLeft => "panel-left",
            Icon::PanelLeftOpen => "panel-left-open",
            Icon::Pause => "pause",
            Icon::Play => "play",
            Icon::Refresh => "refresh",
            Icon::Search => "search",
            Icon::Settings => "settings",
            Icon::Shield => "shield",
            Icon::Sliders => "sliders",
            Icon::Sun => "sun",
            Icon::Trash => "trash",
            Icon::X => "x",
        }
    }
}

static CACHE: OnceLock<Mutex<HashMap<(Icon, u32), TextureHandle>>> = OnceLock::new();

/// 取（或光栅化并缓存）指定像素宽的图标纹理。失败时回退 None（调用方跳过绘制）。
fn texture(ui: &Ui, icon: Icon, px: u32) -> Option<TextureHandle> {
    let ctx = ui.ctx();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(tex) = cache.lock().ok()?.get(&(icon, px)) {
        return Some(tex.clone());
    }

    // stroke 归一为白：纹理乘法 tint 才能得出任意图标色（黑底乘色仍为黑）。
    let svg = std::str::from_utf8(icon.svg_bytes()).ok()?;
    let white = svg.replace("stroke=\"black\"", "stroke=\"#FFFFFF\"");

    // options 类型来自 resvg 重导出，egui_extras 未重导出；用 Default::default()
    // 让参数类型推断补全，避免为此新增 resvg 直接依赖。
    let image = egui_extras::image::load_svg_bytes_with_size(
        white.as_bytes(),
        egui::load::SizeHint::Width(px),
        &Default::default(),
    )
    .ok()?;

    let tex = ctx.load_texture(
        format!("design-icon/{}/{}", icon.name(), px),
        image,
        eframe::egui::TextureOptions::LINEAR,
    );
    cache.lock().ok()?.insert((icon, px), tex.clone());
    Some(tex)
}

/// 在 `center` 处以 `size`（逻辑像素，pt）绘制图标，`color` 为乘法着色。
pub fn draw(ui: &mut Ui, icon: Icon, center: Pos2, size: f32, color: Color32) {
    let ppp = ui.ctx().pixels_per_point();
    // 按物理像素取整分档缓存，兼顾清晰度与缓存命中
    let px = ((size * ppp).round() as u32).max(8);
    if let Some(tex) = texture(ui, icon, px) {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        ui.painter().image(tex.id(), rect, uv, color);
    }
}

/// 图标按钮（spec §4 IconButton：16px 图标 + hover 软底，圆角 6）。
/// 返回普通按钮 Response（hover 提示由调用方 `.on_hover_text` 附加）。
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str, size: f32) -> eframe::egui::Response {
    let pal = super::theme::palette();
    let pad = 5.0;
    let (rect, resp) = ui
        .allocate_exact_size(Vec2::splat(size + pad * 2.0), eframe::egui::Sense::click());
    let p = ui.painter();
    if resp.hovered() || resp.contains_pointer() {
        p.rect_filled(rect, eframe::egui::CornerRadius::same(6), pal.hover);
    }
    let col = if resp.hovered() { pal.fg_primary } else { pal.fg_secondary };
    draw(ui, icon, rect.center(), size, col);
    resp.on_hover_text(tip)
        .on_hover_cursor(eframe::egui::CursorIcon::PointingHand)
}
