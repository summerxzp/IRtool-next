use std::path::PathBuf;

use irtool_browser_forensics::*;
use irtool_core::IrError;

use crate::context::AppContext;
use crate::dto::browser_forensics::*;
use crate::services::extension_connection::ExtensionConnectionState;

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
            Ok(attribute_history(&profile, target_time, ""))
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
    pub async fn scan_history(
        &self,
        browser: BrowserKind,
        profile_name: &str,
        since: Option<i64>,
    ) -> Result<HistoryList, IrError> {
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
    pub async fn attribute_browser_context(&self, req: ContextAttributionRequest) -> Result<EvidenceObject, IrError> {
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

/// 根据扩展归因状态计算 AttributionLevel
///
/// Helper Extension 上报的 status 取值（见 helper-extension/service_worker.js）：
/// - "high-confidence" → 扩展 initiator 铁证，Confirmed
/// - "page-originated" → 页面 origin 发起，非扩展归因，Possible
/// - "browser-owned"  → 无 initiator，非扩展归因，Possible
///
/// 兼容历史值 "matched"（早期文档使用）。
pub fn compute_attribution_level(status: &str) -> AttributionLevel {
    if status == "matched" || status == "high-confidence" {
        AttributionLevel::Confirmed
    } else {
        AttributionLevel::Possible
    }
}

/// 从 Native Messaging 队列读取消息并发布到 EventBus。
///
/// 当前处理以下消息类型：
/// - `network_batch` → 提取 events 数组中的每条请求，发布为 `ExtensionAttribution` 事件
/// - `extension_list` → 记录扩展清单更新
/// - `heartbeat` → 记录心跳时间戳，确保 config 文件存在供 NMH 检测
///
/// 所有消息类型都会更新 `ExtensionConnectionState` 的时间戳，供前端轮询判断扩展是否在线。
///
/// 返回本次处理的消息数量。
pub fn publish_native_events(event_bus: &crate::event_bus::EventBus, conn: &ExtensionConnectionState) -> usize {
    let messages = read_native_messaging_queue();
    if messages.is_empty() {
        return 0;
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut event_count = 0;

    for msg in &messages {
        match msg.msg_type.as_str() {
            "network_batch" => {
                if let Some(events) = msg.payload.get("events").and_then(|v| v.as_array()) {
                    for evt in events {
                        if let Ok(req) = serde_json::from_value::<AttributedWebRequest>(evt.clone()) {
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
                                    resource_type: req.resource_type,
                                    initiator: req.initiator,
                                    attribution_status: req.attribution.status.clone(),
                                    extension_id: req.attribution.extension_id,
                                    extension_name: req.attribution.extension_name,
                                    level: compute_attribution_level(&req.attribution.status),
                                    // webRequest 路径无 CDP target 信息
                                    target_type: None,
                                    target_title: None,
                                    initiator_type: None,
                                },
                            ));

                            event_count += 1;
                        }
                    }
                }
                conn.record_event(now_ms);
            }
            "extension_list" => {
                let mode = msg.payload.get("mode").and_then(|v| v.as_str()).unwrap_or("unknown");
                let count = msg
                    .payload
                    .get("extensions")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                tracing::info!("native extension list: mode={}, count={}", mode, count);

                // 解析扩展清单条目
                if let Some(extensions) = msg.payload.get("extensions").and_then(|v| v.as_array()) {
                    for ext in extensions {
                        if let Ok(entry) = serde_json::from_value::<ExtensionListEntry>(ext.clone()) {
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
                conn.record_event(now_ms);
            }
            "heartbeat" => {
                // 心跳消息 → 扩展在线，记录心跳时间戳并确保 config 文件存在供 NMH 检测
                tracing::debug!("native heartbeat received, extension is online");
                conn.record_heartbeat(now_ms);
                // 确保 NMH host 有 config.json 可检测（内容为空表示不过滤）
                // 此处失败不影响事件处理主流程，仅记录日志
                if !native_config_path().exists() {
                    if let Err(e) = send_config(&[]) {
                        tracing::warn!("failed to create initial config.json on heartbeat: {}", e);
                    }
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
/// 变化后，会通过 stdout 向扩展转发 `{"type":"config",...}` 消息。
///
/// 传递空切片可清除过滤规则（取消域名过滤）。
///
/// 返回 Err 时调用方应向 UI 报错（避免静默失败导致用户以为已下发但实际没写）。
pub fn send_config(filter_domains: &[String]) -> Result<(), String> {
    let config_path = native_config_path();
    if let Some(parent) = config_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err(format!("failed to create config dir {:?}: {}", parent, e));
        }
    }

    let content = serde_json::json!({
        "filterDomains": filter_domains,
    });

    let json_str = serde_json::to_string_pretty(&content).map_err(|e| format!("serialize config failed: {}", e))?;

    std::fs::write(&config_path, &json_str).map_err(|e| format!("failed to write config {:?}: {}", config_path, e))?;

    tracing::info!("config written to {:?} ({} domains)", config_path, filter_domains.len());
    Ok(())
}

/// 读取当前已下发的 filterDomains（用于 UI 启动时同步显示）。
///
/// 从 `%TEMP%\irtool\config.json` 中读取 `filterDomains` 字段。
/// 文件不存在、解析失败或字段缺失时返回空 Vec。
///
/// 用途：UI 重启后 store 清空，但磁盘上的 config 仍然有效，扩展仍按旧配置
/// 过滤。启动时调用此函数把已下发的域名同步回 UI，避免"已下发但 UI 看不到"
/// 的不一致。
pub fn get_native_config_filter_domains() -> Vec<String> {
    let config_path = native_config_path();
    let content = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                path = ?config_path,
                error = %e,
                "failed to parse native config json, returning empty filter list"
            );
            return Vec::new();
        }
    };

    match config.get("filterDomains").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
        None => Vec::new(),
    }
}

/// 向 Helper Extension 下发重连信号（下行通道）。
///
/// 在 config.json 中写入 `reconnectSignal: true`，NMH 检测到文件变化后
/// 通过 stdout 转发给扩展。扩展收到后立即重置退避计数器并尝试重连，
/// 省去等待指数退避的时间。
///
/// 注意：此函数保留现有的 filterDomains 不变，只追加 reconnectSignal 字段。
pub fn send_reconnect_signal() {
    let config_path = native_config_path();
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // 读取现有 config（保留 filterDomains），追加 reconnectSignal
    let mut config: serde_json::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    // 确保是 object
    if !config.is_object() {
        config = serde_json::json!({});
    }

    // 写入 reconnectSignal（用时间戳保证 mtime 变化，即使连续点击也能触发）
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if let Some(obj) = config.as_object_mut() {
        obj.insert("reconnectSignal".to_string(), serde_json::json!(now_ms));
    }

    match std::fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).unwrap_or_else(|e| {
            tracing::error!("serialize config failed: {e}");
            "{}".to_string()
        }),
    ) {
        Ok(_) => tracing::info!("reconnect signal written to {:?} (ts={})", config_path, now_ms),
        Err(e) => tracing::error!("failed to write reconnect signal {:?}: {}", config_path, e),
    }
}

