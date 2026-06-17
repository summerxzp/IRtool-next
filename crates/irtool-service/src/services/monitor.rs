use irtool_core::IrError;
use irtool_monitor::{Alert, EventPage, EventQuery, MonitorConfig, MonitorEvent, RuntimeTelemetry};

use crate::context::AppContext;

pub struct MonitorService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> MonitorService<'a> {
    pub async fn get_config(&self) -> Result<MonitorConfig, IrError> {
        Ok(self.ctx.monitor_engine.lock().await.get_config())
    }

    pub async fn update_config(&self, config: MonitorConfig) -> Result<(), IrError> {
        self.ctx.monitor_engine.lock().await.update_config(config)
    }

    pub async fn enter_background(&self) -> Result<(), IrError> {
        let app_dir = self.ctx.app_dirs.root().to_path_buf();
        tracing::info!("enter_background: app_dir={}", app_dir.display());
        let result = self.ctx.monitor_engine.lock().await.enter_background_mode(&app_dir);
        if let Err(ref e) = result {
            tracing::error!("enter_background failed: {}", e);
        }
        result
    }

    pub async fn exit_background(&self) -> Result<(), IrError> {
        let result = self.ctx.monitor_engine.lock().await.exit_background_mode();
        if let Err(ref e) = result {
            tracing::error!("exit_background failed: {}", e);
        }
        result
    }

    pub async fn get_alerts(&self, limit: u32) -> Result<Vec<Alert>, IrError> {
        self.ctx.monitor_engine.lock().await.get_recent_alerts(limit)
    }

    pub async fn is_background(&self) -> Result<bool, IrError> {
        Ok(self.ctx.monitor_engine.lock().await.is_background_mode())
    }

    pub async fn clear_alerts(&self) -> Result<u64, IrError> {
        self.ctx.monitor_engine.lock().await.clear_alerts()
    }

    pub async fn get_events(&self, limit: u32) -> Result<Vec<MonitorEvent>, IrError> {
        self.ctx.monitor_engine.lock().await.get_recent_events(limit)
    }

    pub async fn get_event_count(&self) -> Result<u64, IrError> {
        self.ctx.monitor_engine.lock().await.get_event_count()
    }

    pub async fn clear_events(&self) -> Result<u64, IrError> {
        self.ctx.monitor_engine.lock().await.clear_events()
    }

    pub async fn event_type_counts(&self) -> Result<Vec<(String, u64)>, IrError> {
        self.ctx.monitor_engine.lock().await.get_event_type_counts()
    }

    pub async fn get_db_size(&self) -> Result<u64, IrError> {
        self.ctx.monitor_engine.lock().await.get_db_size()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn search_events(
        &self,
        source: Option<String>,
        event_type: Option<String>,
        process_name: Option<String>,
        key_field: Option<String>,
        search_text: Option<String>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MonitorEvent>, IrError> {
        let query = EventQuery {
            source,
            event_type,
            process_name,
            key_field,
            is_external: None,
            search_text,
            limit,
            offset,
        };
        self.ctx.monitor_engine.lock().await.search_events(&query)
    }

    pub async fn search_event_page(&self, query: EventQuery) -> Result<EventPage, IrError> {
        self.ctx.monitor_engine.lock().await.search_events_page(&query)
    }

    pub async fn get_telemetry(&self) -> Result<RuntimeTelemetry, IrError> {
        Ok(self.ctx.monitor_engine.lock().await.get_telemetry())
    }

    pub async fn test_feishu(webhook_url: String) -> Result<(), IrError> {
        irtool_monitor::notify::test_feishu_webhook(&webhook_url).await
    }
}
