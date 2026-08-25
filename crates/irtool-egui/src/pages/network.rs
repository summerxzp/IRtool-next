//! 网络监控页 —— P5 范式页（TableShell + 增量数据流 + i18n + risk 高亮 + 右键菜单）。
//!
//! 数据流（rework-plan §6 范式）：
//! ```text
//! EventBus ─ event_bridge drain ─▶ handle_snapshot / handle_enrichment
//!   ├─ 只改数据：store（VecDeque 环形，MAX_ROWS 弹头）
//!   ├─ 显示串（时间戳/端点等）在进 store 时格式化一次（行闭包零分配的前提）
//!   └─ 置 view_dirty
//! 渲染帧：view_dirty 才重建视图索引（过滤+排序后的 store 下标），TableShell 按索引渲染
//! ```
//!
//! 行底色优先级：选中（selected.bg）＞ risk（role.bg 软色）＞ 键盘焦点（hover）。

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::collections::VecDeque;

use eframe::egui;
use irtool_net_monitor::Family;
use irtool_service::context::AppContext;
use irtool_service::dto::network::{
    NetworkEnrichmentPayload, NetworkPollingControl, NetworkSnapshotPayload,
};
use irtool_service::services::network::NetworkService;
use irtool_service::types::{CmdlineStatus, ConnState, NetConn, Proto};
use rust_i18n::t;

use crate::design::table::{self, RowCtx, TableColumn, TableShell};
use crate::design::icon::Icon;
use crate::design::{theme as dtheme, tokens::Palette, widgets};
use crate::theme;
use crate::widgets::banner;

// 注：Family 暂从业务 crate 引入（irtool-service::types 未转发该类型，
// 业务层禁改；P6 可在 service 补一行 re-export 后换回统一入口）。

/// 环形缓冲上限（沿用 MAX_EVENTS = 10000 量级语义，超限弹头丢弃）。
const MAX_ROWS: usize = 10000;

/// 已知恶意 IP 列表（与工作台默认规则 default-malicious-ip-network 同源；
/// S7「恶意 IP 配置化」落地后改为读配置）。
const MALICIOUS_IPS: &[&str] = &[
    "82.23.246.148",
    "161.248.87.175",
    "202.79.169.50",
    "47.76.246.88",
    "202.95.16.13",
    "47.242.90.146",
    "202.79.171.236",
    "143.92.57.157",
    "137.220.135.15",
    "206.238.115.137",
];

// ── 过滤器 ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProtoFilter {
    All,
    Tcp,
    Udp,
}

/// 连接状态多选过滤的全集。
const ALL_STATES: &[ConnState] = &[
    ConnState::Established,
    ConnState::Listen,
    ConnState::SynSent,
    ConnState::SynRcvd,
    ConnState::FinWait1,
    ConnState::FinWait2,
    ConnState::CloseWait,
    ConnState::Closing,
    ConnState::LastAck,
    ConnState::TimeWait,
    ConnState::Closed,
    ConnState::DeleteTcb,
];

// ── 风险分级（spec §2.2 role：danger/warning 软色行高亮）────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Risk {
    None,
    /// 可疑（warning.role）：CLOSE_WAIT 挂起残留。
    Warn,
    /// 危险（danger.role）：命中恶意 IP 列表。
    Danger,
}

fn risk_of(state: ConnState, remote_addr: &str) -> Risk {
    if MALICIOUS_IPS.contains(&remote_addr) {
        Risk::Danger
    } else if state == ConnState::CloseWait {
        Risk::Warn
    } else {
        Risk::None
    }
}

fn risk_bg(risk: Risk, pal: &Palette) -> Option<eframe::egui::Color32> {
    match risk {
        Risk::None => None,
        Risk::Warn => Some(pal.warning.bg),
        Risk::Danger => Some(pal.danger.bg),
    }
}

// ── 行记录（store 存储单元，显示串预格式化）────────────────────

/// 行身份键（对齐 React rowKey：proto|family|local|remote|pid）。
#[derive(Clone, PartialEq, Eq, Hash)]
struct ConnKey {
    proto: Proto,
    family: Family,
    local_addr: String,
    local_port: u16,
    remote_addr: String,
    remote_port: u16,
    pid: u32,
}

impl ConnKey {
    fn of_conn(c: &NetConn) -> Self {
        ConnKey {
            proto: c.proto,
            family: c.family,
            local_addr: c.local.addr.clone(),
            local_port: c.local.port,
            remote_addr: c.remote.addr.clone(),
            remote_port: c.remote.port,
            pid: c.pid,
        }
    }

    fn of_row(r: &NetRow) -> Self {
        ConnKey {
            proto: r.proto,
            family: r.family,
            local_addr: r.local_addr.clone(),
            local_port: r.local_port,
            remote_addr: r.remote_addr.clone(),
            remote_port: r.remote_port,
            pid: r.pid,
        }
    }

    fn matches(&self, r: &NetRow) -> bool {
        self.proto == r.proto
            && self.family == r.family
            && self.local_addr == r.local_addr
            && self.local_port == r.local_port
            && self.remote_addr == r.remote_addr
            && self.remote_port == r.remote_port
            && self.pid == r.pid
    }
}

/// 一条连接记录。原始字段用于排序/导出/详情；`d_*` 为进店时一次性格式化的
/// 显示串（表格行闭包只引用这些 &str，零分配）；`search_blob` 为小写检索串。
#[derive(Clone)]
struct NetRow {
    // 身份
    pid: u32,
    proto: Proto,
    family: Family,
    local_addr: String,
    local_port: u16,
    remote_addr: String,
    remote_port: u16,

    // 载荷
    state: ConnState,
    process_name: Option<String>,
    process_path: Option<String>,
    process_cmdline: Option<String>,
    cmdline_status: CmdlineStatus,
    first_seen: u64,
    last_seen: u64,
    is_current: bool,

    // 派生（refresh_derived 维护）
    risk: Risk,
    d_pid: String,
    d_process: String,
    d_local: String,
    d_remote: String,
    d_proto: String,
    d_family: String,
    d_first_seen: String,
    d_last_seen: String,
    search_blob: String,
}

