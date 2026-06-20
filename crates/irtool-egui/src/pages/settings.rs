//! 设置页 — 告警规则 / 通知 / 数据源 / 导入导出

use std::collections::HashMap;

use eframe::egui;

use irtool_service::context::AppContext;
use irtool_service::services::monitor::MonitorService;
use irtool_service::types::MonitorConfig;

use crate::theme;
use crate::widgets::badge::{self, BadgeVariant};

// ── 事件类型选项 ──────────────────────────────────────────

const EVENT_TYPE_OPTIONS: &[(&str, &str)] = &[
    ("dns", "DNS查询"),
    ("dns_client", "DNS-Client"),
    ("network_connect", "网络连接"),
    ("network_monitor", "网络监控"),
    ("tls_sni", "TLS SNI"),
    ("dns_pcap", "DNS抓包"),
    ("create_remote_thread", "远程线程"),
    ("file_create", "文件创建"),
];

// ── Tab 枚举 ──────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum SettingsTab {
    AlertRules,
    Notification,
    DataSource,
    ImportExport,
}

// ── 异步刷新载荷 ──────────────────────────────────────────

#[derive(Default)]
pub struct SettingsRefresh {
    pub config: Option<MonitorConfig>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub feishu_test_done: bool,
    pub feishu_test_error: Option<String>,
}

// ── 页面状态 ──────────────────────────────────────────────

pub struct SettingsPageState {
    pub config: MonitorConfig,
    pub active_tab: SettingsTab,
    pub saving: bool,
    pub loading: bool,

    /// 每条规则的 targets 输入（逗号分隔字符串）
    pub targets_input: HashMap<String, String>,

    /// 飞书 Webhook URL（独立编辑，保存时写回 config）
    pub feishu_webhook_url: String,
    pub revealed_feishu_url: bool,
    pub testing_feishu: bool,

    pub last_error: Option<String>,
    pub last_success: Option<String>,

    pub refresh_tx: Option<std::sync::mpsc::Sender<SettingsRefresh>>,
    pub config_loaded: bool,
}

impl Default for SettingsPageState {
    fn default() -> Self {
        Self {
            config: MonitorConfig::default(),
            active_tab: SettingsTab::AlertRules,
            saving: false,
            loading: false,
            targets_input: HashMap::new(),
            feishu_webhook_url: String::new(),
            revealed_feishu_url: false,
            testing_feishu: false,
            last_error: None,
            last_success: None,
            refresh_tx: None,
            config_loaded: false,
        }
    }
}

impl SettingsPageState {
    // ── 公共方法 ──────────────────────────────────────────

    pub fn apply_refresh(&mut self, r: SettingsRefresh) {
        if let Some(config) = r.config {
            // 同步 targets_input
            let mut ti = HashMap::new();
            for rule in &config.rules {
                ti.insert(rule.id.clone(), rule.targets.join(", "));
            }
            self.targets_input = ti;
            self.feishu_webhook_url = config.notify_config.feishu_webhook_url.clone();
            self.config = config;
            self.config_loaded = true;
        }
        if let Some(e) = r.error {
            self.last_error = Some(e);
            self.saving = false;
        }
        if let Some(s) = r.success {
            self.last_success = Some(s);
            self.saving = false;
        }
        if r.feishu_test_done {
            self.testing_feishu = false;
            if let Some(e) = r.feishu_test_error {
                self.last_error = Some(e);
            } else {
                self.last_success = Some("飞书测试消息发送成功".to_string());
            }
        }
    }

