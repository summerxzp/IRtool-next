use crate::config;
use crate::matcher;
use crate::notify;
use crate::storage::EventStorage;
use crate::types::*;
use irtool_core::IrError;
use irtool_sysmon::SysmonEvent;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

pub struct MonitorEngine {
    config: Arc<parking_lot::Mutex<MonitorConfig>>,
    storage: Option<Arc<EventStorage>>,
    config_path: PathBuf,
    /// 告警去重：记录最近 60 秒内已告警的 (rule_name, key_field, event_type) 组合
    alert_dedup: Arc<parking_lot::Mutex<HashSet<(String, String, String, i64)>>>,
    /// 进程链缓存：PID → 进程链字符串（避免重复快照）
    chain_cache: Arc<parking_lot::Mutex<HashMap<u32, String>>>,
}

impl MonitorEngine {
    pub fn new(app_dir: &std::path::Path) -> Self {
        let config_path = app_dir.join("config").join("monitor.toml");
        let config = config::load_config(&config_path).unwrap_or_default();
        let db_path = if config.db_path.is_empty() {
            app_dir.join("data").join("monitor.db")
        } else {
            std::path::PathBuf::from(&config.db_path)
        };
        let storage = EventStorage::open(&db_path).ok().map(Arc::new);
        Self {
            config: Arc::new(parking_lot::Mutex::new(config)),
            storage,
            config_path,
            alert_dedup: Arc::new(parking_lot::Mutex::new(HashSet::new())),
            chain_cache: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    pub fn get_config(&self) -> MonitorConfig {
        self.config.lock().clone()
    }

    pub fn update_config(&self, new_config: MonitorConfig) -> Result<(), IrError> {
        config::save_config(&self.config_path, &new_config)?;
        *self.config.lock() = new_config;
        Ok(())
    }

    pub fn is_background_mode(&self) -> bool {
        self.config.lock().background_mode
    }

    /// 进入后台模式：确保存储存在，启动事件处理循环
    pub fn enter_background_mode(&mut self, app_dir: &std::path::Path) -> Result<(), IrError> {
        // Ensure storage exists
        if self.storage.is_none() {
            let db_path = if self.config.lock().db_path.is_empty() {
                app_dir.join("data").join("monitor.db")
            } else {
                std::path::PathBuf::from(&self.config.lock().db_path)
            };
            let storage = EventStorage::open(&db_path)?;
            self.storage = Some(Arc::new(storage));
        }
        self.config.lock().background_mode = true;
        config::save_config(&self.config_path, &self.config.lock())?;
        info!("Entered background monitoring mode");
        Ok(())
    }

    /// 退出后台模式
    pub fn exit_background_mode(&mut self) -> Result<(), IrError> {
        self.config.lock().background_mode = false;
        // Keep storage alive for alert queries
        config::save_config(&self.config_path, &self.config.lock())?;
        info!("Exited background monitoring mode");
        Ok(())
    }

    /// 处理一条 Sysmon 事件：匹配规则 + 可选存储
    pub async fn process_sysmon_event(&self, event: &SysmonEvent) -> Vec<Alert> {
        let mut monitor_event = sysmon_to_monitor_event(event);
        // 对网络连接事件附加进程链（使用缓存避免重复快照）
        if matches!(event.event_type, irtool_sysmon::SysmonEventType::NetworkConnect) && event.process_id > 0 {
            let chain_str = {
                let mut cache = self.chain_cache.lock();
                if let Some(cached) = cache.get(&event.process_id) {
                    Some(cached.clone())
                } else {
                    let result = irtool_process::get_process_chain(event.process_id).ok()
                        .filter(|c| !c.is_empty())
                        .map(|chain| {
                            chain.nodes.iter()
                                .map(|n| format!("{} ({})", n.name, n.pid))
                                .collect::<Vec<_>>()
                                .join("->")
                        });
                    if let Some(ref s) = result {
                        cache.insert(event.process_id, s.clone());
                        // 限制缓存大小
                        if cache.len() > 500 {
                            cache.clear();
                        }
                    }
                    result
                }
            };
            if let Some(chain) = chain_str {
                if let Ok(mut raw_value) = serde_json::from_str::<serde_json::Value>(&monitor_event.raw_json) {
                    raw_value["process_chain"] = serde_json::Value::String(chain);
                    monitor_event.raw_json = serde_json::to_string(&raw_value).unwrap_or_default();
                }
            }
        }
        self.process_monitor_event(&monitor_event).await
    }

    /// 处理一条 MonitorEvent
    pub async fn process_monitor_event(&self, event: &MonitorEvent) -> Vec<Alert> {
        // 在锁内提取所需数据，确保 MutexGuard 不跨 .await
        let (rules, background_mode, persist_event_types) = {
            let config = self.config.lock();
            (config.rules.clone(), config.background_mode, config.persist_event_types.clone())
        };

        let mut alerts = Vec::new();

        // 规则匹配
        for rule in &rules {
            if matcher::matches_rule(event, rule) {
                // 去重：同一规则 + 同一目标 + 同一类型，60秒内只告警一次
                let minute_key = event.timestamp / 60_000;
                let dedup_key = (rule.name.clone(), event.key_field.clone(), event.event_type.clone(), minute_key);
                let should_alert = {
                    let mut dedup = self.alert_dedup.lock();
                    if dedup.contains(&dedup_key) {
                        false
                    } else {
                        dedup.insert(dedup_key);
                        let cutoff = minute_key - 5;
                        dedup.retain(|key| key.3 >= cutoff);
                        true
                    }
                };
                if !should_alert {
                    continue;
                }

                let mut alert = Alert {
                    id: 0,
                    timestamp: event.timestamp,
                    rule_name: rule.name.clone(),
                    event_type: event.event_type.clone(),
                    process_name: event.process_name.clone(),
                    key_field: event.key_field.clone(),
                    action_taken: rule.actions.iter().map(|a| match a {
                        NotifyAction::Popup => "popup".to_string(),
                        NotifyAction::Feishu { .. } => "feishu".to_string(),
                    }).collect::<Vec<_>>().join(","),
                    raw_json: event.raw_json.clone(),
                };

                // 存储告警
                if let Some(storage) = &self.storage {
                    if let Err(e) = storage.insert_alert(&mut alert) {
                        warn!("存储告警失败: {}", e);
                    }
                }

                // 发送通知
                notify::send_notification(&alert, &rule.actions).await;
                alerts.push(alert);
            }
        }

        // 后台模式时持久化事件
        if background_mode {
            if let Some(storage) = &self.storage {
                let should_persist = persist_event_types.is_empty()
                    || persist_event_types.contains(&event.event_type);
                if should_persist {
                    if let Err(e) = storage.insert_events(&[event.clone()]) {
                        warn!("存储事件失败: {}", e);
                    }
                }
            }
        }

        alerts
    }

    /// 处理一条 PcapEvent
    pub async fn process_pcap_event(&self, event: &irtool_pcap::PcapEvent) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // 先用域名匹配
        let domain_event = MonitorEvent {
            id: 0,
            timestamp: event.timestamp,
            source: EventSource::Pcap,
            event_type: match event.event_kind {
                irtool_pcap::PcapEventKind::TlsSni => "tls_sni".to_string(),
                irtool_pcap::PcapEventKind::DnsQuery => "dns_pcap".to_string(),
            },
            process_name: String::new(),
            key_field: event.domain.clone(),
            raw_json: serde_json::to_string(&event).unwrap_or_default(),
        };
        alerts.extend(self.process_monitor_event(&domain_event).await);

        // 再用 IP:Port 匹配（覆盖规则配置了 IP 目标的场景）
        if !event.dst_ip.is_empty() {
            let ip_event = MonitorEvent {
                id: 0,
                timestamp: event.timestamp,
                source: EventSource::Pcap,
                event_type: match event.event_kind {
                    irtool_pcap::PcapEventKind::TlsSni => "tls_sni".to_string(),
                    irtool_pcap::PcapEventKind::DnsQuery => "dns_pcap".to_string(),
                },
                process_name: String::new(),
                key_field: format!("{}:{}", event.dst_ip, event.dst_port),
                raw_json: serde_json::to_string(&event).unwrap_or_default(),
            };
            alerts.extend(self.process_monitor_event(&ip_event).await);
        }

        alerts
    }

    /// 清理过期数据
    pub fn cleanup(&self) -> Result<u64, IrError> {
        let config = self.config.lock();
        if let Some(storage) = &self.storage {
            storage.cleanup_old_events(config.retention_days)
        } else {
            Ok(0)
        }
    }

    /// 获取最近告警
    pub fn get_recent_alerts(&self, limit: u32) -> Result<Vec<Alert>, IrError> {
        if let Some(storage) = &self.storage {
            storage.get_recent_alerts(limit)
        } else {
            Ok(vec![])
        }
    }

    /// 清除所有告警
    pub fn clear_alerts(&self) -> Result<u64, IrError> {
        if let Some(storage) = &self.storage {
            storage.clear_alerts()
        } else {
            Ok(0)
        }
    }

    /// 获取最近事件
    pub fn get_recent_events(&self, limit: u32) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        if let Some(storage) = &self.storage {
            storage.get_recent_events(limit)
        } else {
            Ok(Vec::new())
        }
    }

