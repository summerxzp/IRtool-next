use std::collections::{HashMap, HashSet};

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_service::context::AppContext;
#[allow(unused_imports)]
use irtool_service::dto::network::NetworkSnapshotPayload;
use irtool_service::services::autoruns::AutorunsService;
use irtool_service::services::network::NetworkService;
use irtool_service::services::sysmon::SysmonService;
use irtool_service::services::workspace::WorkspaceService;
use irtool_service::types::{AutorunItem, ConnState, NetConn, RiskLevel, SysmonEvent, SysmonEventType};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;

// ── Rule Engine Types ─────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum RuleTarget {
    Autorun,
    Network,
    Event,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConditionType {
    Contains,
    Regex,
    Equals,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Logic {
    And,
    Or,
}

#[derive(Clone, Debug)]
pub struct Condition {
    pub field: String,
    pub cond_type: ConditionType,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub target: RuleTarget,
    pub conditions: Vec<Condition>,
    pub logic: Logic,
    pub severity: Severity,
    pub family: String,
    pub enabled: bool,
    pub description: Option<String>,
}

// ── Default Rules ─────────────────────────────────────────

fn default_rules() -> Vec<Rule> {
    let malicious_ips = "\
82.23.246.148
161.248.87.175
202.79.169.50
47.76.246.88
202.95.16.13
47.242.90.146
202.79.171.236
143.92.57.157
137.220.135.15
206.238.115.137";

    vec![
        Rule {
            id: "default-temp-persistence".to_string(),
            name: "Temp 目录可疑持久化".to_string(),
            target: RuleTarget::Autorun,
            conditions: vec![Condition {
                field: "image_path".to_string(),
                cond_type: ConditionType::Contains,
                value: "\\Temp\\".to_string(),
            }],
            logic: Logic::And,
            severity: Severity::High,
            family: "持久化".to_string(),
            enabled: true,
            description: Some("检测从 Temp 目录启动的持久化项，常见于恶意软件".to_string()),
        },
        Rule {
            id: "default-appdata-persistence".to_string(),
            name: "AppData 可疑持久化".to_string(),
            target: RuleTarget::Autorun,
            conditions: vec![Condition {
                field: "image_path".to_string(),
                cond_type: ConditionType::Contains,
                value: "\\AppData\\".to_string(),
            }],
            logic: Logic::And,
            severity: Severity::Medium,
            family: "持久化".to_string(),
            enabled: true,
            description: Some("检测从 AppData 目录启动的持久化项，部分合法软件也使用此路径".to_string()),
        },
        Rule {
            id: "default-malicious-ip-network".to_string(),
            name: "恶意 IP 连接 (网络)".to_string(),
            target: RuleTarget::Network,
            conditions: vec![Condition {
                field: "remote_addr".to_string(),
                cond_type: ConditionType::Contains,
                value: malicious_ips.to_string(),
            }],
            logic: Logic::And,
            severity: Severity::Critical,
            family: "恶意IP".to_string(),
            enabled: true,
            description: Some("检测连接已知恶意 IP 的网络连接".to_string()),
        },
        Rule {
            id: "default-malicious-ip-event".to_string(),
            name: "恶意 IP 连接 (事件)".to_string(),
            target: RuleTarget::Event,
            conditions: vec![Condition {
                field: "destination_ip".to_string(),
                cond_type: ConditionType::Contains,
                value: malicious_ips.to_string(),
            }],
            logic: Logic::And,
            severity: Severity::Critical,
            family: "恶意IP".to_string(),
            enabled: true,
            description: Some("检测 Sysmon 事件中连接已知恶意 IP 的事件".to_string()),
        },
    ]
}

// ── Field Accessors ───────────────────────────────────────

fn get_autorun_field(item: &AutorunItem, field: &str) -> String {
    match field {
        "entry" => item.entry.clone(),
        "image_path" => item.image_path.clone().unwrap_or_default(),
        "launch_string" => item.launch_string.clone().unwrap_or_default(),
        "location" => item.location.clone(),
        "publisher" => item.publisher.clone(),
        "description" => item.description.clone(),
        "category" => item.category.clone(),
        "enabled" => item.enabled.to_string(),
        "timestamp" => item.timestamp.clone().unwrap_or_default(),
        "file_exists" => item.file_exists.to_string(),
        "md5" => item.md5.clone().unwrap_or_default(),
        "sha256" => item.sha256.clone().unwrap_or_default(),
        "risk" => item.risk.as_str().to_string(),
        "signature" => format!("{:?}", item.signature),
        _ => String::new(),
    }
}

fn get_network_field(item: &NetConn, field: &str) -> String {
    match field {
        "pid" => item.pid.to_string(),
        "process_name" => item.process_name.clone().unwrap_or_default(),
        "process_path" => item.process_path.clone().unwrap_or_default(),
        "process_cmdline" => item.process_cmdline.clone().unwrap_or_default(),
        "proto" => format!("{:?}", item.proto),
        "family" => format!("{:?}", item.family),
        "local_addr" => item.local.addr.clone(),
        "local_port" => item.local.port.to_string(),
        "remote_addr" => item.remote.addr.clone(),
        "remote_port" => item.remote.port.to_string(),
        "state" => item.state.as_str().to_string(),
        "first_seen" => item.first_seen.to_string(),
        "last_seen" => item.last_seen.to_string(),
        _ => String::new(),
    }
}

fn get_event_field(item: &SysmonEvent, field: &str) -> String {
    match field {
        "event_id" => item.event_id.to_string(),
        "event_type" => item.event_type.label().to_string(),
        "timestamp" => item.timestamp.clone(),
        "process_name" => item.process_name.clone(),
        "process_path" => item.process_path.clone(),
        "user" => item.user.clone(),
        "query_name" => item.query_name.clone(),
        "destination_ip" => item.destination_ip.clone(),
        "destination_port" => item.destination_port.to_string(),
        "source_ip" => item.source_ip.clone(),
        "protocol" => item.protocol.clone(),
        "target_filename" => item.target_filename.clone(),
        _ => String::new(),
    }
}

// ── Match Functions ───────────────────────────────────────

/// Match a single line against a pattern.
/// For Regex, uses the `regex` crate for full regular expression matching.
fn match_single_line(s: &str, pattern: &str, cond_type: &ConditionType) -> bool {
    match cond_type {
        ConditionType::Contains => s.to_lowercase().contains(&pattern.to_lowercase()),
        ConditionType::Equals => s.eq_ignore_ascii_case(pattern),
        ConditionType::Regex => regex::Regex::new(pattern).map(|re| re.is_match(s)).unwrap_or(false),
    }
}

/// Match a condition: splits value by newlines, returns true if ANY line matches.
fn match_condition(value: &str, condition: &Condition) -> bool {
    condition
        .value
        .lines()
        .any(|line| !line.trim().is_empty() && match_single_line(value, line.trim(), &condition.cond_type))
}

/// Match a rule against a field accessor function using AND/OR logic.
fn matches_rule(get_field: impl Fn(&str) -> String, rule: &Rule) -> bool {
    if rule.conditions.is_empty() {
        return false;
    }
    match rule.logic {
        Logic::And => rule.conditions.iter().all(|c| match_condition(&get_field(&c.field), c)),
        Logic::Or => rule.conditions.iter().any(|c| match_condition(&get_field(&c.field), c)),
    }
}

// ── Scan Functions ────────────────────────────────────────

fn scan_autoruns(items: &[AutorunItem], rules: &[Rule]) -> HashMap<u64, Vec<Rule>> {
    let mut result = HashMap::new();
    for item in items {
        let matched: Vec<Rule> = rules
            .iter()
            .filter(|r| r.enabled && r.target == RuleTarget::Autorun)
            .filter(|r| matches_rule(|f| get_autorun_field(item, f), r))
            .cloned()
            .collect();
        if !matched.is_empty() {
            result.insert(item.id, matched);
        }
    }
    result
}

fn scan_network(items: &[NetConn], rules: &[Rule]) -> HashMap<String, Vec<Rule>> {
    let mut result = HashMap::new();
    for item in items {
        let key = network_key(item);
        let matched: Vec<Rule> = rules
            .iter()
            .filter(|r| r.enabled && r.target == RuleTarget::Network)
            .filter(|r| matches_rule(|f| get_network_field(item, f), r))
            .cloned()
            .collect();
        if !matched.is_empty() {
            result.insert(key, matched);
        }
    }
    result
}

fn scan_events(items: &[SysmonEvent], rules: &[Rule]) -> HashMap<String, Vec<Rule>> {
    let mut result = HashMap::new();
    for item in items {
        let key = event_key(item);
        let matched: Vec<Rule> = rules
            .iter()
            .filter(|r| r.enabled && r.target == RuleTarget::Event)
            .filter(|r| matches_rule(|f| get_event_field(item, f), r))
            .cloned()
            .collect();
        if !matched.is_empty() {
            result.insert(key, matched);
        }
    }
    result
}

// ── Search Functions ─────────────────────────────────────

fn search_autoruns(items: &[AutorunItem], query: &str) -> HashSet<u64> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.iter().map(|i| i.id).collect();
    }
    items
        .iter()
        .filter(|item| {
            let blob = format!(
                "{} {} {} {} {} {} {} {} {} {}",
                item.entry,
                item.image_path.as_deref().unwrap_or(""),
                item.launch_string.as_deref().unwrap_or(""),
                item.location,
                item.publisher,
                item.description,
                item.category,
                item.md5.as_deref().unwrap_or(""),
                item.sha256.as_deref().unwrap_or(""),
                item.risk.as_str(),
            )
            .to_lowercase();
            blob.contains(&q)
        })
        .map(|i| i.id)
        .collect()
}

