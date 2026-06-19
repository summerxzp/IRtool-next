use std::collections::HashSet;
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_service::context::AppContext;
use irtool_service::services::autoruns::AutorunsService;
use irtool_service::types::{AutorunItem, ScanOptions, ScanPhase, ScanProgress, SignatureStatus};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;
use crate::widgets::table::{self, SortDir};

// ── Sort Column ────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Enabled,
    Category,
    Signature,
    Entry,
    ImagePath,
    LaunchString,
    Publisher,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SignatureFilter {
    All,
    Valid,
    Invalid,
    Unsigned,
}

// ── AutorunsPageState ─────────────────────────────────────

pub struct AutorunsPageState {
    // Data
    pub items: Vec<AutorunItem>,
    pub last_error: Option<String>,

    // Scan state
    pub scanning: bool,
    pub scan_progress: Option<ScanProgress>,
    pub last_scan_count: Option<usize>,
    pub scan_start_time: Option<Instant>,
    pub last_scan_duration: Option<std::time::Duration>,

    // Filters
    pub search: String,
    pub status_filter: StatusFilter,
    pub sig_filter: SignatureFilter,
    pub category_filter: HashSet<String>,
    pub category_dropdown_open: bool,

    // Table
    pub sort_column: SortColumn,
    pub sort_dir: SortDir,
    pub selected_id: Option<u64>,
    pub selected_item: Option<AutorunItem>,
    pub detail_visible: bool,

    // Context menu
    pub ctx_menu_visible: bool,
    pub ctx_menu_pos: Option<egui::Pos2>,

    // Delete confirmation dialog (DESIGN.md 4.10)
    pub delete_confirm_open: bool,
    pub pending_delete_id: Option<u64>,

    // Refresh channel (set by app to allow async data refresh after hash calc etc.)
    pub refresh_tx: Option<std::sync::mpsc::Sender<Vec<AutorunItem>>>,

    // Sigcheck result dialog
    pub sigcheck_result: Option<(String, String)>,
    pub sigcheck_dialog_open: bool,
    sigcheck_tx: std::sync::mpsc::Sender<(String, String)>,
    sigcheck_rx: std::sync::mpsc::Receiver<(String, String)>,

    // Cache (B2 pattern)
    cached_items: Vec<AutorunItem>,
    cache_dirty: bool,
}

impl Default for AutorunsPageState {
    fn default() -> Self {
        let (sigcheck_tx, sigcheck_rx) = std::sync::mpsc::channel();
        Self {
            items: Vec::new(),
            last_error: None,
            scanning: false,
            scan_progress: None,
            last_scan_count: None,
            scan_start_time: None,
            last_scan_duration: None,
            search: String::new(),
            status_filter: StatusFilter::All,
            sig_filter: SignatureFilter::All,
            category_filter: HashSet::new(),
            category_dropdown_open: false,
            sort_column: SortColumn::Category,
            sort_dir: SortDir::Asc,
            selected_id: None,
            selected_item: None,
            detail_visible: false,
            ctx_menu_visible: false,
            ctx_menu_pos: None,
            delete_confirm_open: false,
            pending_delete_id: None,
            refresh_tx: None,
            sigcheck_result: None,
            sigcheck_dialog_open: false,
            sigcheck_tx,
            sigcheck_rx,
            cached_items: Vec::new(),
            cache_dirty: true,
        }
    }
}

impl AutorunsPageState {
    // ── Event handling ─────────────────────────────────────

    pub fn handle_scan_progress(&mut self, progress: ScanProgress) {
        self.scan_progress = Some(progress);
        self.scanning = !matches!(self.scan_progress.as_ref().map(|p| &p.phase), Some(ScanPhase::Complete));
    }

    pub fn handle_scan_complete(&mut self, count: usize) {
        self.scanning = false;
        self.scan_progress = None;
        self.last_scan_count = Some(count);
        self.last_scan_duration = self.scan_start_time.map(|t| t.elapsed());
        self.scan_start_time = None;
        self.cache_dirty = true;
    }

    pub fn handle_scan_cancelled(&mut self, _task_id: u64) {
        self.scanning = false;
        self.scan_progress = None;
    }

    pub fn handle_scan_failed(&mut self, _task_id: u64, error: String) {
        self.scanning = false;
        self.scan_progress = None;
        self.last_error = Some(error);
    }

