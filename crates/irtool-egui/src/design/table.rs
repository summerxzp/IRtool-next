//! TableShell——`egui_extras::TableBuilder` 的范式封装（P5 定稿，P6 八页照抄）。
//!
//! 能力（对应 React 版 DataTable 基准）：固定表头（caption 字号 fg-secondary +
//! 底部分隔线）、表头点击排序（asc → desc → 无 循环）、列宽拖拽、密度两档
//! （28/34px，spec §3.3）、行选中 / 双击 / 右键菜单、键盘 ↑↓ 移焦点行 + Enter
//! 确认 + `scroll_to_row` 跟随、列宽/密度/排序持久化到 ui-state.json。
//!
//! ## 用法约束（性能红线，spec §6）
//! - 行渲染闭包内**零分配**：禁止 `format!`/`to_string`/`clone`/`on_hover_text`，
//!   显示字符串必须在数据进 store 时预格式化好；悬停提示用 [`row_hover`]。
//! - 行数与索引由页面视图缓存提供（过滤+排序后的索引），组件只按索引渲染，
//!   数据变化时页面置 dirty 重建索引，**禁止整表重建**。
//! - 固定行高走 `TableBody::rows`（内部虚拟化），禁用 `heterogeneous_rows`。
//! - 组件内零硬编码色值：全部经 [`Palette`](super::tokens::Palette)/role 取色。
//!
//! ## 用法示例（P6 范式）
//! ```ignore
//! let mut shell = TableShell::new("my_page", COLUMNS); // COLUMNS: [TableColumn; N]
//! shell.load_table_state(&config_dir);                 // 启动时一次
//! // 每帧：
//! let out = shell.show(ui, view.len(), |row, ctx| {
//!     row.col(|ui| {
//!         paint_row_bg(ui, ctx, sel, risk_bg, pal);    // 可选：行底色
//!         cell_label(ui, item.d_name, fg);             // 预格式化 &str
//!     });
//!     // ... 其余列
//! }, |ui, idx| { /* 右键菜单内容；不需要则留空 */ });
//! if out.persist_dirty || out.sort_changed {
//!     shell.save_table_state(&config_dir);
//! }
//! ```

use std::collections::BTreeMap;

use eframe::egui::{self, Align, Color32, Sense};
use egui_extras::{Column, TableBuilder};
use serde::{Deserialize, Serialize};

use super::fonts;
use super::theme;
use super::tokens::Palette;
use eframe::egui::emath::GuiRounding;

/// 表头高度。表头单行 caption 字号，取与 compact 行高等高保持节奏一致。
pub const HEADER_HEIGHT: f32 = 28.0;

/// 单元格左内边距（对齐 React DataTable 单元格 px-2）。数值常量非色值。
const CELL_PAD_X: f32 = 8.0;

/// 列定义。`id` 是排序态/持久化的稳定键（勿改）；`title_key` 为 i18n 键
/// （如 "network.columns.pid"）；宽度为初始值（持久化后覆盖）与允许范围。
#[derive(Clone, Debug)]
pub struct TableColumn {
    pub id: &'static str,
    pub title_key: &'static str,
    pub width: f32,
    pub min_width: f32,
    pub max_width: f32,
}

/// 密度两档（spec §3.3），默认 Compact。落盘字符串 "compact"/"standard"。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableDensity {
    #[default]
    Compact,
    Standard,
}

impl TableDensity {
    /// 行高（spec §3.3：compact 28px / standard 34px）。
    pub fn row_height(self) -> f32 {
        match self {
            TableDensity::Compact => 26.0,
            TableDensity::Standard => 30.0,
        }
    }

    pub fn title_key(self) -> &'static str {
        match self {
            TableDensity::Compact => "design.table.density-compact",
            TableDensity::Standard => "design.table.density-standard",
        }
    }
}

