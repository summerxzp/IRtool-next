use std::path::PathBuf;

use irtool_browser_forensics::*;
use irtool_core::IrError;

use crate::context::AppContext;
use crate::dto::browser_forensics::*;

pub struct BrowserForensicsService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> BrowserForensicsService<'a> {
    /// 列出所有浏览器的 Profile
    pub async fn list_profiles(&self) -> Result<Vec<BrowserProfile>, IrError> {
        tokio::task::spawn_blocking(|| Ok(enumerate_all_profiles()))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描指定 Profile 的扩展
    pub async fn scan_extensions(
        &self,
        browser: BrowserKind,
        profile_name: &str,
    ) -> Result<ExtensionInventory, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(scan_extensions(&profile))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描所有 Profile 的扩展
    pub async fn scan_all_extensions(&self, browser: BrowserKind) -> Result<Vec<ExtensionInventory>, IrError> {
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            Ok(profiles.iter().map(scan_extensions).collect())
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// History 关联
    pub async fn attribute_history(
        &self,
        browser: BrowserKind,
        profile_name: &str,
        target_time: chrono::DateTime<chrono::Utc>,
    ) -> Result<HistoryAttribution, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(attribute_history(&profile, target_time))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描下载记录
    pub async fn scan_downloads(
        &self,
        browser: BrowserKind,
        profile_name: &str,
    ) -> Result<DownloadAttribution, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(irtool_browser_forensics::scan_downloads(&profile))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描历史记录
    pub async fn scan_history(&self, browser: BrowserKind, profile_name: &str, since: Option<i64>) -> Result<HistoryList, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(irtool_browser_forensics::scan_history(&profile, 500, since))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 恢复当前标签页
    pub async fn recover_tabs(
        &self,
        browser: BrowserKind,
        profile_name: &str,
    ) -> Result<SessionRecoveryResult, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(recover_tabs(&profile))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// Browser Context Attribution
    pub async fn attribute_browser_context(&self, req: ContextAttributionRequest) -> Result<BrowserContext, IrError> {
        tokio::task::spawn_blocking(move || {
            let timestamp = chrono::DateTime::parse_from_rfc3339(&req.timestamp)
                .map_err(|e| IrError::Internal(format!("invalid timestamp: {}", e)))?
                .to_utc();
            Ok(irtool_browser_forensics::attribute_browser_context(
                &req.domain,
                req.ip.as_deref(),
                &req.process_name,
                req.pid,
                timestamp,
                req.cmdline.as_deref(),
            ))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扩展归因 Layer 1
    pub async fn attribute_extension(
        &self,
        process_name: String,
        pid: u32,
        domain: String,
        cmdline: Option<String>,
    ) -> Result<Option<ExtensionAttribution>, IrError> {
        tokio::task::spawn_blocking(move || {
            Ok(irtool_browser_forensics::attribute_extension(
                &process_name,
                pid,
                &domain,
                cmdline.as_deref(),
            ))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }
}

// ── Native Messaging 队列读取 ───────────────────────────────

/// Native Messaging 队列文件所在目录
fn native_queue_dir() -> PathBuf {
    std::env::temp_dir().join("irtool").join("attr-queue")
}

/// Native Messaging 队列文件路径
fn native_queue_path() -> PathBuf {
    native_queue_dir().join("attr-queue.jsonl")
}

/// 读取 Native Messaging 队列文件中的消息并清空队列。
///
/// 使用原子 rename 策略避免并发丢失：将队列文件 rename 为 .processing 后缀，
/// 读取并解析完成后再删除 .processing 文件。Host 端继续写入新的队列文件。
pub fn read_native_messaging_queue() -> Vec<NativeQueueMessage> {
    let queue_path = native_queue_path();
    if !queue_path.exists() {
        return vec![];
    }

    // 原子 rename：queue → queue.processing，Host 端会创建新的 queue 文件
    let processing_path = queue_path.with_extension("jsonl.processing");
    if let Err(e) = std::fs::rename(&queue_path, &processing_path) {
        // rename 失败可能是因为 Host 端刚删除/重建了文件，回退到直接读取
        tracing::debug!("rename queue failed, falling back to direct read: {}", e);
        return read_queue_file(&queue_path);
    }

    let messages = read_queue_file(&processing_path);

    // 处理完成后删除 .processing 文件
    let _ = std::fs::remove_file(&processing_path);

    messages
}

/// 从指定路径读取队列文件并解析
fn read_queue_file(path: &std::path::Path) -> Vec<NativeQueueMessage> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to read queue file {}: {}", path.display(), e);
            return vec![];
        }
    };

    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            match serde_json::from_str::<NativeQueueMessage>(line) {
                Ok(msg) => Some(msg),
                Err(e) => {
                    tracing::warn!("failed to parse native queue message: {}", e);
                    None
                }
            }
        })
        .collect()
}

/// 从 Native Messaging 队列读取消息并发布到 EventBus。
///
/// 当前处理以下消息类型：
/// - `network_batch` → 提取 events 数组中的每条请求，发布为 `ExtensionAttribution` 事件
/// - `extension_list` → 记录扩展清单更新
/// - `heartbeat` → 忽略
///
/// 返回本次处理的消息数量。
pub fn publish_native_events(event_bus: &crate::event_bus::EventBus) -> usize {
    let messages = read_native_messaging_queue();
    if messages.is_empty() {
        return 0;
    }

    let mut event_count = 0;

    for msg in &messages {
        match msg.msg_type.as_str() {
            "network_batch" => {
                if let Some(events) = msg.payload.get("events").and_then(|v| v.as_array()) {
                    for evt in events {
                        if let Ok(req) = serde_json::from_value::<AttributedWebRequest>(evt.clone())
                        {
                            tracing::info!(
                                "native attribution: {} {} → {:?}",
                                req.method,
                                req.url,
                                req.attribution.status,
                            );

                            // 发布扩展归因事件到 EventBus
                            event_bus.publish(crate::event_bus::AppEvent::ExtensionAttribution(
                                ExtensionAttributionPayload {
                                    timestamp: req.timestamp,
                                    request_id: req.request_id,
                                    url: req.url,
                                    method: req.method,
                                    initiator: req.initiator,
                                    attribution_status: req.attribution.status,
                                    extension_id: req.attribution.extension_id,
                                    extension_name: req.attribution.extension_name,
                                },
                            ));

                            event_count += 1;
                        }
                    }
                }
            }
            "extension_list" => {
                let mode = msg
                    .payload
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let count = msg
                    .payload
                    .get("extensions")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                tracing::info!(
                    "native extension list: mode={}, count={}",
                    mode,
                    count
                );

                // 解析扩展清单条目
                if let Some(extensions) = msg.payload.get("extensions").and_then(|v| v.as_array())
                {
                    for ext in extensions {
                        if let Ok(entry) =
                            serde_json::from_value::<ExtensionListEntry>(ext.clone())
                        {
                            tracing::debug!(
                                "  extension: {} (id={}, enabled={})",
                                entry.name.as_deref().unwrap_or("?"),
                                entry.id,
                                entry.enabled,
                            );
                        }
                    }
                }

                event_count += count;
            }
            "heartbeat" => {
                // 心跳消息 → 扩展在线，确保 config 文件存在供 NMH 检测
                tracing::trace!("native heartbeat, ensuring config file exists");
                // 确保 NMH host 有 config.json 可检测（内容为空表示不过滤）
                if !native_config_path().exists() {
                    send_config(&[]);
                }
            }
            other => {
                tracing::debug!("native unhandled message type: {}", other);
            }
        }
    }

    event_count
}

// ── Config 下行通道 ────────────────────────────────────────

/// Native Messaging config 文件路径
fn native_config_path() -> PathBuf {
    std::env::temp_dir().join("irtool").join("config.json")
}

/// 向 Helper Extension 下发配置（下行通道）。
///
/// 写入 `%TEMP%\irtool\config.json`，NMH host 进程在事件循环中检测到此文件
/// 变化后，会通过 stdout 向扩展转发 `{"type":"config","filterDomains":[...]}` 消息。
///
/// 传递空切片可清除过滤规则（取消域名过滤）。
pub fn send_config(filter_domains: &[String]) {
    let config_path = native_config_path();
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let content = serde_json::json!({
        "filterDomains": filter_domains,
    });

    match std::fs::write(&config_path, serde_json::to_string_pretty(&content).unwrap()) {
        Ok(_) => tracing::info!(
            "config written to {:?} ({} domains)",
            config_path,
            filter_domains.len()
        ),
        Err(e) => tracing::error!("failed to write config {:?}: {}", config_path, e),
    }
}
