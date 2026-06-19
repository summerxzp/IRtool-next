use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_service::context::AppContext;
use irtool_service::services::monitor::MonitorService;
use irtool_service::types::{EventQuery, EventSource, MonitorEvent};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;

// ── Async → UI refresh payload ────────────────────────────

#[derive(Default)]
pub struct DbRefresh {
    pub events: Option<Vec<DbEvent>>,
    pub total_count: Option<u64>,
    pub matched_count: Option<u64>,
    pub type_counts: Option<Vec<(String, u64)>>,
    pub db_size: Option<u64>,
    pub error: Option<String>,
    /// When true, events are appended (load-more) instead of replaced.
    pub append: bool,
}

// ── DbEvent (parsed display model) ────────────────────────

#[derive(Clone, Debug)]
pub struct DbEvent {
    pub record_id: i64,
    pub event_type: String,
    pub timestamp: String,
    pub timestamp_epoch: f64,
    pub process_id: u32,
    pub process_name: String,
    pub process_path: String,
    pub user: String,
    pub query_name: String,
    pub query_results: String,
    pub query_status: u32,
    pub source_ip: String,
    pub source_port: u16,
    pub destination_ip: String,
    pub destination_port: u16,
    pub protocol: String,
    pub initiated: bool,
    pub is_external: bool,
    pub source_process_id: u32,
    pub source_process_name: String,
    pub source_process_path: String,
    pub target_process_id: u32,
    pub target_process_name: String,
    pub target_process_path: String,
    pub start_address: String,
    pub start_module: String,
    pub target_filename: String,
    pub creation_utc_time: String,
    pub source: String,
}

// ── DatabasePageState ─────────────────────────────────────

pub struct DatabasePageState {
    // Data
    pub events: Vec<DbEvent>,
    pub selected_event: Option<DbEvent>,
    pub detail_visible: bool,

    // Filters
    pub source: String,
    pub event_type: String,
    pub process_name: String,
    pub key_field: String,
    pub search_text: String,
    pub load_limit: u32,

    // Search state
    pub loading: bool,
    pub offset: u32,
    pub has_more: bool,
    pub total_count: u64,
    pub matched_count: u64,
    pub has_filters: bool,

    // Stats
    pub type_counts: Vec<(String, u64)>,
    pub db_size: u64,

    // Dialogs
    pub clear_confirm_open: bool,
    pub last_error: Option<String>,

    // Async refresh
    pub refresh_tx: Option<std::sync::mpsc::Sender<DbRefresh>>,
}

impl Default for DatabasePageState {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            selected_event: None,
            detail_visible: false,
            source: "all".to_string(),
            event_type: "all".to_string(),
            process_name: String::new(),
            key_field: String::new(),
            search_text: String::new(),
            load_limit: 1000,
            loading: false,
            offset: 0,
            has_more: false,
            total_count: 0,
            matched_count: 0,
            has_filters: false,
            type_counts: Vec::new(),
            db_size: 0,
            clear_confirm_open: false,
            last_error: None,
            refresh_tx: None,
        }
    }
}

impl DatabasePageState {
    // ── Refresh handling ───────────────────────────────────

    /// Apply an async refresh payload.
    pub fn apply_refresh(&mut self, r: DbRefresh) {
        if let Some(e) = r.events {
            if r.append {
                self.events.extend(e);
            } else {
                self.events = e;
                self.offset = 0;
            }
            self.loading = false;
        }
        if let Some(c) = r.total_count {
            self.total_count = c;
        }
        if let Some(c) = r.matched_count {
            self.matched_count = c;
        }
        if let Some(c) = r.type_counts {
            self.type_counts = c;
        }
        if let Some(s) = r.db_size {
            self.db_size = s;
        }
        if let Some(e) = r.error {
            self.last_error = Some(e);
            self.loading = false;
        }
        // Recompute has_more after events change
        self.has_more = (self.offset as u64 + self.events.len() as u64) < self.matched_count;
    }