/// 持久化到 ui-state.json `tables.{table_id}` 的结构（serde default 向后兼容：
/// 旧文件缺字段/新字段缺失均不破坏读写）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TablePersisted {
    /// 列定义 schema 版本：与 TableShell.schema 不一致时整份忽略（列集/默认宽变更后防旧值污染）。
    #[serde(default)]
    schema: u32,
    /// 列 id → 宽度。
    #[serde(default)]
    widths: BTreeMap<String, f32>,
    #[serde(default)]
    density: Option<TableDensity>,
    #[serde(default)]
    sort: Option<TableSortPersisted>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TableSortPersisted {
    col: String,
    asc: bool,
}

/// 排序态：`(列 id, 是否升序)`。None = 未排序（保持视图缓存原序）。
pub type SortState = (&'static str, bool);

/// 行上下文：传给页面行闭包，携带本行的交互/状态位（页面据此画行底色）。
#[derive(Clone, Copy, Debug)]
pub struct RowCtx {
    /// 视图行索引（0..rows_count）。
    pub index: usize,
    /// 该行是否处于键盘焦点（↑↓ 移动的那一行）。
    pub focused: bool,
    /// 该行是否选中。
    pub selected: bool,
}

/// show() 的输出事件（本帧发生的交互；页面在 show 之后统一处理）。
#[derive(Default, Debug)]
pub struct TableOutput {
    /// 左键单击行（未命中已选行时的选择请求也由此给出，页面自行判断去留）。
    pub clicked: Option<usize>,
    pub double_clicked: Option<usize>,
    /// 右键行（页面应先确保该行已选中，再打开菜单内容——菜单本体由组件挂载）。
    pub secondary_clicked: Option<usize>,
    /// Enter 确认的焦点行。
    pub activated: Option<usize>,
    /// 排序态已变化（已写入 `self.sort`）。
    pub sort_changed: bool,
    /// 持久化字段有变（列宽拖拽结束等），应调用 [`TableShell::save_table_state`]。
    pub persist_dirty: bool,
}

/// 「最后活跃表格」的全局记忆键：键盘导航只响应它指向的表格。
const LAST_ACTIVE_TABLE: &str = "design_table_last_active";

pub struct TableShell {
    /// 稳定表标识（持久化键 + egui id_salt），如 "network"。
    pub id: &'static str,
    /// 列定义版本（列集/默认宽变更时 bump，使旧持久化失效）。
    pub schema: u32,
    pub columns: Vec<TableColumn>,
    pub density: TableDensity,
    pub sort: Option<SortState>,
    /// 选中的视图行索引（键盘 Enter 确认后跟随焦点；点击行为由页面覆写）。
    pub selected: Option<usize>,
    /// 键盘焦点行索引（↑↓ 移动，scroll_to_row 跟随）。
    pub focused: Option<usize>,

    // ── 运行时内部态（非持久化）──
    last_widths: Vec<f32>,
    widths_dirty: bool,
    loaded: bool,
}

impl TableShell {
    pub fn new(id: &'static str, columns: Vec<TableColumn>) -> Self {
        Self::new_with_schema(id, 1, columns)
    }

    /// `schema`：列定义版本，列集/默认宽变更时 bump 使旧持久化失效。
    pub fn new_with_schema(id: &'static str, schema: u32, columns: Vec<TableColumn>) -> Self {
        TableShell {
            id,
            schema,
            columns,
            density: TableDensity::default(),
            sort: None,
            selected: None,
            focused: None,
            last_widths: Vec::new(),
            widths_dirty: false,
            loaded: false,
        }
    }

    // ── 持久化（ui-state.json tables.{id}，读改写互不覆盖）──────

    /// 启动时调用一次：读取 tables.{id} 覆盖列初始宽/密度/排序。
    /// 缺文件/缺段/字段不齐 → 保持构造默认值（serde default 向后兼容）。
    pub fn load_table_state(&mut self, config_dir: &std::path::Path) {
        self.loaded = true;
        let state = theme::load_state(config_dir);
        let Some(raw) = state.tables.get(self.id) else {
            return;
        };
        let Ok(p) = serde_json::from_value::<TablePersisted>(raw.clone()) else {
            tracing::warn!("table state for '{}' has unexpected schema, using defaults", self.id);
            return;
        };
        if p.schema != self.schema {
            return; // 列定义已变更，丢弃旧列宽/密度/排序
        }
        for col in &mut self.columns {
            if let Some(w) = p.widths.get(col.id) {
                let clamped = w.clamp(col.min_width, col.max_width);
                col.width = clamped;
            }
        }
        if let Some(d) = p.density {
            self.density = d;
        }
        if let Some(s) = p.sort {
            // 仅接受当前定义中存在的列 id，防旧配置指向已删列
            if let Some(col) = self.columns.iter().find(|c| c.id == s.col) {
                let col = col.id;
                self.sort = Some((col, s.asc));
            }
        }
    }

    /// 把当前列宽/密度/排序写入 ui-state.json tables.{id}（原子写，读改写
    /// 不动 theme/language 等其他段）。触发时机见 [`TableOutput::persist_dirty`]。
    pub fn save_table_state(&self, config_dir: &std::path::Path) {
        let mut widths = BTreeMap::new();
        for col in &self.columns {
            widths.insert(col.id.to_string(), col.width);
        }
        let persisted = TablePersisted {
            schema: self.schema,
            widths,
            density: Some(self.density),
            sort: self.sort.map(|(col, asc)| TableSortPersisted {
                col: col.to_string(),
                asc,
            }),
        };
        let value = serde_json::to_value(&persisted).unwrap_or_default();
        theme::update_state(config_dir, |s| {
            s.tables.insert(self.id.to_string(), value);
        });
    }

    // ── 渲染 ───────────────────────────────────────────────────

    /// 渲染整表。`row_hook` 逐可见行调用（内部只迭代可见行，虚拟化）；
    /// `menu_hook` 在右键菜单展开时调用（参数为视图行索引；不需要就传 `|_,_| {}`）。
    ///
    /// 页面职责：行底色（选中/risk/焦点，用 [`paint_row_bg`]/[`RowCtx`]）、
    /// 单元格内容（零分配）、show 后处理 [`TableOutput`] 并重建视图索引。
    pub fn show<R, M>(
        &mut self,
        ui: &mut egui::Ui,
        rows_count: usize,
        mut row_hook: R,
        mut menu_hook: M,
    ) -> TableOutput
    where
        R: FnMut(&mut egui_extras::TableRow<'_, '_>, RowCtx),
        M: FnMut(&mut egui::Ui, usize),
    {
        debug_assert!(self.loaded, "call load_table_state() once before first show()");
        let pal = theme::palette();
        let table_id = egui::Id::new(self.id);

        let mut out = TableOutput::default();

        // ── 键盘导航（仅本表是「最后活跃表格」且无文本编辑器抢占时响应）──
        let last_active: Option<egui::Id> =
            ui.memory(|m| m.data.get_temp(egui::Id::new(LAST_ACTIVE_TABLE)));
        let is_active = last_active == Some(table_id);
        let mut scroll_target: Option<usize> = None;
        if is_active && rows_count > 0 && !ui.ctx().egui_wants_keyboard_input() {
            let down = ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowDown));
            let up = ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowUp));
            let enter = ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));
            if down || up {
                let cur = self.focused.map(|f| f as isize).unwrap_or(if down { -1 } else { rows_count as isize });
                let next = (cur + if down { 1 } else { -1 }).clamp(0, rows_count as isize - 1) as usize;
                self.focused = Some(next);
                scroll_target = Some(next);
            }
            if enter {
                if let Some(f) = self.focused {
                    if f < rows_count {
                        self.selected = Some(f);
                        out.activated = Some(f);
                    }
                }
            }
        }

        // ── 组建 TableBuilder（列宽：load 时已写回 columns[].width）──
        // 横向滚动：Table 内部 ScrollArea 仅纵轴，外层包 horizontal ScrollArea
        // （表头随横滚移动；Shift+滚轮横滚由 egui 内建支持）。
        let sort = self.sort;
        let mut frame_widths: Vec<f32> = Vec::new();
        let mut clicked_col: Option<&'static str> = None;
        let header_click: Option<&'static str> = egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0; // 行紧贴：高亮贯穿，行高即视觉行距
        let mut builder = TableBuilder::new(ui)
            .id_salt(self.id)
            .striped(false)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(egui::Layout::left_to_right(Align::Center))
            .min_scrolled_height(0.0);
        if let Some(row) = scroll_target {
            builder = builder.scroll_to_row(row, Some(Align::Center));
        }
        for col in &self.columns {
            builder = builder.column(
                Column::initial(col.width)
                    .range(col.min_width..=col.max_width)
                    .clip(true),
            );
        }

        // ── 表头：caption 字号 fg-secondary，点击循环排序 asc→desc→无 ──
            let table = builder.header(HEADER_HEIGHT, |mut header| {
                for col in &self.columns {
                    let sorted = sort.filter(|(cid, _)| *cid == col.id).map(|(_, asc)| asc);
                    let col_id = col.id;
                    header.col(|ui| {
                        if header_cell(ui, &pal, col.title_key, sorted) {
                            clicked_col = Some(col_id);
                        }
                    });
                }
            });

            let _ = table.body(|body| {
                // 快照本帧列宽（拖拽检测 + 写回 + 持久化判定；widths 在 TableBody 上）
                frame_widths = body.widths().to_vec();
                let row_height = self.density.row_height();
                body.rows(row_height, rows_count, |mut row| {
                    let index = row.index();
                    let ctx = RowCtx {
                        index,
                        focused: self.focused == Some(index),
                        selected: self.selected == Some(index),
                    };
                    row_hook(&mut row, ctx);

                    // 行交互（union of cells）。右键菜单每帧挂载以维持开启状态，
                    // 展开时才执行 menu_hook。
                    let rresp = row.response();
                    if rresp.clicked() {
                        out.clicked = Some(index);
                        self.focused = Some(index);
                    }
                    if rresp.double_clicked() {
                        out.double_clicked = Some(index);
                    }
                    if rresp.secondary_clicked() {
                        out.secondary_clicked = Some(index);
                        self.focused = Some(index);
                    }
                    rresp.context_menu(|ui| menu_hook(ui, index));
                });
            });
            clicked_col
            })
            .inner;

        // 列宽变化检测：写回 columns[].width；指针松开后置 persist_dirty
        if self.last_widths.len() == frame_widths.len() && !frame_widths.is_empty() {
            if self.last_widths != frame_widths {
                self.widths_dirty = true;
            }
        } else if self.last_widths.is_empty() && !frame_widths.is_empty() {
            // 首帧建立基线，不触发持久化
        }
        for (col, w) in self.columns.iter_mut().zip(&frame_widths) {
            col.width = *w;
        }
        if !frame_widths.is_empty() {
            if !ui.input(|i| i.pointer.primary_down()) && self.widths_dirty {
                self.widths_dirty = false;
                out.persist_dirty = true;
            }
            self.last_widths = frame_widths;
        }

        if std::env::var("P5_DEBUG").is_ok() {
            eprintln!(
                "[tbl] clicked={:?} sec={:?} act={:?} hdr={:?} sort={:?}",
                out.clicked, out.secondary_clicked, out.activated, header_click, self.sort
            );
        }
        // 点击/激活过本表 → 记为最后活跃表（键盘导航切换目标）
        if out.clicked.is_some()
            || out.double_clicked.is_some()
            || out.secondary_clicked.is_some()
            || out.activated.is_some()
            || header_click.is_some()
        {
            ui.memory_mut(|m| m.data.insert_temp(egui::Id::new(LAST_ACTIVE_TABLE), table_id));
        }

        // 排序循环：无 → 升序 → 降序 → 无；换列从升序开始。
        if let Some(col) = header_click {
            self.sort = match self.sort {
                None => Some((col, true)),
                Some((c, false)) if c == col => None,
                Some((c, true)) if c == col => Some((col, false)),
                Some(_) => Some((col, true)),
            };
            out.sort_changed = true;
        }

        out
    }

    /// 当前排序比较方向提示（页面重建视图索引用）。
    pub fn sort_state(&self) -> Option<SortState> {
        self.sort
    }
}

