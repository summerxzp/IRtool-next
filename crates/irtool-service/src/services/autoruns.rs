use std::path::PathBuf;
use std::sync::atomic::Ordering;

use irtool_autoruns::{AutorunItem, DeleteResult, ScanOptions, ScanPhase, ScanProgress};
use irtool_core::{IrError, TaskId};

use crate::context::AppContext;
use crate::event_bus::AppEvent;

pub struct AutorunsService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> AutorunsService<'a> {
    /// Start an autoruns scan. Returns the task ID.
    /// Progress and completion events are published via the EventBus.
    pub async fn scan(&self, options: ScanOptions) -> Result<TaskId, IrError> {
        let scanner = self.ctx.autoruns_scanner.clone();
        if scanner.is_none() {
            tracing::error!("autoruns scan requested but scanner is not available");
            return Err(IrError::FeatureDisabled("autoruns scanner not available".into()));
        }

        let store = self.ctx.autoruns_store.clone();
        let scanning_flag = self.ctx.autoruns_scanning.clone();
        let tasks = self.ctx.tasks.clone();
        let event_bus = self.ctx.event_bus.clone();
        let (id, token) = tasks.register();

        tracing::info!("autoruns scan started, task_id={}", id);
        scanning_flag.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let task_id = id;
            let event_bus_for_progress = event_bus.clone();
            let event_bus_for_result = event_bus.clone();
            let progress = move |mut p: ScanProgress| {
                p.task_id = task_id;
                event_bus_for_progress.publish(AppEvent::AutorunsProgress(p));
            };

            let result = match scanner.as_ref() {
                Some(s) => s.scan(options, progress, token).await,
                None => Err(IrError::FeatureDisabled("autoruns scanner not available".into())),
            };

            scanning_flag.store(false, Ordering::SeqCst);

            match result {
                Ok(items) => {
                    let count = items.len();
                    store.clear_and_put(items);
                    event_bus_for_result.publish(AppEvent::AutorunsScanComplete { count });
                    // Also publish the final ScanProgress for frontend compatibility
                    event_bus_for_result.publish(AppEvent::AutorunsProgress(ScanProgress {
                        task_id,
                        phase: ScanPhase::Complete,
                        current: count,
                        total: count,
                        message: format!("扫描完成，共 {} 项", count),
                    }));
                }
                Err(IrError::Cancelled) => {
                    tracing::info!("autoruns scan cancelled, task_id={}", id);
                    event_bus_for_result.publish(AppEvent::AutorunsScanCancelled(id));
                }
                Err(e) => {
                    tracing::error!("autoruns scan failed, task_id={}: {}", id, e);
                    event_bus_for_result.publish(AppEvent::AutorunsScanFailed {
                        task_id: id,
                        error: e.to_string(),
                    });
                }
            }
            tasks.finish(id);
        });

        Ok(id)
    }

    pub async fn get_result(&self) -> Result<Vec<AutorunItem>, IrError> {
        Ok(self.ctx.autoruns_store.get_all())
    }

    /// Verify signatures for the given paths. Returns task ID.
    pub async fn verify_signatures(&self, paths: Vec<String>) -> Result<TaskId, IrError> {
        let scanner = self.ctx.autoruns_scanner.clone();
        if scanner.is_none() {
            return Err(IrError::FeatureDisabled("autoruns scanner not available".into()));
        }

        let store = self.ctx.autoruns_store.clone();
        let tasks = self.ctx.tasks.clone();
        let event_bus = self.ctx.event_bus.clone();
        let (id, token) = tasks.register();

        tokio::task::spawn_blocking(move || {
            let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
            let task_id = id;

            if let Some(s) = scanner.as_ref() {
                let results = s.verify_signatures_batch(&path_bufs, |current, total| {
                    if token.is_cancelled() {
                        return;
                    }
                    event_bus.publish(AppEvent::AutorunsSignatureProgress(
                        irtool_autoruns::SignatureProgress {
                            task_id,
                            current,
                            total,
                        },
                    ));
                });

                if !token.is_cancelled() {
                    for (path, status) in results {
                        store.update_signature(&path.to_string_lossy(), status);
                    }
                }
            }

            tasks.finish(id);
        });

        Ok(id)
    }

    pub async fn delete_entry(&self, entry_id: u64) -> Result<DeleteResult, IrError> {
        tracing::info!("delete entry requested: entry_id={}", entry_id);

        let scanner_clone = self.ctx.autoruns_scanner.clone();
        let item = self.ctx.autoruns_store.get(entry_id).ok_or_else(|| {
            tracing::error!("delete entry: entry not found, entry_id={}", entry_id);
            IrError::Internal("entry not found".into())
        })?;

        tracing::info!(
            "delete entry: category={}, entry={}, location={}",
            item.category,
            item.entry,
            item.location
        );

        // Clone item for logging after spawn_blocking consumes it
        let item_for_log = item.clone();
        let result = tokio::task::spawn_blocking(move || match scanner_clone.as_ref() {
            Some(s) => s.delete_entry(&item),
            None => Err(IrError::FeatureDisabled("autoruns scanner not available".into())),
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;

        tracing::info!(
            "delete entry result: success={}, message={}",
            result.success,
            result.message
        );

        if !result.success {
            // 删除失败时输出完整条目信息，便于诊断
            tracing::warn!(
                "delete entry failed, full item: id={}, category={}, entry={}, location={}, \
                 enabled={}, image_path={:?}, launch_string={:?}, service_name={:?}, \
                 description={}, publisher={}",
                item_for_log.id,
                item_for_log.category,
                item_for_log.entry,
                item_for_log.location,
                item_for_log.enabled,
                item_for_log.image_path,
                item_for_log.launch_string,
                item_for_log.service_name,
                item_for_log.description,
                item_for_log.publisher
            );
        }

        if result.success {
            self.ctx.autoruns_store.remove(entry_id);
        }

        Ok(result)
    }

    pub async fn cancel_scan(&self, task_id: TaskId) -> Result<(), IrError> {
        self.ctx.tasks.cancel(task_id);
        Ok(())
    }

    pub async fn calculate_hash(&self, entry_id: u64) -> Result<AutorunItem, IrError> {
        let item = self
            .ctx
            .autoruns_store
            .get(entry_id)
            .ok_or_else(|| IrError::Internal("entry not found".into()))?;
        let path_str = item
            .image_path
            .as_deref()
            .ok_or_else(|| IrError::Io("no image path".into()))?;
        let path = PathBuf::from(path_str);
        let (md5, sha256) =
            tokio::task::spawn_blocking(move || irtool_autoruns::AutorunsScanner::calculate_hash(&path))
                .await
                .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;
        self.ctx.autoruns_store.update_hash(entry_id, md5, sha256);
        self.ctx
            .autoruns_store
            .get(entry_id)
            .ok_or_else(|| IrError::Internal("entry not found after update".into()))
    }

    pub async fn batch_calculate_hash(&self, entry_ids: Vec<u64>) -> Result<TaskId, IrError> {
        let store = self.ctx.autoruns_store.clone();
        let tasks = self.ctx.tasks.clone();
        let event_bus = self.ctx.event_bus.clone();
        let (id, token) = tasks.register();

        tokio::task::spawn_blocking(move || {
            use rayon::prelude::*;

            let task_id = id;
            let total = entry_ids.len();
            let progress_counter = std::sync::atomic::AtomicUsize::new(0);

            type HashResult = (String, String);
            let results: Vec<(u64, Result<HashResult, IrError>)> = entry_ids
                .par_iter()
                .map(|&entry_id| {
                    if token.is_cancelled() {
                        return (entry_id, Err(IrError::Cancelled));
                    }
                    let item = match store.get(entry_id) {
                        Some(item) => item,
                        None => {
                            let current = progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                            event_bus.publish(AppEvent::AutorunsHashProgress(irtool_autoruns::SignatureProgress {
                                task_id,
                                current,
                                total,
                            }));
                            return (entry_id, Err(IrError::Internal("entry not found".into())));
                        }
                    };
                    let path_str = match item.image_path.as_deref() {
                        Some(p) => p,
                        None => {
                            let current = progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                            event_bus.publish(AppEvent::AutorunsHashProgress(irtool_autoruns::SignatureProgress {
                                task_id,
                                current,
                                total,
                            }));
                            return (entry_id, Err(IrError::Io("no image path".into())));
                        }
                    };
                    let path = PathBuf::from(path_str);
                    let result = irtool_autoruns::AutorunsScanner::calculate_hash(&path);
                    let current = progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    event_bus.publish(AppEvent::AutorunsHashProgress(irtool_autoruns::SignatureProgress {
                        task_id,
                        current,
                        total,
                    }));
                    (entry_id, result)
                })
                .collect();

            if token.is_cancelled() {
                tasks.finish(id);
                return;
            }

            for (entry_id, result) in results {
                if let Ok((md5, sha256)) = result {
                    store.update_hash(entry_id, md5, sha256);
                }
            }

            tasks.finish(id);
        });

        Ok(id)
    }

    pub async fn sigcheck(image_path: String) -> Result<String, IrError> {
        let sigcheck_path = irtool_autoruns::find_sigcheck()?;
        let file_path = PathBuf::from(&image_path);
        irtool_autoruns::AutorunsScanner::sigcheck_file(&sigcheck_path, &file_path)
    }

    pub fn open_explorer(path: String) -> Result<(), IrError> {
        irtool_autoruns::open_in_explorer(&path)
    }

    pub fn open_regedit(registry_path: String) -> Result<(), IrError> {
        irtool_autoruns::open_regedit(&registry_path)
    }

    pub fn open_services() -> Result<(), IrError> {
        irtool_autoruns::open_services_msc()
    }

    pub fn extract_icon(image_path: String) -> Result<Option<String>, IrError> {
        irtool_autoruns::extract_icon_base64(&image_path)
    }

    pub fn batch_extract_icons(paths: Vec<String>) -> Result<Vec<(String, Option<String>)>, IrError> {
        Ok(irtool_autoruns::batch_extract_icons(&paths))
    }

    pub fn is_scanning(&self) -> Result<bool, IrError> {
        Ok(self.ctx.autoruns_scanning.load(Ordering::SeqCst))
    }
}
