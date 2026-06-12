use crate::config;
use crate::ingest::EventIngestQueue;
use crate::matcher;
use crate::notify;
use crate::storage::EventStorage;
use crate::types::*;
use irtool_core::IrError;
use irtool_sysmon::SysmonEvent;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use tracing::{info, warn};

type AlertDedupKey = (String, String, String, i64);

pub struct MonitorEngine {
    config: Arc<parking_lot::Mutex<MonitorConfig>>,
    storage: Option<Arc<EventStorage>>,
    ingest_queue: Option<EventIngestQueue>,
    config_path: PathBuf,
    /// 告警去重：记录最近 60 秒内已告警的 (rule_name, key_field, event_type) 组合
    #[allow(clippy::type_complexity)]
    alert_dedup: Arc<parking_lot::Mutex<HashSet<AlertDedupKey>>>,
    /// 进程链缓存：PID → 进程链字符串（避免重复快照）
    chain_cache: Arc<parking_lot::Mutex<HashMap<u32, String>>>,
    /// 遥测：进入后台模式的时间戳
    started_at: AtomicI64,
    /// 遥测：最后事件时间戳
    last_event_at: AtomicI64,
    /// 遥测：前台模式处理的事件数
    foreground_events: AtomicU64,
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
            ingest_queue: None,
            config_path,
            alert_dedup: Arc::new(parking_lot::Mutex::new(HashSet::new())),
            chain_cache: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            started_at: AtomicI64::new(chrono::Utc::now().timestamp_millis()),
            last_event_at: AtomicI64::new(0),
            foreground_events: AtomicU64::new(0),
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
        // 确保摄入队列存在
        if self.ingest_queue.is_none() {
            if let Some(storage) = &self.storage {
                self.ingest_queue = Some(EventIngestQueue::start(storage.clone()));
            }
        }
        self.config.lock().background_mode = true;
        config::save_config(&self.config_path, &self.config.lock())?;
        self.started_at.store(chrono::Utc::now().timestamp_millis(), Ordering::Relaxed);
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
        // 更新遥测
        self.last_event_at.store(event.timestamp, Ordering::Relaxed);
        self.foreground_events.fetch_add(1, Ordering::Relaxed);