    #[allow(dead_code)]
    pub fn handle_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    /// Mark the filtered/sorted cache as dirty. Called externally when items change.
    pub fn mark_cache_dirty(&mut self) {
        self.cache_dirty = true;
    }

    // ── Rendering ──────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // Poll sigcheck results from async tasks
        if let Ok((name, result)) = self.sigcheck_rx.try_recv() {
            self.sigcheck_result = Some((name, result));
            self.sigcheck_dialog_open = true;
        }

        self.render_toolbar(ui, ctx, rt);
        ui.separator();

        // Error banner
        if let Some(ref err) = self.last_error {
            let err = err.clone();
            if crate::widgets::banner::error_banner(ui, &err) {
                self.last_error = None;
            }
            ui.add_space(4.0);
        }

        // Table
        self.render_table(ui);

        // Context menu
        if self.ctx_menu_visible {
            self.render_context_menu(ui, ctx, rt);
        }

        // Delete confirmation dialog (DESIGN.md 4.10)
        if self.delete_confirm_open {
            self.render_delete_confirm(ui, ctx, rt);
        }

        // Sigcheck result dialog
        if self.sigcheck_dialog_open {
            self.render_sigcheck_dialog(ui);
        }
    }

    // ── Toolbar ────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // Scan / Cancel button
            if self.scanning {
                if ui.button("■ 取消扫描").clicked() {
                    if let Some(ref p) = self.scan_progress {
                        let task_id = p.task_id;
                        let ctx_clone = ctx.clone();
                        rt.spawn(async move {
                            let _ = (AutorunsService { ctx: &ctx_clone }).cancel_scan(task_id).await;
                        });
                    }
                }
            } else {
                if ui.button("▶ 开始扫描").clicked() {
                    let ctx_clone = ctx.clone();
                    self.scanning = true;
                    self.scan_progress = None;
                    self.last_error = None;
                    self.scan_start_time = Some(Instant::now());
                    rt.spawn(async move {
                        let opts = ScanOptions {
                            include_hash: false,
                            category_filter: None,
                        };
                        if let Err(e) = (AutorunsService { ctx: &ctx_clone }).scan(opts).await {
                            tracing::error!("autoruns scan failed: {}", e);
                        }
                    });
                }
            }

            ui.separator();

            // Status filter
            let prev_status = self.status_filter;
            egui::ComboBox::from_id_salt("autoruns_status_filter")
                .selected_text(status_filter_label(self.status_filter))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.status_filter, StatusFilter::All, "全部");
                    ui.selectable_value(&mut self.status_filter, StatusFilter::Enabled, "已启用");
                    ui.selectable_value(&mut self.status_filter, StatusFilter::Disabled, "已禁用");
                });
            if prev_status != self.status_filter {
                self.cache_dirty = true;
            }

            // Signature filter
            let prev_sig = self.sig_filter;
            egui::ComboBox::from_id_salt("autoruns_sig_filter")
                .selected_text(sig_filter_label(self.sig_filter))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sig_filter, SignatureFilter::All, "全部签名");
                    ui.selectable_value(&mut self.sig_filter, SignatureFilter::Valid, "已签名");
                    ui.selectable_value(&mut self.sig_filter, SignatureFilter::Invalid, "无效签名");
                    ui.selectable_value(&mut self.sig_filter, SignatureFilter::Unsigned, "未签名");
                });
            if prev_sig != self.sig_filter {
                self.cache_dirty = true;
            }

            // Category filter (multi-select dropdown — button + Area popup)
            let categories = self.collect_categories();
            let cat_label = if self.category_filter.is_empty() {
                "全部分类".to_string()
            } else {
                format!("分类 ({})", self.category_filter.len())
            };
            let cat_btn = ui.add(egui::Button::new(
                egui::RichText::new(&cat_label).color(theme::FG_PRIMARY),
            ));
            if cat_btn.clicked() {
                self.category_dropdown_open = !self.category_dropdown_open;
            }
            let cat_btn_rect = cat_btn.rect;

            if self.category_dropdown_open {
                let popup_id = egui::Id::new("autoruns_cat_popup");
                let popup_pos = egui::pos2(cat_btn_rect.left(), cat_btn_rect.bottom() + 2.0);

                let response = egui::Area::new(popup_id)
                    .order(egui::Order::Foreground)
                    .fixed_pos(popup_pos)
                    .constrain(true)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(cat_btn_rect.width().max(150.0));
                            ui.set_max_height(280.0);
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                // Select All / Deselect All
                                ui.horizontal(|ui| {
                                    if ui.small_button("全选").clicked() {
                                        for cat in &categories {
                                            self.category_filter.insert(cat.clone());
                                        }
                                        self.cache_dirty = true;
                                    }
                                    if ui.small_button("取消全选").clicked() {
                                        self.category_filter.clear();
                                        self.cache_dirty = true;
                                    }
                                });
                                ui.separator();
                                // Per-category checkboxes
                                for cat in &categories {
                                    let mut checked = self.category_filter.contains(cat);
                                    let resp = ui.checkbox(&mut checked, cat);
                                    if resp.changed() {
                                        if checked {
                                            self.category_filter.insert(cat.clone());
                                        } else {
                                            self.category_filter.remove(cat);
                                        }
                                        self.cache_dirty = true;
                                    }
                                }
                            });
                        })
                    });

                // Close when clicking outside
                if ui.input(|i| i.pointer.any_click()) && !response.response.hovered() && !cat_btn.hovered() {
                    self.category_dropdown_open = false;
                }
            }

            ui.separator();

            // Search box
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(200.0)
                    .hint_text("条目 / 路径 / 发布者"),
            );
            if search_resp.changed() {
                self.cache_dirty = true;
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true;
            }

            ui.add_space((ui.available_width() - 100.0).max(0.0));

            // Batch hash
            if ui
                .add_enabled(!self.items.is_empty(), egui::Button::new("# 批量哈希"))
                .clicked()
            {
                let ctx_clone = ctx.clone();
                let entry_ids: Vec<u64> = self
                    .items
                    .iter()
                    .filter(|d| d.image_path.is_some() && d.file_exists && d.sha256.is_none())
                    .map(|d| d.id)
                    .collect();
                if !entry_ids.is_empty() {
                    rt.spawn(async move {
                        let _ = (AutorunsService { ctx: &ctx_clone })
                            .batch_calculate_hash(entry_ids)
                            .await;
                    });
                }
            }
        });
    }

    // ── Table ──────────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui) {
        // Extract Copy fields before borrowing self via get_filtered_sorted_items
        let sc = self.sort_column;
        let sd = self.sort_dir;
        let sel_id = self.selected_id;
        let items = self.get_filtered_sorted_items();

        if items.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.items.is_empty() {
                    ui.label(egui::RichText::new("点击「开始扫描」检测持久化项").color(theme::FG_SECONDARY));
                } else {
                    ui.label(egui::RichText::new("没有匹配当前过滤条件的条目").color(theme::FG_TERTIARY));
                }
            });
            return;
        }

        let mut sort_toggle: Option<SortColumn> = None;
        let mut row_click: Option<u64> = None;
        let mut row_right_click: Option<u64> = None;
        let mut capture_ctx_pos: Option<egui::Pos2> = None;

        let egui_ctx = ui.ctx().clone();

        // Use TableBuilder's built-in scroll (vscroll + hscroll) instead of
        // wrapping in ScrollArea::both(). This makes the table fill the available
        // space and keeps the horizontal scrollbar at the bottom of the panel.
        let available_width = ui.available_width();
        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(40.0).clip(true)) // 启用
            .column(Column::initial(100.0).clip(true)) // 类别
            .column(Column::initial(80.0).clip(true)) // 签名
            .column(Column::initial(200.0).clip(true).resizable(true)) // 条目
            .column(Column::initial(280.0).clip(true).resizable(true)) // 文件路径
            .column(Column::initial(240.0).clip(true).resizable(true)) // 启动命令
            .column(Column::initial(140.0).clip(true)); // 发布者

        let _ = available_width; // suppress unused warning

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    if table::sortable_header(ui, "启用", sc == SortColumn::Enabled, sd) {
                        sort_toggle = Some(SortColumn::Enabled);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "类别", sc == SortColumn::Category, sd) {
                        sort_toggle = Some(SortColumn::Category);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "签名", sc == SortColumn::Signature, sd) {
                        sort_toggle = Some(SortColumn::Signature);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "条目", sc == SortColumn::Entry, sd) {
                        sort_toggle = Some(SortColumn::Entry);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "文件路径", sc == SortColumn::ImagePath, sd) {
                        sort_toggle = Some(SortColumn::ImagePath);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "启动命令", sc == SortColumn::LaunchString, sd) {
                        sort_toggle = Some(SortColumn::LaunchString);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "发布者", sc == SortColumn::Publisher, sd) {
                        sort_toggle = Some(SortColumn::Publisher);
                    }
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let item = &items[row.index()];
                    let is_selected = sel_id == Some(item.id);

                    // Row background tint (file missing / unsigned / disabled)
                    if !item.file_exists {
                        row.col(|ui| {
                            paint_row_bg(ui, theme::SEMANTIC_WARNING, 0.08);
                            cell_enabled(ui, item);
                        });
                    } else if matches!(item.signature, SignatureStatus::Unsigned) {
                        row.col(|ui| {
                            paint_row_bg(ui, theme::SEMANTIC_DANGER, 0.06);
                            cell_enabled(ui, item);
                        });
                    } else if !item.enabled {
                        row.col(|ui| {
                            paint_row_bg(ui, theme::FG_TERTIARY, 0.05);
                            cell_enabled(ui, item);
                        });
                    } else {
                        row.col(|ui| cell_enabled(ui, item));
                    }

                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.category));
                    });
                    row.col(|ui| cell_signature(ui, &item.signature));
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.entry).color(theme::FG_PRIMARY).strong());
                    });
                    row.col(|ui| {
                        let path = item.image_path.as_deref().unwrap_or("");
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(path)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::FG_SECONDARY),
                            )
                            .truncate(),
                        );
                        if !path.is_empty() {
                            resp.on_hover_text(path);
                        }
                    });
                    row.col(|ui| {
                        let cmd = item.launch_string.as_deref().unwrap_or("");
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(cmd)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::FG_TERTIARY),
                            )
                            .truncate(),
                        );
                        if !cmd.is_empty() {
                            resp.on_hover_text(cmd);
                        }
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.publisher));
                    });

                    // Row interaction
                    let row_resp = row.response();
                    if is_selected {
                        row.set_selected(true);
                    }

                    if row_resp.clicked() {
                        if is_selected {
                            row_click = Some(0);
                        } else {
                            row_click = Some(item.id);
                        }
                    }

                    if row_resp.secondary_clicked() {
                        if !is_selected {
                            row_click = Some(item.id);
                        }
                        row_right_click = Some(item.id);
                        if let Some(pos) = egui_ctx.input(|i| i.pointer.interact_pos()) {
                            capture_ctx_pos = Some(pos);
                        }
                    }
                });
            });

        if let Some(pos) = capture_ctx_pos {
            self.ctx_menu_pos = Some(pos);
        }

        if let Some(id) = row_click {
            if id == 0 {
                self.selected_id = None;
                self.selected_item = None;
                self.detail_visible = false;
            } else {
                self.selected_item = self.cached_items.iter().find(|c| c.id == id).cloned();
                self.selected_id = Some(id);
                self.detail_visible = true;
            }
        }

        if row_right_click.is_some() {
            self.ctx_menu_visible = true;
        }

        if let Some(col) = sort_toggle {
            self.toggle_sort(col);
        }
    }

    // ── Context Menu ───────────────────────────────────────

    fn render_context_menu(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let Some(pos) = self.ctx_menu_pos else {
            self.ctx_menu_visible = false;
            return;
        };
        let item = match self.selected_item.clone() {
            Some(i) => i,
            None => {
                self.ctx_menu_visible = false;
                return;
            }
        };

        let mut close_menu = false;
        let mut action: Option<CtxAction> = None;

        egui::Area::new(egui::Id::new("autoruns_ctx_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(200.0);
                    ui.spacing_mut().button_padding = egui::vec2(12.0, 4.0);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if ui.button("复制条目信息").clicked() {
                        action = Some(CtxAction::Copy);
                        close_menu = true;
                    }
                    if ui
                        .add_enabled(item.image_path.is_some(), egui::Button::new("计算哈希"))
                        .clicked()
                    {
                        action = Some(CtxAction::CalcHash);
                        close_menu = true;
                    }
                    if ui
                        .add_enabled(item.image_path.is_some(), egui::Button::new("Sigcheck"))
                        .clicked()
                    {
                        action = Some(CtxAction::Sigcheck);
                        close_menu = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("删除条目").color(theme::SEMANTIC_DANGER),
                            ),
                        )
                        .clicked()
                    {
                        self.pending_delete_id = Some(item.id);
                        self.delete_confirm_open = true;
                        close_menu = true;
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            item.image_path.is_some() && item.file_exists,
                            egui::Button::new("打开文件位置"),
                        )
                        .clicked()
                    {
                        action = Some(CtxAction::OpenExplorer);
                        close_menu = true;
                    }
                    if item.category == "Services" && ui.button("打开服务").clicked() {
                        action = Some(CtxAction::OpenServices);
                        close_menu = true;
                    }
                    if (item.location.contains("HKLM") || item.location.contains("HKCU"))
                        && ui.button("打开注册表").clicked()
                    {
                        action = Some(CtxAction::OpenRegistry);
                        close_menu = true;
                    }
                });
            });

        // Click outside closes menu
        if ui.input(|i| i.pointer.any_click() && !close_menu) {
            if let Some(hover) = ui.ctx().memory(|m| m.area_rect(egui::Id::new("autoruns_ctx_menu"))) {
                if !hover.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or(pos)) {
                    close_menu = true;
                }
            }
        }

        if let Some(act) = action {
            self.execute_ctx_action(act, &item, ctx, rt);
        }

        if close_menu {
            self.ctx_menu_visible = false;
        }
    }

    fn execute_ctx_action(
        &mut self,
        action: CtxAction,
        item: &AutorunItem,
        ctx: &AppContext,
        rt: &tokio::runtime::Handle,
    ) {
        match action {
            CtxAction::Copy => {
                let text = format!(
                    "{} {} {}",
                    item.entry,
                    item.image_path.as_deref().unwrap_or(""),
                    item.publisher
                );
                ui_copy_to_clipboard(rt, text);
            }
            CtxAction::CalcHash => {
                let ctx_clone = ctx.clone();
                let entry_id = item.id;
                let refresh_tx = self.refresh_tx.clone();
                rt.spawn(async move {
                    let _ = (AutorunsService { ctx: &ctx_clone }).calculate_hash(entry_id).await;
                    // Refresh items so the UI shows the newly calculated hash
                    if let Some(tx) = refresh_tx {
                        let items = AutorunsService { ctx: &ctx_clone }
                            .get_result()
                            .await
                            .unwrap_or_default();
                        let _ = tx.send(items);
                    }
                });
            }
            CtxAction::Sigcheck => {
                if let Some(ref path) = item.image_path {
                    let path = path.clone();
                    let entry_name = item.entry.clone();
                    let tx = self.sigcheck_tx.clone();
                    rt.spawn(async move {
                        match AutorunsService::sigcheck(path).await {
                            Ok(output) => {
                                let _ = tx.send((entry_name, output));
                            }
                            Err(e) => {
                                let msg = format!("{}", e);
                                tracing::error!("sigcheck failed: {}", msg);
                                let _ = tx.send((entry_name, format!("Error: {}", msg)));
                            }
                        }
                    });
                }
            }
            CtxAction::OpenExplorer => {
                if let Some(ref path) = item.image_path {
                    if let Err(e) = AutorunsService::open_explorer(path.clone()) {
                        self.last_error = Some(e.to_string());
                    }
                }
            }
            CtxAction::OpenServices => {
                if let Err(e) = AutorunsService::open_services() {
                    self.last_error = Some(e.to_string());
                }
            }
            CtxAction::OpenRegistry => {
                if let Err(e) = AutorunsService::open_regedit(item.location.clone()) {
                    self.last_error = Some(e.to_string());
                }
            }
        }
    }

    // ── Detail Panel ───────────────────────────────────────

    pub fn render_detail_panel(&mut self, ui: &mut egui::Ui, _ctx: &AppContext, _rt: &tokio::runtime::Handle) {
        let item = self.selected_item.clone();
        let Some(item) = item else {
            self.detail_visible = false;
            return;
        };

        egui::ScrollArea::vertical()
            .id_salt("autoruns_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header with close button at top-right (with right margin to avoid scrollbar)
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        // Right margin so close button doesn't overlap with scrollbar
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        // Close button (first in RTL = rightmost)
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_id = None;
                            self.selected_item = None;
                        }
                        // Content fills remaining space to the left
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&item.entry)
                                    .color(theme::FG_PRIMARY)
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                signature_badge(ui, &item.signature);
                                ui.label(egui::RichText::new("·").color(theme::FG_TERTIARY));
                                ui.label(egui::RichText::new(&item.category).color(theme::FG_TERTIARY));
                                if let Some(size) = item.file_size {
                                    ui.label(egui::RichText::new("·").color(theme::FG_TERTIARY));
                                    ui.label(egui::RichText::new(format_size(size)).color(theme::FG_TERTIARY));
                                }
                                if !item.enabled {
                                    ui.label(egui::RichText::new("·").color(theme::FG_TERTIARY));
                                    ui.label(egui::RichText::new("已禁用").color(theme::FG_TERTIARY));
                                }
                            });
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Detail rows
                detail_row(ui, "文件路径", item.image_path.as_deref(), true);
                detail_row(ui, "启动命令", item.launch_string.as_deref(), true);
                detail_row(ui, "位置", Some(&item.location), true);
                detail_row(ui, "发布者", Some(&item.publisher), false);
                detail_row(ui, "描述", Some(&item.description), false);
                detail_row(ui, "时间戳", item.timestamp.as_deref(), false);
                if let Some(ref v) = item.file_version {
                    detail_row(ui, "版本", Some(v), false);
                }
                if let Some(ref s) = item.service_name {
                    detail_row(ui, "服务名", Some(s), false);
                }
                if !item.file_exists {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("! 文件不存在")
                            .color(theme::SEMANTIC_DANGER)
                            .strong(),
                    );
                }
                if let Some(ref h) = item.sha256 {
                    detail_row(ui, "SHA-256", Some(h), true);
                }
                if let Some(ref h) = item.md5 {
                    detail_row(ui, "MD5", Some(h), true);
                }

                // Actions
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let entry_id = item.id;
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("删除").color(theme::SEMANTIC_DANGER),
                            ),
                        )
                        .clicked()
                    {
                        self.pending_delete_id = Some(entry_id);
                        self.delete_confirm_open = true;
                    }
                    if (item.location.contains("HKLM") || item.location.contains("HKCU"))
                        && ui.button("跳转注册表").clicked()
                    {
                        if let Err(e) = AutorunsService::open_regedit(item.location.clone()) {
                            self.last_error = Some(e.to_string());
                        }
                    }
                });
            });
    }

    // ── Delete Confirmation Dialog (DESIGN.md 4.10) ────────

    fn render_delete_confirm(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.delete_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("确认删除")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label("确定要删除此条目吗？此操作不可撤销。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("确认删除").color(theme::SEMANTIC_DANGER),
                            ),
                        )
                        .clicked()
                    {
                        confirmed = true;
                    }
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                });
            });

        if confirmed {
            if let Some(entry_id) = self.pending_delete_id {
                let ctx_clone = ctx.clone();
                let was_selected = self.selected_id == Some(entry_id);
                rt.spawn(async move {
                    match (AutorunsService { ctx: &ctx_clone }).delete_entry(entry_id).await {
                        Ok(result) => {
                            tracing::info!("delete result: success={}, msg={}", result.success, result.message)
                        }
                        Err(e) => tracing::error!("delete failed: {}", e),
                    }
                });
                if was_selected {
                    self.selected_id = None;
                    self.selected_item = None;
                    self.detail_visible = false;
                }
            }
            open = false;
        }
        if cancelled {
            open = false;
        }

        self.delete_confirm_open = open;
        if !open {
            self.pending_delete_id = None;
        }
    }

    // ── Sigcheck Result Dialog ────────────────────────────

    fn render_sigcheck_dialog(&mut self, ui: &mut egui::Ui) {
        let mut open = self.sigcheck_dialog_open;
        let result = self.sigcheck_result.clone();
        let mut close_clicked = false;

        egui::Window::new("Sigcheck 结果")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(600.0)
            .default_height(400.0)
            .show(ui.ctx(), |ui| {
                if let Some((name, text)) = &result {
                    ui.label(egui::RichText::new(format!("文件: {}", name)).strong());
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(text)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::FG_PRIMARY),
                            )
                            .selectable(true),
                        );
                    });
                }
                ui.add_space(8.0);
                if ui.button("关闭").clicked() {
                    close_clicked = true;
                }
            });

        if close_clicked {
            open = false;
        }

        self.sigcheck_dialog_open = open;
    }

    // ── Stats Bar ──────────────────────────────────────────

    pub fn render_stats_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let filtered = self.cached_items.len();
            let total = self.items.len();
            let signed = self
                .items
                .iter()
                .filter(|d| matches!(d.signature, SignatureStatus::Valid { .. }))
                .count();
            let disabled = self.items.iter().filter(|d| !d.enabled).count();

            ui.label(
                egui::RichText::new(format!("当前: {}", filtered))
                    .color(theme::FG_SECONDARY)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new(format!("总数: {}", total))
                    .color(theme::FG_SECONDARY)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new(format!("已签名: {}/{}", signed, total))
                    .color(theme::SEMANTIC_SUCCESS)
                    .size(11.0),
            );
            if disabled > 0 {
                ui.label(
                    egui::RichText::new(format!("已禁用: {}", disabled))
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            }

            ui.add_space((ui.available_width() - 200.0).max(0.0));

            // Scan progress / duration
            if self.scanning {
                if let Some(ref p) = self.scan_progress {
                    let phase_label = phase_label(&p.phase);
                    ui.label(
                        egui::RichText::new(format!("{}… {}/{}", phase_label, p.current, p.total))
                            .color(theme::ACCENT)
                            .size(11.0),
                    );
                } else {
                    ui.label(egui::RichText::new("扫描中…").color(theme::ACCENT).size(11.0));
                }
            } else if let Some(count) = self.last_scan_count {
                let mut text = format!("上次扫描: {} 项", count);
                if let Some(dur) = self.last_scan_duration {
                    let secs = dur.as_secs();
                    text.push_str(&format!("  耗时: {}", format_duration(secs)));
                }
                ui.label(egui::RichText::new(text).color(theme::FG_TERTIARY).size(11.0));
            }
        });
    }

    // ── Helpers ────────────────────────────────────────────

    fn collect_categories(&self) -> Vec<String> {
        let mut set: HashSet<&String> = HashSet::new();
        let mut out = Vec::new();
        for item in &self.items {
            if set.insert(&item.category) {
                out.push(item.category.clone());
            }
        }
        out.sort();
        out
    }

    fn toggle_sort(&mut self, col: SortColumn) {
        if self.sort_column == col {
            self.sort_dir = self.sort_dir.toggle();
        } else {
            self.sort_column = col;
            self.sort_dir = SortDir::Asc;
        }
        self.cache_dirty = true;
    }

    fn get_filtered_sorted_items(&mut self) -> &[AutorunItem] {
        if self.cache_dirty {
            self.cached_items = self.compute_filtered_sorted();
            self.cache_dirty = false;
        }
        &self.cached_items
    }

    fn compute_filtered_sorted(&self) -> Vec<AutorunItem> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<AutorunItem> = self
            .items
            .iter()
            .filter(|item| match self.status_filter {
                StatusFilter::All => true,
                StatusFilter::Enabled => item.enabled,
                StatusFilter::Disabled => !item.enabled,
            })
            .filter(|item| match self.sig_filter {
                SignatureFilter::All => true,
                SignatureFilter::Valid => matches!(item.signature, SignatureStatus::Valid { .. }),
                SignatureFilter::Invalid => matches!(item.signature, SignatureStatus::Invalid { .. }),
                SignatureFilter::Unsigned => matches!(item.signature, SignatureStatus::Unsigned),
            })
            .filter(|item| self.category_filter.is_empty() || self.category_filter.contains(&item.category))
            .filter(|item| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!(
                    "{} {} {} {} {} {}",
                    item.entry,
                    item.image_path.as_deref().unwrap_or(""),
                    item.launch_string.as_deref().unwrap_or(""),
                    item.publisher,
                    item.location,
                    item.category
                )
                .to_lowercase();
                blob.contains(&q)
            })
            .cloned()
            .collect();

        let sc = self.sort_column;
        let sd = self.sort_dir;
        out.sort_by(|a, b| {
            let ord = match sc {
                SortColumn::Enabled => a.enabled.cmp(&b.enabled),
                SortColumn::Category => a.category.cmp(&b.category),
                SortColumn::Signature => sig_sort_key(&a.signature).cmp(&sig_sort_key(&b.signature)),
                SortColumn::Entry => a.entry.cmp(&b.entry),
                SortColumn::ImagePath => a
                    .image_path
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.image_path.as_deref().unwrap_or("")),
                SortColumn::LaunchString => a
                    .launch_string
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.launch_string.as_deref().unwrap_or("")),
                SortColumn::Publisher => a.publisher.cmp(&b.publisher),
            };
            if sd == SortDir::Desc {
                ord.reverse()
            } else {
                ord
            }
        });

        out
    }
}

