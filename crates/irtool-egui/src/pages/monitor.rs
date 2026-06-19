use std::time::Instant;

use eframe::egui;
use irtool_service::context::AppContext;
use irtool_service::services::monitor::MonitorService;
use irtool_service::types::{MonitorConfig, RuntimeTelemetry};

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};

// ── Event type catalogue ──────────────────────────────────

const EVENT_TYPES: &[(&str, &str)] = &[
    ("dns", "DNS 查询"),
    ("network_connect", "网络连接"),
    ("create_remote_thread", "远程线程"),
    ("file_create", "文件创建"),
    ("tls_sni", "TLS SNI"),
    ("dns_pcap", "DNS 抓包"),
];

// ── Async → UI refresh payload ────────────────────────────

#[derive(Default)]
pub struct MonitorRefresh {
    pub config: Option<MonitorConfig>,
    pub telemetry: Option<RuntimeTelemetry>,
    pub is_background: Option<bool>,
    pub event_count: Option<u64>,
    pub db_size: Option<u64>,
}

// ── MonitorPageState ─────────────────────────────────────

pub struct MonitorPageState {
    pub config: MonitorConfig,
    pub telemetry: Option<RuntimeTelemetry>,
    pub is_background: bool,
    pub event_count: u64,
    pub db_size: u64,
    pub saving: bool,
    pub confirm_dialog_open: bool,
    pub dns_sni_dialog_open: bool,
    pub last_error: Option<String>,
    last_poll: Option<Instant>,
    cmdline_enrich: u32,
    pub refresh_tx: Option<std::sync::mpsc::Sender<MonitorRefresh>>,
}

impl Default for MonitorPageState {
    fn default() -> Self {
        Self {
            config: MonitorConfig::default(),
            telemetry: None,
            is_background: false,
            event_count: 0,
            db_size: 0,
            saving: false,
            confirm_dialog_open: false,
            dns_sni_dialog_open: false,
            last_error: None,
            last_poll: None,
            cmdline_enrich: 0,
            refresh_tx: None,
        }
    }
}

impl MonitorPageState {
    /// Apply an async refresh payload.
    pub fn apply_refresh(&mut self, r: MonitorRefresh) {
        if let Some(c) = r.config {
            self.cmdline_enrich = c.cmdline_enrich;
            self.config = c;
            self.saving = false;
        }
        if let Some(t) = r.telemetry {
            self.telemetry = Some(t);
        }
        if let Some(b) = r.is_background {
            self.is_background = b;
        }
        if let Some(c) = r.event_count {
            self.event_count = c;
        }
        if let Some(s) = r.db_size {
            self.db_size = s;
        }
    }

