use eframe::egui::{self, Color32};

use crate::design::theme as rt;

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

// ── 色彩访问器（P3 运行时化）─────────────────────────────────
//
// 旧编译期 const → 函数：内部按当前 resolved palette 返回 design tokens 对应值。
// 调用方（页面/壳）每帧调用，开销为一次 uncontended 读锁 + 小结构体拷贝。
//
// 映射表（旧常量 → design::tokens 字段 → tokens.css 变量）：
// | 旧常量             | 旧值(light) | design 字段      | tokens.css 变量        |
// |--------------------|-------------|------------------|------------------------|
// | BG_PRIMARY         | #ffffff     | bg_elev1         | --bg-elev-1            |
// | BG_SECONDARY       | #f5f5f5     | bg_elev2         | --bg-elev-2            |
// | BG_ELEVATED        | #eaeaea     | hover (demo扩展) | （无，hover）          |
// | FG_PRIMARY         | #1a1a2e     | fg_primary       | --fg-primary           |
// | FG_SECONDARY       | #555555     | fg_secondary     | --fg-secondary         |
// | FG_TERTIARY        | #999999     | fg_tertiary      | --fg-tertiary          |
// | ACCENT             | #2563eb     | accent           | --accent               |
// | ACCENT_HOVER       | #1d4ed8     | accent_hover()   | 派生：light 调暗 18%   |
// | SEMANTIC_SUCCESS   | #16a34a     | success.fg       | --success              |
// | SEMANTIC_INFO      | #2563eb     | info.fg          | --accent               |
// | SEMANTIC_WARNING   | #ca8a04     | warning.fg       | --warning              |
// | SEMANTIC_DANGER    | #dc2626     | danger.fg        | --danger               |
// | SEMANTIC_DEFAULT   | #6b7280     | neutral.fg       | --fg-secondary         |
// | TABLE_HEADER_BG    | #f0f0f0     | bg_elev2         | --bg-elev-2            |
// | TABLE_ROW_SELECTED | accent@12%  | selected.bg      | --accent 12% color-mix |

/// 面板底色（旧 BG_PRIMARY → --bg-elev-1）。
pub fn bg_primary() -> Color32 {
    rt::palette().bg_elev1
}

/// 次级表面（旧 BG_SECONDARY → --bg-elev-2；当前无调用方，映射表完备性保留）。
#[allow(dead_code)]
pub fn bg_secondary() -> Color32 {
    rt::palette().bg_elev2
}

/// hover/抬升表面（旧 BG_ELEVATED → demo 扩展 hover 令牌）。
pub fn bg_elevated() -> Color32 {
    rt::palette().hover
}

/// 主文字（旧 FG_PRIMARY → --fg-primary）。
pub fn fg_primary() -> Color32 {
    rt::palette().fg_primary
}

/// 次级文字（旧 FG_SECONDARY → --fg-secondary）。
pub fn fg_secondary() -> Color32 {
    rt::palette().fg_secondary
}

/// 辅助文字（旧 FG_TERTIARY → --fg-tertiary）。
pub fn fg_tertiary() -> Color32 {
    rt::palette().fg_tertiary
}

/// 品牌色（旧 ACCENT → --accent）。
pub fn accent() -> Color32 {
    rt::palette().accent
}

/// accent hover 派生（旧 ACCENT_HOVER，tokens.css 无对应，P3 派生规则见 tokens.rs；
/// 当前无调用方，映射表完备性保留）。
#[allow(dead_code)]
pub fn accent_hover() -> Color32 {
    rt::palette().accent_hover()
}

pub fn semantic_success() -> Color32 {
    rt::palette().success.fg
}

pub fn semantic_info() -> Color32 {
    rt::palette().info.fg
}

pub fn semantic_warning() -> Color32 {
    rt::palette().warning.fg
}

pub fn semantic_danger() -> Color32 {
    rt::palette().danger.fg
}

pub fn semantic_default() -> Color32 {
    rt::palette().neutral.fg
}

/// 表头底（旧 TABLE_HEADER_BG → --bg-elev-2；当前无调用方，保留对齐用）。
#[allow(dead_code)]
pub fn table_header_bg() -> Color32 {
    rt::palette().bg_elev2
}

/// 行选中底（旧 TABLE_ROW_SELECTED → selected.bg = accent 12% alpha）。
pub fn table_row_selected() -> Color32 {
    rt::palette().selected.bg
}

// ── Layout（数值常量，与主题无关，保留 const）────────────────

pub const TOPBAR_HEIGHT: f32 = 32.0;
/// 旧文字侧栏宽度（P3 壳切换为图标 rail 后闲置，保留数值常量）。
#[allow(dead_code)]
pub const SIDEBAR_WIDTH: f32 = 160.0;
/// 导航 rail 宽度（demo side_rail 基准）。
pub const RAIL_WIDTH: f32 = 58.0;
pub const DETAIL_PANEL_HEIGHT: f32 = 220.0;
/// 旧壳内边距（P3 壳改造后闲置，保留数值常量）。
#[allow(dead_code)]
pub const PANEL_PADDING: f32 = 12.0;
pub const ELEMENT_GAP: f32 = 8.0;
pub const TABLE_ROW_HEIGHT: f32 = 22.0;
pub const TABLE_HEADER_HEIGHT: f32 = 24.0;

/// 全局面板统一 frame：面板底填充、无边框、无阴影、无外边距。
/// 用于 TopBottomPanel / SidePanel / CentralPanel，避免默认阴影/边框造成的黑边。
pub fn panel_frame(inner_margin: egui::Margin) -> egui::Frame {
    egui::Frame::new()
        .fill(bg_primary())
        .inner_margin(inner_margin)
        .outer_margin(egui::Margin::ZERO)
        .stroke(egui::Stroke::NONE)
        .corner_radius(0.0)
        .shadow(egui::Shadow::NONE)
}

/// 应用当前 resolved 主题（Light/Dark/System 三态运行时，P3 起替代旧 apply_light_theme）。
pub fn apply(ctx: &egui::Context) {
    rt::apply(ctx);
}