    pub fn trigger_config_load(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match &self.refresh_tx {
            Some(tx) => tx.clone(),
            None => return,
        };
        let ctx = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx };
            match svc.get_config().await {
                Ok(config) => {
                    let _ = tx.send(SettingsRefresh {
                        config: Some(config),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx.send(SettingsRefresh {
                        error: Some(format!("加载配置失败: {}", e)),
                        ..Default::default()
                    });
                }
            }
        });
    }

    pub fn save_config(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match &self.refresh_tx {
            Some(tx) => tx.clone(),
            None => return,
        };
        // 从 targets_input 构建最终 config
        let mut config = self.config.clone();
        for rule in &mut config.rules {
            if let Some(input) = self.targets_input.get(&rule.id) {
                rule.targets = input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
        config.notify_config.feishu_webhook_url = self.feishu_webhook_url.clone();

        let ctx = ctx.clone();
        rt.spawn(async move {
            let svc = MonitorService { ctx: &ctx };
            match svc.update_config(config).await {
                Ok(()) => {
                    let _ = tx.send(SettingsRefresh {
                        success: Some("配置已保存".to_string()),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx.send(SettingsRefresh {
                        error: Some(format!("保存配置失败: {}", e)),
                        ..Default::default()
                    });
                }
            }
        });
    }

    fn test_feishu(&self, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        let tx = match &self.refresh_tx {
            Some(tx) => tx.clone(),
            None => return,
        };
        let url = self.feishu_webhook_url.clone();
        let ctx = ctx.clone();
        rt.spawn(async move {
            match MonitorService::test_feishu(url).await {
                Ok(()) => {
                    let _ = tx.send(SettingsRefresh {
                        feishu_test_done: true,
                        ..Default::default()
                    });
                }
                Err(e) => {
                    let _ = tx.send(SettingsRefresh {
                        feishu_test_done: true,
                        feishu_test_error: Some(format!("飞书测试失败: {}", e)),
                        ..Default::default()
                    });
                }
            }
            let _ = ctx; // keep ctx alive
        });
    }

    // ── 规则操作 ──────────────────────────────────────────

    fn add_rule(&mut self) {
        let id = format!("rule-{}", chrono::Utc::now().timestamp_millis());
        let rule = irtool_service::types::MonitorRule {
            id: id.clone(),
            name: String::new(),
            targets: vec![],
            event_types: vec![],
            enabled: true,
        };
        self.targets_input.insert(id, String::new());
        self.config.rules.push(rule);
    }

    fn remove_rule(&mut self, id: &str) {
        self.config.rules.retain(|r| r.id != id);
        self.targets_input.remove(id);
        // 从通知配置中移除
        self.config.notify_config.popup_rule_ids.retain(|i| i != id);
        self.config.notify_config.feishu_rule_ids.retain(|i| i != id);
    }

    fn toggle_popup_rule(&mut self, rule_id: &str) {
        let ids = &mut self.config.notify_config.popup_rule_ids;
        if let Some(pos) = ids.iter().position(|i| i == rule_id) {
            ids.remove(pos);
        } else {
            ids.push(rule_id.to_string());
        }
    }

    fn toggle_feishu_rule(&mut self, rule_id: &str) {
        let ids = &mut self.config.notify_config.feishu_rule_ids;
        if let Some(pos) = ids.iter().position(|i| i == rule_id) {
            ids.remove(pos);
        } else {
            ids.push(rule_id.to_string());
        }
    }

    fn select_all_popup(&mut self, select: bool) {
        if select {
            self.config.notify_config.popup_rule_ids = self.config.rules.iter().map(|r| r.id.clone()).collect();
        } else {
            self.config.notify_config.popup_rule_ids.clear();
        }
    }

    fn select_all_feishu(&mut self, select: bool) {
        if select {
            self.config.notify_config.feishu_rule_ids = self.config.rules.iter().map(|r| r.id.clone()).collect();
        } else {
            self.config.notify_config.feishu_rule_ids.clear();
        }
    }

    // ── 主渲染 ────────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        // 首次加载配置
        if !self.config_loaded && !self.loading {
            self.loading = true;
            self.trigger_config_load(ctx, rt);
        }

        // 左侧导航 + 右侧内容
        egui::SidePanel::left("settings_sidebar")
            .default_width(160.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("设置")
                        .color(theme::FG_SECONDARY)
                        .size(12.0)
                        .strong(),
                );
                ui.add_space(8.0);

                let tabs = [
                    (SettingsTab::AlertRules, "告警规则"),
                    (SettingsTab::Notification, "通知"),
                    (SettingsTab::DataSource, "数据源"),
                    (SettingsTab::ImportExport, "导入导出"),
                ];
                for (tab, label) in tabs {
                    let is_active = self.active_tab == tab;
                    let resp = ui.add(
                        egui::Button::new(
                            egui::RichText::new(label)
                                .color(if is_active { theme::ACCENT } else { theme::FG_SECONDARY })
                                .size(12.0),
                        )
                        .frame(false)
                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                    );
                    if resp.clicked() {
                        self.active_tab = tab;
                    }
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("settings_scroll").show(ui, |ui| {
                ui.add_space(4.0);

                // 错误/成功提示
                if let Some(err) = self.last_error.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("! {}", err))
                                .color(theme::SEMANTIC_DANGER)
                                .size(11.0),
                        );
                        if ui.small_button("×").clicked() {
                            self.last_error = None;
                        }
                    });
                    ui.add_space(4.0);
                }
                if let Some(ok) = self.last_success.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("√ {}", ok))
                                .color(theme::SEMANTIC_SUCCESS)
                                .size(11.0),
                        );
                        if ui.small_button("×").clicked() {
                            self.last_success = None;
                        }
                    });
                    ui.add_space(4.0);
                }

                match self.active_tab {
                    SettingsTab::AlertRules => self.render_alert_rules(ui, ctx, rt),
                    SettingsTab::Notification => self.render_notification(ui, ctx, rt),
                    SettingsTab::DataSource => self.render_data_source(ui, ctx, rt),
                    SettingsTab::ImportExport => self.render_import_export(ui, ctx, rt),
                }
            });
        });
    }

    // ── Tab 1: 告警规则 ──────────────────────────────────

    fn render_alert_rules(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("告警规则")
                    .color(theme::FG_PRIMARY)
                    .strong()
                    .size(13.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("导入").clicked() {
                    self.import_rules(ctx);
                }
                if ui.button("导出").clicked() {
                    self.export_rules(ctx);
                }
                if ui.button("添加").clicked() {
                    self.add_rule();
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(if self.saving { "保存中…" } else { "保存" }).color(theme::ACCENT),
                    ))
                    .clicked()
                {
                    self.saving = true;
                    self.save_config(ctx, rt);
                }
            });
        });
        ui.add_space(8.0);

        if self.config.rules.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("暂无告警规则，点击「添加」创建")
                        .color(theme::FG_TERTIARY)
                        .size(12.0),
                );
            });
            return;
        }

        // 规则列表
        let mut remove_id: Option<String> = None;
        for rule in &mut self.config.rules.iter_mut() {
            let rule_id = rule.id.clone();
            egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    // 规则名
                    let mut name = rule.name.clone();
                    ui.add(
                        egui::TextEdit::singleline(&mut name)
                            .hint_text("规则名称")
                            .desired_width(160.0),
                    );
                    rule.name = name;

                    ui.add_space(8.0);

                    // 启用 checkbox
                    let mut enabled = rule.enabled;
                    ui.checkbox(&mut enabled, "");
                    rule.enabled = enabled;
                    ui.label(
                        egui::RichText::new(if enabled { "已启用" } else { "已禁用" })
                            .color(theme::FG_TERTIARY)
                            .size(10.0),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("删除").color(theme::SEMANTIC_DANGER))
                                    .frame(false),
                            )
                            .clicked()
                        {
                            remove_id = Some(rule_id.clone());
                        }
                    });
                });

                ui.add_space(4.0);

                // 匹配目标
                let targets_str = self.targets_input.entry(rule_id.clone()).or_default();
                ui.add(
                    egui::TextEdit::singleline(targets_str)
                        .hint_text("匹配目标（域名/IP/CIDR，逗号分隔）")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(4.0);

                // 事件类型 badges
                ui.horizontal_wrapped(|ui| {
                    for (key, label) in EVENT_TYPE_OPTIONS {
                        let selected = rule.event_types.iter().any(|t| t == *key);
                        let text = if selected {
                            egui::RichText::new(format!("{} √", label))
                                .color(egui::Color32::WHITE)
                                .size(10.0)
                        } else {
                            egui::RichText::new(*label).color(theme::FG_TERTIARY).size(10.0)
                        };
                        let btn = if selected {
                            egui::Button::new(text).fill(theme::ACCENT)
                        } else {
                            egui::Button::new(text).frame(false)
                        };
                        if ui.add(btn).clicked() {
                            if selected {
                                rule.event_types.retain(|t| t != *key);
                            } else {
                                rule.event_types.push((*key).to_string());
                            }
                        }
                        ui.add_space(2.0);
                    }
                });
            });
            ui.add_space(4.0);
        }

        if let Some(id) = remove_id {
            self.remove_rule(&id);
        }
    }

    // ── Tab 2: 通知 ──────────────────────────────────────

    fn render_notification(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("通知").color(theme::FG_PRIMARY).strong().size(13.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(if self.saving { "保存中…" } else { "保存" }).color(theme::ACCENT),
                    ))
                    .clicked()
                {
                    self.saving = true;
                    self.save_config(ctx, rt);
                }
            });
        });
        ui.add_space(8.0);

        // ── 弹窗通知 ──
        egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("弹窗通知")
                        .color(theme::FG_PRIMARY)
                        .strong()
                        .size(12.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("取消全选").clicked() {
                        self.select_all_popup(false);
                    }
                    if ui.small_button("全选").clicked() {
                        self.select_all_popup(true);
                    }
                });
            });
            ui.add_space(4.0);

            if self.config.rules.is_empty() {
                ui.label(egui::RichText::new("暂无规则").color(theme::FG_TERTIARY).size(10.0));
            } else {
                let mut toggles: Vec<String> = Vec::new();
                for rule in &self.config.rules {
                    ui.horizontal(|ui| {
                        let checked = self.config.notify_config.popup_rule_ids.iter().any(|i| i == &rule.id);
                        let mut c = checked;
                        ui.checkbox(&mut c, "");
                        if c != checked {
                            toggles.push(rule.id.clone());
                        }
                        ui.label(
                            egui::RichText::new(if rule.name.is_empty() {
                                "(未命名)"
                            } else {
                                &rule.name
                            })
                            .color(theme::FG_PRIMARY)
                            .size(11.0),
                        );
                        ui.add_space(4.0);
                        for et in &rule.event_types {
                            if let Some((_, label)) = EVENT_TYPE_OPTIONS.iter().find(|(k, _)| k == et) {
                                badge::badge(ui, label, BadgeVariant::Default);
                                ui.add_space(2.0);
                            }
                        }
                    });
                }
                for id in toggles {
                    self.toggle_popup_rule(&id);
                }
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // 弹窗时长
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("弹窗时长").color(theme::FG_SECONDARY).size(11.0));
                ui.add(egui::DragValue::new(&mut self.config.notify_config.popup_duration_secs).range(0..=3600));
                ui.label(
                    egui::RichText::new("秒（0 = 不自动关闭）")
                        .color(theme::FG_TERTIARY)
                        .size(10.0),
                );
            });
        });

        ui.add_space(8.0);

        // ── 飞书通知 ──
        egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("飞书通知")
                        .color(theme::FG_PRIMARY)
                        .strong()
                        .size(12.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("取消全选").clicked() {
                        self.select_all_feishu(false);
                    }
                    if ui.small_button("全选").clicked() {
                        self.select_all_feishu(true);
                    }
                });
            });
            ui.add_space(4.0);

            if self.config.rules.is_empty() {
                ui.label(egui::RichText::new("暂无规则").color(theme::FG_TERTIARY).size(10.0));
            } else {
                let mut toggles: Vec<String> = Vec::new();
                for rule in &self.config.rules {
                    ui.horizontal(|ui| {
                        let checked = self.config.notify_config.feishu_rule_ids.iter().any(|i| i == &rule.id);
                        let mut c = checked;
                        ui.checkbox(&mut c, "");
                        if c != checked {
                            toggles.push(rule.id.clone());
                        }
                        ui.label(
                            egui::RichText::new(if rule.name.is_empty() {
                                "(未命名)"
                            } else {
                                &rule.name
                            })
                            .color(theme::FG_PRIMARY)
                            .size(11.0),
                        );
                        ui.add_space(4.0);
                        for et in &rule.event_types {
                            if let Some((_, label)) = EVENT_TYPE_OPTIONS.iter().find(|(k, _)| k == et) {
                                badge::badge(ui, label, BadgeVariant::Default);
                                ui.add_space(2.0);
                            }
                        }
                    });
                }
                for id in toggles {
                    self.toggle_feishu_rule(&id);
                }
            }

            ui.add_space(8.0);

            // Webhook URL
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Webhook URL").color(theme::FG_SECONDARY).size(11.0));
                let display_url = if self.revealed_feishu_url {
                    self.feishu_webhook_url.clone()
                } else {
                    mask_url(&self.feishu_webhook_url)
                };
                let mut buf = display_url.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .hint_text("https://open.feishu.cn/...")
                        .desired_width(300.0)
                        .password(!self.revealed_feishu_url),
                );
                if self.revealed_feishu_url {
                    self.feishu_webhook_url = buf;
                }
                if ui
                    .button(if self.revealed_feishu_url { "隐藏" } else { "显示" })
                    .clicked()
                {
                    self.revealed_feishu_url = !self.revealed_feishu_url;
                }
                if ui
                    .add_enabled(
                        !self.testing_feishu && !self.feishu_webhook_url.is_empty(),
                        egui::Button::new(
                            egui::RichText::new(if self.testing_feishu { "测试中…" } else { "测试" })
                                .color(theme::ACCENT),
                        ),
                    )
                    .clicked()
                {
                    self.testing_feishu = true;
                    self.test_feishu(ctx, rt);
                }
            });
        });
    }

    // ── Tab 3: 数据源 ────────────────────────────────────

    fn render_data_source(&mut self, ui: &mut egui::Ui, ctx: &AppContext, rt: &tokio::runtime::Handle) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("数据源")
                    .color(theme::FG_PRIMARY)
                    .strong()
                    .size(13.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(if self.saving { "保存中…" } else { "保存" }).color(theme::ACCENT),
                    ))
                    .clicked()
                {
                    self.saving = true;
                    self.save_config(ctx, rt);
                }
            });
        });
        ui.add_space(8.0);

        // TLS SNI
        egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.config.enable_sni, "");
                ui.label(egui::RichText::new("TLS SNI 提取").color(theme::FG_PRIMARY).size(12.0));
            });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("从网络流量中提取 TLS SNI（Server Name Indication）信息")
                    .color(theme::FG_TERTIARY)
                    .size(10.0),
            );
            ui.label(
                egui::RichText::new("数据来源：网络层抓包（TCP:443）")
                    .color(theme::FG_TERTIARY)
                    .size(10.0),
            );
        });

        ui.add_space(8.0);

        // DNS 抓包
        egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.config.enable_dns_pcap, "");
                ui.label(egui::RichText::new("DNS 抓包").color(theme::FG_PRIMARY).size(12.0));
            });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("在网络层捕获 DNS 查询和响应")
                    .color(theme::FG_TERTIARY)
                    .size(10.0),
            );
            ui.label(
                egui::RichText::new("数据来源：网络层抓包（UDP:53）")
                    .color(theme::FG_TERTIARY)
                    .size(10.0),
            );
        });
    }

    // ── Tab 4: 导入导出 ──────────────────────────────────

    fn render_import_export(&mut self, ui: &mut egui::Ui, ctx: &AppContext, _rt: &tokio::runtime::Handle) {
        ui.label(
            egui::RichText::new("导入导出")
                .color(theme::FG_PRIMARY)
                .strong()
                .size(13.0),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("导出当前配置到文件，或从文件导入配置")
                .color(theme::FG_TERTIARY)
                .size(11.0),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("导出配置").clicked() {
                self.export_config(ctx);
            }
            if ui.button("导入配置").clicked() {
                self.import_config(ctx);
            }
            if ui.button("导出规则").clicked() {
                self.export_rules(ctx);
            }
            if ui.button("导入规则").clicked() {
                self.import_rules(ctx);
            }
        });
    }

    // ── 导入导出操作 ──────────────────────────────────────

    fn export_config(&self, ctx: &AppContext) {
        let json = serde_json::to_string_pretty(&self.config).unwrap_or_default();
        let path = ctx
            .app_dirs
            .config_dir()
            .join(format!("irtool-config-{}.json", chrono::Utc::now().format("%Y%m%d")));
        match std::fs::write(&path, &json) {
            Ok(()) => {
                if let Some(tx) = &self.refresh_tx {
                    let _ = tx.send(SettingsRefresh {
                        success: Some(format!("配置已导出到: {}", path.display())),
                        ..Default::default()
                    });
                }
            }
            Err(e) => {
                if let Some(tx) = &self.refresh_tx {
                    let _ = tx.send(SettingsRefresh {
                        error: Some(format!("导出失败: {}", e)),
                        ..Default::default()
                    });
                }
            }
        }
    }

    fn import_config(&mut self, ctx: &AppContext) {
        // 简化：从配置目录读取 irtool-config.json
        let path = ctx.app_dirs.config_dir().join("irtool-config.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<MonitorConfig>(&text) {
                Ok(config) => {
                    let mut ti = HashMap::new();
                    for rule in &config.rules {
                        ti.insert(rule.id.clone(), rule.targets.join(", "));
                    }
                    self.targets_input = ti;
                    self.feishu_webhook_url = config.notify_config.feishu_webhook_url.clone();
                    self.config = config;
                    if let Some(tx) = &self.refresh_tx {
                        let _ = tx.send(SettingsRefresh {
                            success: Some(format!("配置已从 {} 导入", path.display())),
                            ..Default::default()
                        });
                    }
                }
                Err(e) => {
                    if let Some(tx) = &self.refresh_tx {
                        let _ = tx.send(SettingsRefresh {
                            error: Some(format!("解析配置失败: {}", e)),
                            ..Default::default()
                        });
                    }
                }
            },
            Err(e) => {
                if let Some(tx) = &self.refresh_tx {
                    let _ = tx.send(SettingsRefresh {
                        error: Some(format!("读取文件失败: {} (请将配置文件放在 {})", e, path.display())),
                        ..Default::default()
                    });
                }
            }
        }
    }

    fn export_rules(&self, ctx: &AppContext) {
        let json = serde_json::to_string_pretty(&self.config.rules).unwrap_or_default();
        let path = ctx.app_dirs.config_dir().join(format!(
            "irtool-alert-rules-{}.json",
            chrono::Utc::now().format("%Y%m%d")
        ));
        match std::fs::write(&path, &json) {
            Ok(()) => {
                if let Some(tx) = &self.refresh_tx {
                    let _ = tx.send(SettingsRefresh {
                        success: Some(format!("规则已导出到: {}", path.display())),
                        ..Default::default()
                    });
                }
            }
            Err(e) => {
                if let Some(tx) = &self.refresh_tx {
                    let _ = tx.send(SettingsRefresh {
                        error: Some(format!("导出失败: {}", e)),
                        ..Default::default()
                    });
                }
            }
        }
    }

    fn import_rules(&mut self, ctx: &AppContext) {
        let path = ctx.app_dirs.config_dir().join("irtool-alert-rules.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Vec<irtool_service::types::MonitorRule>>(&text) {
                Ok(imported) => {
                    let existing_ids: std::collections::HashSet<_> =
                        self.config.rules.iter().map(|r| r.id.clone()).collect();
                    for rule in imported {
                        if !existing_ids.contains(&rule.id) {
                            self.targets_input.insert(rule.id.clone(), rule.targets.join(", "));
                            self.config.rules.push(rule);
                        }
                    }
                    if let Some(tx) = &self.refresh_tx {
                        let _ = tx.send(SettingsRefresh {
                            success: Some(format!("规则已从 {} 导入", path.display())),
                            ..Default::default()
                        });
                    }
                }
                Err(e) => {
                    if let Some(tx) = &self.refresh_tx {
                        let _ = tx.send(SettingsRefresh {
                            error: Some(format!("解析规则失败: {}", e)),
                            ..Default::default()
                        });
                    }
                }
            },
            Err(e) => {
                if let Some(tx) = &self.refresh_tx {
                    let _ = tx.send(SettingsRefresh {
                        error: Some(format!("读取文件失败: {} (请将规则文件放在 {})", e, path.display())),
                        ..Default::default()
                    });
                }
            }
        }
    }
}

// ── 辅助函数 ──────────────────────────────────────────────

/// 遮罩 URL，只显示首尾部分
fn mask_url(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }
    let head_end = url.char_indices().nth(4).map(|(i, _)| i).unwrap_or(url.len());
    if url.chars().count() <= 12 {
        return format!("{}****", &url[..head_end]);
    }
    let tail_start = url.char_indices().rev().nth(3).map(|(i, _)| i).unwrap_or(0);
    format!("{}****{}", &url[..head_end], &url[tail_start..])
}
