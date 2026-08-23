//! 设计令牌（Semantic 层）——ui-design-spec.md §1/§2 的 egui 实现。
//!
//! 色值唯一基准：React 版 `ui/src/styles/tokens.css`（dark 默认块 + `[data-theme="light"]` 块）。
//! 每个字段都标注了来源；除标注"demo 扩展"的四个中性色外，值与 tokens.css 逐字一致。
//! 页面代码禁止绕过本模块直接写 `Color32::from_rgb`（spec §6-1）。

use eframe::egui::Color32;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// 角色 bg 的 alpha（tokens.css `color-mix ... 12%`，round(0.12*255)=31）。
pub const ROLE_BG_ALPHA: u8 = 31;
/// 角色 border 的 alpha（tokens.css `color-mix ... 25%`，round(0.25*255)=64）。
pub const ROLE_BORDER_ALPHA: u8 = 64;

/// fg 上叠 alpha（简单线性混合，非 gamma 校正——spec §2.2 允许）。
const fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const(c.r(), c.g(), c.b(), a)
}

/// 语义角色三件套（spec §2.2）：fg 前景 / bg 软底（fg 12% alpha）/ border 软边（fg 25% alpha）。
/// bg 与 border 由 fg 经 const fn 派生，不手抄派生值；两主题同一规则。
#[derive(Clone, Copy, Debug)]
pub struct RoleColors {
    /// 前景：文字/图标/状态点/实色徽章背景。
    pub fg: Color32,
    /// 背景：软色徽章/高亮行（= fg 12% alpha）。
    pub bg: Color32,
    /// 边框：软色徽章边框（= fg 25% alpha）。
    pub border: Color32,
}

impl RoleColors {
    pub const fn from_fg(fg: Color32) -> Self {
        RoleColors {
            fg,
            bg: with_alpha(fg, ROLE_BG_ALPHA),
            border: with_alpha(fg, ROLE_BORDER_ALPHA),
        }
    }
}

/// 双主题调色板。中性阶梯取自 tokens.css 对应主题；rail/hover/border_strong/row_line
/// 为 demo（ir-ui-demos/egui-demo）探索保留的扩展中性色，tokens.css 暂无对应。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,

    // ── 中性阶梯（tokens.css）────────────────────────────────
    /// 窗口底 --bg-base。light #f7f8fa / dark #0b0d10
    pub bg_base: Color32,
    /// 卡片/面板/表格 --bg-elev-1。light #ffffff / dark #14171c
    pub bg_elev1: Color32,
    /// 次级表面 --bg-elev-2。light #eef1f5 / dark #1c2127
    pub bg_elev2: Color32,
    /// 分隔线/描边 --border。light #d8dde5 / dark #262c34
    pub border: Color32,
    /// 主文字 --fg-primary。light #1a1d23 / dark #e6e8eb
    pub fg_primary: Color32,
    /// 次级文字/表头 --fg-secondary。light #4a5260 / dark #9aa3ad
    pub fg_secondary: Color32,
    /// 辅助文字/占位 --fg-tertiary。light #788090 / dark #6b7480
    pub fg_tertiary: Color32,
    /// 品牌/选中/链接 --accent。light #3a7af0 / dark #4c8dff
    pub accent: Color32,

    // ── demo 扩展中性色（tokens.css 无对应，来源 demo theme.rs 现值，定稿前保留）──
    /// 侧栏底（demo 扩展，非 tokens.css 来源）。light #fbfbfc / dark #17191c
    pub rail: Color32,
    /// 行/项 hover（demo 扩展，非 tokens.css 来源）。light #ebedf0 / dark #2a2e33
    pub hover: Color32,
    /// 强描边（下拉框 hover 等）（demo 扩展，非 tokens.css 来源）。light #d0d5dd / dark #3a4149
    pub border_strong: Color32,
    /// 表格行分隔线（demo 扩展，非 tokens.css 来源）。light #f2f4f7 / dark #23272c
    pub row_line: Color32,

    // ── 语义角色三件套（spec §2.2，共 8 个 role）──────────────
    /// 最高危（tokens.css --critical：light #911818 / dark #b91c1c）。
    pub critical: RoleColors,
    /// 威胁/错误/失败（tokens.css --danger：light #d63838 / dark #ef4444）。
    pub danger: RoleColors,
    /// 可疑/需注意（tokens.css --warning：light #c98a07 / dark #f0b429）。
    pub warning: RoleColors,
    /// 正面/正常/运行中（tokens.css --success：light #20a04f / dark #2ecc71）。
    pub success: RoleColors,
    /// 进行中/中性动作/链接（fg = accent）。
    pub info: RoleColors,
    /// 默认/辅助/未知（fg = fg_secondary）。
    pub neutral: RoleColors,
    /// 终态/降权/历史（fg = fg_tertiary）。
    pub dim: RoleColors,
    /// 选中/聚焦/当前页（fg = accent，即 React 版选中高亮 accent-soft 的来源）。
    pub selected: RoleColors,

    /// 实色徽章/主按钮文字色（tokens.css --primary-foreground: #ffffff）。
    pub on_accent: Color32,
}

