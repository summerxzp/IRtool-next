use crate::context::AppContext;
use crate::dto::app::AppInfo;
use irtool_core::IrError;

pub struct AppService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> AppService<'a> {
    pub fn app_info(is_admin: bool) -> AppInfo {
        AppInfo {
            name: "IRtool".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            is_admin,
        }
    }

    pub fn log_frontend(message: String) {
        tracing::warn!("[frontend] {}", message);
    }

    pub async fn force_quit(&self) -> Result<(), IrError> {
        tracing::info!("force quit requested, exiting background mode first");
        let _ = self.ctx.monitor_engine.lock().await.exit_background_mode();
        Ok(())
    }
}
