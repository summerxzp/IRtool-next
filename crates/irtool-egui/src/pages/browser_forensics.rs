use std::collections::HashSet;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use irtool_browser_forensics::{
    BrowserKind, BrowserProfile, DangerType, DownloadAttribution, DownloadInfo, ExtensionInfo, ExtensionInventory,
    HistoryAttribution, RecoveredTab, SessionRecoveryResult,
};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};
use crate::widgets::detail_row::detail_row;
use crate::widgets::table::{self, SortDir};

// ── Sub Tab ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SubTab {
    Extensions,
    History,
    Downloads,
    Tabs,
}

impl SubTab {
    fn label(self) -> &'static str {
        match self {
            SubTab::Extensions => "扩展清单",
            SubTab::History => "历史记录",
            SubTab::Downloads => "下载记录",
            SubTab::Tabs => "当前标签页",
        }
    }

    fn all() -> &'static [SubTab] {
        &[SubTab::Extensions, SubTab::History, SubTab::Downloads, SubTab::Tabs]
    }
}

// ── Sort Column ───────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExtSortColumn {
    Enabled,
    Name,
    Version,
    Risk,
    Permissions,
    InstallSource,
    InstallTime,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HistorySortColumn {
    VisitTime,
    Url,
    Title,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DownloadSortColumn {
    StartTime,
    Filename,
    DangerType,
    Size,
}

// ── Filter ────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExtStatusFilter {
    All,
    Enabled,
    Disabled,
}

// ── Data Loading Channel ──────────────────────────────────

enum BrowserForensicsData {
    Profiles(Vec<BrowserProfile>),
    Extensions(ExtensionInventory),
    History(HistoryAttribution),
    Downloads(DownloadAttribution),
    Tabs(SessionRecoveryResult),
}

// ── Page State ────────────────────────────────────────────

pub struct BrowserForensicsPageState {
    // Browser selection
    pub selected_browser: BrowserKind,
    pub selected_profile: Option<String>,
    pub profiles: Vec<BrowserProfile>,

    // Sub tab
    pub active_tab: SubTab,

    // Data
    pub extensions: Vec<ExtensionInfo>,
    pub history: Vec<HistoryEntry>,
    pub downloads: Vec<DownloadInfo>,
    pub tabs: Vec<RecoveredTab>,

    // Loading
    pub loading: bool,
    pub last_error: Option<String>,

    // Filters
    pub search: String,
    pub ext_status_filter: ExtStatusFilter,
    pub ext_risk_filter: HashSet<String>,

    // Sort
    pub ext_sort_column: ExtSortColumn,
    pub ext_sort_dir: SortDir,
    pub history_sort_column: HistorySortColumn,
    pub history_sort_dir: SortDir,
    pub download_sort_column: DownloadSortColumn,
    pub download_sort_dir: SortDir,

    // Detail
    pub selected_ext_index: Option<usize>,
    pub selected_history_index: Option<usize>,
    pub selected_download_index: Option<usize>,
    pub detail_visible: bool,

    // Data loading channel
    data_rx: std::sync::mpsc::Receiver<BrowserForensicsData>,
    data_tx: std::sync::mpsc::Sender<BrowserForensicsData>,

    // Cache
    cached_extensions: Vec<ExtensionInfo>,
    cached_history: Vec<HistoryEntry>,
    cached_downloads: Vec<DownloadInfo>,
    cache_dirty: bool,

    // Track which data has been loaded
    extensions_loaded: bool,
    history_loaded: bool,
    downloads_loaded: bool,
    tabs_loaded: bool,
}

/// Simplified history entry for display
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub visit_time: String,
    pub url: String,
    pub title: String,
}

impl Default for BrowserForensicsPageState {
    fn default() -> Self {
        let (data_tx, data_rx) = std::sync::mpsc::channel();
        Self {
            selected_browser: BrowserKind::Chrome,
            selected_profile: None,
            profiles: Vec::new(),
            active_tab: SubTab::Extensions,
            extensions: Vec::new(),
            history: Vec::new(),
            downloads: Vec::new(),
            tabs: Vec::new(),
            loading: false,
            last_error: None,
            search: String::new(),
            ext_status_filter: ExtStatusFilter::All,
            ext_risk_filter: HashSet::new(),
            ext_sort_column: ExtSortColumn::Name,
            ext_sort_dir: SortDir::Asc,
            history_sort_column: HistorySortColumn::VisitTime,
            history_sort_dir: SortDir::Desc,
            download_sort_column: DownloadSortColumn::StartTime,
            download_sort_dir: SortDir::Desc,
            selected_ext_index: None,
            selected_history_index: None,
            selected_download_index: None,
            detail_visible: false,
            data_rx,
            data_tx,
            cached_extensions: Vec::new(),
            cached_history: Vec::new(),
            cached_downloads: Vec::new(),
            cache_dirty: true,
            extensions_loaded: false,
            history_loaded: false,
            downloads_loaded: false,
            tabs_loaded: false,
        }
    }
}

impl BrowserForensicsPageState {
    // ── Rendering ──────────────────────────────────────────