fn search_network(items: &[NetConn], query: &str) -> HashSet<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.iter().map(network_key).collect();
    }
    items
        .iter()
        .filter(|item| {
            let blob = format!(
                "{} {} {} {} {} {:?} {:?} {}:{} {}:{} {} {}",
                item.pid,
                item.process_name.as_deref().unwrap_or(""),
                item.process_path.as_deref().unwrap_or(""),
                item.process_cmdline.as_deref().unwrap_or(""),
                item.state.as_str(),
                item.proto,
                item.family,
                item.local.addr,
                item.local.port,
                item.remote.addr,
                item.remote.port,
                item.first_seen,
                item.last_seen,
            )
            .to_lowercase();
            blob.contains(&q)
        })
        .map(network_key)
        .collect()
}

fn search_events(items: &[SysmonEvent], query: &str) -> HashSet<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.iter().map(event_key).collect();
    }
    items
        .iter()
        .filter(|item| {
            let blob = format!(
                "{} {} {} {} {} {} {} {} {} {} {} {}",
                item.event_id,
                item.event_type.label(),
                item.timestamp,
                item.process_name,
                item.process_path,
                item.user,
                item.query_name,
                item.destination_ip,
                item.destination_port,
                item.source_ip,
                item.protocol,
                item.target_filename,
            )
            .to_lowercase();
            blob.contains(&q)
        })
        .map(event_key)
        .collect()
}

// ── Key Generators ────────────────────────────────────────

fn network_key(item: &NetConn) -> String {
    format!(
        "{:?}|{:?}|{}:{}|{}:{}|{}",
        item.proto, item.family, item.local.addr, item.local.port, item.remote.addr, item.remote.port, item.pid
    )
}

fn event_key(item: &SysmonEvent) -> String {
    format!("{}-{}-{}", item.record_id.unwrap_or(0), item.timestamp, item.event_id)
}

// ── WorkspaceTab ──────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum WorkspaceTab {
    Autoruns,
    Network,
    Events,
}

// ── Async → UI refresh payload ────────────────────────────

#[derive(Default)]
pub struct WorkspaceRefresh {
    pub autorun_items: Option<Vec<AutorunItem>>,
    pub network_items: Option<Vec<NetConn>>,
    pub event_items: Option<Vec<SysmonEvent>>,
    pub error: Option<String>,
}

// ── WorkspacePageState ────────────────────────────────────

pub struct WorkspacePageState {
    // Data
    pub autorun_items: Vec<AutorunItem>,
    pub network_items: Vec<NetConn>,
    pub event_items: Vec<SysmonEvent>,

    // Filters
    pub search_query: String,
    pub filtered_autorun_ids: Option<HashSet<u64>>,
    pub filtered_network_keys: Option<HashSet<String>>,
    pub filtered_event_keys: Option<HashSet<String>>,

    // Rule scan results
    pub autorun_matched_rules: HashMap<u64, Vec<Rule>>,
    pub network_matched_rules: HashMap<String, Vec<Rule>>,
    pub event_matched_rules: HashMap<String, Vec<Rule>>,

    // Selection
    pub selected_autorun_id: Option<u64>,
    pub selected_network_key: Option<String>,
    pub selected_event_key: Option<String>,
    pub detail_visible: bool,

    // Active tab
    pub active_tab: WorkspaceTab,

    // Rules
    pub rules: Vec<Rule>,

    // State
    pub scanning: bool,
    pub loading: bool,

    // Dialogs
    pub rule_manager_open: bool,
    pub rule_edit_open: bool,
    pub editing_rule: Option<Rule>,
    pub rule_search_query: String,
    pub delete_confirm_open: bool,
    pub pending_delete_id: Option<u64>,
    pub kill_confirm_open: bool,
    pub pending_kill_pid: Option<u32>,

    // Error
    pub last_error: Option<String>,

    // Async refresh
    pub refresh_tx: Option<std::sync::mpsc::Sender<WorkspaceRefresh>>,
}

impl Default for WorkspacePageState {
    fn default() -> Self {
        Self {
            autorun_items: Vec::new(),
            network_items: Vec::new(),
            event_items: Vec::new(),
            search_query: String::new(),
            filtered_autorun_ids: None,
            filtered_network_keys: None,
            filtered_event_keys: None,
            autorun_matched_rules: HashMap::new(),
            network_matched_rules: HashMap::new(),
            event_matched_rules: HashMap::new(),
            selected_autorun_id: None,
            selected_network_key: None,
            selected_event_key: None,
            detail_visible: false,
            active_tab: WorkspaceTab::Autoruns,
            rules: default_rules(),
            scanning: false,
            loading: false,
            rule_manager_open: false,
            rule_edit_open: false,
            editing_rule: None,
            rule_search_query: String::new(),
            delete_confirm_open: false,
            pending_delete_id: None,
            kill_confirm_open: false,
            pending_kill_pid: None,
            last_error: None,
            refresh_tx: None,
        }
    }
}

