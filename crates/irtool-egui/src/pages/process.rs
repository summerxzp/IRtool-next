use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_autoruns::AutorunItem;
use irtool_net_monitor::NetConn;
use irtool_process::{ProcessChain, ProcessEntry, ProcessNode, ProcessSnapshot};
use irtool_service::context::AppContext;
use irtool_service::services::process::ProcessService;
use irtool_service::types::{ConnState, SysmonEvent, SysmonEventType};

use crate::icon_cache::IconCache;
use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;
use crate::widgets::table::{self, SortDir};

// ── 枚举 ──────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Tree,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    All,
    Suspicious,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Pid,
    Ppid,
    Name,
    Path,
    Suspicious,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Chain,
    Network,
    Sysmon,
    Autoruns,
}

// ── 树结构 ────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProcessTreeNode {
    pub entry: ProcessEntry,
    pub children: Vec<ProcessTreeNode>,
    pub is_orphan: bool,
}

/// 构建父子关系树。ppid 在快照中则挂为 child，否则为 root。
fn build_process_tree(processes: &[ProcessEntry]) -> Vec<ProcessTreeNode> {
    // 第一遍：创建 pid → entry 映射
    let mut entry_map: HashMap<u32, &ProcessEntry> = HashMap::new();
    for p in processes {
        entry_map.insert(p.pid, p);
    }

    // 第二遍：构建 ppid → [child pid] 映射，记录哪些 pid 是子节点
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut child_pids: HashSet<u32> = HashSet::new();
    for p in processes {
        // ppid 在快照中且不是自身 → 是子节点
        if entry_map.contains_key(&p.ppid) && p.ppid != p.pid {
            children_map.entry(p.ppid).or_default().push(p.pid);
            child_pids.insert(p.pid);
        }
    }

    // 递归构建树节点
    fn build_node(
        pid: u32,
        entry_map: &HashMap<u32, &ProcessEntry>,
        children_map: &HashMap<u32, Vec<u32>>,
    ) -> ProcessTreeNode {
        let entry = (*entry_map.get(&pid).unwrap()).clone();
        let children = children_map
            .get(&pid)
            .map(|pids| {
                pids.iter()
                    .map(|&cpid| build_node(cpid, entry_map, children_map))
                    .collect()
            })
            .unwrap_or_default();
        ProcessTreeNode {
            entry,
            children,
            is_orphan: false,
        }
    }

    // 第三遍：收集 roots（不是任何节点的子节点）
    let mut roots: Vec<ProcessTreeNode> = Vec::new();
    for p in processes {
        if !child_pids.contains(&p.pid) {
            let mut root = build_node(p.pid, &entry_map, &children_map);
            root.is_orphan = p.ppid != 0 && p.ppid != 4;
            roots.push(root);
        }
    }

    roots
}

/// 收集树中所有有子节点的 pid（展开/收起全部用）
fn collect_parent_pids(node: &ProcessTreeNode) -> Vec<u32> {
    let mut pids = Vec::new();
    if !node.children.is_empty() {
        pids.push(node.entry.pid);
        for child in &node.children {
            pids.extend(collect_parent_pids(child));
        }
    }
    pids
}

fn collect_parent_pids_roots(roots: &[ProcessTreeNode]) -> Vec<u32> {
    roots.iter().flat_map(collect_parent_pids).collect()
}

// ── 异步刷新载荷 ──────────────────────────────────────────

#[derive(Default)]
pub struct ProcessRefresh {
    pub snapshot: Option<ProcessSnapshot>,
    pub chain: Option<ProcessChain>,
    pub network: Option<Vec<NetConn>>,
    pub autoruns: Option<Vec<AutorunItem>>,
    pub error: Option<String>,
}

// ── ProcessPageState ──────────────────────────────────────

pub struct ProcessPageState {
    // 数据
    pub snapshot: Option<ProcessSnapshot>,
    pub last_error: Option<String>,

    // 过滤/视图
    pub search: String,
    pub filter: FilterMode,
    pub view_mode: ViewMode,
    pub sort_column: SortColumn,
    pub sort_dir: SortDir,

    // 树视图
    pub expanded_pids: HashSet<u32>,
    pub expand_all_version: u32,

    // 选中/详情
    pub selected_pid: Option<u32>,
    pub selected_entry: Option<ProcessEntry>,
    pub detail_visible: bool,
    pub active_tab: DetailTab,

    // 详情数据（按需异步加载）
    pub chain: Option<ProcessChain>,
    pub chain_loading: bool,
    pub network_conns: Vec<NetConn>,
    pub network_loading: bool,
    pub autoruns_items: Vec<AutorunItem>,
    pub autoruns_loading: bool,

    // 自动刷新
    pub auto_refresh_ms: u64,
    pub last_refresh: Option<Instant>,
    pub refreshing: bool,

    // 异步通道
    pub refresh_rx: Option<mpsc::Receiver<ProcessRefresh>>,

    // 图标缓存（复用主 UI 的 extract_icon_base64）
    icon_cache: IconCache,
    icons_need_preload: bool,