/// 向 Helper Extension 下发自我卸载信号（手动清理）。
///
/// 在 config.json 中写入 `selfUninstall: <timestamp>`，NMH 检测到文件变化后
/// 通过 stdout 转发给扩展。扩展收到后调用 `chrome.management.uninstallSelf()`
/// 立即卸载自己。
///
/// 用途：应急响应场景，用户在 IRtool UI 点击"清理扩展"按钮时调用。
/// 与自动清理（selfCleanupTimeoutMin）配合使用：
/// - 自动清理：IRtool 离线超时后扩展自动卸载
/// - 手动清理：用户主动触发立即卸载
pub fn send_self_uninstall_signal() {
    let config_path = native_config_path();
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // 读取现有 config（保留 filterDomains / selfCleanupTimeoutMin 等），追加 selfUninstall
    let mut config: serde_json::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if !config.is_object() {
        config = serde_json::json!({});
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if let Some(obj) = config.as_object_mut() {
        obj.insert("selfUninstall".to_string(), serde_json::json!(now_ms));
    }

    match std::fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).unwrap_or_else(|e| {
            tracing::error!("serialize config failed: {e}");
            "{}".to_string()
        }),
    ) {
        Ok(_) => tracing::info!("self-uninstall signal written to {:?} (ts={})", config_path, now_ms),
        Err(e) => tracing::error!("failed to write self-uninstall signal {:?}: {}", config_path, e),
    }
}

/// 设置扩展自我清理超时时间（分钟）。
///
/// 写入 config.json 的 `selfCleanupTimeoutMin` 字段，扩展读取后调整定时器：
/// - 0 = 禁用自动清理
/// - >0 = 启用，IRtool 离线超过该时长后扩展自动卸载
///
/// 默认值 60 分钟（在扩展端 DEFAULT_SELF_CLEANUP_TIMEOUT_MIN 常量中定义）。
/// 此函数用于用户在 IRtool UI 中修改超时时间时持久化到 config。
pub fn set_self_cleanup_timeout(timeout_min: u32) {
    let config_path = native_config_path();
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut config: serde_json::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if !config.is_object() {
        config = serde_json::json!({});
    }

    if let Some(obj) = config.as_object_mut() {
        obj.insert("selfCleanupTimeoutMin".to_string(), serde_json::json!(timeout_min));
    }

    match std::fs::write(
        &config_path,
        serde_json::to_string_pretty(&config).unwrap_or_else(|e| {
            tracing::error!("serialize config failed: {e}");
            "{}".to_string()
        }),
    ) {
        Ok(_) => tracing::info!(
            "self-cleanup timeout written to {:?} ({} min)",
            config_path,
            timeout_min
        ),
        Err(e) => tracing::error!("failed to write self-cleanup timeout {:?}: {}", config_path, e),
    }
}

