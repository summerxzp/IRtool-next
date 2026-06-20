use std::collections::HashSet;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_service::context::AppContext;
use irtool_service::dto::network::{NetworkEnrichmentPayload, NetworkPollingControl, NetworkSnapshotPayload};
use irtool_service::services::network::NetworkService;
use irtool_service::types::{ConnState, NetConn, Proto};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;
use crate::widgets::table::{self, SortDir};

// ── Sort Column ────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    FirstSeen,
    Pid,
    Process,
    Local,
    Remote,
    State,
    Proto,
    Family,
    Path,
    Cmdline,
    LastSeen,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProtoFilter {
    All,
    Tcp,
    Udp,
}

/// All ConnState variants used for multi-select filter.
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

/// Parse "addr:port" into (addr, port). Uses rfind(':') to handle IPv6 addresses like "[::1]:8080".
fn parse_endpoint(s: &str) -> Option<(&str, u16)> {
    let idx = s.rfind(':')?;
    let addr = &s[..idx];
    let port = s[idx + 1..].parse().ok()?;
    Some((addr, port))
}

// ── NetworkPageState ───────────────────────────────────────

pub struct NetworkPageState {
    // Data
    pub snapshot: Option<NetworkSnapshotPayload>,
    pub last_error: Option<String>,

    // Filters
    pub search: String,
    pub proto_filter: ProtoFilter,
    pub state_filter: HashSet<ConnState>,
    pub state_dropdown_open: bool,

    // Table
    pub sort_column: SortColumn,
    pub sort_dir: SortDir,
    pub selected_pid: Option<u32>,
    pub selected_local: Option<String>,
    pub selected_remote: Option<String>,
    pub detail_visible: bool,

    // Context menu
    pub ctx_menu_visible: bool,
    pub ctx_menu_pos: Option<egui::Pos2>,
    /// 标记菜单刚在本帧打开，跳过点击外部关闭检查。
    pub ctx_menu_just_opened: bool,

    // Polling
    pub paused: bool,
    pub interval_ms: u64,

    // History
    pub show_history: bool,

    // B2: filtered+sorted cache
    cached_items: Vec<NetConn>,
    cache_dirty: bool,

    // B3: cached selected connection
    selected_conn: Option<NetConn>,

    // Kill confirmation dialog (DESIGN.md 4.10)
    kill_confirm_open: bool,
    pending_kill_pid: Option<u32>,
}

impl Default for NetworkPageState {
    fn default() -> Self {
        Self {
            snapshot: None,
            last_error: None,
            search: String::new(),
            proto_filter: ProtoFilter::All,
            state_filter: ALL_STATES.iter().copied().collect(),
            state_dropdown_open: false,
            sort_column: SortColumn::FirstSeen,
            sort_dir: SortDir::Desc,
            selected_pid: None,
            selected_local: None,
            selected_remote: None,
            detail_visible: false,
            ctx_menu_visible: false,
            ctx_menu_pos: None,
            ctx_menu_just_opened: false,
            paused: false,
            interval_ms: 1000,
            show_history: true,
            cached_items: Vec::new(),
            cache_dirty: true,
            selected_conn: None,
            kill_confirm_open: false,
            pending_kill_pid: None,
        }
    }
}

impl NetworkPageState {
    // ── Event handling ─────────────────────────────────────

    pub fn handle_snapshot(&mut self, payload: NetworkSnapshotPayload) {
        // B3: refresh selected_conn cache from new snapshot
        if let (Some(pid), Some(ref local), Some(ref remote)) =
            (self.selected_pid, &self.selected_local, &self.selected_remote)
        {
            let local_ep = parse_endpoint(local);
            let remote_ep = parse_endpoint(remote);
            self.selected_conn = payload
                .items
                .iter()
                .find(|c| {
                    c.pid == pid
                        && local_ep.is_some_and(|(addr, port)| c.local.addr == addr && c.local.port == port)
                        && remote_ep.is_some_and(|(addr, port)| c.remote.addr == addr && c.remote.port == port)
                })
                .cloned();
        }
        self.snapshot = Some(payload);
        self.last_error = None;
        self.cache_dirty = true; // B2
    }