    // 缓存 (B2 pattern, DESIGN.md 10.1)
    cached_items: Vec<ProcessEntry>,
    cache_dirty: bool,
    cached_tree: Vec<ProcessTreeNode>,
    tree_cache_dirty: bool,
}

impl Default for ProcessPageState {
    fn default() -> Self {
        Self {
            snapshot: None,
            last_error: None,
            search: String::new(),
            filter: FilterMode::All,
            view_mode: ViewMode::Tree,
            sort_column: SortColumn::Pid,
            sort_dir: SortDir::Asc,
            expanded_pids: HashSet::new(),
            expand_all_version: 0,
            selected_pid: None,
            selected_entry: None,
            detail_visible: false,
            active_tab: DetailTab::Chain,
            chain: None,
            chain_loading: false,
            network_conns: Vec::new(),
            network_loading: false,
            autoruns_items: Vec::new(),
            autoruns_loading: false,
            auto_refresh_ms: 0,
            last_refresh: None,
            refreshing: false,
            refresh_rx: None,
            icon_cache: IconCache::default(),
            icons_need_preload: false,
            cached_items: Vec::new(),
            cache_dirty: true,
            cached_tree: Vec::new(),
            tree_cache_dirty: true,
        }
    }
}

impl ProcessPageState {
    // ── 异步数据拉取 ─────────────────────────────────────

    pub fn trigger_refresh(&mut self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;

        let (tx, rx) = mpsc::channel();
        self.refresh_rx = Some(rx);

        let svc_ctx = ctx.clone();
        rt.spawn(async move {
            let svc = ProcessService { ctx: &svc_ctx };
            let result = svc.snapshot().await;
            let refresh = match result {
                Ok(snap) => ProcessRefresh {
                    snapshot: Some(snap),
                    ..Default::default()
                },
                Err(e) => ProcessRefresh {
                    error: Some(e.to_string()),
                    ..Default::default()
                },
            };
            let _ = tx.send(refresh);
        });
    }

    fn trigger_detail_fetch(&mut self, pid: u32, exe: Option<&str>, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // 清空旧数据
        self.chain = None;
        self.chain_loading = true;
        self.network_conns = Vec::new();
        self.network_loading = true;
        self.autoruns_items = Vec::new();
        self.autoruns_loading = true;

        let (tx, rx) = mpsc::channel();
        self.refresh_rx = Some(rx);

        // 并行拉取 chain / network / autoruns
        let svc_ctx = ctx.clone();
        let tx1 = tx.clone();
        rt.spawn(async move {
            let svc = ProcessService { ctx: &svc_ctx };
            let result = svc.chain(pid).await;
            let refresh = match result {
                Ok(chain) => ProcessRefresh {
                    chain: Some(chain),
                    ..Default::default()
                },
                Err(e) => ProcessRefresh {
                    error: Some(e.to_string()),
                    ..Default::default()
                },
            };
            let _ = tx1.send(refresh);
        });

        let net_history = ctx.net_history.clone();
        let tx2 = tx.clone();
        rt.spawn_blocking(move || {
            let conns = net_history.query_by_pid(pid);
            let _ = tx2.send(ProcessRefresh {
                network: Some(conns),
                ..Default::default()
            });
        });

        if let Some(exe_path) = exe.map(|s| s.to_string()) {
            let autoruns_store = ctx.autoruns_store.clone();
            let tx3 = tx;
            rt.spawn_blocking(move || {
                let items = autoruns_store.query_by_path(&exe_path);
                let _ = tx3.send(ProcessRefresh {
                    autoruns: Some(items),
                    ..Default::default()
                });
            });
        } else {
            // 无可执行路径，直接标记完成
            self.autoruns_loading = false;
        }
    }

    fn check_auto_refresh(&mut self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        if self.auto_refresh_ms == 0 {
            return;
        }
        let should_refresh = match self.last_refresh {
            None => true,
            Some(t) => t.elapsed().as_millis() as u64 >= self.auto_refresh_ms,
        };
        if should_refresh && !self.refreshing {
            self.last_refresh = Some(Instant::now());
            self.trigger_refresh(ctx, rt);
        }
    }

