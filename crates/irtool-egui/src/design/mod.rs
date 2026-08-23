//! 设计系统模块（ui-design-spec.md 的代码落地，P2 纯新增、不接线）。
//!
//! - [`tokens`]：双主题调色板（中性阶梯 + 8 角色三件套），基准 `ui/src/styles/tokens.css`。
//! - [`fonts`]：字族兜底链安装 + spec §3.2 字号阶梯。
//! - [`widgets`]：吃令牌的组件公共样式（badge/chip/empty_state/panel_frame/separator）。
//! - [`preview`]：design board 对照样板与独立预览入口（`run_preview`）。

pub mod fonts;
pub mod preview;
pub mod tokens;
pub mod widgets;