    pub fn show(&mut self, ui: &mut egui::Ui, _ctx: &irtool_service::context::AppContext, rt: &tokio::runtime::Handle) {
        // Auto-load profiles on first show
        if self.profiles.is_empty() && !self.loading {
            let tx = self.data_tx.clone();
            let browser = self.selected_browser;
            self.loading = true;
            rt.spawn_blocking(move || {
                let profiles = irtool_browser_forensics::enumerate_profiles(browser);
                let _ = tx.send(BrowserForensicsData::Profiles(profiles));
            });
        }

        // Poll data from channel
        self.poll_data();

        // Browser + Profile selector
        self.render_browser_selector(ui, rt);
        ui.separator();

        // Sub tabs
        self.render_sub_tabs(ui);
        ui.separator();

        // Error banner
        if let Some(ref err) = self.last_error {
            let err = err.clone();
            if crate::widgets::banner::error_banner(ui, &err) {
                self.last_error = None;
            }
            ui.add_space(4.0);
        }

        // Content area
        match self.active_tab {
            SubTab::Extensions => self.render_extensions_tab(ui),
            SubTab::History => self.render_history_tab(ui, rt),
            SubTab::Downloads => self.render_downloads_tab(ui, rt),
            SubTab::Tabs => self.render_tabs_tab(ui, rt),
        }
    }

    // ── Data Polling ───────────────────────────────────────

    fn poll_data(&mut self) {
        while let Ok(data) = self.data_rx.try_recv() {
            self.loading = false;
            match data {
                BrowserForensicsData::Profiles(profiles) => {
                    self.profiles = profiles;
                    // Auto-select first profile
                    if self.selected_profile.is_none() && !self.profiles.is_empty() {
                        self.selected_profile = Some(self.profiles[0].name.clone());
                    }
                }
                BrowserForensicsData::Extensions(inventory) => {
                    self.extensions = inventory.extensions;
                    self.extensions_loaded = true;
                    self.cache_dirty = true;
                }
                BrowserForensicsData::History(attribution) => {
                    self.history = attribution
                        .recent_browser_activity
                        .iter()
                        .map(|a| HistoryEntry {
                            visit_time: format_rfc3339(&a.visit_time),
                            url: a.url.clone(),
                            title: a.title.clone(),
                        })
                        .collect();
                    self.history_loaded = true;
                    self.cache_dirty = true;
                }
                BrowserForensicsData::Downloads(attribution) => {
                    self.downloads = attribution.downloads;
                    self.downloads_loaded = true;
                    self.cache_dirty = true;
                }
                BrowserForensicsData::Tabs(result) => {
                    self.tabs = result.tabs;
                    self.tabs_loaded = true;
                }
            }
        }
    }

    // ── Browser Selector ───────────────────────────────────