    pub fn handle_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    pub fn handle_enrichment(&mut self, enrichment: NetworkEnrichmentPayload) {
        if let Some(ref mut snap) = self.snapshot {
            for conn in &mut snap.items {
                if conn.pid == enrichment.pid {
                    conn.cmdline_status = enrichment.cmdline_status;
                    if let Some(ref cmdline) = enrichment.process_cmdline {
                        conn.process_cmdline = Some(cmdline.clone());
                    }
                }
            }
        }
        // B3: refresh selected_conn cache if it matches the enriched pid
        if let Some(ref sel) = self.selected_conn {
            if sel.pid == enrichment.pid {
                if let (Some(ref snap), Some(ref local), Some(ref remote)) =
                    (&self.snapshot, &self.selected_local, &self.selected_remote)
                {
                    let local_ep = parse_endpoint(local);
                    let remote_ep = parse_endpoint(remote);
                    self.selected_conn = snap
                        .items
                        .iter()
                        .find(|c| {
                            c.pid == enrichment.pid
                                && local_ep.is_some_and(|(addr, port)| c.local.addr == addr && c.local.port == port)
                                && remote_ep.is_some_and(|(addr, port)| c.remote.addr == addr && c.remote.port == port)
                        })
                        .cloned();
                }
            }
        }
        self.cache_dirty = true; // B2
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

        // Table
        self.render_table(ui);

        // Context menu overlay (using stored position)
        if self.ctx_menu_visible {
            self.render_context_menu(ui, ctx, rt);
        }

        // Kill confirmation dialog (DESIGN.md 4.10)
        if self.kill_confirm_open {
            self.render_kill_confirm(ui, ctx, rt);
        }
    }

    // ── Toolbar ────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // Polling status — clickable to toggle pause/resume
            let polling_status = if self.paused { "‖ 已暂停" } else { "▶ 轮询中" };
            let polling_color = if self.paused {
                theme::SEMANTIC_WARNING
            } else {
                theme::SEMANTIC_SUCCESS
            };
            let polling_resp = ui.add(
                egui::Label::new(egui::RichText::new(polling_status).size(11.0).color(polling_color))
                    .sense(egui::Sense::click()),
            );
            if polling_resp.clicked() {
                self.paused = !self.paused;
                let svc_ctx = ctx.clone();
                let paused = self.paused;
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

            ui.separator();

            // Search box
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(200.0)
                    .hint_text("PID / IP / 端口 / 进程名"),
            );
            if search_resp.changed() {
                self.cache_dirty = true; // B2
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true; // B2
            }

            ui.separator();

            // Protocol filter
            if filter_button(ui, "全部", self.proto_filter == ProtoFilter::All) {
                self.proto_filter = ProtoFilter::All;
                self.cache_dirty = true; // B2
            }
            if filter_button(ui, "TCP", self.proto_filter == ProtoFilter::Tcp) {
                self.proto_filter = ProtoFilter::Tcp;
                self.cache_dirty = true; // B2
            }
            if filter_button(ui, "UDP", self.proto_filter == ProtoFilter::Udp) {
                self.proto_filter = ProtoFilter::Udp;
                self.cache_dirty = true; // B2
            }

            ui.separator();

            // ── State multi-select dropdown ──
            let all_selected = ALL_STATES.iter().all(|s| self.state_filter.contains(s));
            let state_label = if all_selected {
                "状态: 全部".to_string()
            } else {
                format!("状态: {}", self.state_filter.len())
            };