/// 读取当前 selfCleanupTimeoutMin 配置（用于 UI 显示当前值）。
///
/// 文件不存在/字段缺失时返回 None，UI 可用默认值 60 显示。
pub fn get_self_cleanup_timeout() -> Option<u32> {
    let config_path = native_config_path();
    let content = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    config
        .get("selfCleanupTimeoutMin")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_attribution_level_matched_returns_confirmed() {
        assert_eq!(compute_attribution_level("matched"), AttributionLevel::Confirmed);
        assert_eq!(
            compute_attribution_level("high-confidence"),
            AttributionLevel::Confirmed
        );
    }

    #[test]
    fn compute_attribution_level_unmatched_returns_possible() {
        assert_eq!(compute_attribution_level("page-originated"), AttributionLevel::Possible);
        assert_eq!(compute_attribution_level("browser-owned"), AttributionLevel::Possible);
        assert_eq!(compute_attribution_level("unmatched"), AttributionLevel::Possible);
        assert_eq!(compute_attribution_level("unknown"), AttributionLevel::Possible);
        assert_eq!(compute_attribution_level(""), AttributionLevel::Possible);
    }

    // ── C3 回归测试：JS 上报 camelCase，Rust 结构体必须能正确反序列化 ──

    #[test]
    fn attributed_web_request_deserializes_camel_case() {
        // 与 service_worker.js onBeforeRequest 上报格式一致
        let json = serde_json::json!({
            "timestamp": 1719400000000u64,
            "requestId": "req-123",
            "url": "https://evil.com/payload.zip",
            "method": "GET",
            "initiator": "chrome-extension://abcdefghijklmnopqrstuvwxyz123456/",
            "attribution": {
                "status": "high-confidence",
                "extensionId": "abcdefghijklmnopqrstuvwxyz123456",
                "extensionName": "Suspicious Ext"
            }
        });
        let req: AttributedWebRequest = serde_json::from_value(json).expect("camelCase JSON should deserialize");
        assert_eq!(req.request_id, "req-123");
        assert_eq!(
            req.attribution.extension_id.as_deref(),
            Some("abcdefghijklmnopqrstuvwxyz123456")
        );
        assert_eq!(req.attribution.extension_name.as_deref(), Some("Suspicious Ext"));
        assert_eq!(req.attribution.status, "high-confidence");
    }

    #[test]
    fn attributed_web_request_old_event_without_resource_type_deserializes_as_none() {
        // 旧事件（webRequest 时代）没有 resourceType 和 sourceTarget 字段
        let json = serde_json::json!({
            "timestamp": 1719400000000u64,
            "requestId": "req-old-123",
            "url": "https://example.com/",
            "method": "GET",
            "initiator": "https://example.com/",
            "attribution": {
                "status": "page-originated",
                "extensionId": null,
                "extensionName": null
            }
        });
        let req: AttributedWebRequest = serde_json::from_value(json).expect("old event should deserialize");
        assert_eq!(req.resource_type, None, "old event resource_type should be None");
        assert_eq!(req.source_target, None, "old event source_target should be None");
        assert_eq!(req.attribution.status, "page-originated");
    }

    #[test]
    fn attributed_web_request_new_event_with_resource_type_deserializes() {
        // 新事件（debugger API 时代）有 resourceType 和 sourceTarget 字段
        let json = serde_json::json!({
            "timestamp": 1719400000001u64,
            "requestId": "req-new-456",
            "url": "https://evil.com/exfil",
            "method": "POST",
            "resourceType": "Fetch",
            "initiator": "chrome-extension://mbgpppibaejglkaklcbceckkicbhkalp/",
            "attribution": {
                "status": "high-confidence",
                "extensionId": "mbgpppibaejglkaklcbceckkicbhkalp",
                "extensionName": "Suspicious Ext"
            },
            "sourceTarget": {
                "type": "service_worker",
                "targetId": "target-abc",
                "extensionId": "mbgpppibaejglkaklcbceckkicbhkalp"
            }
        });
        let req: AttributedWebRequest = serde_json::from_value(json).expect("new event should deserialize");
        assert_eq!(req.resource_type.as_deref(), Some("Fetch"));
        assert_eq!(
            req.source_target.as_ref().map(|s| s.target_type.as_str()),
            Some("service_worker")
        );
        assert_eq!(
            req.source_target.as_ref().and_then(|s| s.extension_id.as_deref()),
            Some("mbgpppibaejglkaklcbceckkicbhkalp")
        );
    }

    #[test]
    fn extension_list_entry_deserializes_camel_case() {
        // 与 service_worker.js reportExtensionListFull 上报格式一致
        let json = serde_json::json!({
            "id": "extid",
            "name": "Test Extension",
            "version": "1.0.0",
            "enabled": true,
            "hostPermissions": ["<all_urls>"],
            "installType": "development"
        });
        let entry: ExtensionListEntry = serde_json::from_value(json).expect("camelCase JSON should deserialize");
        assert_eq!(entry.install_type, "development");
        assert_eq!(entry.host_permissions.as_deref(), Some(&["<all_urls>".to_string()][..]));
    }
}
