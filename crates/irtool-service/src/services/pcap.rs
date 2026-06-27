use irtool_core::IrError;
use irtool_pcap::{AdapterInfo, PcapConfig, PcapCountersSnapshot, PcapEvent};

use crate::context::AppContext;
use crate::dto::browser_forensics::BrowserMaliciousConnectionPayload;
use crate::event_bus::AppEvent;

pub struct PcapService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> PcapService<'a> {
    pub fn is_available() -> bool {
        irtool_pcap::PcapCollector::is_available()
    }

    pub async fn start(&self, config: PcapConfig) -> Result<(), IrError> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PcapEvent>();

        {
            let mut collector = self.ctx.pcap_collector.lock().await;
            collector.start(config, tx)?;
        }

        // Forward pcap events: rule engine + EventBus
        let monitor_engine = self.ctx.monitor_engine.clone();
        let event_bus = self.ctx.event_bus.clone();
        let browser_ip_index = self.ctx.browser_ip_index.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // Rule engine always processes
                let alerts = monitor_engine.lock().await.process_pcap_event(&event).await;
                for alert in &alerts {
                    event_bus.publish(AppEvent::MonitorAlert(alert.clone()));

                    // T3: pcap 域名事件反查浏览器进程（pcap 无 pid，靠 IP 索引归因）
                    let now_ms = event.timestamp;
                    let entry = browser_ip_index.lock().await.lookup(&event.dst_ip, now_ms).cloned();
                    if let Some(entry) = entry {
                        let payload = BrowserMaliciousConnectionPayload {
                            domain: event.domain.clone(),
                            ip: event.dst_ip.clone(),
                            process_name: entry.process_name.clone(),
                            pid: entry.pid,
                            cmdline: None, // pcap 路径无 cmdline
                            alert_id: alert.id.to_string(),
                        };
                        event_bus.publish(AppEvent::BrowserMaliciousConnection(payload));
                    }
                }
                // Only publish to frontend when not in background mode
                let is_background = monitor_engine.lock().await.is_background_mode();
                if !is_background {
                    event_bus.publish(AppEvent::PcapEvent(event));
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), IrError> {
        let mut collector = self.ctx.pcap_collector.lock().await;
        collector.stop();
        Ok(())
    }

    pub async fn is_running(&self) -> Result<bool, IrError> {
        let collector = self.ctx.pcap_collector.lock().await;
        Ok(collector.is_running())
    }

    pub fn list_adapters() -> Result<Vec<AdapterInfo>, IrError> {
        Ok(irtool_pcap::PcapCollector::list_adapters())
    }

    pub async fn get_counters(&self) -> Result<PcapCountersSnapshot, IrError> {
        let collector = self.ctx.pcap_collector.lock().await;
        Ok(collector.counters().snapshot())
    }
}