        // 在锁内提取所需数据，确保 MutexGuard 不跨 .await
        let (rules, background_mode, persist_event_types, notify_config) = {
            let config = self.config.lock();
            (config.rules.clone(), config.background_mode, config.persist_event_types.clone(), config.notify_config.clone())
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

                // 根据 rule.id 在 notify_config 中查找通知方式
                let mut actions = Vec::new();
                let mut action_taken_parts = Vec::new();

                if notify_config.popup_rule_ids.contains(&rule.id) {
                    actions.push(NotifyAction::Popup);
                    action_taken_parts.push("popup");
                }
                if notify_config.feishu_rule_ids.contains(&rule.id) {
                    actions.push(NotifyAction::Feishu { webhook_url: notify_config.feishu_webhook_url.clone() });
                    action_taken_parts.push("feishu");
                }

                let action_taken = action_taken_parts.join(",");

                let mut alert = Alert {
                    id: 0,
                    timestamp: event.timestamp,
                    rule_name: rule.name.clone(),
                    event_type: event.event_type.clone(),
                    process_name: event.process_name.clone(),
                    key_field: event.key_field.clone(),
                    action_taken,
                    raw_json: event.raw_json.clone(),
                };

                // 存储告警
                if let Some(storage) = &self.storage {
                    if let Err(e) = storage.insert_alert(&mut alert) {
                        warn!("存储告警失败: {}", e);
                    }
                }

                // 发送通知
                notify::send_notification(&alert, &actions).await;
                alerts.push(alert);
            }
        }

        // 后台模式时持久化事件（通过摄入队列批量写入）
        if background_mode {
            if let Some(ingest_queue) = &self.ingest_queue {
                let should_persist = persist_event_types.is_empty()
                    || persist_event_types.contains(&event.event_type);
                if should_persist {
                    if let Err(e) = ingest_queue.push(event.clone()) {
                        warn!("推送事件到摄入队列失败: {}", e);
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

    /// 清除所有事件
    pub fn clear_events(&self) -> Result<u64, IrError> {
        if let Some(storage) = &self.storage {
            storage.clear_events()
        } else {
            Ok(0)
        }
    }

    /// 获取事件类型统计
    pub fn get_event_type_counts(&self) -> Result<Vec<(String, u64)>, IrError> {
        if let Some(storage) = &self.storage {
            storage.get_event_type_counts()
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
    pub fn search_events(&self, query: &EventQuery) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        if let Some(storage) = &self.storage {
            storage.search_events(query)
        } else {
            Ok(Vec::new())
        }
    }

    /// 分页搜索事件，返回总数 + 当前页数据
    pub fn search_events_page(&self, query: &EventQuery) -> Result<EventPage, IrError> {
        if let Some(storage) = &self.storage {
            storage.search_events_page(query)
        } else {
            Ok(EventPage {
                items: Vec::new(),
                total: 0,
                limit: query.limit,
                offset: query.offset,
            })
        }
    }

    /// 获取运行时遥测信息
    pub fn get_telemetry(&self) -> RuntimeTelemetry {
        let mode = if self.config.lock().background_mode {
            RuntimeMode::Background
        } else {
            RuntimeMode::Foreground
        };
        let started_at = self.started_at.load(Ordering::Relaxed);
        let last_event_at = self.last_event_at.load(Ordering::Relaxed);
        let fg_events = self.foreground_events.load(Ordering::Relaxed);

        let (events_written, events_dropped) = if let Some(q) = &self.ingest_queue {
            (q.events_written(), q.events_dropped())
        } else {
            (fg_events, 0)
        };

        RuntimeTelemetry {
            mode,
            started_at: if started_at > 0 { Some(started_at) } else { None },
            events_written,
            events_dropped,
            last_event_at: if last_event_at > 0 { Some(last_event_at) } else { None },
            last_error: None,
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

#[cfg(test)]
mod telemetry_tests {
    use super::*;

    fn temp_engine() -> MonitorEngine {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("irtool_test_{}", ts));
        std::fs::create_dir_all(&temp_dir).unwrap();
        MonitorEngine::new(&temp_dir)
    }

    fn make_event(timestamp: i64) -> MonitorEvent {
        MonitorEvent {
            id: 0,
            timestamp,
            source: EventSource::Sysmon,
            event_type: "dns".to_string(),
            process_name: "test.exe".to_string(),
            key_field: "example.com".to_string(),
            raw_json: "{}".to_string(),
        }
    }

    #[tokio::test]
    async fn telemetry_started_at_set_on_new() {
        let engine = temp_engine();
        let telemetry = engine.get_telemetry();
        assert!(telemetry.started_at.is_some(), "前台模式 started_at 应有值");
        assert!(telemetry.started_at.unwrap() > 0);
        assert_eq!(telemetry.mode, RuntimeMode::Foreground);
    }

    #[tokio::test]
    async fn telemetry_last_event_at_updates_on_process() {
        let engine = temp_engine();
        let event = make_event(1_000_000_000_000);
        engine.process_monitor_event(&event).await;

        let telemetry = engine.get_telemetry();
        assert_eq!(telemetry.last_event_at, Some(1_000_000_000_000));
    }

    #[tokio::test]
    async fn telemetry_foreground_events_increments_on_process() {
        let engine = temp_engine();
        assert_eq!(engine.get_telemetry().events_written, 0);

        engine.process_monitor_event(&make_event(1)).await;
        engine.process_monitor_event(&make_event(2)).await;
        engine.process_monitor_event(&make_event(3)).await;

        // 前台模式 events_written 等于前台处理数
        let telemetry = engine.get_telemetry();
        assert_eq!(telemetry.events_written, 3);
    }

    #[tokio::test]
    async fn telemetry_mode_reflects_background_mode() {
        let mut engine = temp_engine();
        // 默认前台
        assert_eq!(engine.get_telemetry().mode, RuntimeMode::Foreground);
        assert!(!engine.is_background_mode());

        // 进入后台模式
        let temp_dir = std::env::temp_dir();
        engine.enter_background_mode(&temp_dir).unwrap();
        assert_eq!(engine.get_telemetry().mode, RuntimeMode::Background);
        assert!(engine.is_background_mode());

        // 退出后台模式
        engine.exit_background_mode().unwrap();
        assert_eq!(engine.get_telemetry().mode, RuntimeMode::Foreground);
        assert!(!engine.is_background_mode());
    }

    #[tokio::test]
    async fn telemetry_started_at_persists_after_exit_background() {
        let mut engine = temp_engine();
        let initial_started = engine.get_telemetry().started_at;
        assert!(initial_started.is_some());

        let temp_dir = std::env::temp_dir();
        engine.enter_background_mode(&temp_dir).unwrap();
        engine.exit_background_mode().unwrap();

        // 退出后台模式后 started_at 不应重置
        let telemetry = engine.get_telemetry();
        assert!(telemetry.started_at.is_some(), "退出后台后 started_at 不应重置");
    }
}