    /// Kick off an async config fetch.
    pub fn trigger_config_load(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };
        let ctx_clone = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx_clone };
            match svc.get_config().await {
                Ok(c) => {
                    let _ = tx.send(MonitorRefresh {
                        config: Some(c),
                        ..Default::default()
                    });
                }
                Err(e) => tracing::error!("monitor get_config: {}", e),
            }
        });
    }

    /// Kick off async fetches for telemetry, background state, event count, and db size.
    pub fn trigger_poll(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match self.refresh_tx.clone() {
            Some(t) => t,
            None => return,
        };
        let ctx_clone = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx_clone };
            let (telemetry, is_background, event_count, db_size) = tokio::join!(
                svc.get_telemetry(),
                svc.is_background(),
                svc.get_event_count(),
                svc.get_db_size(),
            );
            let refresh = MonitorRefresh {
                telemetry: telemetry.ok(),
                is_background: is_background.ok(),
                event_count: event_count.ok(),
                db_size: db_size.ok(),
                ..Default::default()
            };
            let _ = tx.send(refresh);
        });
    }

    // ── Rendering ──────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // Poll every 3 seconds; on the first poll also load config.
        let should_poll = self.last_poll.map_or(true, |t| t.elapsed().as_secs() >= 3);
        if should_poll {
            let is_first = self.last_poll.is_none();
            self.last_poll = Some(Instant::now());
            if is_first {
                self.trigger_config_load(ctx, rt);
            }
            self.trigger_poll(ctx, rt);
        }

        // Error banner
        if let Some(ref err) = self.last_error {
            let err = err.clone();
            if crate::widgets::banner::error_banner(ui, &err) {
                self.last_error = None;
            }
            ui.add_space(4.0);
        }

        // Scrollable form
        egui::ScrollArea::vertical()
            .id_salt("monitor_page_scroll")
            .show(ui, |ui| {
                self.render_status_section(ui, ctx, rt);
                ui.add_space(8.0);
                self.render_storage_section(ui, ctx, rt);
                ui.add_space(8.0);
                self.render_event_section(ui);
                ui.add_space(8.0);
                self.render_network_section(ui);
                ui.add_space(8.0);
                self.render_rules_section(ui);
                ui.add_space(8.0);
                self.render_presets_section(ui, ctx, rt);
            });

        // Dialogs
        if self.confirm_dialog_open {
            self.render_confirm_dialog(ui, ctx, rt);
        }
        if self.dns_sni_dialog_open {
            self.render_dns_sni_dialog(ui);
        }
    }

    // ── Status Section ────────────────────────────────────

    fn render_status_section(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut enter_bg = false;
        let mut exit_bg = false;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("运行状态")
                    .strong()
                    .color(theme::FG_SECONDARY)
                    .size(13.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.is_background {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("退出后台模式").color(theme::SEMANTIC_DANGER),
                        ))
                        .clicked()
                    {
                        exit_bg = true;
                    }
                } else if ui.button("进入后台模式").clicked() {
                    enter_bg = true;
                }
            });
        });

        ui.add_space(4.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let col_width = (ui.available_width() - 16.0 * 3.0) / 4.0;
            egui::Grid::new("monitor_status_grid")
                .num_columns(4)
                .spacing([16.0, 8.0])
                .min_col_width(col_width.max(60.0))
                .show(ui, |ui| {
                    // Row 1
                    // 模式
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("模式").color(theme::FG_TERTIARY).size(11.0));
                        if self.is_background {
                            badge::badge(ui, "后台运行", BadgeVariant::Success);
                        } else {
                            badge::badge(ui, "前台", BadgeVariant::Default);
                        }
                    });
                    // 运行时长
                    let uptime = self
                        .telemetry
                        .as_ref()
                        .map(|t| theme::fmt_uptime(t.started_at))
                        .unwrap_or_else(|| "-".to_string());
                    status_cell(ui, "运行时长", uptime, theme::FG_PRIMARY);
                    // 事件写入
                    let written = self
                        .telemetry
                        .as_ref()
                        .map(|t| t.events_written.to_string())
                        .unwrap_or_else(|| "0".to_string());
                    status_cell(ui, "事件写入", written, theme::FG_PRIMARY);
                    // 事件丢弃
                    let dropped = self
                        .telemetry
                        .as_ref()
                        .map(|t| t.events_dropped.to_string())
                        .unwrap_or_else(|| "0".to_string());
                    status_cell(ui, "事件丢弃", dropped, theme::FG_PRIMARY);
                    ui.end_row();

                    // Row 2
                    // 最后事件
                    let last_event = self
                        .telemetry
                        .as_ref()
                        .and_then(|t| t.last_event_at)
                        .map(|m| theme::fmt_time_millis(m))
                        .unwrap_or_else(|| "-".to_string());
                    let _last_event_resp = ui.vertical(|ui| {
                        ui.label(egui::RichText::new("最后事件").color(theme::FG_TERTIARY).size(11.0));
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(last_event)
                                    .color(theme::FG_PRIMARY)
                                    .size(12.0)
                                    .monospace(),
                            )
                            .truncate(),
                        );
                    });
                    // 数据库记录
                    status_cell(ui, "数据库记录", self.event_count.to_string(), theme::FG_PRIMARY);
                    // 数据库大小
                    status_cell(ui, "数据库大小", theme::fmt_bytes(self.db_size), theme::FG_PRIMARY);
                    // 最后错误
                    let last_err = self
                        .telemetry
                        .as_ref()
                        .and_then(|t| t.last_error.clone())
                        .unwrap_or_else(|| "-".to_string());
                    let err_color = if last_err == "-" { theme::FG_TERTIARY } else { theme::SEMANTIC_DANGER };
                    status_cell(ui, "最后错误", last_err, err_color);
                    ui.end_row();
                });
        });

        // Actions (after closures to satisfy borrow checker)
        if enter_bg {
            self.confirm_dialog_open = true;
        }
        if exit_bg {
            let ctx_clone = ctx.clone();
            let tx = self.refresh_tx.clone();
            rt.spawn(async move {
                let svc = MonitorService { ctx: &ctx_clone };
                if let Err(e) = svc.exit_background().await {
                    tracing::error!("exit background: {}", e);
                }
                if let Ok(b) = svc.is_background().await {
                    if let Some(tx) = tx {
                        let _ = tx.send(MonitorRefresh {
                            is_background: Some(b),
                            ..Default::default()
                        });
                    }
                }
            });
        }
    }

    // ── Storage Section ────────────────────────────────────

    fn render_storage_section(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut save = false;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("存储配置")
                    .strong()
                    .color(theme::FG_SECONDARY)
                    .size(13.0),
            );
        });
        ui.add_space(4.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // 保留天数
            ui.horizontal(|ui| {
                ui.add_sized([70.0, 18.0], egui::Label::new("保留天数:").selectable(false));
                let mut days_str = self.config.retention_days.to_string();
                let resp = ui.add(egui::TextEdit::singleline(&mut days_str).desired_width(100.0));
                if resp.changed() {
                    if let Ok(v) = days_str.parse::<u32>() {
                        self.config.retention_days = v;
                    }
                }
                ui.label(
                    egui::RichText::new("(0 = 永久保留)")
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            });

            // 大小限制
            ui.horizontal(|ui| {
                ui.add_sized([70.0, 18.0], egui::Label::new("大小限制:").selectable(false));
                let mut size_str = self.config.max_size_mb.to_string();
                let resp = ui.add(egui::TextEdit::singleline(&mut size_str).desired_width(100.0));
                if resp.changed() {
                    if let Ok(v) = size_str.parse::<u32>() {
                        self.config.max_size_mb = v;
                    }
                }
                ui.label(
                    egui::RichText::new("MB (0 = 不限制)")
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            });

            // 存储路径
            ui.horizontal(|ui| {
                ui.add_sized([70.0, 18.0], egui::Label::new("存储路径:").selectable(false));
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.db_path)
                        .desired_width(100.0)
                        .hint_text("留空使用默认路径"),
                );
            });

            ui.add_space(4.0);
            if ui
                .add_enabled(!self.saving, egui::Button::new("保存配置"))
                .clicked()
            {
                save = true;
            }
        });

        if save {
            self.save_config(ctx, rt);
        }
    }

    // ── Event Collection Section ──────────────────────────

    fn render_event_section(&mut self, ui: &mut egui::Ui) {
        let mut open_dns_sni = false;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("日志采集事件")
                    .strong()
                    .color(theme::FG_SECONDARY)
                    .size(13.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("配置").clicked() {
                    open_dns_sni = true;
                }
            });
        });

        ui.add_space(4.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                for &(key, label) in EVENT_TYPES {
                    let active = self.config.persist_event_types.iter().any(|t| t == key);
                    let text = if active {
                        egui::RichText::new(format!("{} √", label))
                            .color(egui::Color32::WHITE)
                            .size(11.0)
                    } else {
                        egui::RichText::new(label).color(theme::FG_SECONDARY).size(11.0)
                    };
                    let btn = if active {
                        egui::Button::new(text).fill(theme::ACCENT)
                    } else {
                        egui::Button::new(text)
                    };
                    if ui.add(btn).clicked() {
                        if active {
                            self.config.persist_event_types.retain(|t| t != key);
                        } else {
                            self.config.persist_event_types.push(key.to_string());
                        }
                    }
                }
            });
        });

        if open_dns_sni {
            self.dns_sni_dialog_open = true;
        }
    }

    // ── Network Section ───────────────────────────────────

    fn render_network_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("网络监控")
                    .strong()
                    .color(theme::FG_SECONDARY)
                    .size(13.0),
            );
        });
        ui.add_space(4.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // 快照间隔
            ui.horizontal(|ui| {
                ui.label("快照间隔:");
                let mut dur_str = self.config.max_duration_secs.to_string();
                let resp = ui.add(egui::TextEdit::singleline(&mut dur_str).desired_width(80.0));
                if resp.changed() {
                    if let Ok(v) = dur_str.parse::<u32>() {
                        self.config.max_duration_secs = v;
                    }
                }
                ui.label(
                    egui::RichText::new("秒 (0 = 不限制)")
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            });

            // 命令行富化
            ui.horizontal(|ui| {
                ui.label("命令行富化:");
                egui::ComboBox::from_id_salt("monitor_cmdline_enrich")
                    .selected_text(if self.cmdline_enrich == 0 { "关闭" } else { "后台" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.cmdline_enrich, 0, "关闭");
                        ui.selectable_value(&mut self.cmdline_enrich, 1, "后台");
                    });
            });
        });
    }

    // ── Rules & Notifications Section ──────────────────────

    fn render_rules_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("规则与通知")
                    .strong()
                    .color(theme::FG_SECONDARY)
                    .size(13.0),
            );
        });
        ui.add_space(4.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.config.rules.is_empty() {
                ui.label(
                    egui::RichText::new("暂无监控规则，请在 设置 → 告警规则 中添加")
                        .color(theme::FG_TERTIARY)
                        .size(12.0),
                );
            } else {
                for rule in &self.config.rules {
                    ui.horizontal(|ui| {
                        if rule.enabled {
                            badge::badge(ui, "启用", BadgeVariant::Success);
                        } else {
                            badge::badge(ui, "禁用", BadgeVariant::Default);
                        }
                        ui.label(
                            egui::RichText::new(&rule.name)
                                .color(theme::FG_PRIMARY)
                                .size(12.0),
                        );
                        for et in &rule.event_types {
                            badge::badge(ui, et, BadgeVariant::Info);
                        }
                        if self.config.notify_config.popup_rule_ids.contains(&rule.id) {
                            badge::badge(ui, "弹窗", BadgeVariant::Warning);
                        }
                        if self.config.notify_config.feishu_rule_ids.contains(&rule.id) {
                            badge::badge(ui, "飞书", BadgeVariant::Info);
                        }
                    });
                    ui.add_space(2.0);
                }
            }

            ui.add_space(4.0);
            // 弹窗时长
            ui.horizontal(|ui| {
                ui.label("弹窗时长:");
                let mut dur_str = self.config.notify_config.popup_duration_secs.to_string();
                let resp = ui.add(egui::TextEdit::singleline(&mut dur_str).desired_width(80.0));
                if resp.changed() {
                    if let Ok(v) = dur_str.parse::<u32>() {
                        self.config.notify_config.popup_duration_secs = v;
                    }
                }
                ui.label(
                    egui::RichText::new("秒 (0 = 不自动关闭)")
                        .color(theme::FG_TERTIARY)
                        .size(11.0),
                );
            });
        });
    }

    // ── Presets Section ────────────────────────────────────

    fn render_presets_section(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut apply_save = false;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("快速预设")
                    .strong()
                    .color(theme::FG_SECONDARY)
                    .size(13.0),
            );
        });
        ui.add_space(4.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if ui.button("低开销").clicked() {
                    self.config.persist_event_types = vec!["dns".to_string(), "network_connect".to_string()];
                    self.config.enable_sni = false;
                    self.config.enable_dns_pcap = false;
                }
                if ui.button("均衡").clicked() {
                    self.config.persist_event_types = vec![
                        "dns".to_string(),
                        "network_connect".to_string(),
                        "create_remote_thread".to_string(),
                        "file_create".to_string(),
                    ];
                    self.config.enable_sni = false;
                    self.config.enable_dns_pcap = false;
                }
                if ui.button("深度捕获").clicked() {
                    self.config.persist_event_types =
                        EVENT_TYPES.iter().map(|(k, _)| k.to_string()).collect();
                    self.config.enable_sni = true;
                    self.config.enable_dns_pcap = true;
                }
                ui.separator();
                if ui
                    .add_enabled(!self.saving, egui::Button::new("应用并保存"))
                    .clicked()
                {
                    apply_save = true;
                }
            });
        });

        if apply_save {
            self.save_config(ctx, rt);
        }
    }

    // ── Enter Background Confirm Dialog ───────────────────

    fn render_confirm_dialog(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let mut open = self.confirm_dialog_open;
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("进入后台监控模式")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                ui.label("进入后台监控模式后，窗口将隐藏到系统托盘，但数据采集将持续运行。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                    if ui.button("确认进入").clicked() {
                        confirmed = true;
                    }
                });
            });

        if cancelled {
            open = false;
        }
        if confirmed {
            open = false;
            let ctx_clone = ctx.clone();
            let tx = self.refresh_tx.clone();
            rt.spawn(async move {
                let svc = MonitorService { ctx: &ctx_clone };
                if let Err(e) = svc.enter_background().await {
                    tracing::error!("enter background: {}", e);
                }
                if let Ok(b) = svc.is_background().await {
                    if let Some(tx) = tx {
                        let _ = tx.send(MonitorRefresh {
                            is_background: Some(b),
                            ..Default::default()
                        });
                    }
                }
            });
        }
        self.confirm_dialog_open = open;
    }

    // ── DNS / SNI Config Dialog ────────────────────────────

    fn render_dns_sni_dialog(&mut self, ui: &mut egui::Ui) {
        let mut open = self.dns_sni_dialog_open;
        let mut close = false;

        egui::Window::new("DNS / SNI 配置")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                ui.checkbox(&mut self.config.enable_dns_pcap, "PCAP DNS 抓包");
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    badge::badge(ui, "较高开销", BadgeVariant::Warning);
                });
                ui.add_space(4.0);
                ui.checkbox(&mut self.config.enable_sni, "TLS SNI 提取");
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    badge::badge(ui, "较高开销", BadgeVariant::Warning);
                });
                ui.add_space(8.0);
                if ui.button("关闭").clicked() {
                    close = true;
                }
            });

        if close {
            open = false;
        }
        self.dns_sni_dialog_open = open;
    }

    // ── Helpers ────────────────────────────────────────────

    fn save_config(&mut self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        self.saving = true;
        let mut config = self.config.clone();
        config.cmdline_enrich = self.cmdline_enrich;
        let ctx_clone = ctx.clone();
        let tx = self.refresh_tx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx_clone };
            match svc.update_config(config).await {
                Ok(()) => {
                    if let Ok(c) = svc.get_config().await {
                        if let Some(tx) = tx {
                            let _ = tx.send(MonitorRefresh {
                                config: Some(c),
                                ..Default::default()
                            });
                        }
                    }
                }
                Err(e) => tracing::error!("monitor update_config: {}", e),
            }
        });
    }
}

// ── Free helpers ──────────────────────────────────────────

/// Render a label + value pair for the status grid.
fn status_cell(ui: &mut egui::Ui, label: &str, value: String, color: egui::Color32) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).color(theme::FG_TERTIARY).size(11.0));
        ui.add(
            egui::Label::new(egui::RichText::new(value).color(color).size(12.0))
                .truncate(),
        );
    });
}