// ── 页面辅助 ───────────────────────────────────────────────────

/// 表头单元格：caption 字号 + fg-secondary；排序列 accent 色 + ▲/▼；
/// 底部 1px 分隔线（spec §4 TableShell）。返回是否被点击。
/// 用真正的 Button 承载表头（自带 label + click sense）：kittest 等
/// accesskit 查询按 label 命中的就是可点击节点，自动化点击可靠；
/// 隐式 `ui.interact` 无 label，click action 会落在文本节点上被忽略。
fn header_cell(ui: &mut egui::Ui, pal: &Palette, title_key: &str, sorted: Option<bool>) -> bool {
    let cell = ui.max_rect();
    let color = if sorted.is_some() { pal.accent } else { pal.fg_secondary };
    // 按钮文本只含标题（排序箭头独立渲染）——保证 accesskit label 稳定，
    // kittest / 读屏按「列名」精确寻址不随排序态变化。
    let title = rust_i18n::t!(title_key).to_string();
    let btn = egui::Button::new(
        eframe::egui::RichText::new(title)
            .font(fonts::caption())
            .color(color),
    )
    .frame(false);
    let clicked = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.set_min_size(egui::vec2(cell.width() - CELL_PAD_X, cell.height()));
            ui.add_space(CELL_PAD_X);
            let clicked = ui.add(btn).clicked();
            if let Some(asc) = sorted {
                ui.label(
                    eframe::egui::RichText::new(if asc { "▲" } else { "▼" })
                        .font(fonts::caption())
                        .color(pal.accent),
                );
            }
            clicked
        })
        .inner;
    // 底部分隔线（gapless 扩张对齐内建行背景画法）
    let gapless = cell.expand2(0.5 * ui.spacing().item_spacing).round_ui();
    ui.painter().hline(gapless.x_range(), gapless.bottom(), egui::Stroke::new(1.0, pal.border));
    clicked
}