    /// Kick off initial load of db_size, total_count, and type_counts.
    pub fn trigger_initial_load(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };

        let ctx1 = ctx.clone();
        let tx1 = tx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx1 };
            match svc.get_db_size().await {
                Ok(s) => {
                    let _ = tx1.send(DbRefresh {
                        db_size: Some(s),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("db size: {}", e),
            }
        });

        let ctx2 = ctx.clone();
        let tx2 = tx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx2 };
            match svc.get_event_count().await {
                Ok(c) => {
                    let _ = tx2.send(DbRefresh {
                        total_count: Some(c),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("event count: {}", e),
            }
        });

        let ctx3 = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx3 };
            match svc.event_type_counts().await {
                Ok(c) => {
                    let _ = tx.send(DbRefresh {
                        type_counts: Some(c),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("event type counts: {}", e),
            }
        });
    }

    /// Kick off an async search. When `offset == 0`, events are replaced;
    /// otherwise results are appended (load-more).
    pub fn trigger_search(&mut self, ctx: &AppContext, rt: &tokio::runtime::Handle, offset: u32) {
        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };

        self.loading = true;
        self.offset = offset;
        self.has_filters = self.compute_has_filters();

        let query = EventQuery {
            source: if self.source == "all" {
                None
            } else {
                Some(self.source.clone())
            },
            event_type: if self.event_type == "all" {
                None
            } else {
                Some(self.event_type.clone())
            },
            process_name: opt_str(&self.process_name),
            key_field: opt_str(&self.key_field),
            is_external: None,
            search_text: opt_str(&self.search_text),
            limit: self.load_limit,
            offset,
        };

        let append = offset > 0;
        let ctx_clone = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx_clone };
            match svc.search_event_page(query).await {
                Ok(page) => {
                    let events: Vec<DbEvent> =
                        page.items.iter().map(monitor_event_to_db_event).collect();
                    let _ = tx.send(DbRefresh {
                        events: Some(events),
                        matched_count: Some(page.total),
                        append,
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx.send(DbRefresh {
                        error: Some(e.to_string()),
                        ..Default::default()
                    });
                }
            }
        });
    }

    // ── Rendering ──────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
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

        self.render_table(ui);

        if self.clear_confirm_open {
            self.render_clear_confirm(ui, ctx, rt);
        }
    }

    // ── Toolbar ────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // Source filter
            ui.label(egui::RichText::new("来源").color(theme::FG_TERTIARY).size(11.0));
            let prev_source = self.source.clone();
            egui::ComboBox::from_id_salt("db_source_filter")
                .selected_text(source_label(&self.source))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.source, "all".to_string(), "全部");
                    ui.selectable_value(&mut self.source, "sysmon".to_string(), "Sysmon");
                    ui.selectable_value(&mut self.source, "dns_client".to_string(), "DNS Client");
                    ui.selectable_value(&mut self.source, "pcap".to_string(), "Pcap");
                    ui.selectable_value(&mut self.source, "net_monitor".to_string(), "网络监控");
                });
            if prev_source != self.source {
                self.has_filters = self.compute_has_filters();
            }

            ui.separator();

            // Event type filter
            ui.label(egui::RichText::new("事件类型").color(theme::FG_TERTIARY).size(11.0));
            let prev_et = self.event_type.clone();
            egui::ComboBox::from_id_salt("db_event_type_filter")
                .selected_text(event_type_filter_label(&self.event_type))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.event_type, "all".to_string(), "全部");
                    ui.selectable_value(&mut self.event_type, "dns".to_string(), "DNS");
                    ui.selectable_value(&mut self.event_type, "dns_client".to_string(), "DNS Client");
                    ui.selectable_value(&mut self.event_type, "network_connect".to_string(), "网络连接");
                    ui.selectable_value(&mut self.event_type, "network_monitor".to_string(), "网络监控");
                    ui.selectable_value(&mut self.event_type, "create_remote_thread".to_string(), "远程线程");
                    ui.selectable_value(&mut self.event_type, "file_create".to_string(), "文件创建");
                    ui.selectable_value(&mut self.event_type, "tls_sni".to_string(), "TLS SNI");
                    ui.selectable_value(&mut self.event_type, "dns_pcap".to_string(), "DNS 抓包");
                });
            if prev_et != self.event_type {
                self.has_filters = self.compute_has_filters();
            }

            ui.separator();

            // Process name
            ui.label(egui::RichText::new("进程名").color(theme::FG_TERTIARY).size(11.0));
            let pn_resp = ui.add(
                egui::TextEdit::singleline(&mut self.process_name)
                    .desired_width(100.0)
                    .hint_text("模糊匹配"),
            );
            if pn_resp.changed() {
                self.has_filters = self.compute_has_filters();
            }

            ui.separator();

            // IP / domain
            ui.label(egui::RichText::new("IP/域名").color(theme::FG_TERTIARY).size(11.0));
            let kf_resp = ui.add(
                egui::TextEdit::singleline(&mut self.key_field)
                    .desired_width(100.0)
                    .hint_text("模糊匹配"),
            );
            if kf_resp.changed() {
                self.has_filters = self.compute_has_filters();
            }

            ui.separator();

            // Full-text search
            ui.label(egui::RichText::new("全文搜索").color(theme::FG_TERTIARY).size(11.0));
            let st_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search_text)
                    .desired_width(120.0)
                    .hint_text("搜索 raw_json"),
            );
            if st_resp.changed() {
                self.has_filters = self.compute_has_filters();
            }

            ui.separator();

            // Load limit
            ui.label(egui::RichText::new("查询数量").color(theme::FG_TERTIARY).size(11.0));
            let mut limit_str = self.load_limit.to_string();
            let limit_resp = ui.add(
                egui::TextEdit::singleline(&mut limit_str)
                    .desired_width(60.0)
                    .font(egui::FontId::monospace(12.0)),
            );
            if limit_resp.changed() {
                if let Ok(v) = limit_str.parse::<u32>() {
                    if v > 0 {
                        self.load_limit = v;
                    }
                }
            }

            ui.separator();

            // Search button
            if ui
                .add_enabled(!self.loading, egui::Button::new("搜索"))
                .clicked()
            {
                self.trigger_search(ctx, rt, 0);
            }

            // Reset button
            if ui.button("重置").clicked() {
                self.source = "all".to_string();
                self.event_type = "all".to_string();
                self.process_name.clear();
                self.key_field.clear();
                self.search_text.clear();
                self.has_filters = false;
            }

            // Load more button
            if self.has_more {
                if ui
                    .add_enabled(!self.loading, egui::Button::new("加载更多"))
                    .clicked()
                {
                    let next_offset = self.offset + self.load_limit;
                    self.trigger_search(ctx, rt, next_offset);
                }
            }

            // Export CSV
            if ui
                .add_enabled(!self.events.is_empty(), egui::Button::new("导出CSV"))
                .clicked()
            {
                self.export_csv(ctx, rt);
            }

            // Clear (danger)
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("清空").color(theme::SEMANTIC_DANGER),
                ))
                .clicked()
            {
                self.clear_confirm_open = true;
            }
        });
    }

    // ── Table ──────────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui) {
        let sel_id = self.selected_event.as_ref().map(|e| e.record_id);
        // Borrow instead of cloning the whole Vec every frame. The table body
        // closure only reads `events` (and `sel_id`); all `&mut self` mutations
        // happen after `body.rows` returns, so there is no borrow conflict.
        let items = &self.events;

        if items.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.loading {
                    ui.label(egui::RichText::new("查询中…").color(theme::FG_SECONDARY));
                } else if self.has_filters {
                    ui.label(
                        egui::RichText::new("没有匹配当前过滤条件的事件")
                            .color(theme::FG_TERTIARY),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("点击「搜索」查询数据库事件").color(theme::FG_SECONDARY),
                    );
                }
            });
            return;
        }

        let mut clicked_id: Option<i64> = None;
        let mut clicked_deselect = false;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(144.0).clip(true)) // 时间
            .column(Column::initial(96.0).clip(true)) // 类型
            .column(Column::initial(176.0).clip(true).resizable(true)) // 目标
            .column(Column::initial(140.0).clip(true)) // 远程地址
            .column(Column::initial(260.0).clip(true).resizable(true)); // 路径

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    ui.label(egui::RichText::new("时间").color(theme::FG_SECONDARY).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("类型").color(theme::FG_SECONDARY).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("目标").color(theme::FG_SECONDARY).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("远程地址").color(theme::FG_SECONDARY).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("路径").color(theme::FG_SECONDARY).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let e = &items[row.index()];
                    let is_selected = sel_id == Some(e.record_id);

                    // 时间
                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(&e.timestamp)
                                .font(egui::FontId::monospace(11.0))
                                .color(theme::FG_SECONDARY),
                        );
                    });
                    // 类型
                    row.col(|ui| {
                        badge::badge(ui, event_type_label(&e.event_type), event_badge_variant(&e.event_type));
                    });
                    // 目标
                    row.col(|ui| {
                        let dest = get_destination(e);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&dest)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::FG_PRIMARY),
                            )
                            .truncate(),
                        );
                    });
                    // 远程地址
                    row.col(|ui| {
                        let addr = get_remote_addr(e);
                        ui.label(
                            egui::RichText::new(&addr)
                                .font(egui::FontId::monospace(11.0))
                                .color(theme::FG_SECONDARY),
                        );
                    });
                    // 路径
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&e.process_path)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::FG_TERTIARY),
                            )
                            .truncate(),
                        );
                        if !e.process_path.is_empty() {
                            resp.on_hover_text(&e.process_path);
                        }
                    });

                    if is_selected {
                        row.set_selected(true);
                    }
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            clicked_deselect = true;
                        } else {
                            clicked_id = Some(e.record_id);
                        }
                    }
                });
            });

        if clicked_deselect {
            self.selected_event = None;
            self.detail_visible = false;
        } else if let Some(id) = clicked_id {
            self.selected_event = self.events.iter().find(|e| e.record_id == id).cloned();
            self.detail_visible = true;
        }
    }

    // ── Detail Panel ───────────────────────────────────────

    pub fn render_detail_panel(&mut self, ui: &mut egui::Ui, _ctx: &AppContext, _rt: &tokio::runtime::Handle) {
        let event = match self.selected_event.clone() {
            Some(e) => e,
            None => {
                self.detail_visible = false;
                return;
            }
        };

        egui::ScrollArea::vertical()
            .id_salt("db_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header with close button at top-right (right_to_left, DESIGN.md 4.7)
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_event = None;
                        }
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                badge::badge(
                                    ui,
                                    event_type_label(&event.event_type),
                                    event_badge_variant(&event.event_type),
                                );
                                ui.label(
                                    egui::RichText::new(format!("来源: {}", event.source))
                                        .color(theme::FG_TERTIARY)
                                        .size(11.0),
                                );
                            });
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(&event.timestamp)
                                    .color(theme::FG_SECONDARY)
                                    .size(11.0),
                            );
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Common fields
                detail_row(ui, "时间", nonempty(&event.timestamp), true);
                if event.process_id > 0 {
                    detail_row(ui, "进程", Some(&format!("{} ({})", event.process_name, event.process_id)), false);
                } else {
                    detail_row(ui, "进程", nonempty(&event.process_name), false);
                }
                detail_row(ui, "路径", nonempty(&event.process_path), true);
                detail_row(ui, "用户", nonempty(&event.user), false);

                // Event-type-specific fields
                match event.event_type.as_str() {
                    "dns" | "dns_client" => {
                        detail_row(ui, "域名", nonempty(&event.query_name), true);
                        detail_row(ui, "解析结果", nonempty(&event.query_results), true);
                        detail_row(ui, "状态", Some(&format_dns_status(event.query_status)), false);
                    }
                    "tls_sni" | "dns_pcap" => {
                        detail_row(ui, "域名", nonempty(&event.query_name), true);
                        detail_row(ui, "结果", nonempty(&event.query_results), false);
                        detail_row(
                            ui,
                            "来源",
                            Some(&format!("{}:{}", event.source_ip, event.source_port)),
                            true,
                        );
                        detail_row(
                            ui,
                            "目标",
                            Some(&format!("{}:{}", event.destination_ip, event.destination_port)),
                            true,
                        );
                        detail_row(ui, "协议", nonempty(&event.protocol), false);
                    }
                    "network_connect" => {
                        detail_row(
                            ui,
                            "来源",
                            Some(&format!("{}:{}", event.source_ip, event.source_port)),
                            true,
                        );
                        detail_row(
                            ui,
                            "目标",
                            Some(&format!("{}:{}", event.destination_ip, event.destination_port)),
                            true,
                        );
                        detail_row(ui, "协议", nonempty(&event.protocol), false);
                        detail_row(ui, "方向", Some(if event.initiated { "出站" } else { "入站" }), false);
                        detail_row(ui, "外部", Some(if event.is_external { "是" } else { "否" }), false);
                    }
                    "network_monitor" => {
                        detail_row(ui, "协议", nonempty(&event.protocol), false);
                        detail_row(
                            ui,
                            "来源",
                            Some(&format!("{}:{}", event.source_ip, event.source_port)),
                            true,
                        );
                        detail_row(
                            ui,
                            "目标",
                            Some(&format!("{}:{}", event.destination_ip, event.destination_port)),
                            true,
                        );
                    }
                    "create_remote_thread" => {
                        detail_row(ui, "源进程", nonempty(&event.source_process_name), false);
                        detail_row(ui, "源路径", nonempty(&event.source_process_path), true);
                        detail_row(ui, "目标进程", nonempty(&event.target_process_name), false);
                        detail_row(ui, "目标路径", nonempty(&event.target_process_path), true);
                        detail_row(ui, "起始地址", nonempty(&event.start_address), true);
                        detail_row(ui, "起始模块", nonempty(&event.start_module), true);
                    }
                    "file_create" => {
                        detail_row(ui, "文件名", nonempty(&event.target_filename), true);
                        detail_row(ui, "创建时间", nonempty(&event.creation_utc_time), false);
                    }
                    _ => {}
                }

                // Footer
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("来源: {} | DB ID: {}", event.source, event.record_id))
                        .color(theme::FG_TERTIARY)
                        .size(10.0),
                );
            });
    }

    // ── Stats Bar ──────────────────────────────────────────

    pub fn render_stats_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.has_filters {
                ui.label(
                    egui::RichText::new(format!("已匹配 {} 条", self.matched_count))
                        .color(theme::FG_SECONDARY)
                        .size(11.0),
                );
                ui.label(
                    egui::RichText::new(format!("| 数据库总计 {} 条", self.total_count))
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!("总计 {} 条", self.total_count))
                        .color(theme::FG_SECONDARY)
                        .size(11.0),
                );
            }

            ui.separator();

            ui.label(
                egui::RichText::new(format!("DB {}", theme::fmt_bytes(self.db_size)))
                    .color(theme::FG_TERTIARY)
                    .size(11.0),
            );

            ui.separator();

            // Type counts
            for (t, c) in self.type_counts.iter().take(6) {
                ui.label(
                    egui::RichText::new(format!("{} {}", event_type_label(t), c))
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            }
        });
    }

    // ── Clear Confirmation Dialog (DESIGN.md 4.10) ────────

    fn render_clear_confirm(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.clear_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("确认清空数据库")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label("将删除数据库中所有事件记录。此操作不可撤销。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("清空").color(theme::SEMANTIC_DANGER),
                            ),
                        )
                        .clicked()
                    {
                        confirmed = true;
                    }
                });
            });

        if confirmed {
            self.clear_events_async(ctx, rt);
            open = false;
        }
        if cancelled {
            open = false;
        }

        self.clear_confirm_open = open;
    }

    // ── Helpers ────────────────────────────────────────────

    fn compute_has_filters(&self) -> bool {
        self.source != "all"
            || self.event_type != "all"
            || !self.process_name.trim().is_empty()
            || !self.key_field.trim().is_empty()
            || !self.search_text.trim().is_empty()
    }

    fn clear_events_async(&mut self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };

        // Clear local state immediately
        self.events.clear();
        self.selected_event = None;
        self.detail_visible = false;
        self.offset = 0;
        self.has_more = false;
        self.matched_count = 0;

        let ctx_clone = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx_clone };
            match svc.clear_events().await {
                Ok(count) => {
                    tracing::info!("cleared {} events", count);
                    // Refresh db_size and total_count after clearing
                    let svc2 = MonitorService { ctx: &ctx_clone };
                    let db_size = svc2.get_db_size().await.unwrap_or(0);
                    let _ = tx.send(DbRefresh {
                        events: Some(Vec::new()),
                        total_count: Some(0),
                        matched_count: Some(0),
                        db_size: Some(db_size),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx.send(DbRefresh {
                        error: Some(e.to_string()),
                        ..Default::default()
                    });
                }
            }
        });
    }

    fn export_csv(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let events = self.events.clone();
        let dir = ctx.app_dirs.root().to_path_buf();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("database_export_{}.csv", secs));
        rt.spawn_blocking(move || match write_csv(&path, &events) {
            Ok(()) => {
                tracing::info!("exported {} events to {}", events.len(), path.display())
            }
            Err(e) => tracing::error!("csv export failed: {}", e),
        });
    }
}