    fn render_browser_selector(&mut self, ui: &mut egui::Ui, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            // Browser buttons
            for kind in BrowserKind::all() {
                let is_selected = self.selected_browser == *kind;
                let btn = if is_selected {
                    egui::Button::new(
                        egui::RichText::new(kind.display_name())
                            .color(theme::bg_primary())
                            .strong(),
                    )
                    .fill(theme::accent())
                } else {
                    egui::Button::new(egui::RichText::new(kind.display_name()).color(theme::fg_primary()))
                };
                if ui.add(btn).clicked() && !is_selected {
                    self.selected_browser = *kind;
                    self.selected_profile = None;
                    self.extensions.clear();
                    self.history.clear();
                    self.downloads.clear();
                    self.tabs.clear();
                    self.extensions_loaded = false;
                    self.history_loaded = false;
                    self.downloads_loaded = false;
                    self.tabs_loaded = false;
                    self.detail_visible = false;
                    self.selected_ext_index = None;
                    self.selected_history_index = None;
                    self.selected_download_index = None;
                    self.cache_dirty = true;
                    let tx = self.data_tx.clone();
                    let browser = self.selected_browser;
                    self.loading = true;
                    rt.spawn_blocking(move || {
                        let profiles = irtool_browser_forensics::enumerate_profiles(browser);
                        let _ = tx.send(BrowserForensicsData::Profiles(profiles));
                    });
                }
            }

            ui.separator();

            // Profile selector
            let selected_profile = self.selected_profile.clone();
            let profile_names: Vec<String> = self.profiles.iter().map(|p| p.name.clone()).collect();
            let selected_text = selected_profile.as_deref().unwrap_or("选择 Profile");
            let mut profile_changed: Option<String> = None;
            egui::ComboBox::from_id_salt("browser_profile_selector")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for name in &profile_names {
                        let is_sel = selected_profile.as_deref() == Some(name.as_str());
                        if ui.selectable_label(is_sel, name).clicked() {
                            profile_changed = Some(name.clone());
                        }
                    }
                });

            if let Some(name) = profile_changed {
                self.selected_profile = Some(name);
                self.extensions.clear();
                self.history.clear();
                self.downloads.clear();
                self.tabs.clear();
                self.extensions_loaded = false;
                self.history_loaded = false;
                self.downloads_loaded = false;
                self.tabs_loaded = false;
                self.detail_visible = false;
                self.cache_dirty = true;
                let profile = self.get_selected_profile();
                if let Some(profile) = profile {
                    let tx = self.data_tx.clone();
                    self.loading = true;
                    rt.spawn_blocking(move || {
                        let inventory = irtool_browser_forensics::scan_extensions(&profile);
                        let _ = tx.send(BrowserForensicsData::Extensions(inventory));
                    });
                }
            }

            // Loading indicator
            if self.loading {
                ui.spinner();
                ui.label(egui::RichText::new("加载中…").color(theme::accent()).size(12.0));
            }
        });
    }

    // ── Sub Tabs ───────────────────────────────────────────

    fn render_sub_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tab in SubTab::all() {
                let is_active = self.active_tab == *tab;
                let btn = if is_active {
                    egui::Button::new(
                        egui::RichText::new(tab.label())
                            .color(theme::accent())
                            .strong()
                            .underline(),
                    )
                    .frame(false)
                } else {
                    egui::Button::new(egui::RichText::new(tab.label()).color(theme::fg_secondary())).frame(false)
                };
                if ui.add(btn).clicked() {
                    self.active_tab = *tab;
                }
            }
        });
    }

    // ── Extensions Tab ─────────────────────────────────────

    fn render_extensions_tab(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        self.render_extensions_toolbar(ui);

        // Table
        self.render_extensions_table(ui);

        // Detail panel
        if self.detail_visible && self.selected_ext_index.is_some() {
            self.render_extension_detail(ui);
        }
    }

    fn render_extensions_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Status filter
            let prev_status = self.ext_status_filter;
            egui::ComboBox::from_id_salt("ext_status_filter")
                .selected_text(ext_status_filter_label(self.ext_status_filter))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ext_status_filter, ExtStatusFilter::All, "全部");
                    ui.selectable_value(&mut self.ext_status_filter, ExtStatusFilter::Enabled, "已启用");
                    ui.selectable_value(&mut self.ext_status_filter, ExtStatusFilter::Disabled, "已禁用");
                });
            if prev_status != self.ext_status_filter {
                self.cache_dirty = true;
            }

            // Risk filter
            let risk_flags = self.collect_risk_flags();
            if !risk_flags.is_empty() {
                let risk_label = if self.ext_risk_filter.is_empty() {
                    "风险标注".to_string()
                } else {
                    format!("风险 ({})", self.ext_risk_filter.len())
                };
                egui::ComboBox::from_id_salt("ext_risk_filter")
                    .selected_text(&risk_label)
                    .show_ui(ui, |ui| {
                        for flag in &risk_flags {
                            let mut checked = self.ext_risk_filter.contains(flag);
                            let label = risk_flag_label(flag);
                            if ui.checkbox(&mut checked, label).changed() {
                                if checked {
                                    self.ext_risk_filter.insert(flag.clone());
                                } else {
                                    self.ext_risk_filter.remove(flag);
                                }
                                self.cache_dirty = true;
                            }
                        }
                    });
            }

            ui.separator();

            // Search
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(200.0)
                    .hint_text("名称 / ID"),
            );
            if search_resp.changed() {
                self.cache_dirty = true;
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true;
            }

            ui.add_space((ui.available_width() - 80.0).max(0.0));

            // Count
            let filtered = self.get_filtered_extensions().len();
            let total = self.extensions.len();
            ui.label(
                egui::RichText::new(format!("{}/{}", filtered, total))
                    .color(theme::fg_tertiary())
                    .size(11.0),
            );
        });
    }

    fn render_extensions_table(&mut self, ui: &mut egui::Ui) {
        let items = self.get_filtered_extensions().to_vec();
        let sc = self.ext_sort_column;
        let sd = self.ext_sort_dir;
        let sel_idx = self.selected_ext_index;

        if items.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.extensions.is_empty() {
                    if self.extensions_loaded {
                        ui.label(egui::RichText::new("未发现扩展").color(theme::fg_secondary()));
                    } else if self.selected_profile.is_some() {
                        ui.label(egui::RichText::new("选择 Profile 后加载扩展").color(theme::fg_secondary()));
                    } else {
                        ui.label(egui::RichText::new("请先选择浏览器和 Profile").color(theme::fg_secondary()));
                    }
                } else {
                    ui.label(egui::RichText::new("没有匹配当前过滤条件的扩展").color(theme::fg_tertiary()));
                }
            });
            return;
        }

        let mut sort_toggle: Option<ExtSortColumn> = None;
        let mut row_click: Option<Option<usize>> = None;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(40.0).clip(true)) // 启用
            .column(Column::initial(160.0).clip(true).resizable(true)) // 名称
            .column(Column::initial(60.0).clip(true)) // 版本
            .column(Column::initial(140.0).clip(true)) // ID
            .column(Column::initial(120.0).clip(true)) // 风险标注
            .column(Column::initial(50.0).clip(true)) // 权限数
            .column(Column::initial(80.0).clip(true)) // 安装来源
            .column(Column::initial(140.0).clip(true)); // 安装时间

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    if table::sortable_header(ui, "启用", sc == ExtSortColumn::Enabled, sd) {
                        sort_toggle = Some(ExtSortColumn::Enabled);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "名称", sc == ExtSortColumn::Name, sd) {
                        sort_toggle = Some(ExtSortColumn::Name);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "版本", sc == ExtSortColumn::Version, sd) {
                        sort_toggle = Some(ExtSortColumn::Version);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "ID", sc == ExtSortColumn::Risk, sd) {
                        sort_toggle = Some(ExtSortColumn::Risk);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "风险", sc == ExtSortColumn::Risk, sd) {
                        sort_toggle = Some(ExtSortColumn::Risk);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "权限", sc == ExtSortColumn::Permissions, sd) {
                        sort_toggle = Some(ExtSortColumn::Permissions);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "来源", sc == ExtSortColumn::InstallSource, sd) {
                        sort_toggle = Some(ExtSortColumn::InstallSource);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "安装时间", sc == ExtSortColumn::InstallTime, sd) {
                        sort_toggle = Some(ExtSortColumn::InstallTime);
                    }
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let idx = row.index();
                    let item = &items[idx];
                    let is_selected = sel_idx == Some(idx);

                    if is_selected {
                        row.set_selected(true);
                    }

                    // Enabled
                    row.col(|ui| {
                        let mut checked = item.enabled;
                        ui.add_enabled(false, egui::Checkbox::new(&mut checked, ""));
                    });
                    // Name
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.name).color(theme::fg_primary()).strong());
                    });
                    // Version
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.version).color(theme::fg_secondary()).size(11.0));
                    });
                    // ID
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&item.id)
                                    .font(egui::FontId::monospace(10.0))
                                    .color(theme::fg_tertiary()),
                            )
                            .truncate(),
                        );
                        resp.on_hover_text(&item.id);
                    });
                    // Risk flags
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            for flag in &item.risk_flags {
                                let variant = risk_flag_badge_variant(flag);
                                badge::badge(ui, risk_flag_label(flag), variant);
                            }
                            if item.risk_flags.is_empty() {
                                ui.label(egui::RichText::new("—").color(theme::fg_tertiary()).size(11.0));
                            }
                        });
                    });
                    // Permissions count
                    row.col(|ui| {
                        let count = item.permissions.len() + item.host_permissions.len();
                        ui.label(egui::RichText::new(count.to_string()).color(theme::fg_secondary()));
                    });
                    // Install source
                    row.col(|ui| {
                        let source = item.install_source.as_deref().unwrap_or("—");
                        ui.label(egui::RichText::new(source).color(theme::fg_secondary()).size(11.0));
                    });
                    // Install time
                    row.col(|ui| {
                        let time = item
                            .install_time
                            .as_deref()
                            .map(format_rfc3339)
                            .unwrap_or_else(|| "—".to_string());
                        ui.label(egui::RichText::new(&time).color(theme::fg_tertiary()).size(11.0));
                    });

                    // Row click
                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            row_click = Some(None);
                        } else {
                            // Find the index in the original extensions list
                            let orig_idx = self.extensions.iter().position(|e| e.id == item.id);
                            row_click = Some(orig_idx);
                        }
                    }
                });
            });

        if let Some(idx) = row_click {
            self.selected_ext_index = idx;
            self.detail_visible = idx.is_some();
            if idx.is_none() {
                self.selected_ext_index = None;
            }
        }

        if let Some(col) = sort_toggle {
            self.toggle_ext_sort(col);
        }
    }

    fn render_extension_detail(&mut self, ui: &mut egui::Ui) {
        let idx = self.selected_ext_index.unwrap();
        let item = match self.extensions.get(idx) {
            Some(i) => i.clone(),
            None => {
                self.detail_visible = false;
                return;
            }
        };

        egui::ScrollArea::vertical()
            .id_salt("browser_ext_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        if ui
                            .add(egui::Button::new(egui::RichText::new("×").size(16.0)).frame(false))
                            .clicked()
                        {
                            self.detail_visible = false;
                            self.selected_ext_index = None;
                        }
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&item.name)
                                    .color(theme::fg_primary())
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                if item.enabled {
                                    badge::badge(ui, "已启用", BadgeVariant::Success);
                                } else {
                                    badge::badge(ui, "已禁用", BadgeVariant::Default);
                                }
                                ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                ui.label(egui::RichText::new(&item.version).color(theme::fg_tertiary()));
                                if !item.risk_flags.is_empty() {
                                    ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                    for flag in &item.risk_flags {
                                        let variant = risk_flag_badge_variant(flag);
                                        badge::badge(ui, risk_flag_label(flag), variant);
                                    }
                                }
                            });
                        });
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Detail rows
                detail_row(ui, "ID", Some(&item.id), true);
                detail_row(ui, "版本", Some(&item.version), false);
                detail_row(ui, "描述", item.description.as_deref(), false);
                detail_row(ui, "路径", Some(&item.path.display().to_string()), true);
                detail_row(ui, "安装来源", item.install_source.as_deref(), false);
                detail_row(
                    ui,
                    "安装时间",
                    item.install_time.as_deref().map(format_rfc3339).as_deref(),
                    false,
                );
                detail_row(ui, "更新 URL", item.update_url.as_deref(), true);
                detail_row(
                    ui,
                    "默认安装",
                    item.was_installed_by_default.map(|b| if b { "是" } else { "否" }),
                    false,
                );
                detail_row(
                    ui,
                    "Content Scripts",
                    Some(if item.has_content_scripts { "是" } else { "否" }),
                    false,
                );
                detail_row(
                    ui,
                    "Background",
                    Some(if item.has_background { "是" } else { "否" }),
                    false,
                );
                detail_row(
                    ui,
                    "Preferences 篡改",
                    Some(if item.preferences_tampered { "是" } else { "否" }),
                    false,
                );

                // Permissions list
                if !item.permissions.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("权限列表")
                            .color(theme::fg_tertiary())
                            .size(11.0)
                            .strong(),
                    );
                    for perm in &item.permissions {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(format!("• {}", perm))
                                    .font(egui::FontId::monospace(10.0))
                                    .color(theme::fg_secondary())
                                    .size(11.0),
                            );
                        });
                    }
                }

                // Host permissions
                if !item.host_permissions.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Host Permissions")
                            .color(theme::fg_tertiary())
                            .size(11.0)
                            .strong(),
                    );
                    for perm in &item.host_permissions {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(format!("• {}", perm))
                                    .font(egui::FontId::monospace(10.0))
                                    .color(theme::fg_secondary())
                                    .size(11.0),
                            );
                        });
                    }
                }

                // IOC matches
                if !item.ioc_matches.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("IOC 匹配")
                            .color(theme::semantic_danger())
                            .size(11.0)
                            .strong(),
                    );
                    for m in &item.ioc_matches {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            badge::badge(ui, &m.ioc_type, BadgeVariant::Danger);
                            ui.label(
                                egui::RichText::new(&m.value)
                                    .font(egui::FontId::monospace(10.0))
                                    .color(theme::fg_secondary())
                                    .size(11.0),
                            );
                            ui.label(egui::RichText::new(&m.value).color(theme::fg_tertiary()).size(10.0));
                        });
                    }
                }
            });
    }

    // ── History Tab ────────────────────────────────────────

    fn render_history_tab(&mut self, ui: &mut egui::Ui, rt: &tokio::runtime::Handle) {
        // Load on demand
        if !self.history_loaded && self.selected_profile.is_some() {
            self.load_history(rt);
        }

        // Toolbar
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(200.0)
                    .hint_text("URL / 标题"),
            );
            if search_resp.changed() {
                self.cache_dirty = true;
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true;
            }

            ui.add_space((ui.available_width() - 80.0).max(0.0));
            ui.label(
                egui::RichText::new(format!("{} 条", self.get_filtered_history().len()))
                    .color(theme::fg_tertiary())
                    .size(11.0),
            );
        });

        let items = self.get_filtered_history().to_vec();
        let sc = self.history_sort_column;
        let sd = self.history_sort_dir;
        let sel_idx = self.selected_history_index;

        if items.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.history.is_empty() {
                    if self.history_loaded {
                        ui.label(egui::RichText::new("无历史记录").color(theme::fg_secondary()));
                    } else {
                        ui.label(egui::RichText::new("选择 Profile 后加载历史记录").color(theme::fg_secondary()));
                    }
                } else {
                    ui.label(egui::RichText::new("没有匹配的记录").color(theme::fg_tertiary()));
                }
            });
            return;
        }

        let mut sort_toggle: Option<HistorySortColumn> = None;
        let mut row_click: Option<Option<usize>> = None;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(140.0).clip(true)) // 访问时间
            .column(Column::initial(400.0).clip(true).resizable(true)) // URL
            .column(Column::remainder().clip(true).resizable(true)); // 标题

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    if table::sortable_header(ui, "访问时间", sc == HistorySortColumn::VisitTime, sd) {
                        sort_toggle = Some(HistorySortColumn::VisitTime);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "URL", sc == HistorySortColumn::Url, sd) {
                        sort_toggle = Some(HistorySortColumn::Url);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "标题", sc == HistorySortColumn::Title, sd) {
                        sort_toggle = Some(HistorySortColumn::Title);
                    }
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let idx = row.index();
                    let item = &items[idx];
                    let is_selected = sel_idx == Some(idx);

                    if is_selected {
                        row.set_selected(true);
                    }

                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(&item.visit_time)
                                .color(theme::fg_tertiary())
                                .size(11.0),
                        );
                    });
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&item.url)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_secondary()),
                            )
                            .truncate(),
                        );
                        resp.on_hover_text(&item.url);
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.title).color(theme::fg_primary()));
                    });

                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            row_click = Some(None);
                        } else {
                            row_click = Some(Some(idx));
                        }
                    }
                });
            });

        if let Some(idx) = row_click {
            self.selected_history_index = idx;
            self.detail_visible = idx.is_some();
        }

        if let Some(col) = sort_toggle {
            self.toggle_history_sort(col);
        }

        // Detail panel
        if self.detail_visible {
            if let Some(idx) = self.selected_history_index {
                if let Some(item) = self.history.get(idx) {
                    self.render_history_detail(ui, item);
                }
            }
        }
    }

    fn render_history_detail(&self, ui: &mut egui::Ui, item: &HistoryEntry) {
        egui::ScrollArea::vertical()
            .id_salt("browser_history_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&item.title)
                                    .color(theme::fg_primary())
                                    .strong()
                                    .size(13.0),
                            );
                        });
                    });
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                detail_row(ui, "访问时间", Some(&item.visit_time), false);
                detail_row(ui, "URL", Some(&item.url), true);
                detail_row(ui, "标题", Some(&item.title), false);
            });
    }

    // ── Downloads Tab ──────────────────────────────────────

    fn render_downloads_tab(&mut self, ui: &mut egui::Ui, rt: &tokio::runtime::Handle) {
        // Load on demand
        if !self.downloads_loaded && self.selected_profile.is_some() {
            self.load_downloads(rt);
        }

        // Toolbar
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("搜索:").size(14.0));
            let search_resp = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(200.0)
                    .hint_text("文件名 / URL"),
            );
            if search_resp.changed() {
                self.cache_dirty = true;
            }
            if !self.search.is_empty() && ui.small_button("×").clicked() {
                self.search.clear();
                self.cache_dirty = true;
            }

            ui.add_space((ui.available_width() - 80.0).max(0.0));
            ui.label(
                egui::RichText::new(format!("{} 条", self.get_filtered_downloads().len()))
                    .color(theme::fg_tertiary())
                    .size(11.0),
            );
        });

        let items = self.get_filtered_downloads().to_vec();
        let sc = self.download_sort_column;
        let sd = self.download_sort_dir;
        let sel_idx = self.selected_download_index;

        if items.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.downloads.is_empty() {
                    if self.downloads_loaded {
                        ui.label(egui::RichText::new("无下载记录").color(theme::fg_secondary()));
                    } else {
                        ui.label(egui::RichText::new("选择 Profile 后加载下载记录").color(theme::fg_secondary()));
                    }
                } else {
                    ui.label(egui::RichText::new("没有匹配的记录").color(theme::fg_tertiary()));
                }
            });
            return;
        }

        let mut sort_toggle: Option<DownloadSortColumn> = None;
        let mut row_click: Option<Option<usize>> = None;

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(140.0).clip(true)) // 下载时间
            .column(Column::initial(140.0).clip(true).resizable(true)) // 文件名
            .column(Column::initial(200.0).clip(true).resizable(true)) // 来源 URL
            .column(Column::initial(140.0).clip(true)) // Referrer
            .column(Column::initial(80.0).clip(true)) // 安全判定
            .column(Column::initial(70.0).clip(true)) // 大小
            .column(Column::initial(50.0).clip(true)); // 已打开

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    if table::sortable_header(ui, "下载时间", sc == DownloadSortColumn::StartTime, sd) {
                        sort_toggle = Some(DownloadSortColumn::StartTime);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "文件名", sc == DownloadSortColumn::Filename, sd) {
                        sort_toggle = Some(DownloadSortColumn::Filename);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "来源 URL", sc == DownloadSortColumn::DangerType, sd) {
                        sort_toggle = Some(DownloadSortColumn::DangerType);
                    }
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("Referrer").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "安全", sc == DownloadSortColumn::DangerType, sd) {
                        sort_toggle = Some(DownloadSortColumn::DangerType);
                    }
                });
                header.col(|ui| {
                    if table::sortable_header(ui, "大小", sc == DownloadSortColumn::Size, sd) {
                        sort_toggle = Some(DownloadSortColumn::Size);
                    }
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("已打开").color(theme::fg_secondary()).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, items.len(), |mut row| {
                    let idx = row.index();
                    let item = &items[idx];
                    let is_selected = sel_idx == Some(idx);

                    if is_selected {
                        row.set_selected(true);
                    }

                    // Download time
                    row.col(|ui| {
                        let time = item
                            .start_time
                            .as_deref()
                            .map(format_rfc3339)
                            .unwrap_or_else(|| "—".to_string());
                        ui.label(egui::RichText::new(&time).color(theme::fg_tertiary()).size(11.0));
                    });
                    // Filename
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&item.filename).color(theme::fg_primary()));
                    });
                    // Source URL
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&item.download_url)
                                    .font(egui::FontId::monospace(10.0))
                                    .color(theme::fg_secondary()),
                            )
                            .truncate(),
                        );
                        resp.on_hover_text(&item.download_url);
                    });
                    // Referrer
                    row.col(|ui| {
                        let ref_text = item.referrer.as_deref().unwrap_or("—");
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(ref_text)
                                    .font(egui::FontId::monospace(10.0))
                                    .color(theme::fg_tertiary()),
                            )
                            .truncate(),
                        );
                        if item.referrer.is_some() {
                            resp.on_hover_text(ref_text);
                        }
                    });
                    // Danger type
                    row.col(|ui| {
                        let (label, variant) = danger_type_badge(&item.danger_type);
                        badge::badge(ui, label, variant);
                    });
                    // Size
                    row.col(|ui| {
                        let size = item
                            .total_bytes
                            .map(|b| format_bytes(b as u64))
                            .unwrap_or_else(|| "—".to_string());
                        ui.label(egui::RichText::new(&size).color(theme::fg_secondary()).size(11.0));
                    });
                    // Opened
                    row.col(|ui| {
                        if item.opened {
                            ui.label(egui::RichText::new("是").color(theme::semantic_success()).size(11.0));
                        } else {
                            ui.label(egui::RichText::new("否").color(theme::fg_tertiary()).size(11.0));
                        }
                    });

                    let row_resp = row.response();
                    if row_resp.clicked() {
                        if is_selected {
                            row_click = Some(None);
                        } else {
                            row_click = Some(Some(idx));
                        }
                    }
                });
            });

        if let Some(idx) = row_click {
            self.selected_download_index = idx;
            self.detail_visible = idx.is_some();
        }

        if let Some(col) = sort_toggle {
            self.toggle_download_sort(col);
        }

        // Detail panel
        if self.detail_visible {
            if let Some(idx) = self.selected_download_index {
                if let Some(item) = self.downloads.get(idx) {
                    self.render_download_detail(ui, item);
                }
            }
        }
    }

    fn render_download_detail(&self, ui: &mut egui::Ui, item: &DownloadInfo) {
        egui::ScrollArea::vertical()
            .id_salt("browser_download_detail_scroll")
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.allocate_space(egui::vec2(8.0, 0.0));
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&item.filename)
                                    .color(theme::fg_primary())
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                let (label, variant) = danger_type_badge(&item.danger_type);
                                badge::badge(ui, label, variant);
                                if item.opened {
                                    ui.label(egui::RichText::new("·").color(theme::fg_tertiary()));
                                    ui.label(egui::RichText::new("已打开").color(theme::semantic_warning()).size(11.0));
                                }
                            });
                        });
                    });
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                detail_row(ui, "本地路径", Some(&item.local_path), true);
                detail_row(ui, "下载 URL", Some(&item.download_url), true);
                detail_row(ui, "Referrer", item.referrer.as_deref(), true);
                detail_row(
                    ui,
                    "开始时间",
                    item.start_time.as_deref().map(format_rfc3339).as_deref(),
                    false,
                );
                detail_row(
                    ui,
                    "完成时间",
                    item.end_time.as_deref().map(format_rfc3339).as_deref(),
                    false,
                );
                detail_row(
                    ui,
                    "大小",
                    item.total_bytes.map(|b| format_bytes(b as u64)).as_deref(),
                    false,
                );
                let (danger_label, _) = danger_type_badge(&item.danger_type);
                detail_row(ui, "安全判定", Some(danger_label), false);
                detail_row(ui, "中断原因", item.interrupt_reason.as_deref(), false);
            });
    }

    // ── Tabs Tab ───────────────────────────────────────────

    fn render_tabs_tab(&mut self, ui: &mut egui::Ui, rt: &tokio::runtime::Handle) {
        // Load on demand
        if !self.tabs_loaded && self.selected_profile.is_some() {
            self.load_tabs(rt);
        }

        if self.tabs.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.tabs_loaded {
                    ui.label(egui::RichText::new("无打开的标签页").color(theme::fg_secondary()));
                } else {
                    ui.label(egui::RichText::new("选择 Profile 后加载标签页").color(theme::fg_secondary()));
                }
            });
            return;
        }

        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0)
            .column(Column::initial(40.0).clip(true)) // 序号
            .column(Column::initial(400.0).clip(true).resizable(true)) // URL
            .column(Column::remainder().clip(true).resizable(true)); // 标题

        table
            .header(theme::TABLE_HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    ui.label(egui::RichText::new("#").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("URL").color(theme::fg_secondary()).size(12.0));
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("标题").color(theme::fg_secondary()).size(12.0));
                });
            })
            .body(|body| {
                body.rows(theme::TABLE_ROW_HEIGHT, self.tabs.len(), |mut row| {
                    let idx = row.index();
                    let tab = &self.tabs[idx];

                    row.col(|ui| {
                        let idx_str = tab
                            .tab_index
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| (idx + 1).to_string());
                        ui.label(egui::RichText::new(&idx_str).color(theme::fg_tertiary()));
                    });
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&tab.url)
                                    .font(egui::FontId::monospace(11.0))
                                    .color(theme::fg_secondary()),
                            )
                            .truncate(),
                        );
                        resp.on_hover_text(&tab.url);
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&tab.title).color(theme::fg_primary()));
                    });
                });
            });
    }

    // ── Data Loading ───────────────────────────────────────

    fn load_history(&mut self, rt: &tokio::runtime::Handle) {
        let profile = match self.get_selected_profile() {
            Some(p) => p,
            None => return,
        };
        let tx = self.data_tx.clone();
        self.loading = true;
        self.history_loaded = true; // Mark as loading to prevent re-trigger
        rt.spawn_blocking(move || {
            // Use current time as target for recent history
            let target_time = chrono::Utc::now();
            let attribution = irtool_browser_forensics::attribute_history(&profile, target_time, "");
            let _ = tx.send(BrowserForensicsData::History(attribution));
        });
    }

    fn load_downloads(&mut self, rt: &tokio::runtime::Handle) {
        let profile = match self.get_selected_profile() {
            Some(p) => p,
            None => return,
        };
        let tx = self.data_tx.clone();
        self.loading = true;
        self.downloads_loaded = true;
        rt.spawn_blocking(move || {
            let attribution = irtool_browser_forensics::scan_downloads(&profile);
            let _ = tx.send(BrowserForensicsData::Downloads(attribution));
        });
    }

    fn load_tabs(&mut self, rt: &tokio::runtime::Handle) {
        let profile = match self.get_selected_profile() {
            Some(p) => p,
            None => return,
        };
        let tx = self.data_tx.clone();
        self.loading = true;
        self.tabs_loaded = true;
        rt.spawn_blocking(move || {
            let result = irtool_browser_forensics::recover_tabs(&profile);
            let _ = tx.send(BrowserForensicsData::Tabs(result));
        });
    }

    fn get_selected_profile(&self) -> Option<BrowserProfile> {
        let name = self.selected_profile.as_ref()?;
        self.profiles.iter().find(|p| &p.name == name).cloned()
    }

    // ── Filtering & Sorting ────────────────────────────────

    fn get_filtered_extensions(&mut self) -> &[ExtensionInfo] {
        if self.cache_dirty {
            self.cached_extensions = self.compute_filtered_extensions();
            self.cache_dirty = false;
        }
        &self.cached_extensions
    }

    fn compute_filtered_extensions(&self) -> Vec<ExtensionInfo> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<ExtensionInfo> = self
            .extensions
            .iter()
            .filter(|item| match self.ext_status_filter {
                ExtStatusFilter::All => true,
                ExtStatusFilter::Enabled => item.enabled,
                ExtStatusFilter::Disabled => !item.enabled,
            })
            .filter(|item| {
                if self.ext_risk_filter.is_empty() {
                    return true;
                }
                item.risk_flags.iter().any(|f| self.ext_risk_filter.contains(f))
            })
            .filter(|item| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!("{} {} {}", item.name, item.id, item.version).to_lowercase();
                blob.contains(&q)
            })
            .cloned()
            .collect();

        let sc = self.ext_sort_column;
        let sd = self.ext_sort_dir;
        out.sort_by(|a, b| {
            let ord = match sc {
                ExtSortColumn::Enabled => a.enabled.cmp(&b.enabled),
                ExtSortColumn::Name => a.name.cmp(&b.name),
                ExtSortColumn::Version => a.version.cmp(&b.version),
                ExtSortColumn::Risk => a.risk_flags.len().cmp(&b.risk_flags.len()),
                ExtSortColumn::Permissions => (a.permissions.len() + a.host_permissions.len())
                    .cmp(&(b.permissions.len() + b.host_permissions.len())),
                ExtSortColumn::InstallSource => a
                    .install_source
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.install_source.as_deref().unwrap_or("")),
                ExtSortColumn::InstallTime => a
                    .install_time
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.install_time.as_deref().unwrap_or("")),
            };
            if sd == SortDir::Desc {
                ord.reverse()
            } else {
                ord
            }
        });

        out
    }

    fn get_filtered_history(&mut self) -> &[HistoryEntry] {
        if self.cache_dirty {
            self.cached_history = self.compute_filtered_history();
            self.cache_dirty = false;
        }
        &self.cached_history
    }

    fn compute_filtered_history(&self) -> Vec<HistoryEntry> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<HistoryEntry> = self
            .history
            .iter()
            .filter(|item| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!("{} {}", item.url, item.title).to_lowercase();
                blob.contains(&q)
            })
            .cloned()
            .collect();

        let sc = self.history_sort_column;
        let sd = self.history_sort_dir;
        out.sort_by(|a, b| {
            let ord = match sc {
                HistorySortColumn::VisitTime => a.visit_time.cmp(&b.visit_time),
                HistorySortColumn::Url => a.url.cmp(&b.url),
                HistorySortColumn::Title => a.title.cmp(&b.title),
            };
            if sd == SortDir::Desc {
                ord.reverse()
            } else {
                ord
            }
        });

        out
    }

    fn get_filtered_downloads(&mut self) -> &[DownloadInfo] {
        if self.cache_dirty {
            self.cached_downloads = self.compute_filtered_downloads();
            self.cache_dirty = false;
        }
        &self.cached_downloads
    }

    fn compute_filtered_downloads(&self) -> Vec<DownloadInfo> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<DownloadInfo> = self
            .downloads
            .iter()
            .filter(|item| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!("{} {} {}", item.filename, item.download_url, item.local_path).to_lowercase();
                blob.contains(&q)
            })
            .cloned()
            .collect();

        let sc = self.download_sort_column;
        let sd = self.download_sort_dir;
        out.sort_by(|a, b| {
            let ord = match sc {
                DownloadSortColumn::StartTime => a
                    .start_time
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.start_time.as_deref().unwrap_or("")),
                DownloadSortColumn::Filename => a.filename.cmp(&b.filename),
                DownloadSortColumn::DangerType => {
                    danger_type_sort_key(&a.danger_type).cmp(&danger_type_sort_key(&b.danger_type))
                }
                DownloadSortColumn::Size => a.total_bytes.unwrap_or(0).cmp(&b.total_bytes.unwrap_or(0)),
            };
            if sd == SortDir::Desc {
                ord.reverse()
            } else {
                ord
            }
        });

        out
    }

    fn collect_risk_flags(&self) -> Vec<String> {
        let mut set = HashSet::new();
        let mut out = Vec::new();
        for ext in &self.extensions {
            for flag in &ext.risk_flags {
                if set.insert(flag.clone()) {
                    out.push(flag.clone());
                }
            }
        }
        out.sort();
        out
    }

    // ── Sort Toggle ────────────────────────────────────────

    fn toggle_ext_sort(&mut self, col: ExtSortColumn) {
        if self.ext_sort_column == col {
            self.ext_sort_dir = self.ext_sort_dir.toggle();
        } else {
            self.ext_sort_column = col;
            self.ext_sort_dir = SortDir::Asc;
        }
        self.cache_dirty = true;
    }

    fn toggle_history_sort(&mut self, col: HistorySortColumn) {
        if self.history_sort_column == col {
            self.history_sort_dir = self.history_sort_dir.toggle();
        } else {
            self.history_sort_column = col;
            self.history_sort_dir = SortDir::Asc;
        }
        self.cache_dirty = true;
    }

    fn toggle_download_sort(&mut self, col: DownloadSortColumn) {
        if self.download_sort_column == col {
            self.download_sort_dir = self.download_sort_dir.toggle();
        } else {
            self.download_sort_column = col;
            self.download_sort_dir = SortDir::Asc;
        }
        self.cache_dirty = true;
    }
}