// ── Context action ────────────────────────────────────────

enum CtxAction {
    Copy,
    CalcHash,
    Sigcheck,
    OpenExplorer,
    OpenServices,
    OpenRegistry,
}

// ── Cell renderers ────────────────────────────────────────

fn cell_enabled(ui: &mut egui::Ui, item: &AutorunItem) {
    let mut checked = item.enabled;
    ui.add_enabled(false, egui::Checkbox::new(&mut checked, ""));
}

fn cell_signature(ui: &mut egui::Ui, sig: &SignatureStatus) {
    match sig {
        SignatureStatus::Valid { signer } => {
            ui.horizontal(|ui| {
                badge::badge(ui, "签", BadgeVariant::Success);
                if !signer.is_empty() {
                    // Use Label with truncate instead of hard-coded char limit
                    ui.add(
                        egui::Label::new(egui::RichText::new(signer).color(theme::FG_SECONDARY).size(11.0)).truncate(),
                    );
                }
            });
        }
        SignatureStatus::Invalid { .. } => {
            badge::badge(ui, "无效", BadgeVariant::Danger);
        }
        SignatureStatus::Unsigned => {
            badge::badge(ui, "未签", BadgeVariant::Warning);
        }
        SignatureStatus::NotVerified => {
            ui.label(egui::RichText::new("—").color(theme::FG_TERTIARY).size(11.0));
        }
    }
}