// ── Free helpers ──────────────────────────────────────────

/// Parse a `MonitorEvent` (from the database) into a display `DbEvent`.
fn monitor_event_to_db_event(me: &MonitorEvent) -> DbEvent {
    let raw: serde_json::Value = serde_json::from_str(&me.raw_json).unwrap_or_default();
    let source_str = format!("{:?}", me.source); // "Sysmon" / "DnsClient" / "NetMonitor" / "Pcap"
    let timestamp = theme::fmt_time_millis(me.timestamp);

    // Helpers to extract typed values from JSON
    fn s(v: &serde_json::Value, key: &str) -> String {
        v.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }
    fn n(v: &serde_json::Value, key: &str) -> u32 {
        v.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as u32
    }
    fn n16(v: &serde_json::Value, key: &str) -> u16 {
        v.get(key).and_then(|v| v.as_u64()).unwrap_or(0) as u16
    }
    fn f(v: &serde_json::Value, key: &str) -> f64 {
        v.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }

    let mut event = DbEvent {
        record_id: me.id,
        event_type: me.event_type.clone(),
        timestamp,
        timestamp_epoch: me.timestamp as f64 / 1000.0,
        process_id: 0,
        process_name: me.process_name.clone(),
        process_path: String::new(),
        user: String::new(),
        query_name: String::new(),
        query_results: String::new(),
        query_status: 0,
        source_ip: String::new(),
        source_port: 0,
        destination_ip: String::new(),
        destination_port: 0,
        protocol: String::new(),
        initiated: false,
        is_external: false,
        source_process_id: 0,
        source_process_name: String::new(),
        source_process_path: String::new(),
        target_process_id: 0,
        target_process_name: String::new(),
        target_process_path: String::new(),
        start_address: String::new(),
        start_module: String::new(),
        target_filename: String::new(),
        creation_utc_time: String::new(),
        source: source_str,
    };

    match me.source {
        EventSource::Pcap => {
            let event_kind = s(&raw, "event_kind");
            let is_tls = event_kind == "tls_sni";
            event.event_type = if is_tls { "tls_sni".to_string() } else { "dns_pcap".to_string() };
            let domain = s(&raw, "domain");
            event.query_name = if domain.is_empty() { me.key_field.clone() } else { domain };
            event.query_results = s(&raw, "query_type");
            event.source_ip = s(&raw, "src_ip");
            event.source_port = n16(&raw, "src_port");
            event.destination_ip = s(&raw, "dst_ip");
            event.destination_port = n16(&raw, "dst_port");
            event.protocol = if is_tls { "TCP".to_string() } else { "UDP".to_string() };
            event.initiated = true;
        }
        EventSource::NetMonitor => {
            event.event_type = "network_monitor".to_string();
            event.process_id = n(&raw, "pid");
            let pname = s(&raw, "process_name");
            event.process_name = if pname.is_empty() { me.process_name.clone() } else { pname };
            event.process_path = s(&raw, "process_path");
            if let Some(local) = raw.get("local") {
                event.source_ip = s(local, "addr");
                event.source_port = n16(local, "port");
            }
            if let Some(remote) = raw.get("remote") {
                event.destination_ip = s(remote, "addr");
                event.destination_port = n16(remote, "port");
            }
            if let Some(proto) = raw.get("proto").and_then(|v| v.as_str()) {
                event.protocol = proto.to_uppercase();
            }
            event.initiated = true;
        }
        EventSource::Sysmon | EventSource::DnsClient => {
            event.process_id = n(&raw, "process_id");
            let pname = s(&raw, "process_name");
            event.process_name = if pname.is_empty() { me.process_name.clone() } else { pname };
            event.process_path = s(&raw, "process_path");
            event.user = s(&raw, "user");
            event.query_name = s(&raw, "query_name");
            event.query_results = s(&raw, "query_results");
            event.query_status = n(&raw, "query_status");
            event.source_ip = s(&raw, "source_ip");
            event.source_port = n16(&raw, "source_port");
            event.destination_ip = s(&raw, "destination_ip");
            event.destination_port = n16(&raw, "destination_port");
            event.protocol = s(&raw, "protocol");
            event.initiated = raw.get("initiated").and_then(|v| v.as_bool()).unwrap_or(false);
            event.is_external = raw.get("is_external").and_then(|v| v.as_bool()).unwrap_or(false);
            event.source_process_id = n(&raw, "source_process_id");
            event.source_process_name = s(&raw, "source_process_name");
            event.source_process_path = s(&raw, "source_process_path");
            event.target_process_id = n(&raw, "target_process_id");
            event.target_process_name = s(&raw, "target_process_name");
            event.target_process_path = s(&raw, "target_process_path");
            event.start_address = s(&raw, "start_address");
            event.start_module = s(&raw, "start_module");
            event.target_filename = s(&raw, "target_filename");
            event.creation_utc_time = s(&raw, "creation_utc_time");
            // Override timestamp if raw has timestamp_epoch
            let ts_epoch = f(&raw, "timestamp_epoch");
            if ts_epoch > 0.0 {
                event.timestamp_epoch = ts_epoch;
                event.timestamp = theme::fmt_time(ts_epoch as u64);
            }
        }
    }
    event
}