/// 整行软底填充（risk 高亮 role.bg / 自绘选中底色等）。必须在单元格闭包内、
/// 添加内容之前调用；扩张方式与 egui_extras 内建行背景一致（gapless）。
pub fn paint_row_bg(ui: &mut egui::Ui, color: Color32) {
    let mut gapless = ui.max_rect().expand2(0.5 * ui.spacing().item_spacing).round_ui();
    // 纵向各扩 1px：覆盖行缘抗锯齿/亚像素缝，保证选中/risk 高亮贯穿完整
    gapless.extend_with(egui::pos2(gapless.left(), gapless.top() - 1.0));
    gapless.extend_with(egui::pos2(gapless.right(), gapless.bottom() + 1.0));
    ui.painter().rect_filled(gapless, 0.0, color);
}

/// 单元格文本标签（table 字号 + 不可选中，防文本光标）。
pub fn cell_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(
        egui::Label::new(
            eframe::egui::RichText::new(text)
                .font(fonts::table())
                .color(color),
        )
        .selectable(false),
    );
}

/// 单元格等宽文本（时间戳/端点/PID/路径等，spec §3.1 mono 清单）。
pub fn cell_mono_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(
        egui::Label::new(
            eframe::egui::RichText::new(text)
                .font(fonts::mono_table())
                .color(color),
        )
        .selectable(false),
    );
}

/// 单元格左内边距（每个 col 闭包开头先调用）。
pub fn cell_pad(ui: &mut egui::Ui) {
    ui.add_space(CELL_PAD_X);
}

/// 悬停全文提示（截断列用）。零分配版本：闭包内直接引用原 &str，
/// 替代会 String 化的 `Response::on_hover_text`（行闭包红线）。
/// 传入 `row.col(...)` 返回的 Response（owned），原样返回以备链用。
pub fn row_hover(resp: egui::Response, full: &str) -> egui::Response {
    if resp.hovered() && !full.is_empty() {
        resp.on_hover_ui(|h| {
            h.label(eframe::egui::RichText::new(full).font(fonts::mono_caption()));
        })
    } else {
        resp
    }
}