impl WorkspacePageState {
    // ── Public API ────────────────────────────────────────

    /// Apply an async refresh payload.
    pub fn apply_refresh(&mut self, r: WorkspaceRefresh) {
        if let Some(items) = r.autorun_items {
            self.autorun_items = items;
            self.loading = false;
        }
        if let Some(items) = r.network_items {
            self.network_items = items;
            self.loading = false;
        }
        if let Some(items) = r.event_items {
            self.event_items = items;
            self.loading = false;
        }
        if let Some(err) = r.error {
            self.last_error = Some(err);
            self.loading = false;
        }
    }

    /// Kick off async fetches for autorun items and network snapshot.
    pub fn trigger_refresh(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };

        // Fetch autoruns
        let ctx1 = ctx.clone();
        let tx1 = tx.clone();
        rt.spawn(async move {
            match (AutorunsService { ctx: &ctx1 }).get_result().await {
                Ok(items) => {
                    let _ = tx1.send(WorkspaceRefresh {
                        autorun_items: Some(items),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx1.send(WorkspaceRefresh {
                        error: Some(e.to_string()),
                        ..Default::default()
                    });
                }
            }
        });

        // Fetch network snapshot
        let ctx2 = ctx.clone();
        let tx2 = tx.clone();
        rt.spawn(async move {
            match (NetworkService { ctx: &ctx2 }).snapshot().await {
                Ok(payload) => {
                    let _ = tx2.send(WorkspaceRefresh {
                        network_items: Some(payload.items),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx2.send(WorkspaceRefresh {
                        error: Some(e.to_string()),
                        ..Default::default()
                    });
                }
            }
        });

        // Fetch sysmon events
        let ctx3 = ctx.clone();
        let tx3 = tx.clone();
        rt.spawn(async move {
            let svc = SysmonService { ctx: &ctx3 };
            match svc.get_existing_events(1000, Vec::new()).await {
                Ok(events) => {
                    let _ = tx3.send(WorkspaceRefresh {
                        event_items: Some(events),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx3.send(WorkspaceRefresh {
                        error: Some(e.to_string()),
                        ..Default::default()
                    });
                }
            }
        });
    }

    /// Run search across all data sources.
    pub fn do_search(&mut self, query: &str) {
        if query.trim().is_empty() {
            self.filtered_autorun_ids = None;
            self.filtered_network_keys = None;
            self.filtered_event_keys = None;
        } else {
            self.filtered_autorun_ids = Some(search_autoruns(&self.autorun_items, query));
            self.filtered_network_keys = Some(search_network(&self.network_items, query));
            self.filtered_event_keys = Some(search_events(&self.event_items, query));
        }
    }

    /// Run rule scan across all data sources.
    pub fn do_rule_scan(&mut self) {
        self.scanning = true;
        self.autorun_matched_rules = scan_autoruns(&self.autorun_items, &self.rules);
        self.network_matched_rules = scan_network(&self.network_items, &self.rules);
        self.event_matched_rules = scan_events(&self.event_items, &self.rules);
        self.scanning = false;
    }

    // ── Rendering ──────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // Initial load
        if !self.loading && self.autorun_items.is_empty() && self.network_items.is_empty() && self.refresh_tx.is_some()
        {
            self.loading = true;
            self.trigger_refresh(ctx, rt);
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

        self.render_tabs(ui);
        ui.separator();
        self.render_table(ui);

        // Dialogs
        if self.rule_manager_open {
            self.render_rule_manager(ui, ctx);
        }
        if self.rule_edit_open {
            self.render_rule_edit(ui);
        }
        if self.delete_confirm_open {
            self.render_delete_confirm(ui, ctx, rt);
        }
        if self.kill_confirm_open {
            self.render_kill_confirm(ui, ctx, rt);
        }
    }

    // ── Toolbar ────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // Search box
            ui.label(egui::RichText::new("搜索:").size(14.0));
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(200.0)
                    .hint_text("搜索条目、路径、IP..."),
            );

            if ui.button("搜索").clicked() {
                let q = self.search_query.clone();
                self.do_search(&q);
            }
            if ui.button("重置").clicked() {
                self.search_query.clear();
                self.do_search("");
            }

            ui.separator();

            // Rule scan
            if ui.add_enabled(!self.scanning, egui::Button::new("规则扫描")).clicked() {
                self.do_rule_scan();
            }
            if ui.button("规则管理").clicked() {
                self.rule_manager_open = true;
            }
            if ui.button("导出CSV").clicked() {
                self.export_csv(ctx, rt);
            }
            if ui.add_enabled(!self.loading, egui::Button::new("刷新数据")).clicked() {
                self.loading = true;
                self.trigger_refresh(ctx, rt);
            }
        });
    }

    // ── Tabs ───────────────────────────────────────────────

    fn render_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let autorun_count = self.autorun_items.len();
            let network_count = self.network_items.len();
            let event_count = self.event_items.len();

            let autorun_label = format!("持久化 ({})", autorun_count);
            let network_label = format!("网络 ({})", network_count);
            let event_label = format!("事件 ({})", event_count);

            let autorun_active = self.active_tab == WorkspaceTab::Autoruns;
            let network_active = self.active_tab == WorkspaceTab::Network;
            let event_active = self.active_tab == WorkspaceTab::Events;

            let autorun_color = if autorun_active {
                theme::accent()
            } else {
                theme::fg_secondary()
            };
            let network_color = if network_active {
                theme::accent()
            } else {
                theme::fg_secondary()
            };
            let event_color = if event_active {
                theme::accent()
            } else {
                theme::fg_secondary()
            };

            if ui
                .add(egui::Button::new(egui::RichText::new(&autorun_label).color(autorun_color).strong()).frame(false))
                .clicked()
            {
                self.active_tab = WorkspaceTab::Autoruns;
            }
            ui.separator();
            if ui
                .add(egui::Button::new(egui::RichText::new(&network_label).color(network_color).strong()).frame(false))
                .clicked()
            {
                self.active_tab = WorkspaceTab::Network;
            }
            ui.separator();
            if ui
                .add(egui::Button::new(egui::RichText::new(&event_label).color(event_color).strong()).frame(false))
                .clicked()
            {
                self.active_tab = WorkspaceTab::Events;
            }
        });
    }

    // ── Table ──────────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui) {
        match self.active_tab {
            WorkspaceTab::Autoruns => self.render_autoruns_table(ui),
            WorkspaceTab::Network => self.render_network_table(ui),
            WorkspaceTab::Events => self.render_events_table(ui),
        }
    }

    fn render_autoruns_table(&mut self, ui: &mut egui::Ui) {
        let filtered_ids = self.filtered_autorun_ids.clone();
        let matched_rules = self.autorun_matched_rules.clone();
        let sel_id = self.selected_autorun_id;

        let items: Vec<&AutorunItem> = self
            .autorun_items
            .iter()
            .filter(|item| match &filtered_ids {
                None => true,
                Some(ids) => ids.contains(&item.id),
            })
            .collect();

        if items.is_empty() {
            self.render_empty_state(ui, "持久化");
            return;
        }

        let mut clicked_id: Option<u64> = None;
        let mut clicked_deselect = false;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(200.0).clip(true).resizable(true))
            .column(Column::initial(300.0).clip(true).resizable(true))
            .column(Column::initial(120.0).clip(true))
            .column(Column::initial(80.0).clip(true));

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    ui.label(egui::RichText::new("条目").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("文件路径").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("类别").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("风险").color(theme::fg_secondary()).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let item = items[row.index()];
                    let is_selected = sel_id == Some(item.id);
                    let has_match = matched_rules.contains_key(&item.id);

                    // 条目
                    row.col(|ui| {
                        let prefix = if has_match { "! " } else { "" };
                        ui.label(
                            egui::RichText::new(format!("{}{}", prefix, item.entry))
                                .color(theme::fg_primary())
                                .strong(),
                        );
                    });
                    // 文件路径
                    row.col(|ui| {
                        let p = item.image_path.as_deref().unwrap_or("");
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(p)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_tertiary()),
                            )
                            .truncate(),
                        );
                        if !p.is_empty() {
                            resp.on_hover_text(p);
                        }
                    });
                    // 类别
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.category).color(theme::fg_secondary()));
                    });
                    // 风险
                    row.col(|ui| {
                        badge::badge(ui, item.risk.as_str(), risk_badge_variant(&item.risk));
                    });

                    if is_selected {
                        row.set_selected(true);
                    }
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            clicked_deselect = true;
                        } else {
                            clicked_id = Some(item.id);
                        }
                    }
                });
            });

        if clicked_deselect {
            self.selected_autorun_id = None;
            self.detail_visible = false;
        } else if let Some(id) = clicked_id {
            self.selected_autorun_id = Some(id);
            self.detail_visible = true;
        }
    }

    fn render_network_table(&mut self, ui: &mut egui::Ui) {
        let filtered_keys = self.filtered_network_keys.clone();
        let matched_rules = self.network_matched_rules.clone();
        let sel_key = self.selected_network_key.clone();

        // Build filtered + sorted items with keys
        let mut items: Vec<(String, &NetConn)> = self
            .network_items
            .iter()
            .filter(|item| match &filtered_keys {
                None => true,
                Some(keys) => keys.contains(&network_key(item)),
            })
            .map(|item| (network_key(item), item))
            .collect();
        items.sort_by_key(|b| std::cmp::Reverse(b.1.last_seen));

        if items.is_empty() {
            self.render_empty_state(ui, "网络");
            return;
        }

        let mut clicked_key: Option<String> = None;
        let mut clicked_deselect = false;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(140.0).clip(true))
            .column(Column::initial(60.0).clip(true))
            .column(Column::initial(200.0).clip(true).resizable(true))
            .column(Column::initial(90.0).clip(true))
            .column(Column::initial(60.0).clip(true))
            .column(Column::initial(140.0).clip(true).resizable(true));

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    ui.label(egui::RichText::new("时间").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("协议").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("远程").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("状态").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("PID").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("进程").color(theme::fg_secondary()).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let (key, conn) = &items[row.index()];
                    let is_selected = sel_key.as_deref() == Some(key.as_str());
                    let has_match = matched_rules.contains_key(key);

                    // 时间
                    row.col(|ui| {
                        let prefix = if has_match { "! " } else { "" };
                        ui.label(
                            egui::RichText::new(format!("{}{}", prefix, theme::fmt_time(conn.last_seen)))
                                .font(egui::FontId::monospace(11.0))
                                .color(theme::fg_secondary()),
                        );
                    });
                    // 协议
                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:?}", conn.proto).to_uppercase())
                                .color(theme::fg_tertiary())
                                .size(11.0),
                        );
                    });
                    // 远程
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("{}:{}", conn.remote.addr, conn.remote.port))
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_primary()),
                            )
                            .truncate(),
                        );
                    });
                    // 状态
                    row.col(|ui| {
                        badge::badge(ui, conn.state.as_str(), conn_state_badge_variant(&conn.state));
                    });
                    // PID
                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(conn.pid.to_string())
                                .font(egui::FontId::monospace(11.0))
                                .color(theme::fg_secondary()),
                        );
                    });
                    // 进程
                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(conn.process_name.as_deref().unwrap_or("")).color(theme::fg_primary()),
                        );
                    });

                    if is_selected {
                        row.set_selected(true);
                    }
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            clicked_deselect = true;
                        } else {
                            clicked_key = Some(key.clone());
                        }
                    }
                });
            });

        if clicked_deselect {
            self.selected_network_key = None;
            self.detail_visible = false;
        } else if let Some(k) = clicked_key {
            self.selected_network_key = Some(k);
            self.detail_visible = true;
        }
    }

    fn render_events_table(&mut self, ui: &mut egui::Ui) {
        let filtered_keys = self.filtered_event_keys.clone();
        let matched_rules = self.event_matched_rules.clone();
        let sel_key = self.selected_event_key.clone();

        let mut items: Vec<(String, &SysmonEvent)> = self
            .event_items
            .iter()
            .filter(|item| match &filtered_keys {
                None => true,
                Some(keys) => keys.contains(&event_key(item)),
            })
            .map(|item| (event_key(item), item))
            .collect();
        items.sort_by(|a, b| {
            b.1.timestamp_epoch
                .partial_cmp(&a.1.timestamp_epoch)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if items.is_empty() {
            self.render_empty_state(ui, "事件");
            return;
        }

        let mut clicked_key: Option<String> = None;
        let mut clicked_deselect = false;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(140.0).clip(true))
            .column(Column::initial(100.0).clip(true))
            .column(Column::initial(200.0).clip(true).resizable(true))
            .column(Column::initial(140.0).clip(true).resizable(true));

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
                    ui.label(egui::RichText::new("进程").color(theme::fg_secondary()).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let (key, event) = &items[row.index()];
                    let is_selected = sel_key.as_deref() == Some(key.as_str());
                    let has_match = matched_rules.contains_key(key);

                    // 时间
                    row.col(|ui| {
                        let prefix = if has_match { "! " } else { "" };
                        let ts = if event.timestamp_valid && event.timestamp_epoch > 0.0 {
                            theme::fmt_time(event.timestamp_epoch as u64)
                        } else {
                            event.timestamp.clone()
                        };
                        ui.label(
                            egui::RichText::new(format!("{}{}", prefix, ts))
                                .font(egui::FontId::monospace(11.0))
                                .color(theme::fg_secondary()),
                        );
                    });
                    // 类型
                    row.col(|ui| {
                        badge::badge(ui, event.event_type.label(), event_badge_variant(&event.event_type));
                    });
                    // 目标
                    row.col(|ui| {
                        let dest = event_destination_for(event);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&dest)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_primary()),
                            )
                            .truncate(),
                        );
                    });
                    // 进程
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&event.process_name).color(theme::fg_primary()));
                    });

                    if is_selected {
                        row.set_selected(true);
                    }
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            clicked_deselect = true;
                        } else {
                            clicked_key = Some(key.clone());
                        }
                    }
                });
            });

        if clicked_deselect {
            self.selected_event_key = None;
            self.detail_visible = false;
        } else if let Some(k) = clicked_key {
            self.selected_event_key = Some(k);
            self.detail_visible = true;
        }
    }

    fn render_empty_state(&self, ui: &mut egui::Ui, tab_name: &str) {
        ui.add_space(80.0);
        ui.vertical_centered(|ui| {
            let msg = match tab_name {
                "持久化" => {
                    if self.autorun_items.is_empty() {
                        "点击「刷新数据」加载持久化项"
                    } else {
                        "没有匹配当前过滤条件的条目"
                    }
                }
                "网络" => {
                    if self.network_items.is_empty() {
                        "点击「刷新数据」加载网络连接"
                    } else {
                        "没有匹配当前过滤条件的连接"
                    }
                }
                "事件" => {
                    if self.event_items.is_empty() {
                        "无事件数据，请在 Sysmon 页面开始采集"
                    } else {
                        "没有匹配当前过滤条件的事件"
                    }
                }
                _ => "无数据",
            };
            ui.label(egui::RichText::new(msg).color(theme::fg_secondary()));
        });
    }

    // ── Detail Panel ───────────────────────────────────────

    pub fn render_detail_panel(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        match self.active_tab {
            WorkspaceTab::Autoruns => self.render_autoruns_detail(ui, ctx, rt),
            WorkspaceTab::Network => self.render_network_detail(ui, ctx, rt),
            WorkspaceTab::Events => self.render_events_detail(ui),
        }
    }

    fn render_autoruns_detail(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let item = match self.selected_autorun_id {
            Some(id) => match self.autorun_items.iter().find(|i| i.id == id) {
                Some(i) => i.clone(),
                None => {
                    self.detail_visible = false;
                    return;
                }
            },
            None => {
                self.detail_visible = false;
                return;
            }
        };

        let matched = self.autorun_matched_rules.get(&item.id).cloned().unwrap_or_default();

        egui::ScrollArea::vertical()
            .id_salt("ws_autoruns_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header with close button (right_to_left, DESIGN.md 4.7)
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_autorun_id = None;
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&item.entry)
                                    .color(theme::fg_primary())
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                badge::badge(ui, item.risk.as_str(), risk_badge_variant(&item.risk));
                                ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                ui.label(egui::RichText::new(&item.category).color(theme::fg_tertiary()));
                                if !item.enabled {
                                    ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                    ui.label(egui::RichText::new("已禁用").color(theme::fg_tertiary()));
                                }
                            });
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Detail rows
                detail_row(ui, "类别", Some(&item.category), false);
                detail_row(
                    ui,
                    "启用状态",
                    Some(if item.enabled { "已启用" } else { "已禁用" }),
                    false,
                );
                detail_row(ui, "文件路径", item.image_path.as_deref(), true);
                detail_row(ui, "启动命令", item.launch_string.as_deref(), true);
                detail_row(ui, "注册表位置", Some(&item.location), true);
                detail_row(ui, "发布者", Some(&item.publisher), false);
                detail_row(ui, "描述", Some(&item.description), false);
                detail_row(ui, "MD5", item.md5.as_deref(), true);
                detail_row(ui, "SHA256", item.sha256.as_deref(), true);
                detail_row(ui, "风险", Some(item.risk.as_str()), false);

                // Matched rules
                if !matched.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("匹配规则")
                            .strong()
                            .color(theme::fg_secondary())
                            .size(12.0),
                    );
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        for rule in &matched {
                            badge::badge(ui, &rule.name, severity_badge_variant(&rule.severity));
                            ui.add_space(2.0);
                        }
                    });
                }

                // Actions
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let entry_id = item.id;
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("删除").color(theme::semantic_danger()),
                        ))
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
                    if item.image_path.is_some() && item.file_exists && ui.button("打开文件位置").clicked() {
                        if let Some(ref path) = item.image_path {
                            if let Err(e) = AutorunsService::open_explorer(path.clone()) {
                                self.last_error = Some(e.to_string());
                            }
                        }
                    }

                    // 更多操作 dropdown
                    let image_path = item.image_path.clone();
                    ui.menu_button("更多操作", |ui| {
                        if ui.button("取消隐藏").clicked() {
                            if let Some(ref path) = image_path {
                                let path = path.clone();
                                rt.spawn_blocking(move || {
                                    if let Err(e) = WorkspaceService::unhide_path(path) {
                                        tracing::error!("unhide: {}", e);
                                    }
                                });
                            }
                            // egui 0.36: 菜单默认 CloseOnClick，点击后自动关闭，无需 close_menu()
                        }
                        if ui.button("获取所有权").clicked() {
                            if let Some(ref path) = image_path {
                                let path = path.clone();
                                rt.spawn_blocking(move || {
                                    if let Err(e) = WorkspaceService::take_ownership(path) {
                                        tracing::error!("take ownership: {}", e);
                                    }
                                });
                            }
                        }
                        if ui.button("取样").clicked() {
                            if let Some(ref path) = image_path {
                                let path = path.clone();
                                let output_dir = ctx.app_dirs.root().join("samples");
                                let output_dir_str = output_dir.to_string_lossy().to_string();
                                rt.spawn_blocking(move || {
                                    if let Err(e) =
                                        WorkspaceService::sample_path(path, output_dir_str, "infected".to_string())
                                    {
                                        tracing::error!("sample: {}", e);
                                    }
                                });
                            }
                        }
                    });
                });
            });
    }

    fn render_network_detail(&mut self, ui: &mut egui::Ui, _ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let sel_key = match &self.selected_network_key {
            Some(k) => k.clone(),
            None => {
                self.detail_visible = false;
                return;
            }
        };

        let conn = match self.network_items.iter().find(|c| network_key(c) == sel_key) {
            Some(c) => c.clone(),
            None => {
                self.detail_visible = false;
                return;
            }
        };

        let key = network_key(&conn);
        let matched = self.network_matched_rules.get(&key).cloned().unwrap_or_default();

        egui::ScrollArea::vertical()
            .id_salt("ws_network_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header with close button
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_network_key = None;
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(conn.process_name.as_deref().unwrap_or("未知进程"))
                                    .color(theme::fg_primary())
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                badge::badge(ui, conn.state.as_str(), conn_state_badge_variant(&conn.state));
                                ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                ui.label(
                                    egui::RichText::new(format!("{:?}", conn.proto).to_uppercase())
                                        .color(theme::fg_tertiary())
                                        .size(11.0),
                                );
                            });
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Detail rows
                detail_row(ui, "协议", Some(&format!("{:?}", conn.proto)), false);
                detail_row(
                    ui,
                    "本地地址",
                    Some(&format!("{}:{}", conn.local.addr, conn.local.port)),
                    true,
                );
                detail_row(
                    ui,
                    "远程地址",
                    Some(&format!("{}:{}", conn.remote.addr, conn.remote.port)),
                    true,
                );
                detail_row(ui, "状态", Some(conn.state.as_str()), false);
                detail_row(ui, "PID", Some(&conn.pid.to_string()), true);
                detail_row(ui, "进程名", conn.process_name.as_deref(), false);
                detail_row(ui, "进程路径", conn.process_path.as_deref(), true);
                detail_row(ui, "命令行", conn.process_cmdline.as_deref(), true);
                detail_row(ui, "首次出现", Some(&theme::fmt_time(conn.first_seen)), false);
                detail_row(ui, "最近出现", Some(&theme::fmt_time(conn.last_seen)), false);

                // Matched rules
                if !matched.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("匹配规则")
                            .strong()
                            .color(theme::fg_secondary())
                            .size(12.0),
                    );
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        for rule in &matched {
                            badge::badge(ui, &rule.name, severity_badge_variant(&rule.severity));
                            ui.add_space(2.0);
                        }
                    });
                }

                // Actions
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let pid = conn.pid;
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("终止进程").color(theme::semantic_danger()),
                        ))
                        .clicked()
                    {
                        self.pending_kill_pid = Some(pid);
                        self.kill_confirm_open = true;
                    }
                    if conn.process_path.is_some() && ui.button("打开文件位置").clicked() {
                        if let Some(ref path) = conn.process_path {
                            if let Err(e) = AutorunsService::open_explorer(path.clone()) {
                                self.last_error = Some(e.to_string());
                            }
                        }
                    }
                    let _ = rt;
                });
            });
    }

    fn render_events_detail(&mut self, ui: &mut egui::Ui) {
        let sel_key = match &self.selected_event_key {
            Some(k) => k.clone(),
            None => {
                self.detail_visible = false;
                return;
            }
        };

        let event = match self.event_items.iter().find(|e| event_key(e) == sel_key) {
            Some(e) => e.clone(),
            None => {
                self.detail_visible = false;
                return;
            }
        };

        let key = event_key(&event);
        let matched = self.event_matched_rules.get(&key).cloned().unwrap_or_default();

        egui::ScrollArea::vertical()
            .id_salt("ws_events_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header with close button
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_event_key = None;
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
                            let ts = if event.timestamp_valid && event.timestamp_epoch > 0.0 {
                                theme::fmt_time(event.timestamp_epoch as u64)
                            } else {
                                event.timestamp.clone()
                            };
                            ui.label(egui::RichText::new(ts).color(theme::fg_secondary()).size(11.0));
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Detail rows
                detail_row(ui, "类型", Some(event.event_type.label()), false);
                detail_row(ui, "时间", Some(&event.timestamp), false);
                detail_row(ui, "进程", Some(&event.process_name), false);
                detail_row(ui, "路径", Some(&event.process_path), true);
                detail_row(ui, "用户", Some(&event.user), false);
                detail_row(ui, "域名", Some(&event.query_name), true);
                detail_row(
                    ui,
                    "目标IP",
                    Some(&format!("{}:{}", event.destination_ip, event.destination_port)),
                    true,
                );
                detail_row(ui, "协议", Some(&event.protocol), false);

                // Matched rules
                if !matched.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("匹配规则")
                            .strong()
                            .color(theme::fg_secondary())
                            .size(12.0),
                    );
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        for rule in &matched {
                            badge::badge(ui, &rule.name, severity_badge_variant(&rule.severity));
                            ui.add_space(2.0);
                        }
                    });
                }
            });
    }

    // ── Stats Bar ──────────────────────────────────────────

    pub fn render_stats_bar(&self, ui: &mut egui::Ui) {
        let autorun_filtered = self
            .filtered_autorun_ids
            .as_ref()
            .map(|s| s.len())
            .unwrap_or(self.autorun_items.len());
        let network_filtered = self
            .filtered_network_keys
            .as_ref()
            .map(|s| s.len())
            .unwrap_or(self.network_items.len());
        let event_filtered = self
            .filtered_event_keys
            .as_ref()
            .map(|s| s.len())
            .unwrap_or(self.event_items.len());

        let autorun_matched = self.autorun_matched_rules.len();
        let network_matched = self.network_matched_rules.len();
        let event_matched = self.event_matched_rules.len();

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("持久化 {}/{}", autorun_filtered, self.autorun_items.len()))
                    .color(theme::fg_secondary())
                    .size(11.0),
            );
            if autorun_matched > 0 {
                ui.label(
                    egui::RichText::new(format!("! {}", autorun_matched))
                        .color(theme::semantic_danger())
                        .size(11.0),
                );
            }

            ui.separator();

            ui.label(
                egui::RichText::new(format!("网络 {}/{}", network_filtered, self.network_items.len()))
                    .color(theme::fg_secondary())
                    .size(11.0),
            );
            if network_matched > 0 {
                ui.label(
                    egui::RichText::new(format!("! {}", network_matched))
                        .color(theme::semantic_danger())
                        .size(11.0),
                );
            }

            ui.separator();

            ui.label(
                egui::RichText::new(format!("事件 {}/{}", event_filtered, self.event_items.len()))
                    .color(theme::fg_secondary())
                    .size(11.0),
            );
            if event_matched > 0 {
                ui.label(
                    egui::RichText::new(format!("! {}", event_matched))
                        .color(theme::semantic_danger())
                        .size(11.0),
                );
            }

            ui.add_space(ui.available_width().max(0.0) - 100.0);

            if self.scanning {
                ui.label(egui::RichText::new("扫描中…").color(theme::accent()).size(11.0));
            } else if self.loading {
                ui.label(egui::RichText::new("加载中…").color(theme::accent()).size(11.0));
            }
        });
    }

    // ── Rule Manager Dialog ────────────────────────────────

    fn render_rule_manager(&mut self, ui: &mut egui::Ui, ctx: &AppContext) {
        let mut open = self.rule_manager_open;
        let mut add_rule = false;
        let mut edit_rule: Option<Rule> = None;
        let mut delete_rule_id: Option<String> = None;
        let mut export_rules = false;

        egui::Window::new("规则管理")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.rule_search_query)
                            .desired_width(200.0)
                            .hint_text("搜索规则名称/分类..."),
                    );
                    if ui.button("➕ 添加").clicked() {
                        add_rule = true;
                    }
                    if ui.button("导出").clicked() {
                        export_rules = true;
                    }
                    ui.add_enabled(false, egui::Button::new("导入"))
                        .on_hover_text("导入功能尚未实现");
                });
                ui.separator();

                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    let q = self.rule_search_query.trim().to_lowercase();
                    for rule in self.rules.iter_mut() {
                        if !q.is_empty()
                            && !rule.name.to_lowercase().contains(&q)
                            && !rule.family.to_lowercase().contains(&q)
                        {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut rule.enabled, "");

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&rule.name).color(theme::fg_primary()).strong());
                                    badge::badge(
                                        ui,
                                        severity_label(&rule.severity),
                                        severity_badge_variant(&rule.severity),
                                    );
                                });
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {} · {} 条件",
                                        target_label(&rule.target),
                                        rule.family,
                                        rule.conditions.len()
                                    ))
                                    .color(theme::fg_tertiary())
                                    .size(11.0),
                                );
                            });

                            ui.add_space(ui.available_width().max(0.0) - 120.0);

                            if ui.button("编辑").clicked() {
                                edit_rule = Some(rule.clone());
                            }
                            if ui
                                .add(egui::Button::new(
                                    egui::RichText::new("删除").color(theme::semantic_danger()),
                                ))
                                .clicked()
                            {
                                delete_rule_id = Some(rule.id.clone());
                            }
                        });
                        ui.separator();
                    }
                });
            });

        // Apply actions
        if add_rule {
            self.editing_rule = Some(Rule {
                id: format!(
                    "rule-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                ),
                name: String::new(),
                target: RuleTarget::Autorun,
                conditions: vec![],
                logic: Logic::And,
                severity: Severity::Medium,
                family: String::new(),
                enabled: true,
                description: None,
            });
            self.rule_edit_open = true;
        }
        if let Some(rule) = edit_rule {
            self.editing_rule = Some(rule);
            self.rule_edit_open = true;
        }
        if let Some(id) = delete_rule_id {
            self.rules.retain(|r| r.id != id);
        }
        if export_rules {
            self.export_rules_json(ctx);
        }

        self.rule_manager_open = open;
    }

    // ── Rule Edit Dialog ───────────────────────────────────

    fn render_rule_edit(&mut self, ui: &mut egui::Ui) {
        let mut open = self.rule_edit_open;
        let mut save = false;
        let mut cancel = false;
        let mut add_condition = false;
        let mut remove_condition: Option<usize> = None;

        // Take the rule out of self to avoid borrow issues
        let mut rule = match self.editing_rule.take() {
            Some(r) => r,
            None => {
                self.rule_edit_open = false;
                return;
            }
        };

        egui::Window::new("编辑规则")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(440.0)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("规则名称:");
                    ui.add(
                        egui::TextEdit::singleline(&mut rule.name)
                            .desired_width(240.0)
                            .hint_text("输入规则名称"),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("目标:");
                    egui::ComboBox::from_id_salt("ws_rule_target")
                        .selected_text(target_label(&rule.target))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut rule.target, RuleTarget::Autorun, "持久化");
                            ui.selectable_value(&mut rule.target, RuleTarget::Network, "网络");
                            ui.selectable_value(&mut rule.target, RuleTarget::Event, "事件");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("严重级别:");
                    egui::ComboBox::from_id_salt("ws_rule_severity")
                        .selected_text(severity_label(&rule.severity))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut rule.severity, Severity::Critical, "严重");
                            ui.selectable_value(&mut rule.severity, Severity::High, "高");
                            ui.selectable_value(&mut rule.severity, Severity::Medium, "中");
                            ui.selectable_value(&mut rule.severity, Severity::Low, "低");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("分类:");
                    ui.add(
                        egui::TextEdit::singleline(&mut rule.family)
                            .desired_width(180.0)
                            .hint_text("规则分类"),
                    );
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut rule.enabled, "启用");
                    ui.separator();
                    ui.label("逻辑:");
                    egui::ComboBox::from_id_salt("ws_rule_logic")
                        .selected_text(match rule.logic {
                            Logic::And => "AND (全部满足)",
                            Logic::Or => "OR (任一满足)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut rule.logic, Logic::And, "AND (全部满足)");
                            ui.selectable_value(&mut rule.logic, Logic::Or, "OR (任一满足)");
                        });
                });

                ui.add_space(4.0);
                ui.label("描述:");
                let mut desc = rule.description.clone().unwrap_or_default();
                ui.add(
                    egui::TextEdit::multiline(&mut desc)
                        .desired_width(400.0)
                        .desired_rows(2),
                );
                rule.description = if desc.is_empty() { None } else { Some(desc) };

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("条件")
                            .strong()
                            .color(theme::fg_secondary())
                            .size(12.0),
                    );
                    if ui.button("➕ 添加条件").clicked() {
                        add_condition = true;
                    }
                });

                let fields = fields_for_target(&rule.target);
                let mut condition_remove: Option<usize> = None;
                for (i, cond) in rule.conditions.iter_mut().enumerate() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(format!("ws_cond_field_{}", i))
                            .selected_text(if cond.field.is_empty() {
                                "选择字段".to_string()
                            } else {
                                cond.field.clone()
                            })
                            .show_ui(ui, |ui| {
                                for f in fields {
                                    ui.selectable_value(&mut cond.field, f.to_string(), *f);
                                }
                            });

                        egui::ComboBox::from_id_salt(format!("ws_cond_type_{}", i))
                            .selected_text(match cond.cond_type {
                                ConditionType::Contains => "包含",
                                ConditionType::Regex => "正则",
                                ConditionType::Equals => "等于",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut cond.cond_type, ConditionType::Contains, "包含");
                                ui.selectable_value(&mut cond.cond_type, ConditionType::Regex, "正则");
                                ui.selectable_value(&mut cond.cond_type, ConditionType::Equals, "等于");
                            });

                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("删除").color(theme::semantic_danger()),
                            ))
                            .clicked()
                        {
                            condition_remove = Some(i);
                        }
                    });
                    ui.add(
                        egui::TextEdit::multiline(&mut cond.value)
                            .desired_width(400.0)
                            .desired_rows(2)
                            .hint_text("每行一个匹配值"),
                    );
                }
                remove_condition = condition_remove;

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        save = true;
                    }
                    if ui.button("取消").clicked() {
                        cancel = true;
                    }
                });
            });

        // Apply condition add/remove
        if add_condition {
            rule.conditions.push(Condition {
                field: String::new(),
                cond_type: ConditionType::Contains,
                value: String::new(),
            });
        }
        if let Some(idx) = remove_condition {
            if idx < rule.conditions.len() {
                rule.conditions.remove(idx);
            }
        }

        // Apply save/cancel
        if save {
            if let Some(existing) = self.rules.iter_mut().find(|r| r.id == rule.id) {
                *existing = rule;
            } else {
                self.rules.push(rule);
            }
            self.rule_edit_open = false;
        } else if cancel || !open {
            self.editing_rule = None;
            self.rule_edit_open = false;
        } else {
            // Put the rule back for next frame
            self.editing_rule = Some(rule);
        }
        self.rule_edit_open = open;
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
                        .add(egui::Button::new(
                            egui::RichText::new("确认删除").color(theme::semantic_danger()),
                        ))
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
                let was_selected = self.selected_autorun_id == Some(entry_id);
                rt.spawn(async move {
                    match (AutorunsService { ctx: &ctx_clone }).delete_entry(entry_id).await {
                        Ok(result) => {
                            tracing::info!("delete result: success={}, msg={}", result.success, result.message)
                        }
                        Err(e) => tracing::error!("delete failed: {}", e),
                    }
                });
                if was_selected {
                    self.selected_autorun_id = None;
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

    // ── Kill Confirmation Dialog (DESIGN.md 4.10) ───────────

    fn render_kill_confirm(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.kill_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("确认终止进程")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label("确定要终止此进程吗？此操作不可撤销。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("确认终止").color(theme::semantic_danger()),
                        ))
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
            if let Some(pid) = self.pending_kill_pid {
                let ctx_clone = ctx.clone();
                rt.spawn(async move {
                    let svc = NetworkService { ctx: &ctx_clone };
                    if let Err(e) = svc.kill_process(pid).await {
                        tracing::error!("kill process failed: {}", e);
                    }
                });
            }
            open = false;
        }
        if cancelled {
            open = false;
        }

        self.kill_confirm_open = open;
        if !open {
            self.pending_kill_pid = None;
        }
    }

    // ── Helpers ────────────────────────────────────────────

    fn export_csv(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let dir = ctx.app_dirs.root().to_path_buf();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();

        match self.active_tab {
            WorkspaceTab::Autoruns => {
                let items = self.autorun_items.clone();
                let path = dir.join(format!("workspace_autoruns_{}.csv", timestamp));
                rt.spawn_blocking(move || {
                    if let Err(e) = write_autoruns_csv(&path, &items) {
                        tracing::error!("csv export failed: {}", e);
                    } else {
                        tracing::info!("exported {} autoruns to {}", items.len(), path.display());
                    }
                });
            }
            WorkspaceTab::Network => {
                let items = self.network_items.clone();
                let path = dir.join(format!("workspace_network_{}.csv", timestamp));
                rt.spawn_blocking(move || {
                    if let Err(e) = write_network_csv(&path, &items) {
                        tracing::error!("csv export failed: {}", e);
                    } else {
                        tracing::info!("exported {} connections to {}", items.len(), path.display());
                    }
                });
            }
            WorkspaceTab::Events => {
                let items = self.event_items.clone();
                let path = dir.join(format!("workspace_events_{}.csv", timestamp));
                rt.spawn_blocking(move || {
                    if let Err(e) = write_events_csv(&path, &items) {
                        tracing::error!("csv export failed: {}", e);
                    } else {
                        tracing::info!("exported {} events to {}", items.len(), path.display());
                    }
                });
            }
        }
    }

    fn export_rules_json(&self, ctx: &AppContext) {
        let json = match serde_json::to_string_pretty(
            &self
                .rules
                .iter()
                .map(|r| RuleJson {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    target: target_label(&r.target).to_string(),
                    conditions: r
                        .conditions
                        .iter()
                        .map(|c| ConditionJson {
                            field: c.field.clone(),
                            cond_type: format!("{:?}", c.cond_type),
                            value: c.value.clone(),
                        })
                        .collect(),
                    logic: format!("{:?}", r.logic),
                    severity: severity_label(&r.severity).to_string(),
                    family: r.family.clone(),
                    enabled: r.enabled,
                    description: r.description.clone(),
                })
                .collect::<Vec<_>>(),
        ) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("serialize rules: {}", e);
                return;
            }
        };
        let dir = ctx.app_dirs.root().to_path_buf();
        let path = dir.join(format!(
            "irtool_rules_export_{}.json",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ));
        if let Err(e) = std::fs::write(&path, json) {
            tracing::error!("write rules export: {}", e);
        } else {
            tracing::info!("exported rules to {}", path.display());
        }
    }
}