            let state_btn = ui.add(egui::Button::new(
                egui::RichText::new(&state_label).color(theme::FG_PRIMARY),
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
                                // Select All / Deselect All
                                ui.horizontal(|ui| {
                                    if ui.small_button("全选").clicked() {
                                        self.state_filter = ALL_STATES.iter().copied().collect();
                                        self.cache_dirty = true; // B2
                                    }
                                    if ui.small_button("取消全选").clicked() {
                                        self.state_filter.clear();
                                        self.cache_dirty = true; // B2
                                    }
                                });
                                ui.separator();

                                // Per-state checkboxes
                                for state in ALL_STATES {
                                    let mut checked = self.state_filter.contains(state);
                                    let text = state.as_str();
                                    let resp = ui.checkbox(&mut checked, text);
                                    if resp.changed() {
                                        if checked {
                                            self.state_filter.insert(*state);
                                        } else {
                                            self.state_filter.remove(state);
                                        }
                                        self.cache_dirty = true; // B2
                                    }
                                }
                            });
                        })
                    });

                // Close when clicking outside
                if ui.input(|i| i.pointer.any_click()) && !response.response.hovered() && !state_btn.hovered() {
                    self.state_dropdown_open = false;
                }
            }

            ui.separator();

            // Pause / Resume
            let pause_label = if self.paused { "▶ 恢复" } else { "‖ 暂停" };
            let pause_color = if self.paused {
                theme::SEMANTIC_SUCCESS
            } else {
                theme::FG_SECONDARY
            };
            if ui
                .add(egui::Button::new(egui::RichText::new(pause_label).color(pause_color)))
                .clicked()
            {
                self.paused = !self.paused;
                let svc_ctx = ctx.clone();
                let paused = self.paused;
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

            ui.separator();

            // Clear history
            if ui.button("清空历史").clicked() {
                let svc_ctx = ctx.clone();
                rt.spawn(async move {
                    let svc = NetworkService { ctx: &svc_ctx };
                    let _ = svc.clear_history().await;
                });
            }

            // History toggle
            let hist_label = if self.show_history {
                "历史: 开"
            } else {
                "历史: 关"
            };
            let hist_color = if self.show_history {
                theme::ACCENT
            } else {
                theme::FG_TERTIARY
            };
            if ui
                .add(egui::Button::new(egui::RichText::new(hist_label).color(hist_color)))
                .clicked()
            {
                self.show_history = !self.show_history;
                self.cache_dirty = true; // B2
            }

            // Export CSV
            if ui
                .add_enabled(self.snapshot.is_some(), egui::Button::new("导出 CSV"))
                .clicked()
            {
                self.export_csv(ctx, rt);
            }

            // Snapshot count (includes history count)
            if let Some(ref snap) = self.snapshot {
                let total = snap.items.len();
                let history_count = snap.items.iter().filter(|c| !c.is_current).count();
                let count_text = if history_count > 0 {
                    format!("{} 连接 ([{}] 历史)", total, history_count)
                } else {
                    format!("{} 连接", total)
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui_label(ui, egui::RichText::new(count_text).size(12.0).color(theme::FG_TERTIARY));
                });
            }
        });
    }

    // ── Table ──────────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui) {
        // B2: extract fields before borrowing self via get_filtered_sorted_items
        let sc = self.sort_column;
        let sd = self.sort_dir;
        let snapshot_is_none = self.snapshot.is_none();
        let sel_pid = self.selected_pid;
        let sel_local = self.selected_local.clone();
        let sel_remote = self.selected_remote.clone();
        let sel_local_ep = sel_local.as_deref().and_then(parse_endpoint);
        let sel_remote_ep = sel_remote.as_deref().and_then(parse_endpoint);

        let items = self.get_filtered_sorted_items();

        if items.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                if snapshot_is_none {
                    ui.spinner();
                    ui_label(ui, egui::RichText::new("等待网络数据...").color(theme::FG_SECONDARY));
                } else {
                    ui_label(
                        ui,
                        egui::RichText::new("没有匹配当前过滤条件的连接").color(theme::FG_TERTIARY),
                    );
                }
            });
            return;
        }

        let mut sort_toggle: Option<SortColumn> = None;
        let mut row_click: Option<(u32, String, String)> = None;
        let mut row_right_click: Option<(u32, String, String)> = None;
        // Capture the right-click position locally to avoid the menu
        // following the cursor (interact_pos() changes every frame).
        let mut capture_ctx_pos: Option<egui::Pos2> = None;

        // Capture Context before TableBuilder borrows ui mutably
        let egui_ctx = ui.ctx().clone();
        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(140.0).clip(true)) // First Seen
            .column(Column::initial(55.0).clip(true)) // PID
            .column(Column::initial(140.0).clip(true)) // Process
            .column(Column::initial(180.0).clip(true)) // Local
            .column(Column::initial(180.0).clip(true)) // Remote
            .column(Column::initial(100.0).clip(true)) // State
            .column(Column::initial(50.0).clip(true)) // Proto
            .column(Column::initial(45.0).clip(true)) // Fam
            .column(Column::initial(200.0).clip(true)) // Path
            .column(Column::initial(160.0).clip(true)) // Cmdline
            .column(Column::initial(140.0).clip(true)); // Last Seen

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    if table::sortable_header(ui, "首次出现", sc == SortColumn::FirstSeen, sd) {
                        sort_toggle = Some(SortColumn::FirstSeen);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "PID", sc == SortColumn::Pid, sd) {
                        sort_toggle = Some(SortColumn::Pid);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "进程名", sc == SortColumn::Process, sd) {
                        sort_toggle = Some(SortColumn::Process);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "本地地址", sc == SortColumn::Local, sd) {
                        sort_toggle = Some(SortColumn::Local);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "远程地址", sc == SortColumn::Remote, sd) {
                        sort_toggle = Some(SortColumn::Remote);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "状态", sc == SortColumn::State, sd) {
                        sort_toggle = Some(SortColumn::State);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "协议", sc == SortColumn::Proto, sd) {
                        sort_toggle = Some(SortColumn::Proto);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "族", sc == SortColumn::Family, sd) {
                        sort_toggle = Some(SortColumn::Family);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "路径", sc == SortColumn::Path, sd) {
                        sort_toggle = Some(SortColumn::Path);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "命令行", sc == SortColumn::Cmdline, sd) {
                        sort_toggle = Some(SortColumn::Cmdline);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "最近出现", sc == SortColumn::LastSeen, sd) {
                        sort_toggle = Some(SortColumn::LastSeen);
                    }
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let conn = &items[row.index()];
                    // B2: use extracted fields to avoid borrowing self
                    let is_selected = (sel_pid == Some(conn.pid))
                        && sel_local_ep.is_none_or(|(addr, port)| conn.local.addr == addr && conn.local.port == port)
                        && sel_remote_ep
                            .is_none_or(|(addr, port)| conn.remote.addr == addr && conn.remote.port == port);
                    let is_history = !conn.is_current;
                    let local_ep = format!("{}:{}", conn.local.addr, conn.local.port);
                    let remote_ep = format!("{}:{}", conn.remote.addr, conn.remote.port);

                    // ── Render cells with history gray ──
                    let _ = cell_first_seen(&mut row, conn, is_history);
                    let _ = cell_pid(&mut row, conn, is_history);
                    let _ = cell_process(&mut row, conn, is_history);
                    let _ = cell_local(&mut row, conn, is_history);
                    let _ = cell_remote(&mut row, conn, is_history);
                    let _ = cell_state(&mut row, conn, is_history);
                    let _ = cell_proto(&mut row, conn, is_history);
                    let _ = cell_family(&mut row, conn, is_history);
                    let _ = cell_path(&mut row, conn, is_history);
                    let _ = cell_cmdline(&mut row, conn, is_history);
                    let _ = cell_last_seen(&mut row, conn, is_history);

                    // ── Row interaction ──
                    let row_resp = row.response();

                    // Selected highlight
                    if is_selected {
                        row.set_selected(true);
                    }

                    // Left click → select/deselect
                    if row_resp.clicked() {
                        if is_selected {
                            row_click = Some((0, String::new(), String::new()));
                        } else {
                            row_click = Some((conn.pid, local_ep.clone(), remote_ep.clone()));
                        }
                    }

                    // Right click → context menu (capture position at click time)
                    if row_resp.secondary_clicked() {
                        // Pre-select the row if not already selected
                        if !is_selected {
                            row_click = Some((conn.pid, local_ep.clone(), remote_ep.clone()));
                        }
                        row_right_click = Some((conn.pid, local_ep, remote_ep));

                        // Capture the click position NOW, when the click happens
                        if let Some(pos) = egui_ctx.input(|i| i.pointer.interact_pos()) {
                            capture_ctx_pos = Some(pos);
                        }
                    }
                });
            });

        // Store captured position into state (after body closure)
        if let Some(pos) = capture_ctx_pos {
            self.ctx_menu_pos = Some(pos);
        }

        // Apply interactions after table render
        if let Some((pid, local, remote)) = row_click {
            if pid == 0 {
                self.selected_pid = None;
                self.selected_local = None;
                self.selected_remote = None;
                self.selected_conn = None; // B3
                self.detail_visible = false;
            } else {
                // B3: cache the selected conn from cached_items
                self.selected_conn = self.cached_items.iter().find(|c| c.pid == pid).cloned();
                self.selected_pid = Some(pid);
                self.selected_local = Some(local);
                self.selected_remote = Some(remote);
                self.detail_visible = true;
            }
        }
        if row_right_click.is_some() {
            self.ctx_menu_visible = true;
            self.ctx_menu_just_opened = true;
        }

        if let Some(col) = sort_toggle {
            self.toggle_sort(col);
        }
    }

    // ── Context Menu ───────────────────────────────────────

    fn render_context_menu(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // Use the stored position captured at right-click time
        let Some(pos) = self.ctx_menu_pos else {
            // If no stored position, fall back to current pointer (shouldn't normally happen)
            self.ctx_menu_visible = false;
            return;
        };

        // B3: clone selected conn to avoid borrowing self inside the closure
        let conn = self.find_selected_conn().cloned();

        let menu_id = egui::Id::new("net_ctx_menu");
        let mut should_close = false;

        let _response = egui::Area::new(menu_id)
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(160.0);
                    ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);

                    if ui.button("复制详情").clicked() {
                        if let Some(ref conn) = conn {
                            let text = format!(
                                "PID: {}\nProcess: {}\nLocal: {}:{}\nRemote: {}:{}\nState: {}\nProto: {:?}",
                                conn.pid,
                                conn.process_name.as_deref().unwrap_or("-"),
                                conn.local.addr,
                                conn.local.port,
                                conn.remote.addr,
                                conn.remote.port,
                                conn.state.as_str(),
                                conn.proto,
                            );
                            ui.ctx().copy_text(text.clone());
                        }
                        should_close = true;
                    }

                    if ui.button("复制 IP:Port").clicked() {
                        if let Some(ref conn) = conn {
                            let text = format!("{}:{}", conn.remote.addr, conn.remote.port);
                            ui.ctx().copy_text(text.clone());
                        }
                        should_close = true;
                    }

                    ui.separator();

                    if ui.button("刷新命令行").clicked() {
                        if let Some(ref conn) = conn {
                            let pid = conn.pid;
                            let svc_ctx = ctx.clone();
                            rt.spawn(async move {
                                let svc = NetworkService { ctx: &svc_ctx };
                                let _ = svc.refresh_cmdline(pid).await;
                            });
                        }
                        should_close = true;
                    }

                    ui.separator();

                    let kill_btn = ui.add(egui::Button::new(
                        egui::RichText::new("终止进程").color(theme::SEMANTIC_DANGER),
                    ));
                    if kill_btn.clicked() {
                        if let Some(ref conn) = conn {
                            self.pending_kill_pid = Some(conn.pid);
                            self.kill_confirm_open = true;
                        }
                        should_close = true;
                    }
                })
            });

        // Close when clicking outside or after action.
        // Skip close check on the frame the menu was just opened, because
        // the right-click that opened the menu also triggers any_click().
        if self.ctx_menu_just_opened {
            self.ctx_menu_just_opened = false;
        } else if should_close {
            self.ctx_menu_visible = false;
            self.ctx_menu_pos = None;
        } else if ui.input(|i| i.pointer.any_click()) {
            if let Some(rect) = ui.ctx().memory(|m| m.area_rect(egui::Id::new("net_ctx_menu"))) {
                if !rect.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or(pos)) {
                    self.ctx_menu_visible = false;
                    self.ctx_menu_pos = None;
                }
            }
        }
    }

    // ── Detail Panel ───────────────────────────────────────

    pub fn render_detail_panel(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // B3: clone selected conn to avoid borrowing self for subsequent mutations
        let conn = self.find_selected_conn().cloned();
        let Some(conn) = conn else {
            self.detail_visible = false;
            return;
        };

        egui::ScrollArea::vertical()
            .id_salt("network_detail_scroll")
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
                            self.selected_pid = None;
                            self.selected_local = None;
                            self.selected_remote = None;
                            self.selected_conn = None;
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(format!("PID {}", conn.pid))
                                    .color(theme::FG_PRIMARY)
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                if !conn.is_current {
                                    badge::badge(ui, "历史", BadgeVariant::Warning);
                                    ui.label(egui::RichText::new("·").color(theme::FG_TERTIARY));
                                }
                                ui.label(
                                    egui::RichText::new(conn.state.as_str())
                                        .color(theme::FG_TERTIARY)
                                        .size(11.0),
                                );
                                ui.label(egui::RichText::new("·").color(theme::FG_TERTIARY));
                                ui.label(
                                    egui::RichText::new(format!("{:?}", conn.proto).to_uppercase())
                                        .color(theme::FG_TERTIARY)
                                        .size(11.0),
                                );
                            });
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Detail rows with click-to-copy (DESIGN.md 4.4)
                detail_row(ui, "进程", conn.process_name.as_deref(), false);
                detail_row(ui, "状态", Some(conn.state.as_str()), false);
                detail_row(ui, "协议", Some(&format!("{:?}", conn.proto)), false);
                detail_row(
                    ui,
                    "本地地址",
                    Some(&format_endpoint(&conn.local.addr, conn.local.port)),
                    true,
                );
                detail_row(
                    ui,
                    "远程地址",
                    Some(&format_endpoint(&conn.remote.addr, conn.remote.port)),
                    true,
                );
                detail_row(ui, "首次出现", Some(&theme::fmt_time(conn.first_seen)), true);
                detail_row(ui, "最近出现", Some(&theme::fmt_time(conn.last_seen)), true);
                detail_row(ui, "路径", conn.process_path.as_deref(), true);
                detail_row(ui, "命令行", conn.process_cmdline.as_deref(), true);

                // Actions (DESIGN.md 4.9 — not full width, danger button red)
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("终止进程").color(theme::SEMANTIC_DANGER),
                        ))
                        .clicked()
                    {
                        self.pending_kill_pid = Some(conn.pid);
                        self.kill_confirm_open = true;
                    }
                    if ui.button("刷新命令行").clicked() {
                        let pid = conn.pid;
                        let svc_ctx = ctx.clone();
                        rt.spawn(async move {
                            let svc = NetworkService { ctx: &svc_ctx };
                            let _ = svc.refresh_cmdline(pid).await;
                        });
                    }
                });
            });
    }

    // ── Kill Confirmation Dialog (DESIGN.md 4.10) ──────────

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
                            egui::RichText::new("确认终止").color(theme::SEMANTIC_DANGER),
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

    // ── CSV Export ────────────────────────────────────────

    fn export_csv(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let items = match &self.snapshot {
            Some(snap) => snap.items.clone(),
            None => return,
        };
        let dir = ctx.app_dirs.root().to_path_buf();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("network_export_{}.csv", secs));
        rt.spawn_blocking(move || match write_csv(&path, &items) {
            Ok(()) => {
                tracing::info!("exported {} connections to {}", items.len(), path.display())
            }
            Err(e) => tracing::error!("csv export failed: {}", e),
        });
    }

    // ── Helpers ─────────────────────────────────────────────

    fn toggle_sort(&mut self, col: SortColumn) {
        if self.sort_column == col {
            self.sort_dir = self.sort_dir.toggle();
        } else {
            self.sort_column = col;
            self.sort_dir = SortDir::Desc;
        }
        self.cache_dirty = true; // B2
    }

    #[allow(dead_code)]
    fn is_selected(&self, conn: &NetConn) -> bool {
        (self.selected_pid == Some(conn.pid))
            && self
                .selected_local
                .as_deref()
                .is_none_or(|l| l == format!("{}:{}", conn.local.addr, conn.local.port))
            && self
                .selected_remote
                .as_deref()
                .is_none_or(|r| r == format!("{}:{}", conn.remote.addr, conn.remote.port))
    }

    fn find_selected_conn(&self) -> Option<&NetConn> {
        self.selected_conn.as_ref()
    }

    fn get_filtered_sorted_items(&mut self) -> &[NetConn] {
        if self.cache_dirty {
            self.cached_items = self.compute_filtered_sorted();
            self.cache_dirty = false;
        }
        &self.cached_items
    }

    fn compute_filtered_sorted(&self) -> Vec<NetConn> {
        let Some(ref snap) = self.snapshot else {
            return vec![];
        };

        let mut items: Vec<NetConn> = snap
            .items
            .iter()
            .filter(|c| {
                // History toggle: hide non-current connections when show_history is false
                self.show_history || c.is_current
            })
            .filter(|c| match self.proto_filter {
                ProtoFilter::All => true,
                ProtoFilter::Tcp => c.proto == Proto::Tcp,
                ProtoFilter::Udp => c.proto == Proto::Udp,
            })
            .filter(|c| {
                // State filter: include if connection has no state or state is in filter
                if c.state == ConnState::None {
                    true
                } else {
                    self.state_filter.contains(&c.state)
                }
            })
            .filter(|c| {
                if self.search.is_empty() {
                    return true;
                }
                let q = self.search.to_lowercase();
                c.process_name.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || c.pid.to_string().contains(&q)
                    || c.local.addr.to_lowercase().contains(&q)
                    || c.local.port.to_string().contains(&q)
                    || c.remote.addr.to_lowercase().contains(&q)
                    || c.remote.port.to_string().contains(&q)
                    || c.process_path.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || c.state.as_str().to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let cmp = match self.sort_column {
                SortColumn::FirstSeen => a.first_seen.cmp(&b.first_seen),
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Process => a
                    .process_name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.process_name.as_deref().unwrap_or("")),
                SortColumn::Local => a.local.addr.cmp(&b.local.addr).then(a.local.port.cmp(&b.local.port)),
                SortColumn::Remote => a
                    .remote
                    .addr
                    .cmp(&b.remote.addr)
                    .then(a.remote.port.cmp(&b.remote.port)),
                SortColumn::State => a.state.as_str().cmp(b.state.as_str()),
                SortColumn::Proto => {
                    let proto_key = |p: &Proto| match p {
                        Proto::Tcp => "tcp",
                        Proto::Udp => "udp",
                    };
                    proto_key(&a.proto).cmp(proto_key(&b.proto))
                }
                SortColumn::Family => (a.family as u8).cmp(&(b.family as u8)),
                SortColumn::Path => a
                    .process_path
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.process_path.as_deref().unwrap_or("")),
                SortColumn::Cmdline => a
                    .process_cmdline
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.process_cmdline.as_deref().unwrap_or("")),
                SortColumn::LastSeen => a.last_seen.cmp(&b.last_seen),
            };
            match self.sort_dir {
                SortDir::Asc => cmp,
                SortDir::Desc => cmp.reverse(),
            }
        });

        items
    }
}

