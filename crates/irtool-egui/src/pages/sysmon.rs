use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_service::context::AppContext;
use irtool_service::services::monitor::MonitorService;
use irtool_service::services::pcap::PcapService;
use irtool_service::services::process::ProcessService;
use irtool_service::services::sysmon::SysmonService;
use irtool_service::types::PcapConfig;
use irtool_service::types::{
    EventConfigEntry, PcapEvent, PcapEventKind, ProcessChain, SysmonEvent, SysmonEventType, SysmonStatus,
};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;

// ── Async → UI refresh payload ────────────────────────────

#[derive(Default)]
pub struct SysmonRefresh {
    pub status: Option<SysmonStatus>,
    pub configs: Option<Vec<EventConfigEntry>>,
    pub log_max_size: Option<u64>,
    pub events: Option<Vec<SysmonEvent>>,
    pub chain: Option<ProcessChain>,
}

const MAX_EVENTS: usize = 10000;
// Synthetic record_id base for pcap-derived events (avoids collision with real sysmon record ids).
// pcap_seq 在应用生命周期内不会接近 u64::MAX，无需担心溢出。
// 即使每秒处理 100 万事件，也需要约 584 年才能溢出。
const PCAP_ID_BASE: u64 = 1_000_000_000;

// ── SysmonPageState ───────────────────────────────────────

pub struct SysmonPageState {
    // Data
    pub events: VecDeque<SysmonEvent>,
    pub collecting: bool,
    pub start_time: Option<Instant>,
    pub sysmon_status: Option<SysmonStatus>,
    pub event_configs: Vec<EventConfigEntry>,
    pub enabled_event_keys: Vec<String>,
    pub search: String,
    pub event_type_filter: HashSet<SysmonEventType>,
    pub event_type_dropdown_open: bool,
    pub external_only: bool,
    pub selected_event: Option<SysmonEvent>,
    pub detail_visible: bool,
    pub last_error: Option<String>,
    pub log_max_size_mb: u64,
    pub process_chain: Option<ProcessChain>,

    // Helpers
    selected_record_id: Option<u64>,
    config_dialog_open: bool,
    clear_confirm_open: bool,
    uninstall_confirm_open: bool,
    install_confirm_open: bool,
    /// 标记安装确认来自"开始采集"按钮，安装完成后自动开始采集
    pending_start_collect: bool,
    pcap_seq: u64,
    chain_pid: Option<u32>,
    pub refresh_tx: Option<std::sync::mpsc::Sender<SysmonRefresh>>,
    cached_filtered: Vec<SysmonEvent>,
    cache_dirty: bool,
}

impl Default for SysmonPageState {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            collecting: false,
            start_time: None,
            sysmon_status: None,
            event_configs: Vec::new(),
            enabled_event_keys: Vec::new(),
            search: String::new(),
            event_type_filter: HashSet::new(),
            event_type_dropdown_open: false,
            external_only: false,
            selected_event: None,
            detail_visible: false,
            last_error: None,
            log_max_size_mb: 0,
            process_chain: None,
            selected_record_id: None,
            config_dialog_open: false,
            clear_confirm_open: false,
            uninstall_confirm_open: false,
            install_confirm_open: false,
            pending_start_collect: false,
            pcap_seq: 0,
            chain_pid: None,
            refresh_tx: None,
            cached_filtered: Vec::new(),
            cache_dirty: true,
        }
    }
}

impl SysmonPageState {
    // ── Event handling ─────────────────────────────────────

    pub fn handle_sysmon_event(&mut self, event: SysmonEvent) {
        self.events.push_back(event);
        self.trim_events();
        self.mark_cache_dirty();
    }

    pub fn handle_pcap_event(&mut self, event: PcapEvent) {
        let id = PCAP_ID_BASE + self.pcap_seq;
        self.pcap_seq += 1;
        self.events.push_back(pcap_to_sysmon_event(event, id));
        self.trim_events();
        self.mark_cache_dirty();
    }