// ── Free Helpers ──────────────────────────────────────────

fn risk_badge_variant(risk: &RiskLevel) -> BadgeVariant {
    match risk {
        RiskLevel::Safe => BadgeVariant::Success,
        RiskLevel::Suspicious => BadgeVariant::Warning,
        RiskLevel::HighRisk => BadgeVariant::Danger,
    }
}

fn conn_state_badge_variant(state: &ConnState) -> BadgeVariant {
    match state {
        ConnState::Established => BadgeVariant::Success,
        ConnState::Listen => BadgeVariant::Info,
        ConnState::TimeWait | ConnState::CloseWait => BadgeVariant::Warning,
        ConnState::None => BadgeVariant::Default,
        _ => BadgeVariant::Default,
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

fn event_destination_for(e: &SysmonEvent) -> String {
    match e.event_type {
        SysmonEventType::NetworkConnect => {
            if e.destination_ip.is_empty() && e.destination_port == 0 {
                String::new()
            } else {
                format!("{}:{}", e.destination_ip, e.destination_port)
            }
        }
        SysmonEventType::Dns | SysmonEventType::DnsClient => e.query_name.clone(),
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

fn target_label(t: &RuleTarget) -> &'static str {
    match t {
        RuleTarget::Autorun => "持久化",
        RuleTarget::Network => "网络",
        RuleTarget::Event => "事件",
    }
}

fn severity_label(s: &Severity) -> &'static str {
    match s {
        Severity::Critical => "严重",
        Severity::High => "高",
        Severity::Medium => "中",
        Severity::Low => "低",
    }
}

fn severity_badge_variant(s: &Severity) -> BadgeVariant {
    match s {
        Severity::Critical => BadgeVariant::Danger,
        Severity::High => BadgeVariant::Warning,
        Severity::Medium => BadgeVariant::Info,
        Severity::Low => BadgeVariant::Default,
    }
}

fn fields_for_target(t: &RuleTarget) -> &'static [&'static str] {
    match t {
        RuleTarget::Autorun => &[
            "entry",
            "image_path",
            "launch_string",
            "location",
            "publisher",
            "description",
            "category",
            "md5",
            "sha256",
            "risk",
        ],
        RuleTarget::Network => &[
            "pid",
            "process_name",
            "process_path",
            "process_cmdline",
            "proto",
            "family",
            "local_addr",
            "local_port",
            "remote_addr",
            "remote_port",
            "state",
        ],
        RuleTarget::Event => &[
            "event_id",
            "event_type",
            "process_name",
            "process_path",
            "user",
            "query_name",
            "destination_ip",
            "destination_port",
            "protocol",
            "target_filename",
        ],
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn write_autoruns_csv(path: &std::path::Path, items: &[AutorunItem]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "entry,image_path,category,risk,enabled,publisher,location")?;
    for item in items {
        writeln!(
            f,
            "{},{},{},{},{},{},{}",
            csv_escape(&item.entry),
            csv_escape(item.image_path.as_deref().unwrap_or("")),
            csv_escape(&item.category),
            csv_escape(item.risk.as_str()),
            item.enabled,
            csv_escape(&item.publisher),
            csv_escape(&item.location),
        )?;
    }
    Ok(())
}

fn write_network_csv(path: &std::path::Path, items: &[NetConn]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "last_seen,proto,local,remote,state,pid,process_name,process_path")?;
    for conn in items {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{}",
            theme::fmt_time(conn.last_seen),
            format_args!("{:?}", conn.proto),
            csv_escape(&format!("{}:{}", conn.local.addr, conn.local.port)),
            csv_escape(&format!("{}:{}", conn.remote.addr, conn.remote.port)),
            conn.state.as_str(),
            conn.pid,
            csv_escape(conn.process_name.as_deref().unwrap_or("")),
            csv_escape(conn.process_path.as_deref().unwrap_or("")),
        )?;
    }
    Ok(())
}

fn write_events_csv(path: &std::path::Path, items: &[SysmonEvent]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "timestamp,event_type,event_id,process_name,destination,query_name")?;
    for e in items {
        let dest = event_destination_for(e);
        writeln!(
            f,
            "{},{},{},{},{},{}",
            csv_escape(&e.timestamp),
            csv_escape(e.event_type.label()),
            e.event_id,
            csv_escape(&e.process_name),
            csv_escape(&dest),
            csv_escape(&e.query_name),
        )?;
    }
    Ok(())
}

// ── Rule JSON serialization (for export) ───────────────────

#[derive(serde::Serialize)]
struct ConditionJson {
    field: String,
    cond_type: String,
    value: String,
}

#[derive(serde::Serialize)]
struct RuleJson {
    id: String,
    name: String,
    target: String,
    conditions: Vec<ConditionJson>,
    logic: String,
    severity: String,
    family: String,
    enabled: bool,
    description: Option<String>,
}