impl NetRow {
    fn from_conn(c: &NetConn) -> Self {
        let mut row = NetRow {
            pid: c.pid,
            proto: c.proto,
            family: c.family,
            local_addr: c.local.addr.clone(),
            local_port: c.local.port,
            remote_addr: c.remote.addr.clone(),
            remote_port: c.remote.port,
            state: c.state,
            process_name: c.process_name.clone(),
            process_path: c.process_path.clone(),
            process_cmdline: c.process_cmdline.clone(),
            cmdline_status: c.cmdline_status,
            first_seen: c.first_seen,
            last_seen: c.last_seen,
            is_current: c.is_current,
            risk: Risk::None,
            d_pid: String::new(),
            d_process: String::new(),
            d_local: String::new(),
            d_remote: String::new(),
            d_proto: String::new(),
            d_family: String::new(),
            d_first_seen: String::new(),
            d_last_seen: String::new(),
            search_blob: String::new(),
        };
        row.refresh_derived();
        row
    }

    /// 用快照项原地更新载荷字段（身份字段按构造约定一致）。返回是否有变化。
    fn absorb(&mut self, c: &NetConn) -> bool {
        let mut changed = false;
        if self.state != c.state {
            self.state = c.state;
            changed = true;
        }
        if self.first_seen != c.first_seen {
            self.first_seen = c.first_seen;
            changed = true;
        }
        if self.last_seen != c.last_seen {
            self.last_seen = c.last_seen;
            changed = true;
        }
        if self.is_current != c.is_current {
            self.is_current = c.is_current;
            changed = true;
        }
        if self.cmdline_status != c.cmdline_status {
            self.cmdline_status = c.cmdline_status;
            changed = true;
        }
        if self.process_name != c.process_name {
            self.process_name.clone_from(&c.process_name);
            changed = true;
        }
        if self.process_path != c.process_path {
            self.process_path.clone_from(&c.process_path);
            changed = true;
        }
        if self.process_cmdline != c.process_cmdline {
            self.process_cmdline.clone_from(&c.process_cmdline);
            changed = true;
        }
        changed
    }

    /// 重算全部派生字段（原始字段变更后调用；仅发生在事件写入路径，不在渲染帧）。
    fn refresh_derived(&mut self) {
        self.risk = risk_of(self.state, &self.remote_addr);
        self.d_pid = self.pid.to_string();
        self.d_process = self.process_name.clone().unwrap_or_else(|| "-".into());
        self.d_local = format_endpoint(&self.local_addr, self.local_port);
        self.d_remote = format_endpoint(&self.remote_addr, self.remote_port);
        self.d_proto = match self.proto {
            Proto::Tcp => "TCP".into(),
            Proto::Udp => "UDP".into(),
        };
        self.d_family = format!("{:?}", self.family).to_uppercase();
        self.d_first_seen = theme::fmt_time(self.first_seen);
        self.d_last_seen = theme::fmt_time(self.last_seen);
        self.search_blob = format!(
            "{} {} {} {}:{} {}:{} {}",
            self.pid,
            self.process_name.as_deref().unwrap_or(""),
            self.process_path.as_deref().unwrap_or(""),
            self.local_addr,
            self.local_port,
            self.remote_addr,
            self.remote_port,
            self.state.as_str(),
        )
        .to_lowercase();
    }
}

fn format_endpoint(addr: &str, port: u16) -> String {
    let a = if addr.is_empty() || addr == "0.0.0.0" || addr == "::" {
        "*"
    } else {
        addr
    };
    let p = if port == 0 { "*".to_string() } else { port.to_string() };
    format!("{}:{}", a, p)
}

// ── 表格列定义（列 id/顺序/初始宽 对齐 React columns.tsx 默认值）──

fn columns_def() -> Vec<TableColumn> {
    vec![
        TableColumn { id: "first_seen", title_key: "network.columns.first-seen", width: 150.0, min_width: 90.0, max_width: 260.0 },
        TableColumn { id: "pid", title_key: "network.columns.pid", width: 48.0, min_width: 35.0, max_width: 120.0 },
        TableColumn { id: "process", title_key: "network.columns.process", width: 160.0, min_width: 80.0, max_width: 320.0 },
        TableColumn { id: "local", title_key: "network.columns.local", width: 170.0, min_width: 100.0, max_width: 360.0 },
        TableColumn { id: "remote", title_key: "network.columns.remote", width: 170.0, min_width: 100.0, max_width: 360.0 },
        TableColumn { id: "state", title_key: "network.columns.state", width: 96.0, min_width: 60.0, max_width: 200.0 },
        TableColumn { id: "proto", title_key: "network.columns.proto", width: 60.0, min_width: 36.0, max_width: 120.0 },
        TableColumn { id: "family", title_key: "network.columns.family", width: 50.0, min_width: 30.0, max_width: 120.0 },
        TableColumn { id: "path", title_key: "network.columns.path", width: 280.0, min_width: 100.0, max_width: 520.0 },
        TableColumn { id: "cmdline", title_key: "network.columns.cmdline", width: 200.0, min_width: 100.0, max_width: 520.0 },
        TableColumn { id: "last_seen", title_key: "network.columns.last-seen", width: 160.0, min_width: 90.0, max_width: 260.0 },
    ]
}

// ── 右键菜单动作（闭包内收集，show 之后执行）──────────────────

#[derive(Clone, Copy)]
enum MenuAction {
    None,
    CopyRow,
    Kill(u32),
}

// ── 页面状态 ───────────────────────────────────────────────────

pub struct NetworkPageState {
    // 数据层：store + 主键索引 + 视图缓存
    store: VecDeque<NetRow>,
    index: HashMap<ConnKey, u32>,
    index_dirty: bool,
    view: Vec<u32>,
    view_dirty: bool,
    got_snapshot: bool,
    /// 工具栏下拉互斥开合槽（widgets::dropdown）。
    open_menu: Option<u8>,

    // 错误横幅
    last_error: Option<String>,

    // 过滤器（变化即置 view_dirty）
    search: String,
    proto_filter: ProtoFilter,
    state_filter: HashSet<ConnState>,
    state_dropdown_open: bool,
    show_history: bool,

