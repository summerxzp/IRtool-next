use irtool_core::IrError;
use irtool_sysmon::{EventConfigEntry, SysmonEvent, SysmonStatus};

use crate::context::AppContext;
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
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // Rule engine always processes
                let alerts = monitor_engine.lock().await.process_sysmon_event(&event).await;
                for alert in &alerts {
                    event_bus.publish(AppEvent::MonitorAlert(alert.clone()));
                }
                // Only publish to frontend when not in background mode
                let is_background = monitor_engine.lock().await.is_background_mode();
                if !is_background {
                    event_bus.publish(AppEvent::SysmonEvent(event));
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