impl Palette {
    /// tokens.css `[data-theme="light"]` 块。
    pub const fn light() -> Self {
        let bg_base = rgb(0xF7F8FA);
        let bg_elev1 = rgb(0xFFFFFF);
        let bg_elev2 = rgb(0xEEF1F5);
        let border = rgb(0xD8DDE5);
        let fg_primary = rgb(0x1A1D23);
        let fg_secondary = rgb(0x4A5260);
        let fg_tertiary = rgb(0x788090);
        let accent = rgb(0x3A7AF0);
        Palette {
            dark: false,
            bg_base,
            bg_elev1,
            bg_elev2,
            border,
            fg_primary,
            fg_secondary,
            fg_tertiary,
            accent,
            // demo 扩展中性色：保留 demo theme.rs 现值（非 tokens.css 来源）
            rail: rgb(0xFBFBFC),
            hover: rgb(0xEBEDF0),
            border_strong: rgb(0xD0D5DD),
            row_line: rgb(0xF2F4F7),
            critical: RoleColors::from_fg(rgb(0x911818)),
            danger: RoleColors::from_fg(rgb(0xD63838)),
            warning: RoleColors::from_fg(rgb(0xC98A07)),
            success: RoleColors::from_fg(rgb(0x20A04F)),
            info: RoleColors::from_fg(accent),
            neutral: RoleColors::from_fg(fg_secondary),
            dim: RoleColors::from_fg(fg_tertiary),
            selected: RoleColors::from_fg(accent),
            on_accent: Color32::WHITE,
        }
    }

    /// tokens.css 默认块（`:root` 即 dark）。
    pub const fn dark() -> Self {
        let bg_base = rgb(0x0B0D10);
        let bg_elev1 = rgb(0x14171C);
        let bg_elev2 = rgb(0x1C2127);
        let border = rgb(0x262C34);
        let fg_primary = rgb(0xE6E8EB);
        let fg_secondary = rgb(0x9AA3AD);
        let fg_tertiary = rgb(0x6B7480);
        let accent = rgb(0x4C8DFF);
        Palette {
            dark: true,
            bg_base,
            bg_elev1,
            bg_elev2,
            border,
            fg_primary,
            fg_secondary,
            fg_tertiary,
            accent,
            // demo 扩展中性色：保留 demo theme.rs 现值（非 tokens.css 来源）
            rail: rgb(0x17191C),
            hover: rgb(0x2A2E33),
            border_strong: rgb(0x3A4149),
            row_line: rgb(0x23272C),
            critical: RoleColors::from_fg(rgb(0xB91C1C)),
            danger: RoleColors::from_fg(rgb(0xEF4444)),
            warning: RoleColors::from_fg(rgb(0xF0B429)),
            success: RoleColors::from_fg(rgb(0x2ECC71)),
            info: RoleColors::from_fg(accent),
            neutral: RoleColors::from_fg(fg_secondary),
            dim: RoleColors::from_fg(fg_tertiary),
            selected: RoleColors::from_fg(accent),
            on_accent: Color32::WHITE,
        }
    }
}