    /// 获取事件总数
    pub fn get_event_count(&self) -> Result<u64, IrError> {
        if let Some(storage) = &self.storage {
            storage.get_event_count()
        } else {
            Ok(0)
        }
    }

    /// 搜索事件，支持多种过滤条件
    pub fn search_events(
        &self,
        source: Option<&str>,
        event_type: Option<&str>,
        process_name: Option<&str>,
        key_field: Option<&str>,
        search_text: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        if let Some(storage) = &self.storage {
            storage.search_events(source, event_type, process_name, key_field, search_text, limit, offset)
        } else {
            Ok(Vec::new())
        }
    }
}

/// SysmonEvent → MonitorEvent 转换
fn sysmon_to_monitor_event(event: &SysmonEvent) -> MonitorEvent {
    let key_field = match event.event_type {
        irtool_sysmon::SysmonEventType::Dns | irtool_sysmon::SysmonEventType::DnsClient => {
            event.query_name.clone()
        }
        irtool_sysmon::SysmonEventType::NetworkConnect => {
            format!("{}:{}", event.destination_ip, event.destination_port)
        }
        _ => String::new(),
    };

    let source = match event.event_type {
        irtool_sysmon::SysmonEventType::DnsClient => EventSource::DnsClient,
        _ => EventSource::Sysmon,
    };

    let event_type = match event.event_type {
        irtool_sysmon::SysmonEventType::Dns => "dns".to_string(),
        irtool_sysmon::SysmonEventType::DnsClient => "dns_client".to_string(),
        irtool_sysmon::SysmonEventType::NetworkConnect => "network_connect".to_string(),
        irtool_sysmon::SysmonEventType::CreateRemoteThread => "create_remote_thread".to_string(),
        irtool_sysmon::SysmonEventType::FileCreate => "file_create".to_string(),
        _ => format!("unknown_{}", event.event_id),
    };

    let timestamp = chrono::DateTime::parse_from_rfc3339(
        &event.timestamp.replace('Z', "+00:00")
    )
        .map(|dt| dt.timestamp_millis())
        .unwrap_or((event.timestamp_epoch * 1000.0) as i64);

    let raw_json = serde_json::to_string(&event).unwrap_or_default();

    MonitorEvent {
        id: 0,
        timestamp,
        source,
        event_type,
        process_name: event.process_name.clone(),
        key_field,
        raw_json,
    }
}
