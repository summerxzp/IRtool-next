//! 设计系统模块（ui-design-spec.md 的代码落地，P2 纯新增、不接线）。
//!
//! - [`tokens`]：双主题调色板（中性阶梯 + 8 角色三件套），基准 `ui/src/styles/tokens.css`。
//! - [`fonts`]：字族兜底链安装 + spec §3.2 字号阶梯。
//! - [`widgets`]：吃令牌的组件公共样式（badge/chip/empty_state/panel_frame/separator）。
//! - [`preview`]：design board 对照样板与独立预览入口（`run_preview`）。
//! - [`theme`]：主题运行时（P3）：Light/Dark/System 三态 + 全局状态 + egui 应用。
//! - [`icon`]：SVG 内嵌 lucide 图标 + 光栅化缓存（P3 定稿方案，spec §4.1）。

pub mod fonts;
pub mod icon;
pub mod preview;
pub mod theme;
pub mod tokens;
pub mod widgets;