    // 表格组件
    shell: TableShell,

    // 选中与详情面板
    selected: Option<ConnKey>,
    /// 选中行在 store 中的下标缓存（视图重建时解析；详情面板快速路径）。
    sel_store_idx: Option<u32>,
    pub detail_visible: bool,

    // 轮询控制
    paused: bool,
    interval_ms: u64,

    // 终止进程确认框
    kill_confirm_open: bool,
    pending_kill_pid: Option<u32>,
}

impl Default for NetworkPageState {
    fn default() -> Self {
        let mut shell = TableShell::new_with_schema("network", 2, columns_def());
        // 默认按最近出现降序：新/活跃记录恒在顶部
        shell.sort = Some(("last_seen", false));
        Self {
            store: VecDeque::new(),
            index: HashMap::new(),
            index_dirty: false,
            view: Vec::new(),
            view_dirty: true,
            got_snapshot: false,
            open_menu: None,
            last_error: None,
            search: String::new(),
            proto_filter: ProtoFilter::All,
            state_filter: ALL_STATES.iter().copied().collect(),
            state_dropdown_open: false,
            show_history: true,
            shell,
            selected: None,
            sel_store_idx: None,
            detail_visible: false,
            paused: false,
            interval_ms: 1000,
            kill_confirm_open: false,
            pending_kill_pid: None,
        }
    }
}

impl NetworkPageState {
    // ── 表格持久化（转发 TableShell；启动恢复 / 退出保存由 app.rs 调用，
    //    渲染期变更在 render_table 内即时写盘）──────────────────────

    pub fn load_table_state(&mut self, config_dir: &std::path::Path) {
        self.shell.load_table_state(config_dir);
    }

    pub fn save_table_state(&self, config_dir: &std::path::Path) {
        self.shell.save_table_state(config_dir);
    }

    // ── 数据接入（EventBus → store，只改数据不碰 UI）────────────

    pub fn handle_snapshot(&mut self, payload: NetworkSnapshotPayload) {
        self.got_snapshot = true;
        self.last_error = None;
        self.ensure_index();

        // 本轮快照命中的 store 下标（用于把消失连接翻转为历史）
        let mut touched: HashSet<u32> = HashSet::with_capacity(payload.items.len());

        for conn in &payload.items {
            let key = ConnKey::of_conn(conn);
            if let Some(idx) = self.index.get(&key).copied() {
                touched.insert(idx);
                if let Some(row) = self.store.get_mut(idx as usize) {
                    if row.absorb(conn) {
                        row.refresh_derived();
                    }
                }
            } else {
                let idx = self.store.len() as u32;
                self.store.push_back(NetRow::from_conn(conn));
                self.index.insert(key, idx);
            }
        }

        // 快照中消失的连接 → 翻转为历史（必须在弹头前，下标才与 touched 对齐）
        for (i, row) in self.store.iter_mut().enumerate() {
            let i = i as u32;
            if row.is_current && !touched.contains(&i) {
                row.is_current = false;
            }
        }

        // 环形上限：超限弹头丢弃（索引整体失效，下轮快照前惰性重建）
        while self.store.len() > MAX_ROWS {
            self.store.pop_front();
            self.index_dirty = true;
        }

        self.view_dirty = true;
    }

    pub fn handle_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    pub fn handle_enrichment(&mut self, enrichment: NetworkEnrichmentPayload) {
        for row in self.store.iter_mut() {
            if row.pid == enrichment.pid {
                row.cmdline_status = enrichment.cmdline_status;
                if let Some(ref cmdline) = enrichment.process_cmdline {
                    row.process_cmdline = Some(cmdline.clone());
                }
                row.refresh_derived();
            }
        }
        self.view_dirty = true;
    }

    /// 「清空历史」按钮：本地 store 同步清空（服务端由 NetworkService.clear_history 处理）。
    fn clear_local_history(&mut self) {
        self.store.clear();
        self.index.clear();
        self.index_dirty = false; // 已清空即一致
        self.sel_store_idx = None;
        self.shell.focused = None;
        self.view_dirty = true;
    }

    /// 惰性重建主键索引（弹头/插入后失效一次，重建 O(n)）。
    fn ensure_index(&mut self) {
        if !self.index_dirty {
            return;
        }
        self.index.clear();
        self.index.extend(
            self.store
                .iter()
                .enumerate()
                .map(|(i, r)| (ConnKey::of_row(r), i as u32)),
        );
        self.index_dirty = false;
    }

    // ── 视图缓存（dirty 才重建：过滤 + 排序后的 store 下标）────