    /// 排空异步通道，应用刷新数据
    pub fn apply_refresh(&mut self, r: ProcessRefresh) {
        if let Some(snap) = r.snapshot {
            self.snapshot = Some(snap);
            self.refreshing = false;
            self.cache_dirty = true;
            self.tree_cache_dirty = true;
            self.icons_need_preload = true;
            // 首次获取快照时默认展开全部
            if self.expanded_pids.is_empty() {
                let tree = build_process_tree(&self.snapshot.as_ref().unwrap().processes);
                self.expanded_pids = collect_parent_pids_roots(&tree).into_iter().collect();
                self.cached_tree = tree;
                self.tree_cache_dirty = false;
            }
            // B3: 刷新选中进程缓存
            if let Some(pid) = self.selected_pid {
                self.selected_entry = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.processes.iter().find(|e| e.pid == pid).cloned());
            }
        }
        if let Some(chain) = r.chain {
            self.chain = Some(chain);
            self.chain_loading = false;
        }
        if let Some(network) = r.network {
            self.network_conns = network;
            self.network_loading = false;
        }
        if let Some(autoruns) = r.autoruns {
            self.autoruns_items = autoruns;
            self.autoruns_loading = false;
        }
        if let Some(err) = r.error {
            self.last_error = Some(err);
            self.refreshing = false;
        }
    }

    /// 排空通道中所有消息
    pub fn drain_refresh_channel(&mut self) {
        let rx = match self.refresh_rx.take() {
            Some(rx) => rx,
            None => return,
        };
        let mut pending: Vec<ProcessRefresh> = Vec::new();
        while let Ok(r) = rx.try_recv() {
            pending.push(r);
        }
        // 保留通道供后续使用
        self.refresh_rx = Some(rx);
        for r in pending {
            self.apply_refresh(r);
        }
    }

    // ── 渲染入口 ─────────────────────────────────────────

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &AppContext,
        rt: &tokio::runtime::Handle,
        _sysmon_events: &VecDeque<SysmonEvent>,
    ) {
        // 0. 排空异步通道
        self.drain_refresh_channel();

        // 1. 自动刷新检查
        self.check_auto_refresh(ctx, rt);

        // 1b. 图标预加载 / 轮询
        if self.icons_need_preload {
            self.icons_need_preload = false;
            if let Some(ref snap) = self.snapshot {
                let paths: Vec<String> = snap.processes.iter().filter_map(|p| p.exe.clone()).collect();
                self.icon_cache.preload(rt, paths);
            }
        }
        self.icon_cache.poll(ui.ctx());

        // 2. 工具栏
        self.render_toolbar(ui, ctx, rt);
        ui.separator();

        // 3. 错误 banner
        if let Some(ref err) = self.last_error {
            let err = err.clone();
            if crate::widgets::banner::error_banner(ui, &err) {
                self.last_error = None;
            }
            ui.add_space(4.0);
        }

        // 4. 主内容区
        match self.view_mode {
            ViewMode::List => self.render_table(ui, ctx, rt),
            ViewMode::Tree => self.render_tree(ui, ctx, rt),
        }
    }

    // ── 工具栏 ────────────────────────────────────────────

    fn render_toolbar(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // 刷新按钮
            let refresh_btn = if self.refreshing {
                egui::Button::new(egui::RichText::new("↻ 刷新中...").color(theme::FG_TERTIARY))
            } else {
                egui::Button::new(egui::RichText::new("↻ 刷新").color(theme::FG_PRIMARY))
            };
            if ui.add_enabled(!self.refreshing, refresh_btn).clicked() {
                self.trigger_refresh(ctx, rt);
            }
            if self.refreshing {
                ui.spinner();
            }

            ui.separator();

            // 自动刷新 Play/Pause
            if self.auto_refresh_ms == 0 {
                if ui.button("▶ 自动").clicked() {
                    self.auto_refresh_ms = 2000;
                    self.last_refresh = Some(Instant::now());
                }
            } else {
                if ui.button("‖ 暂停").clicked() {
                    self.auto_refresh_ms = 0;
                    self.last_refresh = None;
                }
                // 间隔下拉
                let intervals = [(1000, "1s"), (2000, "2s"), (5000, "5s"), (10000, "10s")];
                egui::ComboBox::from_id_salt("auto_refresh_interval")
                    .selected_text(
                        intervals
                            .iter()
                            .find(|(ms, _)| *ms == self.auto_refresh_ms)
                            .map(|(_, label)| *label)
                            .unwrap_or("?"),
                    )
                    .show_ui(ui, |ui| {
                        for (ms, label) in intervals {
                            if ui.selectable_label(self.auto_refresh_ms == ms, label).clicked() {
                                self.auto_refresh_ms = ms;
                            }
                        }
                    });
            }

            ui.separator();

            // 视图切换
            if filter_button(ui, "列表", self.view_mode == ViewMode::List) {
                self.view_mode = ViewMode::List;
            }
            if filter_button(ui, "树", self.view_mode == ViewMode::Tree) {
                self.view_mode = ViewMode::Tree;
            }

            // 树视图时显示展开/收起按钮
            if self.view_mode == ViewMode::Tree {
                self.toggle_expand_all(ui);
            }

            ui.separator();

            // 过滤下拉
            egui::ComboBox::from_id_salt("process_filter")
                .selected_text(match self.filter {
                    FilterMode::All => "全部",
                    FilterMode::Suspicious => "可疑",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.filter == FilterMode::All, "全部").clicked() {
                        self.filter = FilterMode::All;
                        self.cache_dirty = true;
                        self.tree_cache_dirty = true;
                    }
                    if ui
                        .selectable_label(self.filter == FilterMode::Suspicious, "可疑")
                        .clicked()
                    {
                        self.filter = FilterMode::Suspicious;
                        self.cache_dirty = true;
                        self.tree_cache_dirty = true;
                    }
                });

            ui.separator();

            // 搜索框
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(160.0)
                    .hint_text("PID / 进程名 / 路径"),
            );
            if search_resp.changed() {
                self.cache_dirty = true;
                self.tree_cache_dirty = true;
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true;
                self.tree_cache_dirty = true;
            }

            // 右对齐：快照时间戳和进程数
            if let Some(ref snap) = self.snapshot {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} 进程 · {}",
                            snap.processes.len(),
                            theme::fmt_time(snap.timestamp)
                        ))
                        .size(12.0)
                        .color(theme::FG_TERTIARY),
                    );
                });
            }
        });
    }

    /// 展开/收起全部按钮
    fn toggle_expand_all(&mut self, ui: &mut egui::Ui) {
        let tree = self.get_cached_tree().to_vec();
        // 只有有子节点的 pid 才需要展开/收起
        let all_parent_pids: HashSet<u32> = tree.iter().flat_map(collect_parent_pids).collect();
        let all_expanded =
            !all_parent_pids.is_empty() && all_parent_pids.iter().all(|pid| self.expanded_pids.contains(pid));
        let label = if all_expanded { "收起全部" } else { "展开全部" };
        if ui.button(label).clicked() {
            self.expand_all_version += 1;
            if all_expanded {
                self.expanded_pids.clear();
            } else {
                self.expanded_pids.extend(all_parent_pids);
            }
        }
    }

    // ── 列表视图 ──────────────────────────────────────────

    fn render_table(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let sc = self.sort_column;
        let sd = self.sort_dir;
        let snapshot_is_none = self.snapshot.is_none();
        let sel_pid = self.selected_pid;

        let items = self.get_filtered_sorted_items().to_vec();
        let icon_cache = &self.icon_cache;

        if items.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                if snapshot_is_none {
                    ui.spinner();
                    ui_label(ui, egui::RichText::new("等待进程数据...").color(theme::FG_SECONDARY));
                } else {
                    ui_label(ui, egui::RichText::new("没有匹配的进程").color(theme::FG_TERTIARY));
                }
            });
            return;
        }

        let mut sort_toggle: Option<SortColumn> = None;
        let mut row_click: Option<Option<u32>> = None;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(70.0).clip(true)) // PID
            .column(Column::initial(70.0).clip(true)) // PPID
            .column(Column::initial(160.0).clip(true)) // 名称
            .column(Column::initial(350.0).clip(true)) // 路径
            .column(Column::initial(80.0).clip(true)); // 可疑

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    if table::sortable_header(ui, "PID", sc == SortColumn::Pid, sd) {
                        sort_toggle = Some(SortColumn::Pid);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "PPID", sc == SortColumn::Ppid, sd) {
                        sort_toggle = Some(SortColumn::Ppid);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "名称", sc == SortColumn::Name, sd) {
                        sort_toggle = Some(SortColumn::Name);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "路径", sc == SortColumn::Path, sd) {
                        sort_toggle = Some(SortColumn::Path);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "可疑", sc == SortColumn::Suspicious, sd) {
                        sort_toggle = Some(SortColumn::Suspicious);
                    }
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let entry = &items[row.index()];
                    let is_selected = sel_pid == Some(entry.pid);
                    let is_suspicious = entry.is_suspicious;

                    // PID
                    row.col(|ui| {
                        cell_text(
                            ui,
                            egui::RichText::new(entry.pid.to_string())
                                .monospace()
                                .size(12.0)
                                .color(theme::FG_PRIMARY),
                        );
                    });
                    // PPID
                    row.col(|ui| {
                        cell_text(
                            ui,
                            egui::RichText::new(entry.ppid.to_string())
                                .monospace()
                                .size(12.0)
                                .color(theme::FG_SECONDARY),
                        );
                    });
                    // 名称（带图标）
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            icon_cache.icon_or_placeholder(ui, entry.exe.as_deref(), 16.0);
                            ui.add_space(4.0);
                            let color = if is_suspicious {
                                theme::SEMANTIC_WARNING
                            } else {
                                theme::FG_PRIMARY
                            };
                            cell_text(ui, egui::RichText::new(&entry.name).color(color));
                        });
                    });
                    // 路径
                    row.col(|ui| {
                        let path = entry.exe.as_deref().unwrap_or("");
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(path)
                                    .monospace()
                                    .size(12.0)
                                    .color(theme::FG_SECONDARY),
                            )
                            .selectable(false)
                            .truncate(),
                        );
                        if !path.is_empty() {
                            resp.on_hover_text(path);
                        }
                    });
                    // 可疑
                    row.col(|ui| {
                        if is_suspicious {
                            badge::badge(ui, "⚠ 可疑", BadgeVariant::Warning);
                        }
                    });

                    // 选中高亮
                    if is_selected {
                        row.set_selected(true);
                    }
                    // 可疑行高亮
                    if is_suspicious && !is_selected {
                        // 用 warning 色半透明背景
                        row.set_selected(true);
                    }

                    // 行点击
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            row_click = Some(None);
                        } else {
                            row_click = Some(Some(entry.pid));
                        }
                    }
                });
            });

        // 应用交互
        if let Some(pid_opt) = row_click {
            match pid_opt {
                None => {
                    // 取消选中
                    self.selected_pid = None;
                    self.selected_entry = None;
                    self.detail_visible = false;
                }
                Some(pid) => {
                    // B3: 从 cached_items 缓存选中 entry
                    self.selected_entry = self.cached_items.iter().find(|e| e.pid == pid).cloned();
                    self.selected_pid = Some(pid);
                    self.detail_visible = true;
                    let exe = self.selected_entry.as_ref().and_then(|e| e.exe.clone());
                    self.trigger_detail_fetch(pid, exe.as_deref(), ctx, rt);
                }
            }
        }

        if let Some(col) = sort_toggle {
            self.toggle_sort(col);
        }
    }

    // ── 树视图 ────────────────────────────────────────────

    fn render_tree(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tree = self.get_cached_tree().to_vec();
        let snapshot_is_none = self.snapshot.is_none();

        if tree.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                if snapshot_is_none {
                    ui.spinner();
                    ui_label(ui, egui::RichText::new("等待进程数据...").color(theme::FG_SECONDARY));
                } else {
                    ui_label(ui, egui::RichText::new("没有匹配的进程").color(theme::FG_TERTIARY));
                }
            });
            return;
        }

        let sel_pid = self.selected_pid;
        let expanded = self.expanded_pids.clone();
        let icon_cache = &self.icon_cache;

        let mut clicked_pid: Option<(u32, bool)> = None;

        egui::ScrollArea::vertical()
            .id_salt("process_tree_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for node in &tree {
                    render_tree_node(ui, node, 0, sel_pid, &expanded, icon_cache, &mut clicked_pid);
                }
            });

        // 处理点击
        if let Some((pid, has_children)) = clicked_pid {
            if sel_pid == Some(pid) {
                // 取消选中
                self.selected_pid = None;
                self.selected_entry = None;
                self.detail_visible = false;
            } else {
                // B3: 缓存选中 entry
                self.selected_entry = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.processes.iter().find(|e| e.pid == pid).cloned());
                self.selected_pid = Some(pid);
                self.detail_visible = true;
                let exe = self.selected_entry.as_ref().and_then(|e| e.exe.clone());
                self.trigger_detail_fetch(pid, exe.as_deref(), ctx, rt);
            }
            // 仅对有子节点的行 toggle 展开
            if has_children {
                if self.expanded_pids.contains(&pid) {
                    self.expanded_pids.remove(&pid);
                } else {
                    self.expanded_pids.insert(pid);
                }
            }
        }
    }

    // ── 详情面板 ──────────────────────────────────────────

    pub fn render_detail_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &AppContext,
        rt: &tokio::runtime::Handle,
        sysmon_events: &VecDeque<SysmonEvent>,
    ) {
        let entry = self.selected_entry.clone();
        let Some(ref entry) = entry else {
            self.detail_visible = false;
            return;
        };

        let chain = self.chain.clone();
        let network_conns = self.network_conns.clone();
        let autoruns_items = self.autoruns_items.clone();
        let chain_loading = self.chain_loading;
        let network_loading = self.network_loading;
        let autoruns_loading = self.autoruns_loading;
        let active_tab = self.active_tab;
        let selected_pid = self.selected_pid;

        egui::ScrollArea::vertical()
            .id_salt("process_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header（right_to_left 布局，DESIGN.md 4.7）
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        // 右侧边距避免与滚动条重合
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_pid = None;
                            self.selected_entry = None;
                        }
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&entry.name)
                                        .color(theme::FG_PRIMARY)
                                        .strong()
                                        .size(14.0),
                                );
                                if entry.is_suspicious {
                                    ui.label(egui::RichText::new("⚠").color(theme::SEMANTIC_WARNING));
                                }
                            });
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("PID {}", entry.pid))
                                        .monospace()
                                        .color(theme::FG_TERTIARY)
                                        .size(11.0),
                                );
                                ui.label(egui::RichText::new("·").color(theme::FG_TERTIARY));
                                ui.label(
                                    egui::RichText::new(format!("PPID {}", entry.ppid))
                                        .monospace()
                                        .color(theme::FG_TERTIARY)
                                        .size(11.0),
                                );
                            });
                        });
                    });
                });

                // 路径
                if let Some(ref exe) = entry.exe {
                    detail_row(ui, "路径", Some(exe.as_str()), true);
                }

                // 可疑原因
                if let Some(ref reason) = entry.suspicious_reason {
                    detail_row(ui, "可疑原因", Some(reason.as_str()), false);
                }

                ui.add_space(4.0);

                // Tab bar
                ui.horizontal(|ui| {
                    let chain_count = chain.as_ref().map(|c| c.nodes.len()).unwrap_or(0);
                    let net_count = network_conns.len();
                    let sysmon_count = sysmon_events.iter().filter(|e| e.process_id == entry.pid).count();
                    let autoruns_count = autoruns_items.len();

                    if tab_button(ui, "进程链", chain_count, active_tab == DetailTab::Chain) {
                        self.active_tab = DetailTab::Chain;
                    }
                    if tab_button(ui, "网络", net_count, active_tab == DetailTab::Network) {
                        self.active_tab = DetailTab::Network;
                    }
                    if tab_button(ui, "Sysmon", sysmon_count, active_tab == DetailTab::Sysmon) {
                        self.active_tab = DetailTab::Sysmon;
                    }
                    if tab_button(ui, "持久化", autoruns_count, active_tab == DetailTab::Autoruns) {
                        self.active_tab = DetailTab::Autoruns;
                    }
                });

                ui.separator();
                ui.add_space(4.0);

                // Tab content
                match active_tab {
                    DetailTab::Chain => {
                        if chain_loading {
                            ui.spinner();
                            ui.label(egui::RichText::new("加载中...").color(theme::FG_TERTIARY));
                        } else if let Some(ref chain) = chain {
                            if chain.nodes.is_empty() {
                                ui.label(egui::RichText::new("无进程链数据").color(theme::FG_TERTIARY));
                            } else {
                                // 倒序显示：root→target
                                let nodes: Vec<&ProcessNode> = chain.nodes.iter().rev().collect();
                                for (i, node) in nodes.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.add_space(i as f32 * 12.0);
                                        ui.label(egui::RichText::new("└─").color(theme::FG_TERTIARY).size(11.0));
                                        let name_color = if node.is_suspicious {
                                            theme::SEMANTIC_WARNING
                                        } else if node.is_target {
                                            theme::ACCENT
                                        } else {
                                            theme::FG_PRIMARY
                                        };
                                        ui.label(
                                            egui::RichText::new(format!("{} ({})", node.name, node.pid))
                                                .color(name_color)
                                                .size(12.0),
                                        );
                                        if node.is_suspicious {
                                            ui.label(egui::RichText::new("⚠").color(theme::SEMANTIC_WARNING));
                                        }
                                        if node.is_target {
                                            badge::badge(ui, "目标", BadgeVariant::Info);
                                        }
                                    });
                                    // 展开详情
                                    if let Some(ref exe) = node.exe {
                                        ui.horizontal(|ui| {
                                            ui.add_space((i + 1) as f32 * 12.0 + 16.0);
                                            detail_row(ui, "路径", Some(exe.as_str()), true);
                                        });
                                    }
                                    if let Some(ref cmdline) = node.cmdline {
                                        ui.horizontal(|ui| {
                                            ui.add_space((i + 1) as f32 * 12.0 + 16.0);
                                            detail_row(ui, "命令行", Some(cmdline.as_str()), true);
                                        });
                                    }
                                    if let Some(ref ct) = node.create_time {
                                        ui.horizontal(|ui| {
                                            ui.add_space((i + 1) as f32 * 12.0 + 16.0);
                                            detail_row(ui, "创建时间", Some(ct.as_str()), true);
                                        });
                                    }
                                }
                            }
                        } else {
                            ui.label(egui::RichText::new("点击进程查看进程链").color(theme::FG_TERTIARY));
                        }
                    }
                    DetailTab::Network => {
                        if network_loading {
                            ui.spinner();
                            ui.label(egui::RichText::new("加载中...").color(theme::FG_TERTIARY));
                        } else if network_conns.is_empty() {
                            ui.label(egui::RichText::new("无关联网络连接").color(theme::FG_TERTIARY));
                        } else {
                            for conn in &network_conns {
                                ui.horizontal(|ui| {
                                    // 协议
                                    ui.label(
                                        egui::RichText::new(format!("{:?}", conn.proto).to_uppercase())
                                            .size(11.0)
                                            .color(theme::FG_TERTIARY),
                                    );
                                    // local→remote
                                    ui.label(
                                        egui::RichText::new(format_endpoint(&conn.local.addr, conn.local.port))
                                            .monospace()
                                            .size(11.0)
                                            .color(theme::FG_PRIMARY),
                                    );
                                    ui.label(egui::RichText::new("→").color(theme::FG_TERTIARY));
                                    ui.label(
                                        egui::RichText::new(format_endpoint(&conn.remote.addr, conn.remote.port))
                                            .monospace()
                                            .size(11.0)
                                            .color(theme::FG_PRIMARY),
                                    );
                                    // 状态 badge
                                    badge::badge(ui, conn.state.as_str(), state_badge_variant(&conn.state));
                                    // 最近出现
                                    ui.label(
                                        egui::RichText::new(theme::fmt_time(conn.last_seen))
                                            .monospace()
                                            .size(10.0)
                                            .color(theme::FG_TERTIARY),
                                    );
                                });
                            }
                        }
                    }
                    DetailTab::Sysmon => {
                        let pid_events: Vec<&SysmonEvent> = sysmon_events
                            .iter()
                            .filter(|e| e.process_id == entry.pid)
                            .take(50)
                            .collect();
                        if pid_events.is_empty() {
                            ui.label(egui::RichText::new("无关联 Sysmon 事件").color(theme::FG_TERTIARY));
                        } else {
                            for evt in &pid_events {
                                ui.horizontal(|ui| {
                                    // 事件类型 label
                                    badge::badge(
                                        ui,
                                        evt.event_type.label(),
                                        sysmon_event_badge_variant(&evt.event_type),
                                    );
                                    // 关键信息
                                    let key_info = sysmon_event_key_info(evt);
                                    ui.label(egui::RichText::new(key_info).size(11.0).color(theme::FG_PRIMARY));
                                    // 时间
                                    ui.label(
                                        egui::RichText::new(&evt.timestamp)
                                            .monospace()
                                            .size(10.0)
                                            .color(theme::FG_TERTIARY),
                                    );
                                });
                            }
                        }
                    }
                    DetailTab::Autoruns => {
                        if autoruns_loading {
                            ui.spinner();
                            ui.label(egui::RichText::new("加载中...").color(theme::FG_TERTIARY));
                        } else if entry.exe.is_none() {
                            ui.label(egui::RichText::new("无可执行路径").color(theme::FG_TERTIARY));
                        } else if autoruns_items.is_empty() {
                            ui.label(egui::RichText::new("无关联持久化项").color(theme::FG_TERTIARY));
                        } else {
                            for item in &autoruns_items {
                                ui.horizontal(|ui| {
                                    // 分类 badge
                                    badge::badge(ui, &item.category, BadgeVariant::Default);
                                    // entry
                                    ui.label(egui::RichText::new(&item.entry).size(11.0).color(theme::FG_PRIMARY));
                                    // location
                                    ui.label(
                                        egui::RichText::new(&item.location)
                                            .monospace()
                                            .size(10.0)
                                            .color(theme::FG_TERTIARY),
                                    );
                                });
                            }
                        }
                    }
                }

                // 让编译器知道这些变量被使用了
                let _ = (ctx, rt, selected_pid);
            });
    }

    // ── 缓存 ──────────────────────────────────────────────

    fn get_filtered_sorted_items(&mut self) -> &[ProcessEntry] {
        if self.cache_dirty {
            self.cached_items = self.compute_filtered_sorted();
            self.cache_dirty = false;
        }
        &self.cached_items
    }

    fn get_cached_tree(&mut self) -> &[ProcessTreeNode] {
        if self.tree_cache_dirty {
            let items = self.get_filtered_sorted_items().to_vec();
            self.cached_tree = build_process_tree(&items);
            self.tree_cache_dirty = false;
        }
        &self.cached_tree
    }

    fn compute_filtered_sorted(&self) -> Vec<ProcessEntry> {
        let Some(ref snap) = self.snapshot else {
            return vec![];
        };

        let mut items: Vec<ProcessEntry> = snap
            .processes
            .iter()
            .filter(|e| match self.filter {
                FilterMode::All => true,
                FilterMode::Suspicious => e.is_suspicious,
            })
            .filter(|e| {
                if self.search.is_empty() {
                    return true;
                }
                let q = self.search.to_lowercase();
                e.name.to_lowercase().contains(&q)
                    || e.exe.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || e.pid.to_string().contains(&q)
                    || e.suspicious_reason.as_deref().unwrap_or("").to_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            // 默认排序：is_suspicious 优先，然后 pid 升序
            let cmp = match self.sort_column {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Ppid => a.ppid.cmp(&b.ppid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Path => a
                    .exe
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .cmp(&b.exe.as_deref().unwrap_or("").to_lowercase()),
                SortColumn::Suspicious => {
                    // 可疑优先
                    let sa = if a.is_suspicious { 0 } else { 1 };
                    let sb = if b.is_suspicious { 0 } else { 1 };
                    sa.cmp(&sb)
                }
            };
            match self.sort_dir {
                SortDir::Asc => cmp,
                SortDir::Desc => cmp.reverse(),
            }
        });

        items
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
}

// ── 树节点渲染 ────────────────────────────────────────────

fn render_tree_node(
    ui: &mut egui::Ui,
    node: &ProcessTreeNode,
    depth: usize,
    selected_pid: Option<u32>,
    expanded: &HashSet<u32>,
    icon_cache: &IconCache,
    clicked_pid: &mut Option<(u32, bool)>,
) {
    let has_children = !node.children.is_empty();
    let is_expanded = expanded.contains(&node.entry.pid);
    let is_selected = selected_pid == Some(node.entry.pid);
    let is_suspicious = node.entry.is_suspicious;

    // 选中行背景高亮
    let bg = if is_selected {
        theme::TABLE_ROW_SELECTED
    } else if is_suspicious {
        egui::Color32::from_rgba_premultiplied(0xca, 0x8a, 0x04, 15)
    } else {
        egui::Color32::TRANSPARENT
    };

    // 分配可点击的行区域
    let row_height = 20.0;
    let (rect, row_resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), egui::Sense::click());

    // 绘制背景
    if bg != egui::Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg);
    }

    // 在分配的区域内绘制内容
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(depth as f32 * 16.0 + 4.0);

            // 展开箭头
            if has_children {
                let arrow = if is_expanded { "▼" } else { "▶" };
                ui.label(egui::RichText::new(arrow).size(10.0).color(theme::FG_TERTIARY));
            } else {
                ui.add_space(12.0);
            }

            // orphan 标记
            if node.is_orphan {
                ui.label(egui::RichText::new("⚠").color(theme::SEMANTIC_WARNING).size(11.0))
                    .on_hover_text("父进程不在快照中");
            }

            // PID
            ui.label(
                egui::RichText::new(node.entry.pid.to_string())
                    .monospace()
                    .size(12.0)
                    .color(theme::FG_TERTIARY),
            );

            // 图标 + 进程名
            icon_cache.icon_or_placeholder(ui, node.entry.exe.as_deref(), 16.0);
            ui.add_space(4.0);
            let name_color = if is_suspicious {
                theme::SEMANTIC_WARNING
            } else if is_selected {
                theme::ACCENT
            } else {
                theme::FG_PRIMARY
            };
            ui.label(egui::RichText::new(&node.entry.name).color(name_color).size(13.0));

            // 可疑标记
            if is_suspicious {
                let reason = node.entry.suspicious_reason.as_deref().unwrap_or("");
                let resp = ui.label(egui::RichText::new("⚠").color(theme::SEMANTIC_WARNING).size(11.0));
                if !reason.is_empty() {
                    resp.on_hover_text(reason);
                }
            }
        });
    });

    // 行点击
    if row_resp.clicked() {
        *clicked_pid = Some((node.entry.pid, has_children));
    }

    // 递归渲染子节点
    if is_expanded && has_children {
        for child in &node.children {
            render_tree_node(ui, child, depth + 1, selected_pid, expanded, icon_cache, clicked_pid);
        }
    }
}