// ── Cell renderers ─────────────────────────────────────────

/// Non-selectable label (prevents text cursor/edit mode).
fn ui_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).selectable(false))
}

/// Non-selectable label for table cells.
fn cell_text(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui_label(ui, text)
}

/// For history rows, returns FG_TERTIARY (gray), otherwise the given color.
fn fg_color(history_base: egui::Color32, is_history: bool) -> egui::Color32 {
    if is_history {
        theme::FG_TERTIARY
    } else {
        history_base
    }
}

fn cell_first_seen(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(theme::fmt_time(conn.first_seen))
                .monospace()
                .size(12.0)
                .color(fg_color(theme::FG_SECONDARY, is_history)),
        );
    });
    r
}

fn cell_pid(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(conn.pid.to_string())
                .monospace()
                .size(12.0)
                .color(fg_color(theme::FG_PRIMARY, is_history)),
        );
    });
    r
}

fn cell_process(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        let text = if is_history {
            format!("[{}]", conn.process_name.as_deref().unwrap_or("-"))
        } else {
            conn.process_name.as_deref().unwrap_or("-").to_string()
        };
        ui_label(
            ui,
            egui::RichText::new(text).color(fg_color(theme::FG_PRIMARY, is_history)),
        );
    });
    r
}

fn cell_local(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(format_endpoint(&conn.local.addr, conn.local.port))
                .monospace()
                .size(12.0)
                .color(fg_color(theme::FG_PRIMARY, is_history)),
        );
    });
    r
}

