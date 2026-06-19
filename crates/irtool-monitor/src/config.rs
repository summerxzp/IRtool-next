use crate::types::{generate_rule_id, MonitorConfig, MonitorRule, NotifyAction, NotifyConfig};
use irtool_core::IrError;
use std::path::Path;

/// 从旧配置结构迁移
fn migrate_config(mut config: MonitorConfig) -> MonitorConfig {
    // 为没有 id 的规则生成 id
    for rule in &mut config.rules {
        if rule.id.is_empty() {
            rule.id = generate_rule_id(&rule.name);
        }
    }
    config
}

pub fn load_config(path: &Path) -> Result<MonitorConfig, IrError> {
    if !path.exists() {
        return Ok(MonitorConfig::default());
    }
    let content = std::fs::read_to_string(path).map_err(|e| IrError::Io(e.to_string()))?;

    // 尝试解析为新格式
    let config: MonitorConfig = match toml::from_str(&content) {
        Ok(cfg) => cfg,
        Err(_) => {
            // 尝试解析为旧格式（包含 rules 中的 actions 字段）
            #[derive(serde::Deserialize)]
            struct OldMonitorRule {
                pub name: String,
                pub targets: Vec<String>,
                pub event_types: Vec<String>,
                #[serde(default)]
                pub actions: Vec<NotifyAction>,
                pub enabled: bool,
            }

            #[derive(serde::Deserialize)]
            struct OldMonitorConfig {
                pub background_mode: bool,
                pub persist_event_types: Vec<String>,
                pub retention_days: u32,
                #[serde(default)]
                pub rules: Vec<OldMonitorRule>,
                pub db_path: String,
                pub enable_sni: bool,
                pub enable_dns_pcap: bool,
                pub load_limit: u32,
                #[serde(default = "default_max_size_mb")]
                pub max_size_mb: u32,
            }

            fn default_max_size_mb() -> u32 {
                512
            }

            let old_config: OldMonitorConfig =
                toml::from_str(&content).map_err(|e| IrError::Parse(format!("监控配置解析失败: {}", e)))?;

            // 迁移规则
            let mut popup_rule_ids = Vec::new();
            let mut feishu_rule_ids = Vec::new();
            let mut feishu_webhook_url = String::new();

            let rules: Vec<MonitorRule> = old_config
                .rules
                .into_iter()
                .map(|old_rule| {
                    let id = generate_rule_id(&old_rule.name);

                    // 检查 actions 并迁移
                    for action in &old_rule.actions {
                        match action {
                            NotifyAction::Popup => {
                                popup_rule_ids.push(id.clone());
                            }
                            NotifyAction::Feishu { webhook_url } => {
                                if !feishu_webhook_url.is_empty() && feishu_webhook_url != *webhook_url {
                                    // 如果有多个不同的 webhook，保留第一个
                                } else if feishu_webhook_url.is_empty() {
                                    feishu_webhook_url = webhook_url.clone();
                                }
                                feishu_rule_ids.push(id.clone());
                            }
                        }
                    }

                    MonitorRule {
                        id,
                        name: old_rule.name,
                        targets: old_rule.targets,
                        event_types: old_rule.event_types,
                        enabled: old_rule.enabled,
                    }
                })
                .collect();

            MonitorConfig {
                background_mode: old_config.background_mode,
                persist_event_types: old_config.persist_event_types,
                retention_days: old_config.retention_days,
                rules,
                db_path: old_config.db_path,
                enable_sni: old_config.enable_sni,
                enable_dns_pcap: old_config.enable_dns_pcap,
                adapter_ip: None,
                max_duration_secs: 0,
                load_limit: old_config.load_limit,
                max_size_mb: old_config.max_size_mb,
                cmdline_enrich: 0,
                notify_config: NotifyConfig {
                    popup_rule_ids,
                    feishu_rule_ids,
                    feishu_webhook_url,
                    popup_duration_secs: 10,
                },
            }
        }
    };

    // 应用迁移（处理没有 id 的规则）
    let config = migrate_config(config);

    Ok(config)
}

pub fn save_config(path: &Path, config: &MonitorConfig) -> Result<(), IrError> {
    let content = toml::to_string_pretty(config).map_err(|e| IrError::Internal(format!("序列化配置失败: {}", e)))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| IrError::Io(e.to_string()))?;
    }
    std::fs::write(path, content).map_err(|e| IrError::Io(e.to_string()))?;
    Ok(())
}