// ── 辅助函数 ──────────────────────────────────────────────

fn filter_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let btn = if active {
        egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE)).fill(theme::ACCENT)
    } else {
        egui::Button::new(egui::RichText::new(label).color(theme::FG_SECONDARY))
    };
    ui.add(btn).clicked()
}

fn tab_button(ui: &mut egui::Ui, label: &str, count: usize, active: bool) -> bool {
    let text = format!("{} ({})", label, count);
    let btn = if active {
        egui::Button::new(egui::RichText::new(&text).color(egui::Color32::WHITE)).fill(theme::ACCENT)
    } else {
        egui::Button::new(egui::RichText::new(&text).color(theme::FG_SECONDARY))
    };
    ui.add(btn).clicked()
}

fn ui_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).selectable(false))
}

fn cell_text(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui_label(ui, text)
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

fn format_endpoint(addr: &str, port: u16) -> String {
    let a = if addr.is_empty() || addr == "0.0.0.0" || addr == "::" {
        "*"
    } else {
        addr
    };
    let p = if port == 0 { "*".to_string() } else { port.to_string() };
    format!("{}:{}", a, p)
}

fn sysmon_event_badge_variant(event_type: &SysmonEventType) -> BadgeVariant {
    match event_type {
        SysmonEventType::ProcessCreate | SysmonEventType::NetworkConnect | SysmonEventType::Dns => BadgeVariant::Info,
        SysmonEventType::ProcessTerminate => BadgeVariant::Default,
        SysmonEventType::CreateRemoteThread | SysmonEventType::RawAccessRead => BadgeVariant::Danger,
        SysmonEventType::FileCreate | SysmonEventType::FileCreateTime | SysmonEventType::FileDelete => {
            BadgeVariant::Warning
        }
        _ => BadgeVariant::Default,
    }
}

fn sysmon_event_key_info(evt: &SysmonEvent) -> String {
    match evt.event_type {
        SysmonEventType::ProcessCreate => {
            // 显示目标进程
            if !evt.process_name.is_empty() {
                format!("→ {}", evt.process_name)
            } else {
                String::new()
            }
        }
        SysmonEventType::NetworkConnect => {
            // 显示远程地址
            let dest = evt.raw_data.get("Destination");
            let dest_port = evt.raw_data.get("DestinationPort");
            match (dest, dest_port) {
                (Some(d), Some(p)) => format!("→ {}:{}", d, p),
                (Some(d), None) => format!("→ {}", d),
                _ => String::new(),
            }
        }
        SysmonEventType::Dns => {
            if !evt.query_name.is_empty() {
                format!("→ {}", evt.query_name)
            } else {
                String::new()
            }
        }
        SysmonEventType::FileCreate | SysmonEventType::FileCreateTime | SysmonEventType::FileDelete => {
            let target = evt.raw_data.get("TargetFilename");
            target.cloned().unwrap_or_default()
        }
        _ => String::new(),
    }
}