    fn trim_events(&mut self) {
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    /// Mark the filtered cache as dirty. Called externally when data changes.
    pub fn mark_cache_dirty(&mut self) {
        self.cache_dirty = true;
    }

    /// Apply an async refresh payload (status / configs / log size / events / chain).
    pub fn apply_refresh(&mut self, r: SysmonRefresh) {
        if let Some(s) = r.status {
            self.sysmon_status = Some(s);
        }
        if let Some(c) = r.configs {
            if self.enabled_event_keys.is_empty() {
                self.enabled_event_keys = c.iter().filter(|e| e.default_enabled).map(|e| e.key.clone()).collect();
            }
            self.event_configs = c;
        }
        if let Some(s) = r.log_max_size {
            self.log_max_size_mb = s;
        }
        if let Some(e) = r.events {
            self.events = e.into();
            self.cache_dirty = true;
        }
        if let Some(c) = r.chain {
            self.process_chain = Some(c);
        }
    }

    /// Kick off async fetches for sysmon status, default event configs, and log max size.
    pub fn refresh_status(&mut self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // Sync the collecting flag from the live polling state (cheap atomic read).
        self.collecting = SysmonService { ctx }.is_subscribing();

        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };

        let ctx1 = ctx.clone();
        let tx1 = tx.clone();
        rt.spawn(async move {
            let svc = SysmonService { ctx: &ctx1 };
            match svc.status().await {
                Ok(s) => {
                    let _ = tx1.send(SysmonRefresh {
                        status: Some(s),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("sysmon status: {}", e),
            }
        });

        let tx2 = tx.clone();
        rt.spawn(async move {
            match SysmonService::default_event_configs().await {
                Ok(c) => {
                    let _ = tx2.send(SysmonRefresh {
                        configs: Some(c),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("sysmon configs: {}", e),
            }
        });

        let ctx3 = ctx.clone();
        let tx3 = tx.clone();
        rt.spawn(async move {
            let svc = SysmonService { ctx: &ctx3 };
            match svc.get_log_max_size().await {
                Ok(s) => {
                    let _ = tx3.send(SysmonRefresh {
                        log_max_size: Some(s),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("sysmon log max size: {}", e),
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

        if self.config_dialog_open {
            self.render_config_dialog(ui, ctx, rt);
        }
        if self.clear_confirm_open {
            self.render_clear_confirm(ui);
        }
        if self.uninstall_confirm_open {
            self.render_uninstall_confirm(ui, ctx, rt);
        }
        if self.install_confirm_open {
            self.render_install_confirm(ui, ctx, rt);
        }
    }

    // ── Toolbar ────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // Start / Stop collect
            if self.collecting {
                if ui.button("■ 停止采集").clicked() {
                    self.collecting = false;
                    self.start_time = None;
                    let ctx_clone = ctx.clone();
                    rt.spawn(async move {
                        if let Err(e) = (SysmonService { ctx: &ctx_clone }).stop_subscription().await {
                            tracing::error!("stop subscription: {}", e);
                        }
                        // 停止 pcap
                        let pcap_svc = PcapService { ctx: &ctx_clone };
                        if let Err(e) = pcap_svc.stop().await {
                            tracing::warn!("pcap stop failed: {}", e);
                        }
                    });
                }
            } else if ui.button("▶ 开始采集").clicked() {
                let installed = self.sysmon_status.as_ref().is_some_and(|s| s.installed);
                if !installed {
                    // Sysmon 未安装，弹窗确认是否安装后开始采集
                    self.install_confirm_open = true;
                    self.pending_start_collect = true;
                } else {
                    let ids = self.enabled_event_ids();
                    let ctx_clone = ctx.clone();
                    self.collecting = true;
                    self.start_time = Some(Instant::now());
                    self.last_error = None;
                    rt.spawn(async move {
                        if let Err(e) = (SysmonService { ctx: &ctx_clone })
                            .start_subscription(ids, Some(500))
                            .await
                        {
                            tracing::error!("start subscription: {}", e);
                        }
                        // 启动 pcap（根据 monitor 配置）
                        // 参考 ui/src/features/log-collector/hooks.ts 的逻辑
                        let monitor_svc = MonitorService { ctx: &ctx_clone };
                        if let Ok(config) = monitor_svc.get_config().await {
                            if config.enable_sni || config.enable_dns_pcap {
                                let pcap_svc = PcapService { ctx: &ctx_clone };
                                let pcap_config = PcapConfig {
                                    enable_sni: config.enable_sni,
                                    enable_dns_pcap: config.enable_dns_pcap,
                                    adapter_ip: config.adapter_ip.clone(),
                                    max_duration_secs: config.max_duration_secs,
                                };
                                if let Err(e) = pcap_svc.start(pcap_config).await {
                                    tracing::warn!("pcap start failed: {}", e);
                                }
                            }
                        }
                    });
                }
            }

            // Load history
            if ui.button("加载历史").clicked() {
                let ids = self.enabled_event_ids();
                let ctx_clone = ctx.clone();
                let tx = self.refresh_tx.clone();
                rt.spawn(async move {
                    let svc = SysmonService { ctx: &ctx_clone };
                    match svc.get_existing_events(1000, ids).await {
                        Ok(events) => {
                            if let Some(tx) = tx {
                                let _ = tx.send(SysmonRefresh {
                                    events: Some(events),
                                    ..Default::default()
                                });
                            }
                        }
                        Err(e) => tracing::error!("get existing events: {}", e),
                    }
                });
            }

            ui.separator();

            // Event type multi-select dropdown (Button + Area popup, see DESIGN.md 4.5)
            let types = self.collect_event_types();
            let type_label = if self.event_type_filter.is_empty() {
                "全部类型".to_string()
            } else {
                format!("类型 ({})", self.event_type_filter.len())
            };
            let type_btn = ui.add(egui::Button::new(
                egui::RichText::new(&type_label).color(theme::fg_primary()),
            ));
            if type_btn.clicked() {
                self.event_type_dropdown_open = !self.event_type_dropdown_open;
            }
            let btn_rect = type_btn.rect;

            if self.event_type_dropdown_open {
                let popup_id = egui::Id::new("sysmon_type_popup");
                let popup_pos = egui::pos2(btn_rect.left(), btn_rect.bottom() + 2.0);

                let response = egui::Area::new(popup_id)
                    .order(egui::Order::Foreground)
                    .fixed_pos(popup_pos)
                    .constrain(true)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(btn_rect.width().max(150.0));
                            ui.set_max_height(280.0);
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.small_button("全选").clicked() {
                                        for t in &types {
                                            self.event_type_filter.insert(t.clone());
                                        }
                                        self.cache_dirty = true;
                                    }
                                    if ui.small_button("取消全选").clicked() {
                                        self.event_type_filter.clear();
                                        self.cache_dirty = true;
                                    }
                                });
                                ui.separator();
                                for t in &types {
                                    let mut checked = self.event_type_filter.contains(t);
                                    let resp = ui.checkbox(&mut checked, t.label());
                                    if resp.changed() {
                                        if checked {
                                            self.event_type_filter.insert(t.clone());
                                        } else {
                                            self.event_type_filter.remove(t);
                                        }
                                        self.cache_dirty = true;
                                    }
                                }
                            });
                        })
                    });

                if ui.input(|i| i.pointer.any_click()) && !response.response.hovered() && !type_btn.hovered() {
                    self.event_type_dropdown_open = false;
                }
            }

            ui.separator();

            // External only
            let prev_ext = self.external_only;
            ui.checkbox(&mut self.external_only, "仅外部流量");
            if prev_ext != self.external_only {
                self.cache_dirty = true;
            }

            ui.separator();

            // Search box
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(180.0)
                    .hint_text("进程 / IP / 域名 / 路径"),
            );
            if search_resp.changed() {
                self.cache_dirty = true;
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true;
            }

            ui.separator();

            // Config
            if ui.button("配置").clicked() {
                self.config_dialog_open = true;
            }

            // Install / Uninstall
            let installed = self.sysmon_status.as_ref().is_some_and(|s| s.installed);
            if installed {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("卸载").color(theme::semantic_danger()),
                    ))
                    .clicked()
                {
                    self.uninstall_confirm_open = true;
                }
            } else if ui.button("安装 Sysmon").clicked() {
                self.install_confirm_open = true;
                self.pending_start_collect = false;
            }

            // Export
            if ui
                .add_enabled(!self.events.is_empty(), egui::Button::new("导出"))
                .clicked()
            {
                self.export_csv(ctx, rt);
            }

            // Clear
            if ui.button("清除").clicked() {
                self.clear_confirm_open = true;
            }
        });
    }

    // ── Table ──────────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui) {
        let sel_rid = self.selected_record_id;
        let items = self.get_filtered_sorted();

        if items.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.events.is_empty() {
                    ui.label(egui::RichText::new("点击「开始采集」或「加载历史」获取事件").color(theme::fg_secondary()));
                } else {
                    ui.label(egui::RichText::new("没有匹配当前过滤条件的事件").color(theme::fg_tertiary()));
                }
            });
            return;
        }

        let mut clicked_rid: Option<u64> = None;
        let mut clicked_deselect = false;

        // TableBuilder built-in scroll (DESIGN.md 4.6 — no outer ScrollArea).
        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(144.0).clip(true)) // 时间
            .column(Column::initial(96.0).clip(true)) // 类型
            .column(Column::initial(176.0).clip(true).resizable(true)) // 目标
            .column(Column::initial(300.0).clip(true).resizable(true)); // 路径

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    ui.label(egui::RichText::new("时间").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("类型").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("目标").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("路径").color(theme::fg_secondary()).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let e = &items[row.index()];
                    let is_selected = sel_rid == e.record_id;

                    // 时间
                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(fmt_event_time(e))
                                .font(egui::FontId::monospace(11.0))
                                .color(theme::fg_secondary()),
                        );
                    });
                    // 类型
                    row.col(|ui| {
                        badge::badge(ui, e.event_type.label(), event_badge_variant(&e.event_type));
                    });
                    // 目标
                    row.col(|ui| {
                        let dest = destination_for(e);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&dest)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_primary()),
                            )
                            .truncate(),
                        );
                    });
                    // 路径
                    row.col(|ui| {
                        let p = path_for(e);
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&p)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_tertiary()),
                            )
                            .truncate(),
                        );
                        if !p.is_empty() {
                            resp.on_hover_text(&p);
                        }
                    });

                    if is_selected {
                        row.set_selected(true);
                    }
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            clicked_deselect = true;
                        } else if let Some(rid) = e.record_id {
                            clicked_rid = Some(rid);
                        }
                    }
                });
            });

        if clicked_deselect {
            self.selected_event = None;
            self.selected_record_id = None;
            self.detail_visible = false;
            self.process_chain = None;
            self.chain_pid = None;
        } else if let Some(rid) = clicked_rid {
            self.selected_event = self.cached_filtered.iter().find(|e| e.record_id == Some(rid)).cloned();
            self.selected_record_id = Some(rid);
            self.detail_visible = true;
            self.process_chain = None;
            self.chain_pid = None;
        }
    }

    // ── Detail Panel ───────────────────────────────────────

    pub fn render_detail_panel(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let event = match self.selected_event.clone() {
            Some(e) => e,
            None => {
                self.detail_visible = false;
                return;
            }
        };

        // Fetch process chain when the selected PID changes (DESIGN.md: PID > 0).
        let pid = event.process_id;
        if pid > 0 && self.chain_pid != Some(pid) {
            self.chain_pid = Some(pid);
            self.process_chain = None;
            if let Some(tx) = self.refresh_tx.clone() {
                let ctx_clone = ctx.clone();
                rt.spawn(async move {
                    let svc = ProcessService { ctx: &ctx_clone };
                    match svc.chain(pid).await {
                        Ok(c) => {
                            let _ = tx.send(SysmonRefresh {
                                chain: Some(c),
                                ..Default::default()
                            });
                        }
                        Err(e) => tracing::error!("process chain: {}", e),
                    }
                });
            }
        }

        egui::ScrollArea::vertical()
            .id_salt("sysmon_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header with close button at top-right (right_to_left, DESIGN.md 4.7)
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        // Right margin so close button doesn't overlap with scrollbar
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_event = None;
                            self.selected_record_id = None;
                            self.process_chain = None;
                            self.chain_pid = None;
                        }
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                badge::badge(ui, event.event_type.label(), event_badge_variant(&event.event_type));
                                ui.label(
                                    egui::RichText::new(format!("EventID {}", event.event_id))
                                        .color(theme::fg_tertiary())
                                        .size(11.0),
                                );
                                if event.is_suspicious {
                                    ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                    badge::badge(ui, "可疑", BadgeVariant::Danger);
                                }
                            });
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(fmt_event_time(&event))
                                    .color(theme::fg_secondary())
                                    .size(11.0),
                            );
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Common fields
                detail_row(ui, "进程", nonempty(&event.process_name), false);
                if event.process_id > 0 {
                    detail_row(ui, "PID", Some(&event.process_id.to_string()), true);
                }
                detail_row(ui, "路径", nonempty(&event.process_path), true);
                detail_row(ui, "用户", nonempty(&event.user), false);

                // Event-type-specific fields
                match event.event_type {
                    SysmonEventType::Dns | SysmonEventType::DnsClient => {
                        detail_row(ui, "域名", nonempty(&event.query_name), true);
                        detail_row(ui, "查询结果", nonempty(&event.query_results), true);
                        detail_row(ui, "状态", Some(&format_dns_status(event.query_status)), false);
                    }
                    SysmonEventType::NetworkConnect => {
                        detail_row(
                            ui,
                            "源地址",
                            Some(&format!("{}:{}", event.source_ip, event.source_port)),
                            true,
                        );
                        detail_row(
                            ui,
                            "目标地址",
                            Some(&format!("{}:{}", event.destination_ip, event.destination_port)),
                            true,
                        );
                        detail_row(ui, "协议", nonempty(&event.protocol), false);
                        detail_row(ui, "是否外部", Some(if event.is_external { "是" } else { "否" }), false);
                    }
                    SysmonEventType::CreateRemoteThread => {
                        detail_row(ui, "源进程", nonempty(&event.source_process_name), false);
                        detail_row(ui, "目标进程", nonempty(&event.target_process_name), false);
                        detail_row(ui, "起始地址", nonempty(&event.start_address), true);
                        detail_row(ui, "起始模块", nonempty(&event.start_module), true);
                    }
                    SysmonEventType::FileCreate => {
                        detail_row(ui, "文件名", nonempty(&event.target_filename), true);
                        detail_row(ui, "创建时间", nonempty(&event.creation_utc_time), false);
                    }
                    _ => {}
                }

                // Process chain
                if pid > 0 {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("进程链")
                            .strong()
                            .color(theme::fg_secondary())
                            .size(12.0),
                    );
                    ui.add_space(2.0);
                    if let Some(ref chain) = self.process_chain {
                        if chain.is_empty() {
                            // 实时查询返回空链（短命进程已退出），尝试从 raw_data 读取捕获时的链
                            if let Some(chain_str) = event.raw_data.get("process_chain") {
                                if chain_str.is_empty() {
                                    ui.label(egui::RichText::new("无进程链信息").color(theme::fg_tertiary()).size(11.0));
                                } else {
                                    for (i, part) in chain_str.split("->").enumerate() {
                                        let arrow = if i == 0 { "" } else { "↑ " };
                                        ui.label(
                                            egui::RichText::new(format!("{}{}", arrow, part))
                                                .font(egui::FontId::monospace(11.0))
                                                .color(theme::fg_primary()),
                                        );
                                    }
                                }
                            } else {
                                ui.label(egui::RichText::new("无进程链信息").color(theme::fg_tertiary()).size(11.0));
                            }
                        } else {
                            for (i, node) in chain.nodes.iter().enumerate() {
                                let arrow = if i == 0 { "" } else { "↑ " };
                                let susp = if node.is_suspicious { " !" } else { "" };
                                ui.label(
                                    egui::RichText::new(format!("{}{} (pid={}){}", arrow, node.name, node.pid, susp))
                                        .font(egui::FontId::monospace(11.0))
                                        .color(if node.is_suspicious {
                                            theme::semantic_danger()
                                        } else {
                                            theme::fg_primary()
                                        }),
                                );
                                if let Some(ref exe) = node.exe {
                                    ui.label(
                                        egui::RichText::new(format!("    {}", exe))
                                            .font(egui::FontId::monospace(10.0))
                                            .color(theme::fg_tertiary()),
                                    );
                                }
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new("加载中…").color(theme::fg_tertiary()).size(11.0));
                    }
                }
            });
    }

    // ── Stats Bar ──────────────────────────────────────────

    pub fn render_stats_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Collecting status
            if self.collecting {
                badge::badge(ui, "采集中", BadgeVariant::Success);
            } else {
                badge::badge(ui, "已停止", BadgeVariant::Default);
            }

            ui.separator();

            ui.label(
                egui::RichText::new(format!("事件: {}", self.events.len()))
                    .color(theme::fg_secondary())
                    .size(11.0),
            );

            // Event type counts (top 4)
            let counts = self.event_type_counts();
            for (t, c) in counts.iter().take(4) {
                ui.label(
                    egui::RichText::new(format!("{}: {}", t.label(), c))
                        .color(theme::fg_tertiary())
                        .size(11.0),
                );
            }

            ui.separator();

            // Collection duration
            if self.collecting {
                if let Some(t) = self.start_time {
                    let d = t.elapsed().as_secs();
                    ui.label(
                        egui::RichText::new(format!("时长: {}", format_hms(d)))
                            .color(theme::accent())
                            .size(11.0),
                    );
                }
            }

            ui.separator();

            // Sysmon running status
            if let Some(ref s) = self.sysmon_status {
                if s.running {
                    badge::badge(ui, "Sysmon 运行", BadgeVariant::Success);
                } else if s.installed {
                    badge::badge(ui, "Sysmon 已停", BadgeVariant::Warning);
                } else {
                    badge::badge(ui, "Sysmon 未安装", BadgeVariant::Default);
                }
            }

            ui.label(
                egui::RichText::new(format!("日志上限: {} MB", self.log_max_size_mb))
                    .color(theme::fg_tertiary())
                    .size(11.0),
            );
        });
    }

    // ── Config Dialog ──────────────────────────────────────

    fn render_config_dialog(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.config_dialog_open;
        let mut apply = false;
        let mut save_log_size = false;

        egui::Window::new("Sysmon 配置")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                ui.label(
                    egui::RichText::new("事件类型")
                        .strong()
                        .color(theme::fg_secondary())
                        .size(12.0),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    for c in &self.event_configs {
                        let mut checked = self.enabled_event_keys.contains(&c.key);
                        let resp = ui.checkbox(&mut checked, format!("{} (ID {})", c.name, c.event_id));
                        if resp.changed() {
                            if checked {
                                self.enabled_event_keys.push(c.key.clone());
                            } else {
                                self.enabled_event_keys.retain(|k| k != &c.key);
                            }
                        }
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("日志大小上限 (MB):");
                    let mut size_str = self.log_max_size_mb.to_string();
                    let resp = ui.add(egui::TextEdit::singleline(&mut size_str).desired_width(80.0));
                    if resp.changed() {
                        if let Ok(v) = size_str.parse::<u64>() {
                            self.log_max_size_mb = v;
                        }
                    }
                    if ui.button("保存").clicked() {
                        save_log_size = true;
                    }
                });
                ui.add_space(8.0);
                if ui.button("应用配置").clicked() {
                    apply = true;
                }
            });

        self.config_dialog_open = open;

        if apply {
            let keys = self.enabled_event_keys.clone();
            let ctx_clone = ctx.clone();
            let tx = self.refresh_tx.clone();
            rt.spawn(async move {
                let svc = SysmonService { ctx: &ctx_clone };
                match svc.generate_config(keys).await {
                    Ok(_config) => match svc.update_config().await {
                        Ok((ok, msg)) => {
                            tracing::info!("update config: ok={} msg={}", ok, msg);
                            if let Ok(s) = svc.status().await {
                                if let Some(tx) = tx {
                                    let _ = tx.send(SysmonRefresh {
                                        status: Some(s),
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                        Err(e) => tracing::error!("update config: {}", e),
                    },
                    Err(e) => tracing::error!("generate config: {}", e),
                }
            });
            self.config_dialog_open = false;
        }

        if save_log_size {
            let size = self.log_max_size_mb;
            let ctx_clone = ctx.clone();
            rt.spawn(async move {
                if let Err(e) = (SysmonService { ctx: &ctx_clone }).set_log_max_size(size).await {
                    tracing::error!("set log max size: {}", e);
                }
            });
        }
    }

    // ── Clear Confirmation Dialog (DESIGN.md 4.10) ────────

    fn render_clear_confirm(&mut self, ui: &mut egui::Ui) {
        let mut open = self.clear_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("确认清除事件")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label("将清除当前所有已采集的事件。此操作不可撤销。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("清除").color(theme::semantic_danger()),
                        ))
                        .clicked()
                    {
                        confirmed = true;
                    }
                });
            });

        if confirmed {
            self.events.clear();
            self.selected_event = None;
            self.selected_record_id = None;
            self.detail_visible = false;
            self.process_chain = None;
            self.chain_pid = None;
            self.cache_dirty = true;
            open = false;
        }
        if cancelled {
            open = false;
        }

        self.clear_confirm_open = open;
    }

    // ── Uninstall Confirmation Dialog (DESIGN.md 4.10) ────

    fn render_uninstall_confirm(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.uninstall_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("确认卸载 Sysmon")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label("将卸载 Sysmon 驱动并停止事件采集。此操作不可撤销。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("卸载").color(theme::semantic_danger()),
                        ))
                        .clicked()
                    {
                        confirmed = true;
                    }
                });
            });

        if confirmed {
            let ctx_clone = ctx.clone();
            let tx = self.refresh_tx.clone();
            rt.spawn(async move {
                match (SysmonService { ctx: &ctx_clone }).uninstall().await {
                    Ok((ok, msg)) => {
                        tracing::info!("uninstall: ok={} msg={}", ok, msg);
                        let svc = SysmonService { ctx: &ctx_clone };
                        if let Ok(s) = svc.status().await {
                            if let Some(tx) = tx {
                                let _ = tx.send(SysmonRefresh {
                                    status: Some(s),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    Err(e) => tracing::error!("uninstall: {}", e),
                }
            });
            open = false;
        }
        if cancelled {
            open = false;
        }

        self.uninstall_confirm_open = open;
    }

    /// 安装 Sysmon 确认对话框。
    /// 触发场景：(1) 点击「安装 Sysmon」按钮；(2) 点击「开始采集」时检测到 Sysmon 未安装。
    /// 若 `pending_start_collect` 为 true，安装完成后自动开始采集。
    fn render_install_confirm(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.install_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;
        let pending_start = self.pending_start_collect;

        let title = if pending_start {
            "确认安装 Sysmon 并开始采集"
        } else {
            "确认安装 Sysmon"
        };
        let confirm_label = if pending_start {
            "安装并开始采集"
        } else {
            "安装"
        };

        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "将安装 Sysmon 驱动以启用系统事件采集。\n\
                         Sysmon 是 Microsoft Sysinternals 提供的免费系统监控工具，安装后会在后台运行。",
                    )
                    .size(12.0)
                    .color(theme::fg_secondary()),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                    if ui.button(confirm_label).clicked() {
                        confirmed = true;
                    }
                });
            });

        if confirmed {
            let ctx_clone = ctx.clone();
            let tx = self.refresh_tx.clone();
            rt.spawn(async move {
                match (SysmonService { ctx: &ctx_clone }).install(true).await {
                    Ok((ok, msg)) => {
                        tracing::info!("install: ok={} msg={}", ok, msg);
                        let svc = SysmonService { ctx: &ctx_clone };
                        if let Ok(s) = svc.status().await {
                            if let Some(tx) = tx {
                                let _ = tx.send(SysmonRefresh {
                                    status: Some(s),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    Err(e) => tracing::error!("install: {}", e),
                }
            });
            open = false;
        }
        if cancelled {
            self.pending_start_collect = false;
            open = false;
        }

        self.install_confirm_open = open;
    }

    // ── Helpers ────────────────────────────────────────────

    fn enabled_event_ids(&self) -> Vec<u32> {
        self.event_configs
            .iter()
            .filter(|c| self.enabled_event_keys.contains(&c.key))
            .map(|c| c.event_id)
            .collect()
    }

    fn collect_event_types(&self) -> Vec<SysmonEventType> {
        let mut seen: HashSet<SysmonEventType> = HashSet::new();
        let mut out = Vec::new();
        for e in &self.events {
            if seen.insert(e.event_type.clone()) {
                out.push(e.event_type.clone());
            }
        }
        out.sort_by_key(|t| t.label());
        out
    }

    fn event_type_counts(&self) -> Vec<(SysmonEventType, usize)> {
        let mut map: HashMap<SysmonEventType, usize> = HashMap::new();
        for e in &self.events {
            *map.entry(e.event_type.clone()).or_insert(0) += 1;
        }
        let mut v: Vec<_> = map.into_iter().collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.1));
        v
    }

    fn export_csv(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let events: Vec<SysmonEvent> = self.events.iter().cloned().collect();
        let dir = ctx.app_dirs.root().to_path_buf();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("sysmon_export_{}.csv", secs));
        rt.spawn_blocking(move || match write_csv(&path, &events) {
            Ok(()) => {
                tracing::info!("exported {} events to {}", events.len(), path.display())
            }
            Err(e) => tracing::error!("csv export failed: {}", e),
        });
    }

    fn get_filtered_sorted(&mut self) -> &[SysmonEvent] {
        if self.cache_dirty {
            self.cached_filtered = self.compute_filtered_sorted();
            self.cache_dirty = false;
        }
        &self.cached_filtered
    }

    fn compute_filtered_sorted(&self) -> Vec<SysmonEvent> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<SysmonEvent> = self
            .events
            .iter()
            .filter(|e| self.event_type_filter.is_empty() || self.event_type_filter.contains(&e.event_type))
            .filter(|e| {
                if !self.external_only {
                    return true;
                }
                match e.event_type {
                    SysmonEventType::NetworkConnect => e.is_external,
                    _ => true,
                }
            })
            .filter(|e| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!(
                    "{} {} {} {} {} {} {} {} {}",
                    e.process_name,
                    e.process_path,
                    e.user,
                    e.query_name,
                    e.destination_ip,
                    e.destination_port,
                    e.source_ip,
                    e.target_filename,
                    e.event_type.label()
                );
                blob.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        out.sort_by(|a, b| {
            b.timestamp_epoch
                .partial_cmp(&a.timestamp_epoch)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }
}

// ── Free helpers ──────────────────────────────────────────

fn pcap_to_sysmon_event(event: PcapEvent, synthetic_id: u64) -> SysmonEvent {
    let (event_type, event_id) = match event.event_kind {
        PcapEventKind::DnsQuery => (SysmonEventType::DnsPcap, 0),
        PcapEventKind::TlsSni => (SysmonEventType::TlsSni, 0),
    };
    let ts = event.timestamp;
    SysmonEvent {
        event_id,
        event_type,
        timestamp: theme::fmt_time_millis(ts),
        timestamp_epoch: ts as f64 / 1000.0,
        timestamp_valid: true,
        record_id: Some(synthetic_id),
        raw_data: HashMap::new(),
        process_id: 0,
        process_name: String::new(),
        process_path: String::new(),
        user: String::new(),
        rule_name: String::new(),
        query_name: event.domain,
        query_results: String::new(),
        query_status: 0,
        source_ip: event.src_ip,
        source_port: event.src_port,
        destination_ip: event.dst_ip,
        destination_port: event.dst_port,
        protocol: String::new(),
        initiated: true,
        is_external: false,
        source_process_id: 0,
        source_process_name: String::new(),
        source_process_path: String::new(),
        target_process_id: 0,
        target_process_name: String::new(),
        target_process_path: String::new(),
        start_address: String::new(),
        start_module: String::new(),
        start_function: String::new(),
        is_suspicious: false,
        target_filename: String::new(),
        creation_utc_time: String::new(),
    }
}

fn destination_for(e: &SysmonEvent) -> String {
    match e.event_type {
        SysmonEventType::NetworkConnect => {
            if e.destination_ip.is_empty() && e.destination_port == 0 {
                String::new()
            } else {
                format!("{}:{}", e.destination_ip, e.destination_port)
            }
        }
        SysmonEventType::Dns | SysmonEventType::DnsClient => e.query_name.clone(),
        SysmonEventType::CreateRemoteThread => {
            if e.source_process_name.is_empty() && e.target_process_name.is_empty() {
                String::new()
            } else {
                format!("{} → {}", e.source_process_name, e.target_process_name)
            }
        }
        SysmonEventType::FileCreate => e.target_filename.clone(),
        _ => {
            if !e.query_name.is_empty() {
                e.query_name.clone()
            } else {
                e.process_name.clone()
            }
        }
    }
}

fn path_for(e: &SysmonEvent) -> String {
    match e.event_type {
        SysmonEventType::CreateRemoteThread => {
            if !e.source_process_path.is_empty() {
                e.source_process_path.clone()
            } else {
                e.target_process_path.clone()
            }
        }
        _ => e.process_path.clone(),
    }
}

fn event_badge_variant(t: &SysmonEventType) -> BadgeVariant {
    match t {
        SysmonEventType::NetworkConnect => BadgeVariant::Info,
        SysmonEventType::Dns | SysmonEventType::DnsClient => BadgeVariant::Success,
        SysmonEventType::CreateRemoteThread => BadgeVariant::Danger,
        SysmonEventType::FileCreate | SysmonEventType::FileDelete | SysmonEventType::FileDeleteDetected => {
            BadgeVariant::Warning
        }
        _ => BadgeVariant::Default,
    }
}

fn format_dns_status(status: u32) -> String {
    if status == 0 {
        "成功 (0)".to_string()
    } else {
        format!("错误 ({})", status)
    }
}

fn format_hms(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Format event timestamp as UTC+8 (DESIGN.md 4.8).
/// Uses `timestamp_epoch` when valid, otherwise falls back to the raw string.
fn fmt_event_time(e: &SysmonEvent) -> String {
    if e.timestamp_valid && e.timestamp_epoch > 0.0 {
        theme::fmt_time(e.timestamp_epoch as u64)
    } else {
        e.timestamp.clone()
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

fn write_csv(path: &std::path::Path, events: &[SysmonEvent]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "timestamp,event_type,event_id,process_name,pid,user,destination,query_name,source_ip,destination_ip,destination_port,protocol,path"
    )?;
    for e in events {
        let dest = destination_for(e);
        let p = path_for(e);
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_escape(&e.timestamp),
            csv_escape(e.event_type.label()),
            e.event_id,
            csv_escape(&e.process_name),
            e.process_id,
            csv_escape(&e.user),
            csv_escape(&dest),
            csv_escape(&e.query_name),
            csv_escape(&e.source_ip),
            csv_escape(&e.destination_ip),
            e.destination_port,
            csv_escape(&e.protocol),
            csv_escape(&p),
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