// ── Helper Functions ──────────────────────────────────────

fn ext_status_filter_label(f: ExtStatusFilter) -> &'static str {
    match f {
        ExtStatusFilter::All => "全部",
        ExtStatusFilter::Enabled => "已启用",
        ExtStatusFilter::Disabled => "已禁用",
    }
}

fn risk_flag_label(flag: &str) -> &str {
    match flag {
        "high_privilege_combo" => "高危组合",
        "broad_host_access" => "广泛访问",
        "content_script_inject" => "内容注入",
        "side_loaded" => "侧载",
        "unknown_update_url" => "未知来源",
        "preferences_tampered" => "篡改",
        "recently_installed" => "近期安装",
        _ => flag,
    }
}

fn risk_flag_badge_variant(flag: &str) -> BadgeVariant {
    match flag {
        "high_privilege_combo" => BadgeVariant::Danger,
        "broad_host_access" => BadgeVariant::Warning,
        _ => BadgeVariant::Warning,
    }
}

fn danger_type_badge(dt: &DangerType) -> (&str, BadgeVariant) {
    match dt {
        DangerType::NotDangerous => ("安全", BadgeVariant::Success),
        DangerType::DangerousUrl => ("危险URL", BadgeVariant::Danger),
        DangerType::DangerousContent => ("危险内容", BadgeVariant::Danger),
        DangerType::DangerousHost => ("危险主机", BadgeVariant::Danger),
        DangerType::UncommonUrl => ("不常见", BadgeVariant::Warning),
        DangerType::PotentiallyUnwanted => ("潜在风险", BadgeVariant::Warning),
        DangerType::AllowlistedByPolicy => ("策略白名单", BadgeVariant::Info),
        DangerType::Unknown => ("未知", BadgeVariant::Default),
    }
}

fn danger_type_sort_key(dt: &DangerType) -> u8 {
    match dt {
        DangerType::DangerousUrl => 0,
        DangerType::DangerousContent => 1,
        DangerType::DangerousHost => 2,
        DangerType::PotentiallyUnwanted => 3,
        DangerType::UncommonUrl => 4,
        DangerType::NotDangerous => 5,
        DangerType::AllowlistedByPolicy => 6,
        DangerType::Unknown => 7,
    }
}

/// Format RFC3339 timestamp to project convention: YYYY/MM/DD HH:MM:SS
fn format_rfc3339(s: &str) -> String {
    // Try parsing RFC3339 and reformat
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        let cst = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
        dt.with_timezone(&cst).format("%Y/%m/%d %H:%M:%S").to_string()
    } else {
        s.to_string()
    }
}

/// Format bytes as human-readable
fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let i = (bytes.ilog2() / 10) as usize;
    let i = i.min(UNITS.len() - 1);
    let val = bytes as f64 / (1024_u64.pow(i as u32) as f64);
    let val_str = if val >= 100.0 {
        format!("{:.0}", val)
    } else {
        format!("{:.1}", val)
    };
    format!("{} {}", val_str, UNITS[i])
}