fn cell_remote(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(format_endpoint(&conn.remote.addr, conn.remote.port))
                .monospace()
                .size(12.0)
                .color(fg_color(theme::FG_PRIMARY, is_history)),
        );
    });
    r
}

fn cell_state(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        let s = conn.state.as_str();
        if s.is_empty() {
            cell_text(ui, egui::RichText::new("-").color(theme::FG_TERTIARY));
        } else if is_history {
            // Historical state — show as gray text instead of colored badge
            cell_text(ui, egui::RichText::new(s).color(theme::FG_TERTIARY));
        } else {
            badge::badge(ui, s, state_badge_variant(&conn.state));
        }
    });
    r
}

fn cell_proto(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(format!("{:?}", conn.proto).to_uppercase())
                .color(fg_color(theme::FG_PRIMARY, is_history)),
        );
    });
    r
}

fn cell_family(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(format!("{:?}", conn.family).to_uppercase())
                .color(fg_color(theme::FG_PRIMARY, is_history)),
        );
    });
    r
}

fn cell_path(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        let path = conn.process_path.as_deref().unwrap_or("");
        let col = fg_color(theme::FG_SECONDARY, is_history);
        let resp = ui.add(
            egui::Label::new(egui::RichText::new(path).monospace().size(12.0).color(col))
                .selectable(false)
                .truncate(),
        );
        if !path.is_empty() {
            resp.on_hover_text(path);
        }
    });
    r
}