    fn rebuild_view(&mut self) {
        if !self.view_dirty {
            return;
        }
        let q = self.search.trim().to_lowercase();
        let want_history = self.show_history;
        let proto_f = self.proto_filter;

        self.view.clear();
        for (i, r) in self.store.iter().enumerate() {
            if !want_history && !r.is_current {
                continue;
            }
            let proto_ok = match proto_f {
                ProtoFilter::All => true,
                ProtoFilter::Tcp => r.proto == Proto::Tcp,
                ProtoFilter::Udp => r.proto == Proto::Udp,
            };
            if !proto_ok {
                continue;
            }
            // 无状态（NONE/空）始终放行，其余按多选集合过滤（沿用旧语义）
            if r.state != ConnState::None && !self.state_filter.contains(&r.state) {
                continue;
            }
            if !q.is_empty() && !r.search_blob.contains(&q) {
                continue;
            }
            self.view.push(i as u32);
        }

        // 排序：按列原始值比较（零格式化），稳定排序保持插入序并列
        if let Some((col, asc)) = self.shell.sort_state() {
            let store = &self.store;
            let cmp = |a: &u32, b: &u32| -> std::cmp::Ordering {
                let a = &store[*a as usize];
                let b = &store[*b as usize];
                match col {
                    "first_seen" => a.first_seen.cmp(&b.first_seen),
                    "pid" => a.pid.cmp(&b.pid),
                    "process" => a.process_name.as_deref().unwrap_or("").cmp(b.process_name.as_deref().unwrap_or("")),
                    "local" => a.local_addr.cmp(&b.local_addr).then(a.local_port.cmp(&b.local_port)),
                    "remote" => a.remote_addr.cmp(&b.remote_addr).then(a.remote_port.cmp(&b.remote_port)),
                    "state" => a.state.as_str().cmp(b.state.as_str()),
                    "proto" => proto_rank(a.proto).cmp(&proto_rank(b.proto)),
                    "family" => fam_rank(a.family).cmp(&fam_rank(b.family)),
                    "path" => a.process_path.as_deref().unwrap_or("").cmp(b.process_path.as_deref().unwrap_or("")),
                    "cmdline" => a.process_cmdline.as_deref().unwrap_or("").cmp(b.process_cmdline.as_deref().unwrap_or("")),
                    "last_seen" => a.last_seen.cmp(&b.last_seen),
                    _ => std::cmp::Ordering::Equal,
                }
            };
            if asc {
                self.view.sort_by(cmp);
            } else {
                self.view.sort_by(|a, b| cmp(a, b).reverse());
            }
        }

        // 解析选中行的 store 下标（被过滤掉也能在详情面板显示——扫全量兜底）
        self.sel_store_idx = self.selected.as_ref().and_then(|k| {
            self.store
                .iter()
                .position(|r| k.matches(r))
                .map(|p| p as u32)
        });

        // 视图长度变化后收敛键盘焦点
        let len = self.view.len();
        if self.shell.focused.is_some_and(|f| f >= len) {
            self.shell.focused = None;
        }
        if self.shell.selected.is_some_and(|f| f >= len) {
            self.shell.selected = None;
        }

        self.view_dirty = false;
    }

    // ── 渲染 ───────────────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        self.render_toolbar(ui, ctx, rt);
        ui.separator();

        if let Some(ref err) = self.last_error {
            let err = err.clone();
            if banner::error_banner(ui, &err) {
                self.last_error = None;
            }
            ui.add_space(4.0);
        }

        self.rebuild_view();

        if self.view.is_empty() {
            self.render_empty(ui);
        } else {
            self.render_table(ui, ctx);
        }