/// Chinese label for an event type identifier.
fn event_type_label(et: &str) -> &str {
    match et {
        "process_create" => "进程创建",
        "file_create_time" => "文件创建时间修改",
        "network_connect" => "网络连接",
        "process_terminate" => "进程终止",
        "driver_load" => "驱动加载",
        "image_load" => "DLL加载",
        "create_remote_thread" => "远程线程",
        "raw_access_read" => "原始磁盘访问",
        "process_access" => "进程访问",
        "file_create" => "文件创建",
        "registry_event" => "注册表事件",
        "file_create_stream_hash" => "文件流哈希",
        "pipe_event" => "管道事件",
        "wmi_event" => "WMI事件",
        "dns" => "DNS查询",
        "dns_client" => "DNS-Client",
        "file_delete" => "文件删除",
        "clipboard_change" => "剪贴板变化",
        "process_tampering" => "进程篡改",
        "file_delete_detected" => "文件删除检测",
        "unknown" => "未知",
        "tls_sni" => "TLS SNI",
        "dns_pcap" => "DNS抓包",
        "network_monitor" => "网络监控",
        _ => et,
    }
}

/// Compute the "目标" (destination) column value for a table row.
fn get_destination(e: &DbEvent) -> String {
    match e.event_type.as_str() {
        "network_connect" => {
            if e.destination_ip.is_empty() && e.destination_port == 0 {
                String::new()
            } else {
                format!("{}:{}", e.destination_ip, e.destination_port)
            }
        }
        "dns" | "dns_client" | "tls_sni" | "dns_pcap" => e.query_name.clone(),
        "create_remote_thread" => {
            if e.source_process_name.is_empty() && e.target_process_name.is_empty() {
                String::new()
            } else {
                format!("{} → {}", e.source_process_name, e.target_process_name)
            }
        }
        "file_create" => e.target_filename.clone(),
        _ => {
            if !e.query_name.is_empty() {
                e.query_name.clone()
            } else {
                e.process_name.clone()
            }
        }
    }
}

