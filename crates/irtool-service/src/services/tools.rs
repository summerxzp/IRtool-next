use irtool_core::IrError;
use irtool_tools::{self as tools, ToolStatus};

use crate::context::AppContext;
use crate::event_bus::AppEvent;

pub struct ToolsService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> ToolsService<'a> {
    pub async fn check(&self) -> Result<Vec<ToolStatus>, IrError> {
        let tools_dir = self.ctx.app_dirs.tools_dir();
        if !tools_dir.exists() {
            let _ = std::fs::create_dir_all(&tools_dir);
        }
        Ok(tools::check_tools(&tools_dir))
    }

    pub async fn download(&self, tool_ids: Vec<String>) -> Result<(), IrError> {
        let tools_dir = self.ctx.app_dirs.tools_dir();
        if !tools_dir.exists() {
            let _ = std::fs::create_dir_all(&tools_dir);
        }

        let event_bus = self.ctx.event_bus.clone();
        let mut handles = Vec::new();

        for tool_id in tool_ids {
            let event_bus_clone = event_bus.clone();
            let tools_dir_clone = tools_dir.clone();
            let id = tool_id.clone();

            let handle = tokio::spawn(async move {
                let event_bus_for_progress = event_bus_clone.clone();
                let id_for_progress = id.clone();
                let result = tools::download_tool(&id, &tools_dir_clone, move |downloaded, total| {
                    event_bus_for_progress.publish(AppEvent::ToolsDownloadProgress {
                        tool_id: id_for_progress.clone(),
                        downloaded,
                        total,
                    });
                })
                .await;

                if result.is_ok() {
                    if let Err(e) = tools::accept_eula(&id, &tools_dir_clone).await {
                        tracing::warn!("接受 EULA 失败 (非致命): {}", e);
                    }
                }

                (id, result)
            });

            handles.push(handle);
        }

        let mut errors = Vec::new();

        for handle in handles {
            match handle.await {
                Ok((tool_id, Err(e))) => {
                    tracing::error!("下载工具 {} 失败: {}", tool_id, e);
                    event_bus.publish(AppEvent::ToolsDownloadError {
                        tool_id,
                        error: e.to_string(),
                    });
                    errors.push(e);
                }
                Ok((_, Ok(()))) => {}
                Err(e) => {
                    tracing::error!("下载任务异常: {}", e);
                }
            }
        }

        event_bus.publish(AppEvent::ToolsDownloadComplete { errors: errors.len() });

        Ok(())
    }

    pub async fn import_zip(&self, tool_id: String, zip_path: String) -> Result<(), IrError> {
        let tools_dir = self.ctx.app_dirs.tools_dir();
        if !tools_dir.exists() {
            let _ = std::fs::create_dir_all(&tools_dir);
        }
        let zip = std::path::PathBuf::from(&zip_path);
        if !zip.exists() {
            return Err(IrError::Internal(format!("ZIP 文件不存在: {}", zip_path)));
        }
        tools::import_tool_zip(&tool_id, &tools_dir, &zip)?;

        if let Err(e) = tools::accept_eula(&tool_id, &tools_dir).await {
            tracing::warn!("接受 EULA 失败 (非致命): {}", e);
        }

        Ok(())
    }
}