fn cell_cmdline(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        let cmd = conn.process_cmdline.as_deref().unwrap_or("");
        if cmd.is_empty() {
            cell_text(ui, egui::RichText::new("-").color(theme::FG_TERTIARY));
        } else {
            let col = fg_color(theme::FG_SECONDARY, is_history);
            let resp = ui.add(
                egui::Label::new(egui::RichText::new(cmd).monospace().size(12.0).color(col))
                    .selectable(false)
                    .truncate(),
            );
            resp.on_hover_text(cmd);
        }
    });
    r
}

fn cell_last_seen(row: &mut egui_extras::TableRow<'_, '_>, conn: &NetConn, is_history: bool) -> egui::Response {
    let (_, r) = row.col(|ui| {
        cell_text(
            ui,
            egui::RichText::new(theme::fmt_time(conn.last_seen))
                .monospace()
                .size(12.0)
                .color(fg_color(theme::FG_SECONDARY, is_history)),
        );
    });
    r
}

// ── Utility functions ──────────────────────────────────────

fn filter_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let btn = if active {
        egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE)).fill(theme::ACCENT)
    } else {
        egui::Button::new(egui::RichText::new(label).color(theme::FG_SECONDARY))
    };
    ui.add(btn).clicked()
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

fn state_badge_variant(state: &ConnState) -> BadgeVariant {
    match state {
        ConnState::Established => BadgeVariant::Success,
        ConnState::Listen => BadgeVariant::Info,
        ConnState::TimeWait => BadgeVariant::Warning,
        ConnState::CloseWait => BadgeVariant::Danger,
        _ => BadgeVariant::Default,
    }
}

fn write_csv(path: &std::path::Path, items: &[NetConn]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "PID,Process,LocalAddr,RemoteAddr,State,Proto,Family,Path,Cmdline")?;
    for c in items {
        let local = format!("{}:{}", c.local.addr, c.local.port);
        let remote = format!("{}:{}", c.remote.addr, c.remote.port);
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{}",
            c.pid,
            csv_escape(c.process_name.as_deref().unwrap_or("")),
            csv_escape(&local),
            csv_escape(&remote),
            csv_escape(c.state.as_str()),
            csv_escape(&format!("{:?}", c.proto)),
            csv_escape(&format!("{:?}", c.family)),
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