/// Compute the "远程地址" (remote address) column value.
fn get_remote_addr(e: &DbEvent) -> String {
    if !e.destination_ip.is_empty() {
        if e.destination_port > 0 {
            format!("{}:{}", e.destination_ip, e.destination_port)
        } else {
            e.destination_ip.clone()
        }
    } else if !e.source_ip.is_empty() {
        e.source_ip.clone()
    } else {
        String::new()
    }
}

/// Badge variant for an event type.
fn event_badge_variant(et: &str) -> BadgeVariant {
    match et {
        "dns" | "dns_client" | "tls_sni" | "dns_pcap" => BadgeVariant::Success,
        "network_connect" | "network_monitor" => BadgeVariant::Info,
        "create_remote_thread" => BadgeVariant::Danger,
        "file_create" | "file_delete" | "file_delete_detected" => BadgeVariant::Warning,
        _ => BadgeVariant::Default,
    }
}

/// Label for the source filter ComboBox.
fn source_label(s: &str) -> &str {
    match s {
        "all" => "全部",
        "sysmon" => "Sysmon",
        "dns_client" => "DNS Client",
        "pcap" => "Pcap",
        "net_monitor" => "网络监控",
        _ => s,
    }
}

/// Label for the event-type filter ComboBox.
fn event_type_filter_label(et: &str) -> &str {
    match et {
        "all" => "全部",
        "dns" => "DNS",
        "dns_client" => "DNS Client",
        "network_connect" => "网络连接",
        "network_monitor" => "网络监控",
        "create_remote_thread" => "远程线程",
        "file_create" => "文件创建",
        "tls_sni" => "TLS SNI",
        "dns_pcap" => "DNS 抓包",
        _ => et,
    }
}

fn format_dns_status(status: u32) -> String {
    if status == 0 {
        "成功 (0)".to_string()
    } else {
        format!("错误 ({})", status)
    }
}

/// Return Some(value) only when the string is non-empty.
fn nonempty(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Convert a trimmed string to Option<String> (None if empty).
fn opt_str(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn write_csv(path: &std::path::Path, events: &[DbEvent]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "timestamp,event_type,source,process_name,process_id,destination,query_name,source_ip,destination_ip,destination_port,protocol,process_path"
    )?;
    for e in events {
        let dest = get_destination(e);
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_escape(&e.timestamp),
            csv_escape(&e.event_type),
            csv_escape(&e.source),
            csv_escape(&e.process_name),
            e.process_id,
            csv_escape(&dest),
            csv_escape(&e.query_name),
            csv_escape(&e.source_ip),
            csv_escape(&e.destination_ip),
            e.destination_port,
            csv_escape(&e.protocol),
            csv_escape(&e.process_path),
        )?;
    }
    Ok(())
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
