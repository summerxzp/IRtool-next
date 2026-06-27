use irtool_browser_forensics::browser_kind_from_process_name;
use irtool_core::IrError;
use irtool_sysmon::{EventConfigEntry, SysmonEvent, SysmonEventType, SysmonStatus};

use crate::context::AppContext;
use crate::dto::browser_forensics::BrowserMaliciousConnectionPayload;
use crate::event_bus::AppEvent;

pub struct SysmonService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> SysmonService<'a> {
    pub async fn status(&self) -> Result<SysmonStatus, IrError> {
        Ok(self.ctx.sysmon_config.get_status_info())
    }

    pub async fn is_channel_available(&self) -> Result<bool, IrError> {
        Ok(self.ctx.sysmon_reader.is_channel_available())
    }

    pub async fn install(&self, accept_eula: bool) -> Result<(bool, String), IrError> {
        let config = self.ctx.sysmon_config.clone();
        tokio::task::spawn_blocking(move || config.install(accept_eula))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub async fn uninstall(&self) -> Result<(bool, String), IrError> {
        let config = self.ctx.sysmon_config.clone();
        tokio::task::spawn_blocking(move || config.uninstall())
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub async fn update_config(&self) -> Result<(bool, String), IrError> {
        let config = self.ctx.sysmon_config.clone();
        tokio::task::spawn_blocking(move || config.update_config())
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub async fn get_existing_events(
        &self,
        limit: u32,
        enabled_event_ids: Vec<u32>,
    ) -> Result<Vec<SysmonEvent>, IrError> {
        tracing::info!(
            "get_existing_events called, limit={}, event_ids={:?}",
            limit,
            enabled_event_ids
        );
        let reader = self.ctx.sysmon_reader.clone();
        let result = tokio::task::spawn_blocking(move || reader.get_existing_events(limit, &enabled_event_ids))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?;
        match &result {
            Ok(events) => tracing::info!("get_existing_events returned {} events", events.len()),
            Err(e) => tracing::error!("get_existing_events error: {}", e),
        }
        result
    }

    pub async fn default_event_configs() -> Result<Vec<EventConfigEntry>, IrError> {
        Ok(irtool_sysmon::default_event_configs())
    }

    pub async fn generate_config(&self, enabled_events: Vec<String>) -> Result<String, IrError> {
        tracing::info!("Generating sysmon config with events: {:?}", enabled_events);
        let config = self.ctx.sysmon_config.clone();
        tokio::task::spawn_blocking(move || config.generate_config(&enabled_events))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// Start real-time Sysmon event subscription.
    /// Events are published via EventBus after being processed by the rule engine.
    pub async fn start_subscription(
        &self,
        enabled_event_ids: Vec<u32>,
        poll_interval_ms: Option<u64>,
    ) -> Result<(), IrError> {
        let reader = self.ctx.sysmon_reader.clone();
        if reader.is_polling() {
            return Ok(());
        }

        // Enable DNS Client event log if DNS Client is enabled
        if enabled_event_ids.contains(&3008) {
            let dns_manager = self.ctx.dns_client_manager.clone();
            tokio::task::spawn_blocking(move || {
                let mut m = dns_manager.lock();
                let _ = m.enable();
            })
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?;
        }

        // Init last_record_id to skip existing events
        let init_reader = reader.clone();
        let init_event_ids = enabled_event_ids.clone();
        tokio::task::spawn_blocking(move || init_reader.init_last_record_id(&init_event_ids))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SysmonEvent>();

        let interval = poll_interval_ms.unwrap_or(500);
        reader.start_polling(enabled_event_ids, interval, tx);

        // Forward events: rule engine processing + EventBus publish
        let monitor_engine = self.ctx.monitor_engine.clone();
        let event_bus = self.ctx.event_bus.clone();
        let browser_ip_index = self.ctx.browser_ip_index.clone();
        tokio::spawn(async move {
            while let Some(mut event) = rx.recv().await {
                // 一次锁：获取进程链 + 检查后台模式（减少锁竞争）
                let (chain, is_background) = {
                    let engine = monitor_engine.lock().await;
                    let chain = if event.process_id > 0 {
                        engine.get_chain_string(event.process_id)
                    } else {
                        None
                    };
                    (chain, engine.is_background_mode())
                };
                // 在捕获时附加进程链到 SysmonEvent.raw_data（取证关键：短命进程退出后无法回溯链）
                if let Some(chain) = chain {
                    event.raw_data.insert("process_chain".to_string(), chain);
                }
                // Rule engine always processes
                let alerts = monitor_engine.lock().await.process_sysmon_event(&event).await;
                for alert in &alerts {
                    event_bus.publish(AppEvent::MonitorAlert(alert.clone()));

                    // 浏览器进程的告警 → 触发 BrowserMaliciousConnection（让浏览器取证模块自动归因）
                    if let Some((domain, ip)) = extract_browser_connection_from_sysmon(&event) {
                        if browser_kind_from_process_name(&event.process_name).is_some() {
                            // T3: 写入 IP→浏览器进程索引，供 pcap 域名事件反查
                            if !ip.is_empty() {
                                let now_ms = chrono::Utc::now().timestamp_millis();
                                browser_ip_index.lock().await.insert(
                                    &ip,
                                    event.process_id,
                                    &event.process_name,
                                    now_ms,
                                );
                            }
                            let payload = BrowserMaliciousConnectionPayload {
                                domain,
                                ip,
                                process_name: event.process_name.clone(),
                                pid: event.process_id,
                                cmdline: Some(event.process_path.clone()),
                                alert_id: alert.id.to_string(),
                            };
                            event_bus.publish(AppEvent::BrowserMaliciousConnection(payload));
                        }
                    }
                }
                // Only publish to frontend when not in background mode
                if !is_background {
                    event_bus.publish(AppEvent::SysmonEvent(Box::new(event)));
                }
            }
        });

        Ok(())
    }

    pub async fn stop_subscription(&self) -> Result<(), IrError> {
        self.ctx.sysmon_reader.stop_polling();

        let dns_manager = self.ctx.dns_client_manager.clone();
        tokio::task::spawn_blocking(move || {
            let mut m = dns_manager.lock();
            let _ = m.restore();
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?;

        Ok(())
    }

    pub async fn get_log_max_size(&self) -> Result<u64, IrError> {
        let config = self.ctx.sysmon_config.clone();
        tokio::task::spawn_blocking(move || config.get_log_max_size_mb())
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub async fn set_log_max_size(&self, size_mb: u64) -> Result<(), IrError> {
        let config = self.ctx.sysmon_config.clone();
        tokio::task::spawn_blocking(move || config.set_log_max_size_mb(size_mb))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub fn is_subscribing(&self) -> bool {
        self.ctx.sysmon_reader.is_polling()
    }

    pub async fn get_event_count(&self, enabled_event_ids: Vec<u32>) -> Result<u64, IrError> {
        let reader = self.ctx.sysmon_reader.clone();
        tokio::task::spawn_blocking(move || reader.get_event_count(&enabled_event_ids))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }
}

/// 从 sysmon 事件提取浏览器连接信息（纯函数，便于测试）
///
/// 返回 (domain, ip) 元组：
/// - DNS 事件：domain=query_name, ip=""
/// - NetworkConnect 事件：domain="", ip=destination_ip
/// - 其他事件：None
fn extract_browser_connection_from_sysmon(event: &SysmonEvent) -> Option<(String, String)> {
    match event.event_type {
        SysmonEventType::Dns | SysmonEventType::DnsClient => {
            if event.query_name.is_empty() {
                None
            } else {
                Some((event.query_name.clone(), String::new()))
            }
        }
        SysmonEventType::NetworkConnect => {
            if event.destination_ip.is_empty() {
                None
            } else {
                Some((String::new(), event.destination_ip.clone()))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use irtool_sysmon::SysmonEvent;
    use std::collections::HashMap;

    fn make_dns_event(query: &str, proc_name: &str) -> SysmonEvent {
        SysmonEvent {
            event_id: 22,
            event_type: SysmonEventType::Dns,
            timestamp: String::new(),
            timestamp_epoch: 0.0,
            timestamp_valid: false,
            record_id: None,
            raw_data: HashMap::new(),
            process_id: 100,
            process_name: proc_name.to_string(),
            process_path: format!("C:\\{}", proc_name),
            user: String::new(),
            rule_name: String::new(),
            query_name: query.to_string(),
            query_results: String::new(),
            query_status: 0,
            source_ip: String::new(),
            source_port: 0,
            destination_ip: String::new(),
            destination_port: 0,
            protocol: String::new(),
            initiated: false,
            is_external: false,
            source_process_id: 0,
            source_process_name: String::new(),
            source_process_path: String::new(),
            target_process_id: 0,
            target_process_name: String::new(),
            target_process_path: String::new(),
            start_address: String::new(),
            start_module: String::new(),
            start_function: String::new(),
            is_suspicious: false,
            target_filename: String::new(),
            creation_utc_time: String::new(),
        }
    }

    fn make_net_event(ip: &str, proc_name: &str) -> SysmonEvent {
        let mut e = make_dns_event("", proc_name);
        e.event_id = 3;
        e.event_type = SysmonEventType::NetworkConnect;
        e.destination_ip = ip.to_string();
        e.destination_port = 443;
        e
    }

    #[test]
    fn extract_browser_connection_dns_returns_domain() {
        let e = make_dns_event("ip138.com", "chrome.exe");
        let result = extract_browser_connection_from_sysmon(&e);
        assert_eq!(result, Some(("ip138.com".to_string(), "".to_string())));
    }

    #[test]
    fn extract_browser_connection_network_returns_ip() {
        let e = make_net_event("1.2.3.4", "msedge.exe");
        let result = extract_browser_connection_from_sysmon(&e);
        assert_eq!(result, Some(("".to_string(), "1.2.3.4".to_string())));
    }

    #[test]
    fn extract_browser_connection_empty_dns_returns_none() {
        let e = make_dns_event("", "chrome.exe");
        assert!(extract_browser_connection_from_sysmon(&e).is_none());
    }

    #[test]
    fn extract_browser_connection_empty_ip_returns_none() {
        let e = make_net_event("", "chrome.exe");
        assert!(extract_browser_connection_from_sysmon(&e).is_none());
    }

    #[test]
    fn extract_browser_connection_other_event_returns_none() {
        let mut e = make_dns_event("ip138.com", "chrome.exe");
        e.event_type = SysmonEventType::ProcessCreate;
        assert!(extract_browser_connection_from_sysmon(&e).is_none());
    }
}