        if self.kill_confirm_open {
            self.render_kill_confirm(ui, ctx, rt);
        }
    }

    fn render_empty(&mut self, ui: &mut egui::Ui) {
        ui.add_space(40.0);
        if !self.got_snapshot {
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new(t!("network.empty.waiting").to_string())
                        .font(crate::design::fonts::body())
                        .color(dtheme::palette().fg_secondary),
                );
            });
        } else {
            let pal = dtheme::palette();
            table_empty_state(ui, &pal);
        }
    }

    // ── 工具栏 ────────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let pal = dtheme::palette();

        ui.horizontal(|ui| {
            // 暂停 / 恢复（主按钮：accent 底 + 图标，对齐 React 版）
            let (pause_label, pause_icon) = if self.paused {
                (t!("network.toolbar.resume").to_string(), Icon::Play)
            } else {
                (t!("network.toolbar.pause").to_string(), Icon::Pause)
            };
            if widgets::flat_button(
                ui,
                Some(pause_icon),
                &pause_label,
                pal.accent,
                pal.accent_hover(),
                pal.on_accent,
                None,
                true,
            )
            .clicked()
            {
                let new_paused = !self.paused;
                self.set_paused(new_paused, ctx, rt);
            }

            // 协议下拉（对齐 React「全部协议」）
            let proto_labels = [
                t!("network.toolbar.proto-all").to_string(),
                "TCP".to_string(),
                "UDP".to_string(),
            ];
            let proto_idx = match self.proto_filter {
                ProtoFilter::All => 0,
                ProtoFilter::Tcp => 1,
                ProtoFilter::Udp => 2,
            };
            if let Some(i) = widgets::dropdown(
                ui,
                "network_dd_proto",
                0,
                &mut self.open_menu,
                &proto_labels,
                proto_idx,
                104.0,
                &pal,
            ) {
                self.proto_filter = match i {
                    1 => ProtoFilter::Tcp,
                    2 => ProtoFilter::Udp,
                    _ => ProtoFilter::All,
                };
                self.view_dirty = true;
            }

            // 状态多选下拉（保留多选语义）
            self.render_state_dropdown(ui, &pal);

            // 弹性搜索框（surface 底 + border 描边 + focus accent）
            {
                let v = &mut ui.style_mut().visuals;
                v.extreme_bg_color = pal.bg_elev1;
                v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, pal.border);
                v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, pal.border_strong);
                v.widgets.active.bg_stroke = egui::Stroke::new(1.0, pal.accent);
            }
            let placeholder = t!("network.toolbar.search-placeholder");
            let reserved = (ui.available_width() * 0.42).min(560.0);
            let search_w = (ui.available_width() - reserved).clamp(180.0, 300.0);
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(search_w)
                    .hint_text(placeholder.as_ref()),
            );
            if search_resp.changed() {
                self.view_dirty = true;
            }

            widgets::vsep(ui, &pal);

            // 终止进程（危险按钮，需选中行）
            let sel_pid = self
                .sel_store_idx
                .and_then(|i| self.store.get(i as usize))
                .map(|r| r.pid);
            if widgets::flat_button(
                ui,
                Some(Icon::X),
                &t!("network.toolbar.kill-process").to_string(),
                pal.danger.fg,
                pal.danger_hover(),
                pal.on_accent,
                None,
                sel_pid.is_some(),
            )
            .clicked()
            {
                if let Some(pid) = sel_pid {
                    self.request_kill(pid);
                }
            }

            // 导出 CSV（描边按钮 + 图标）
            if widgets::flat_button(
                ui,
                Some(Icon::Download),
                &t!("network.toolbar.export-csv").to_string(),
                pal.bg_elev1,
                pal.hover,
                pal.fg_primary,
                Some(pal.border),
                !self.store.is_empty(),
            )
            .clicked()
            {
                self.export_csv(ctx, rt);
            }

            // 清空历史（描边按钮 + 图标）
            if widgets::flat_button(
                ui,
                Some(Icon::Trash),
                &t!("network.toolbar.clear-history").to_string(),
                pal.bg_elev1,
                pal.hover,
                pal.fg_primary,
                Some(pal.border),
                true,
            )
            .clicked()
            {
                self.clear_local_history();
                let svc_ctx = ctx.clone();
                rt.spawn(async move {
                    let svc = NetworkService { ctx: &svc_ctx };
                    let _ = svc.clear_history().await;
                });
            }

            widgets::vsep(ui, &pal);

            // 显示历史（描边 toggle，开 = accent 文字）
            {
                let hist_label = t!("network.toolbar.show-history").to_string();
                let (rest, fg) = if self.show_history {
                    (pal.selected.bg, pal.accent)
                } else {
                    (pal.bg_elev1, pal.fg_primary)
                };
                if widgets::flat_button(ui, None, &hist_label, rest, pal.hover, fg, Some(pal.border), true)
                    .clicked()
                {
                    self.show_history = !self.show_history;
                    self.view_dirty = true;
                }
            }

            // 密度切换（28/34px，spec §3.3）
            // 密度切换（文字胶囊，点击循环紧凑/标准；React DataTable 同形态）
            let density_label = t!(self.shell.density.title_key()).to_string();
            let density_tip = t!("design.table.density").to_string();
            let density_resp = widgets::flat_button(ui, None, &density_label, pal.bg_elev1, pal.hover, pal.fg_primary, Some(pal.border), true);
            if density_resp.clicked() {
                self.shell.density = match self.shell.density {
                    crate::design::table::TableDensity::Compact => crate::design::table::TableDensity::Standard,
                    crate::design::table::TableDensity::Standard => crate::design::table::TableDensity::Compact,
                };
                self.shell.save_table_state(&ctx.app_dirs.config_dir());
            }
            density_resp.on_hover_text(density_tip);

            // 计数（右侧）
            let total = self.store.len();
            let history_count = self.store.iter().filter(|r| !r.is_current).count();
            let count_text = if history_count > 0 {
                t!(
                    "network.stats.count-history",
                    total = total.to_string(),
                    history = history_count.to_string()
                )
                .to_string()
            } else {
                t!("network.stats.count", total = total.to_string()).to_string()
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(count_text)
                        .font(crate::design::fonts::caption())
                        .color(pal.fg_tertiary),
                );
            });
        });
    }

    fn set_paused(&mut self, paused: bool, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        self.paused = paused;
        let svc_ctx = ctx.clone();
        let interval = self.interval_ms;
        rt.spawn(async move {
            let svc = NetworkService { ctx: &svc_ctx };
            let _ = svc
                .set_polling(NetworkPollingControl {
                    interval_ms: Some(interval),
                    paused: Some(paused),
                    retention: None,
                })
                .await;
        });
    }

    fn render_state_dropdown(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        let all_selected = ALL_STATES.iter().all(|s| self.state_filter.contains(s));
        let state_label = if all_selected {
            t!("network.toolbar.state-all").to_string()
        } else {
            format!(
                "{} ({})",
                t!("network.toolbar.state-label"),
                self.state_filter.len()
            )
        };

        let state_btn = ui.add(egui::Button::new(
            egui::RichText::new(state_label).color(pal.fg_primary),
        ));
        if state_btn.clicked() {
            self.state_dropdown_open = !self.state_dropdown_open;
        }
        let btn_rect = state_btn.rect;

        if self.state_dropdown_open {
            let popup_id = egui::Id::new("net_state_popup");
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
                                if ui.small_button(t!("network.toolbar.select-all").as_ref()).clicked() {
                                    self.state_filter = ALL_STATES.iter().copied().collect();
                                    self.view_dirty = true;
                                }
                                if ui.small_button(t!("network.toolbar.clear-all").as_ref()).clicked() {
                                    self.state_filter.clear();
                                    self.view_dirty = true;
                                }
                            });
                            ui.separator();

                            for state in ALL_STATES {
                                let mut checked = self.state_filter.contains(state);
                                let resp = ui.checkbox(&mut checked, state.as_str());
                                if resp.changed() {
                                    if checked {
                                        self.state_filter.insert(*state);
                                    } else {
                                        self.state_filter.remove(state);
                                    }
                                    self.view_dirty = true;
                                }
                            }
                        });
                    });
                });

            // 点击外部关闭
            if ui.input(|i| i.pointer.any_click()) && !response.response.hovered() && !state_btn.hovered() {
                self.state_dropdown_open = false;
            }
        }
    }

    // ── 表格 ──────────────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui, ctx: &AppContext) {
        let pal = dtheme::palette();
        let config_dir = ctx.app_dirs.config_dir();

        // 字段级解构：shell 可变借用与 store/view/selected 等不可变借用并存
        let NetworkPageState {
            store,
            view,
            shell,
            selected,
            detail_visible,
            sel_store_idx,
            view_dirty,
            kill_confirm_open,
            pending_kill_pid,
            ..
        } = self;

        let menu_action = Cell::new(MenuAction::None);

        let out = shell.show(
            ui,
            view.len(),
            |row, rctx| paint_row(row, rctx, store, view, selected.as_ref(), &pal),
            |menu_ui, vidx| build_context_menu(menu_ui, store, view, vidx, &menu_action),
        );

        // ── show 后统一处理交互 ──
        if let Some(vi) = out.clicked.or(out.activated) {
            let item = &store[view[vi] as usize];
            let already = selected.as_ref().is_some_and(|k| k.matches(item));
            if already {
                // 点击已选行 → 取消选中并收起详情（旧行为）
                *selected = None;
                *detail_visible = false;
                *sel_store_idx = None;
                shell.selected = None;
            } else {
                *selected = Some(ConnKey::of_row(item));
                *detail_visible = true;
                *sel_store_idx = Some(view[vi]);
            }
        }
        if out.secondary_clicked.is_some() {
            // 右键未选行时先选中并打开详情（旧行为）
            if let Some(vi) = out.secondary_clicked {
                let item = &store[view[vi] as usize];
                if !selected.as_ref().is_some_and(|k| k.matches(item)) {
                    *selected = Some(ConnKey::of_row(item));
                    *sel_store_idx = Some(view[vi]);
                    *detail_visible = true;
                }
            }
        }

        // 排序变化 → 视图重建；持久化字段变化 → 写盘
        if out.sort_changed {
            *view_dirty = true;
        }
        if out.persist_dirty || out.sort_changed {
            shell.save_table_state(&config_dir);
        }

        // 右键菜单动作（菜单闭包内只收集，此处执行）
        match menu_action.get() {
            MenuAction::None => {}
            MenuAction::CopyRow => {
                copy_row_text(store, selected.as_ref(), ui);
            }
            MenuAction::Kill(pid) => {
                *pending_kill_pid = Some(pid);
                *kill_confirm_open = true;
            }
        }
    }

    // ── 详情面板（app.rs 在 detail_visible 时调用）─────────────

    pub fn render_detail_panel(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let pal = dtheme::palette();
        let Some(row_idx) = self.resolve_selected_idx() else {
            self.detail_visible = false;
            self.selected = None;
            return;
        };
        // 克隆一条供渲染（面板内容少，且避免跨闭包借用冲突）
        let conn = self.store[row_idx as usize].clone();

        // 固定 header：PID + 徽章 + 关闭（不随内容滚动，关闭始终可见）
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(format!("PID {}", conn.pid))
                        .font(crate::design::fonts::body())
                        .strong()
                        .color(pal.fg_primary),
                );
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if !conn.is_current {
                        soft_badge(ui, t!("network.stats.history").as_ref(), pal.warning);
                        ui.label(egui::RichText::new("·").color(pal.fg_tertiary));
                    }
                    ui.label(
                        egui::RichText::new(conn.state.as_str())
                            .font(crate::design::fonts::caption())
                            .color(pal.fg_tertiary),
                    );
                    ui.label(egui::RichText::new("·").color(pal.fg_tertiary));
                    soft_badge(ui, &conn.d_proto, pal.info);
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let (xrect, xresp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
                let p = ui.painter();
                if xresp.hovered() {
                    p.rect_filled(xrect, 4.0, pal.hover);
                }
                crate::design::icon::draw(
                    ui,
                    crate::design::icon::Icon::X,
                    xrect.center(),
                    12.0,
                    if xresp.hovered() { pal.fg_primary } else { pal.fg_secondary },
                );
                if xresp.clicked() {
                    self.close_detail();
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("network_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);



                // 明细行（点击复制）
                let hint = t!("network.detail.click-to-copy");
                detail_row(ui, t!("network.detail.process").as_ref(), Some(&conn.d_process), false, &hint);
                detail_row(ui, t!("network.columns.state").as_ref(), Some(conn.state.as_str()), false, &hint);
                detail_row(ui, t!("network.columns.proto").as_ref(), Some(&conn.d_proto), false, &hint);
                detail_row(ui, t!("network.columns.local").as_ref(), Some(&conn.d_local), true, &hint);
                detail_row(ui, t!("network.columns.remote").as_ref(), Some(&conn.d_remote), true, &hint);
                detail_row(ui, t!("network.detail.first-seen").as_ref(), Some(&conn.d_first_seen), true, &hint);
                detail_row(ui, t!("network.detail.last-seen").as_ref(), Some(&conn.d_last_seen), true, &hint);
                detail_row(ui, t!("network.columns.path").as_ref(), conn.process_path.as_deref(), true, &hint);

                // 命令行段：状态徽章 + 刷新按钮 + 内容
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(t!("network.detail.command-line").to_string())
                            .font(crate::design::fonts::caption())
                            .color(pal.fg_tertiary),
                    );
                    cmdline_status_badge(ui, &pal, conn.cmdline_status);
                    if ui
                        .small_button(t!("network.detail.refresh-cmdline").as_ref())
                        .clicked()
                    {
                        let svc_ctx = ctx.clone();
                        let pid = conn.pid;
                        rt.spawn(async move {
                            let svc = NetworkService { ctx: &svc_ctx };
                            let _ = svc.refresh_cmdline(pid).await;
                        });
                    }
                });
                match conn.process_cmdline.as_deref() {
                    Some(cmd) if !cmd.is_empty() => {
                        ui.label(
                            egui::RichText::new(cmd)
                                .font(crate::design::fonts::mono_caption())
                                .color(pal.fg_secondary),
                        );
                    }
                    _ => {
                        let pending = t!("network.detail.command-line-pending");
                        ui.label(
                            egui::RichText::new(pending.as_ref())
                                .font(crate::design::fonts::mono_caption())
                                .color(pal.fg_tertiary),
                        );
                    }
                }

                // 动作区
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new(t!("network.context-menu.kill").as_ref())
                                .color(pal.danger.fg),
                        ))
                        .clicked()
                    {
                        self.request_kill(conn.pid);
                    }
                    if ui.button(t!("network.detail.refresh-cmdline").as_ref()).clicked() {
                        let svc_ctx = ctx.clone();
                        let pid = conn.pid;
                        rt.spawn(async move {
                            let svc = NetworkService { ctx: &svc_ctx };
                            let _ = svc.refresh_cmdline(pid).await;
                        });
                    }
                });
            });
    }

    /// 选中行的 store 下标（缓存失效则全量扫描兜底）。
    fn resolve_selected_idx(&self) -> Option<u32> {
        let key = self.selected.as_ref()?;
        if let Some(i) = self.sel_store_idx {
            if let Some(r) = self.store.get(i as usize) {
                if key.matches(r) {
                    return Some(i);
                }
            }
        }
        self.store.iter().position(|r| key.matches(r)).map(|p| p as u32)
    }

    fn close_detail(&mut self) {
        self.detail_visible = false;
        self.selected = None;
        self.sel_store_idx = None;
        self.shell.selected = None;
    }

    fn request_kill(&mut self, pid: u32) {
        self.pending_kill_pid = Some(pid);
        self.kill_confirm_open = true;
    }

    // ── 终止进程确认框 ────────────────────────────────────────

    fn render_kill_confirm(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.kill_confirm_open;
        let mut confirmed = false;
        let mut cancelled = false;

        let title = t!("network.kill-confirm.title");
        let message = t!(
            "network.kill-confirm.message",
            pid = self.pending_kill_pid.map(|p| p.to_string()).unwrap_or_default(),
            name = "-".to_string()
        );
        let confirm = t!("network.kill-confirm.confirm");
        let cancel = t!("network.kill-confirm.cancel");
        let pal = dtheme::palette();

        egui::Window::new(title.as_ref())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.add_space(4.0);
                ui.label(message.as_ref());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new(confirm.as_ref()).color(pal.danger.fg),
                        ))
                        .clicked()
                    {
                        confirmed = true;
                    }
                    if ui.button(cancel.as_ref()).clicked() {
                        cancelled = true;
                    }
                });
            });

        if confirmed {
            if let Some(pid) = self.pending_kill_pid {
                let svc_ctx = ctx.clone();
                rt.spawn(async move {
                    let svc = NetworkService { ctx: &svc_ctx };
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

    // ── CSV 导出（后台线程写盘，列集沿用旧版）──────────────────

    fn export_csv(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let items: Vec<NetRow> = self.store.iter().cloned().collect();
        if items.is_empty() {
            return;
        }
        let dir = ctx.app_dirs.root().to_path_buf();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("network_export_{}.csv", secs));
        rt.spawn_blocking(move || match write_csv(&path, &items) {
            Ok(()) => tracing::info!("exported {} connections to {}", items.len(), path.display()),
            Err(e) => tracing::error!("csv export failed: {}", e),
        });
    }
}

// ── 表格行绘制（零分配热路径）──────────────────────────────────

/// 行背景优先级：选中 ＞ risk ＞ 键盘焦点。
fn paint_row(
    row: &mut egui_extras::TableRow<'_, '_>,
    rctx: RowCtx,
    store: &VecDeque<NetRow>,
    view: &[u32],
    selected: Option<&ConnKey>,
    pal: &Palette,
) {
    let item = &store[view[rctx.index] as usize];
    let is_sel = selected.is_some_and(|k| k.matches(item));
    let history = !item.is_current;

    let fg_main = if history { pal.fg_tertiary } else { pal.fg_primary };
    let fg_sec = if history { pal.fg_tertiary } else { pal.fg_secondary };

    let bg = if is_sel {
        Some(pal.selected.bg)
    } else {
        risk_bg(item.risk, pal)
    };
    let focus_bg = if rctx.focused && !is_sel { Some(pal.hover) } else { None };
    let fill = bg.or(focus_bg);

    macro_rules! cell {
        ($row:expr, $body:expr) => {{
            $row.col(|ui| {
                if let Some(c) = fill {
                    table::paint_row_bg(ui, c);
                }
                table::cell_pad(ui);
                $body(ui)
            });
        }};
    }

    // 1 first_seen / 11 last_seen（时间戳 mono，spec §3.1）
    cell!(row, |ui: &mut egui::Ui| table::cell_mono_label(ui, &item.d_first_seen, fg_sec));
    // 2 pid
    cell!(row, |ui: &mut egui::Ui| table::cell_mono_label(ui, &item.d_pid, fg_main));
    // 3 process
    cell!(row, |ui: &mut egui::Ui| table::cell_label(ui, &item.d_process, fg_main));
    // 4 local / 5 remote
    cell!(row, |ui: &mut egui::Ui| table::cell_mono_label(ui, &item.d_local, fg_main));
    cell!(row, |ui: &mut egui::Ui| table::cell_mono_label(ui, &item.d_remote, fg_main));
    // 6 state：无状态 "-"、历史灰字、当前软色徽章（spec §2.2 role 映射）
    cell!(row, |ui: &mut egui::Ui| {
        if item.state == ConnState::None || item.state.as_str().is_empty() {
            table::cell_label(ui, "-", pal.fg_tertiary);
        } else if history {
            table::cell_label(ui, item.state.as_str(), pal.fg_tertiary);
        } else {
            let roles = state_roles(&item.state, pal);
            crate::design::widgets::badge(ui, item.state.as_str(), roles.fg, roles.bg, roles.border);
        }
    });
    // 7 proto / 8 family
    cell!(row, |ui: &mut egui::Ui| table::cell_label(ui, &item.d_proto, fg_main));
    cell!(row, |ui: &mut egui::Ui| table::cell_label(ui, &item.d_family, fg_main));
    // 9 path（截断 + 悬停全文）
    {
        let (_, r) = row.col(|ui| {
            if let Some(c) = fill {
                table::paint_row_bg(ui, c);
            }
            table::cell_pad(ui);
            table::cell_mono_label(ui, item.process_path.as_deref().unwrap_or(""), fg_sec);
        });
        if let Some(path) = item.process_path.as_deref() {
            table::row_hover(r, path);
        }
    }
    // 10 cmdline（状态图标 + 内容，截断 + 悬停全文）
    {
        let cmd = item.process_cmdline.as_deref().unwrap_or("");
        let (_, r) = row.col(|ui| {
            if let Some(c) = fill {
                table::paint_row_bg(ui, c);
            }
            table::cell_pad(ui);
            if cmd.is_empty() {
                if item.cmdline_status != CmdlineStatus::Unknown && !history {
                    cmdline_status_icon(ui, item.cmdline_status, pal);
                }
                table::cell_label(ui, "-", pal.fg_tertiary);
            } else {
                if !history {
                    cmdline_status_icon(ui, item.cmdline_status, pal);
                }
                table::cell_mono_label(ui, cmd, fg_sec);
            }
        });
        table::row_hover(r, cmd);
    }
    // 11 last_seen
    cell!(row, |ui: &mut egui::Ui| table::cell_mono_label(ui, &item.d_last_seen, fg_sec));
}

/// 状态徽章的 role 映射（spec §2.2：ESTABLISHED→success / LISTEN→info /
/// TIME_WAIT→warning / CLOSE_WAIT→danger / 其余 neutral）。
fn state_roles(state: &ConnState, pal: &Palette) -> crate::design::tokens::RoleColors {
    match state {
        ConnState::Established => pal.success,
        ConnState::Listen => pal.info,
        ConnState::TimeWait => pal.warning,
        ConnState::CloseWait => pal.danger,
        _ => pal.neutral,
    }
}

/// 命令行获取状态的行内图标（React CmdlineStatusIcon 对齐）：字符 + role 色。
/// 命令行采集状态图标（lucide 纹理；Unicode ⟳⊘✗ 在雅黑缺字形渲染为 tofu）。
fn cmdline_status_icon(ui: &mut egui::Ui, status: CmdlineStatus, pal: &Palette) {
    use crate::design::icon::Icon;
    let (icon, col) = match status {
        CmdlineStatus::Pending => (Icon::Refresh, pal.info.fg),
        CmdlineStatus::Ready => (Icon::Check, pal.success.fg),
        CmdlineStatus::Denied => (Icon::X, pal.warning.fg),
        CmdlineStatus::Exited => (Icon::X, pal.dim.fg),
        CmdlineStatus::Failed => (Icon::X, pal.danger.fg),
        CmdlineStatus::Unknown => return,
    };
    let c = ui.cursor();
    crate::design::icon::draw(ui, icon, egui::Pos2::new(c.left() + 6.0, c.center().y), 11.0, col);
    ui.add_space(13.0);
}

// ── 右键菜单（egui PopupMenu，菜单项对齐 React 版）─────────────

fn build_context_menu(
    ui: &mut egui::Ui,
    store: &VecDeque<NetRow>,
    view: &[u32],
    vidx: usize,
    action: &Cell<MenuAction>,
) {
    let item = &store[view[vidx] as usize];
    ui.set_min_width(180.0);
    ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);

    let copy_label = t!("network.context-menu.copy-row");
    if ui.button(copy_label.as_ref()).clicked() {
        action.set(MenuAction::CopyRow);
        ui.close();
    }

    let kill_label = t!("network.context-menu.kill");
    if ui.button(kill_label.as_ref()).clicked() {
        action.set(MenuAction::Kill(item.pid));
        ui.close();
    }

    ui.separator();

    let ws_label = t!("network.context-menu.search-workspace");
    ui.add_enabled(false, egui::Button::new(ws_label.as_ref()));
}

/// 「复制行」动作：以当前选中行拼接整行概要（对齐 React 版格式）。
fn copy_row_text(store: &VecDeque<NetRow>, selected: Option<&ConnKey>, ui: &egui::Ui) {
    let Some(key) = selected else { return };
    let Some(item) = store.iter().find(|r| key.matches(r)) else { return };
    let text = format!(
        "{} {}:{} -> {}:{} pid={} {}",
        item.d_proto,
        item.local_addr,
        item.local_port,
        item.remote_addr,
        item.remote_port,
        item.pid,
        item.process_name.as_deref().unwrap_or(""),
    );
    ui.ctx().copy_text(text);
}

// ── 小组件 ────────────────────────────────────────────────────

/// 软色徽章（页面侧薄封装：role 三件套直接展开）。
fn soft_badge(ui: &mut egui::Ui, text: &str, role: crate::design::tokens::RoleColors) {
    crate::design::widgets::badge(ui, text, role.fg, role.bg, role.border);
}

/// 命令行获取状态徽章（详情面板；标签 i18n，颜色走 role）。
fn cmdline_status_badge(ui: &mut egui::Ui, pal: &Palette, status: CmdlineStatus) {
    let (key, role) = match status {
        CmdlineStatus::Pending => ("network.detail.cmdline-status.pending", pal.info),
        CmdlineStatus::Ready => ("network.detail.cmdline-status.ready", pal.success),
        CmdlineStatus::Denied => ("network.detail.cmdline-status.denied", pal.warning),
        CmdlineStatus::Exited => ("network.detail.cmdline-status.exited", pal.dim),
        CmdlineStatus::Failed => ("network.detail.cmdline-status.failed", pal.danger),
        CmdlineStatus::Unknown => ("network.detail.cmdline-status.unknown", pal.neutral),
    };
    let label = t!(key);
    crate::design::widgets::badge(ui, label.as_ref(), role.fg, role.bg, role.border);
}

/// 空态（有快照但过滤后为空）。
fn table_empty_state(ui: &mut egui::Ui, pal: &Palette) {
    let title = t!("network.empty.no-match");
    let hint = t!("network.empty.no-match-hint");
    crate::design::widgets::empty_state(ui, pal, "⇄", title.as_ref(), hint.as_ref());
}

/// 明细行：label（caption/fg-tertiary）+ 值（可点击复制，悬停提示）。
fn detail_row(ui: &mut egui::Ui, label: &str, value: Option<&str>, mono: bool, hint: &str) {
    let Some(value) = value else { return };
    if value.is_empty() {
        return;
    }
    let pal = dtheme::palette();
    ui.label(
        egui::RichText::new(label)
            .font(crate::design::fonts::caption())
            .color(pal.fg_tertiary),
    );
    let text = if mono {
        egui::RichText::new(value)
            .font(crate::design::fonts::mono_caption())
            .color(pal.fg_primary)
    } else {
        egui::RichText::new(value).color(pal.fg_primary)
    };
    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
    if resp.clicked() {
        ui.ctx().copy_text(value.to_string());
    }
    if resp.hovered() {
        resp.on_hover_text(hint.to_string());
    }
    ui.add_space(2.0);
}

fn proto_rank(p: Proto) -> u8 {
    match p {
        Proto::Tcp => 0,
        Proto::Udp => 1,
    }
}

fn fam_rank(f: Family) -> u8 {
    match f {
        Family::V4 => 0,
        Family::V6 => 1,
    }
}

fn write_csv(path: &std::path::Path, items: &[NetRow]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "PID,Process,LocalAddr,RemoteAddr,State,Proto,Family,Path,Cmdline")?;
    for c in items {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{}",
            c.pid,
            csv_escape(c.process_name.as_deref().unwrap_or("")),
            csv_escape(&format!("{}:{}", c.local_addr, c.local_port)),
            csv_escape(&format!("{}:{}", c.remote_addr, c.remote_port)),
            csv_escape(c.state.as_str()),
            csv_escape(&c.d_proto),
            csv_escape(&c.d_family),
            csv_escape(c.process_path.as_deref().unwrap_or("")),
            csv_escape(c.process_cmdline.as_deref().unwrap_or("")),
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