fn signature_badge(ui: &mut egui::Ui, sig: &SignatureStatus) {
    match sig {
        SignatureStatus::Valid { signer } => {
            badge::badge(ui, "已签名", BadgeVariant::Success);
            if !signer.is_empty() {
                // Show full signer text with wrap, no truncation
                ui.label(
                    egui::RichText::new(format!("({})", signer))
                        .color(theme::FG_TERTIARY)
                        .size(10.0),
                );
            }
        }
        SignatureStatus::Invalid { .. } => badge::badge(ui, "签名无效", BadgeVariant::Danger),
        SignatureStatus::Unsigned => badge::badge(ui, "未签名", BadgeVariant::Warning),
        SignatureStatus::NotVerified => badge::badge(ui, "未验证", BadgeVariant::Default),
    }
}

fn paint_row_bg(ui: &mut egui::Ui, color: egui::Color32, alpha: f32) {
    let _ = (color, alpha);
    // Row tinting handled via row.set_selected; this is a placeholder for
    // custom background which egui_extras doesn't expose easily.
    let _ = ui;
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn format_duration(secs: u64) -> String {
    if secs >= 60 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

fn phase_label(phase: &ScanPhase) -> &'static str {
    match phase {
        ScanPhase::RunningAutorunsc => "运行 autorunsc",
        ScanPhase::ParsingCsv => "解析 CSV",
        ScanPhase::CheckingFiles => "检查文件",
        ScanPhase::EvaluatingRisk => "评估风险",
        ScanPhase::VerifyingSignatures => "验证签名",
        ScanPhase::Complete => "完成",
    }
}

fn sig_sort_key(sig: &SignatureStatus) -> u8 {
    match sig {
        SignatureStatus::Valid { .. } => 0,
        SignatureStatus::NotVerified => 1,
        SignatureStatus::Unsigned => 2,
        SignatureStatus::Invalid { .. } => 3,
    }
}

fn status_filter_label(f: StatusFilter) -> &'static str {
    match f {
        StatusFilter::All => "全部",
        StatusFilter::Enabled => "已启用",
        StatusFilter::Disabled => "已禁用",
    }
}

fn sig_filter_label(f: SignatureFilter) -> &'static str {
    match f {
        SignatureFilter::All => "全部签名",
        SignatureFilter::Valid => "已签名",
        SignatureFilter::Invalid => "无效签名",
        SignatureFilter::Unsigned => "未签名",
    }
}

fn ui_copy_to_clipboard(_rt: &tokio::runtime::Handle, text: String) {
    // Best-effort clipboard copy via arboard; fall back to log.
    // We avoid adding a new dependency by using the OS command.
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("Set-Clipboard -Value '{}'", text.replace('\'', "''")),
            ])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        tracing::info!("clipboard copy (non-windows): {}", text);
    }
}
