//! CDP 抓包服务：通过 CDP 远程调试抓取所有 target 的网络请求。
//!
//! 当 Helper Extension 因 Chrome 安全隔离无法 attach 到其他扩展的 SW 时，
//! 改用 CDP 远程调试方案：连接浏览器 `--remote-debugging-port=9222` 启动后暴露的
//! WebSocket，selective_attach 所有 page/service_worker/background_page target，
//! 监听 `Network.requestWillBeSent` 事件并发布为 `ExtensionAttribution` 事件。
//!
//! ## 设计要点
//!
//! - 不修改 irtool-cdp，在 service 层实现自己的过滤逻辑（含 `background_page`）
//! - 单一后台 task，`select!` 多路复用：事件 / 定期校准 / 停止信号
//! - WebSocket 断开自动重连（5s 退避）
//! - 收到事件后转换为 `ExtensionAttributionPayload` 发布到 EventBus

use std::sync::Arc;
use std::time::Duration;

use irtool_cdp::discovery::discover_targets;
use irtool_cdp::session::TargetInfo;
use irtool_cdp::{
    extract_extension_id, parse_request_will_be_sent, AttachResult, CdpClient, CdpError, CdpEvent, CdpTarget,
    SessionId, SessionManager,
};
use irtool_core::IrError;
use parking_lot::Mutex;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::dto::browser_forensics::ExtensionAttributionPayload;
use crate::event_bus::{AppEvent, EventBus};
use crate::services::browser_forensics::compute_attribution_level;

/// 周期校准间隔（秒）
const CALIBRATE_INTERVAL_SECS: u64 = 60;

/// WebSocket 断开后重连退避（秒）
const RECONNECT_BACKOFF_SECS: u64 = 5;

// ── 过滤与归因纯函数（可单测） ─────────────────────────────────

/// 判断 target 类型是否需要 attach。
///
/// 比 `SessionManager::should_attach` 多支持 `background_page`：
/// Edge 的 MV2 扩展在某些版本下用此类型承载 background 上下文。
fn should_attach_target(target_type: &str) -> bool {
    matches!(target_type, "page" | "service_worker" | "background_page")
}

/// 根据 target 类型和扩展 ID 是否存在，推断归因状态。
///
/// - 扩展 SW / background_page 且能提取出 extension_id → `high-confidence`
/// - page → `page-originated`
/// - 其他（含扩展 SW 但无法识别 extension_id）→ `browser-owned`
fn compute_attribution_status(target_type: &str, extension_id: Option<&String>) -> String {
    match target_type {
        "service_worker" | "background_page" => {
            if extension_id.is_some() {
                "high-confidence".to_string()
            } else {
                "browser-owned".to_string()
            }
        }
        "page" => "page-originated".to_string(),
        _ => "browser-owned".to_string(),
    }
}

/// 从 `Network.requestWillBeSent` 事件构造 `ExtensionAttributionPayload`。
///
/// 入参：
/// - `event`: CDP 事件（method 已确认是 `Network.requestWillBeSent`）
/// - `target_info`: 事件所属 session 对应的 target 信息
///
/// 错误：解析 params 失败时返回 `IrError::Internal`。
fn build_payload(event: &CdpEvent, target_info: &TargetInfo) -> Result<ExtensionAttributionPayload, IrError> {
    let cdp_req = parse_request_will_be_sent(&event.params)
        .map_err(|e| IrError::Internal(format!("parse cdp request failed: {}", e)))?;

    let extension_id = extract_extension_id(&target_info.url);
    let attribution_status = compute_attribution_status(&target_info.target_type, extension_id.as_ref());
    let level = compute_attribution_level(&attribution_status);
    let resource_type = event.params.get("type").and_then(|v| v.as_str()).map(String::from);

    // CDP 路径无法访问 chrome.management API 获取扩展友好名称，
    // 用 target_info.url 作为源头标识（如 chrome-extension://<id>/background.js）。
    // 这样前端能展示请求是由哪个 target（扩展 SW / 页面）发起的。
    let source_label = if extension_id.is_some() {
        Some(target_info.url.clone())
    } else {
        None
    };

    Ok(ExtensionAttributionPayload {
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        request_id: cdp_req.request_id,
        url: cdp_req.url,
        method: cdp_req.method,
        resource_type,
        initiator: cdp_req.initiator.url,
        attribution_status,
        extension_id,
        extension_name: source_label,
        level,
    })
}

// ── selective_attach 扩展版（含 background_page） ───────────────

/// 对 page/service_worker/background_page target 执行选择性 attach。
///
/// 与 `irtool_cdp::selective_attach` 的区别：
/// - 过滤逻辑用本模块的 `should_attach_target`（额外支持 `background_page`）
/// - 跳过已 attach 的 target（避免周期校准时重复 attach）
async fn selective_attach_with_bg_page(
    client: &CdpClient,
    manager: &mut SessionManager,
) -> Result<AttachResult, CdpError> {
    let targets = client.get_targets().await?;
    let mut attached = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for target in targets {
        if !should_attach_target(&target.target_type) {
            continue;
        }

        // 跳过已 attach 的 target（避免周期校准时重复 attach）
        if manager.iter().any(|(_, t)| t.target_id == target.target_id) {
            continue;
        }

        if manager.is_full() {
            skipped.push(target);
            continue;
        }

        match client.attach_to_target(&target.target_id).await {
            Ok(session_id) => {
                manager.insert(session_id.clone(), target.clone());
                attached.push((session_id, target));
            }
            Err(e) => {
                failed.push((target, e));
            }
        }
    }

    if !skipped.is_empty() {
        warn!(
            "selective_attach_with_bg_page: skipped {} targets (limit {})",
            skipped.len(),
            manager.limit()
        );
    }
    if !failed.is_empty() {
        warn!("selective_attach_with_bg_page: failed {} targets", failed.len());
    }
    info!(
        "selective_attach_with_bg_page: attached={}, skipped={}, failed={}",
        attached.len(),
        skipped.len(),
        failed.len()
    );

    Ok(AttachResult {
        attached,
        skipped,
        failed,
    })
}

/// 启用 `Target.setDiscoverTargets`，让浏览器推送 targetCreated/Destroyed 事件。
async fn enable_target_discovery(client: &CdpClient) -> Result<(), CdpError> {
    let mut params = serde_json::Map::new();
    params.insert("discover".to_string(), serde_json::Value::Bool(true));
    client
        .send_command("Target.setDiscoverTargets", serde_json::Value::Object(params))
        .await?;
    Ok(())
}

/// detach 所有已 attach 的 session（用于停止/重连前清理）。
async fn detach_all(client: &CdpClient, manager: &mut SessionManager) {
    let sids: Vec<SessionId> = manager.iter().map(|(s, _)| s.clone()).collect();
    for sid in sids {
        if let Err(e) = client.detach_from_target(&sid).await {
            warn!("detach failed for session {}: {}", sid.0, e);
        }
        manager.remove(&sid);
    }
}

/// 处理单个 CDP 事件。
///
/// - `Network.requestWillBeSent` → 构造 payload 发布到 EventBus
/// - `Target.targetCreated` → 新的 page/SW/background_page target 自动 attach
/// - `Target.targetDestroyed` → 对应 session detach
async fn handle_event(event: &CdpEvent, client: &CdpClient, manager: &mut SessionManager, event_bus: &EventBus) {
    match event.method.as_str() {
        "Network.requestWillBeSent" => {
            let Some(sid_str) = event.session_id.as_ref() else {
                return;
            };
            let sid = SessionId(sid_str.clone());
            let Some(target_info) = manager.get(&sid) else {
                return;
            };
            match build_payload(event, target_info) {
                Ok(payload) => {
                    event_bus.publish(AppEvent::ExtensionAttribution(payload));
                }
                Err(e) => {
                    warn!("build_payload failed: {}", e);
                }
            }
        }
        "Target.targetCreated" => {
            let Some(target_info) = event.params.get("targetInfo") else {
                return;
            };
            let target_type = target_info.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if !should_attach_target(target_type) {
                return;
            }
            let Some(target_id) = target_info.get("targetId").and_then(|v| v.as_str()) else {
                return;
            };
            let url = target_info
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = target_info
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let info = TargetInfo {
                target_id: target_id.to_string(),
                target_type: target_type.to_string(),
                url,
                title,
            };

            // 跳过已 attach 的 target
            if manager.iter().any(|(_, t)| t.target_id == target_id) {
                return;
            }

            if manager.is_full() {
                warn!("targetCreated: session full, skipping {}", target_id);
                return;
            }

            match client.attach_to_target(target_id).await {
                Ok(sid) => {
                    manager.insert(sid.clone(), info.clone());
                    if let Err(e) = client.enable_network(&sid).await {
                        warn!("enable_network for new target {} failed: {}", target_id, e);
                    }
                    info!(
                        "targetCreated: attached new target {} ({})",
                        target_id, info.target_type
                    );
                }
                Err(e) => {
                    warn!("targetCreated: attach {} failed: {}", target_id, e);
                }
            }
        }
        "Target.targetDestroyed" => {
            let Some(target_id) = event.params.get("targetId").and_then(|v| v.as_str()) else {
                return;
            };
            let sid_opt: Option<SessionId> = manager
                .iter()
                .find(|(_, t)| t.target_id == target_id)
                .map(|(s, _)| s.clone());
            if let Some(sid) = sid_opt {
                if let Err(e) = client.detach_from_target(&sid).await {
                    warn!("targetDestroyed: detach failed: {}", e);
                }
                manager.remove(&sid);
                info!("targetDestroyed: detached target {}", target_id);
            }
        }
        _ => {}
    }
}

// ── CdpCaptureService ──────────────────────────────────────────

pub struct CdpCaptureService {
    /// 停止信号发送端（`stop` 时 take 出发送；drop 时自动关闭，task 也会退出）
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
    /// 后台抓包 task 句柄
    task_handle: Mutex<Option<JoinHandle<()>>>,
}

impl CdpCaptureService {
    /// 创建并启动 CDP 抓包服务。
    ///
    /// 流程：
    /// 1. `discover_targets()` 发现浏览器调试端口（9222/9223/9229）
    /// 2. 取第一个 target 的 ws_url，启动后台 task
    /// 3. 后台 task 内部处理 connect/attach/event loop/重连
    pub async fn start(event_bus: EventBus) -> Result<Self, IrError> {
        let targets = discover_targets().await;
        let cdp_target = targets.into_iter().next().ok_or_else(|| {
            IrError::Internal(
                "no CDP target discovered (browser not running with --remote-debugging-port?)".to_string(),
            )
        })?;

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let task_handle = tokio::spawn(async move {
            run_capture_loop(cdp_target, event_bus, stop_rx).await;
        });

        Ok(Self {
            stop_tx: Mutex::new(Some(stop_tx)),
            task_handle: Mutex::new(Some(task_handle)),
        })
    }

    /// 停止抓包服务（detach 所有 session，关闭 WebSocket）。
    pub async fn stop(&self) -> Result<(), IrError> {
        let tx_opt = self.stop_tx.lock().take();
        let handle_opt = self.task_handle.lock().take();
        if let Some(tx) = tx_opt {
            let _ = tx.send(());
        }
        if let Some(handle) = handle_opt {
            let _ = handle.await;
        }
        Ok(())
    }
}

/// 后台抓包主循环：连接 → attach → 事件循环 → 断开重连。
///
/// 外层循环处理重连，内层 `select!` 多路复用事件/校准/停止信号。
/// `stop_rx` 完成（收到信号或发送端 drop）即清理 session 后退出。
async fn run_capture_loop(cdp_target: CdpTarget, event_bus: EventBus, mut stop_rx: oneshot::Receiver<()>) {
    let ws_url = cdp_target.web_socket_debugger_url.clone();
    info!("cdp capture loop started: ws_url={}", ws_url);

    loop {
        // 连接 CDP
        let client = match CdpClient::connect(&ws_url).await {
            Ok(c) => Arc::new(c),
            Err(e) => {
                error!("cdp connect failed: {}, retrying in {}s", e, RECONNECT_BACKOFF_SECS);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(RECONNECT_BACKOFF_SECS)) => {}
                    _ = &mut stop_rx => {
                        info!("cdp capture: stop signal during connect backoff");
                        return;
                    }
                }
                continue;
            }
        };

        let mut manager = SessionManager::new();

        // 启用 Target.setDiscoverTargets 让浏览器推送 targetCreated/Destroyed 事件
        if let Err(e) = enable_target_discovery(&client).await {
            warn!("enable_target_discovery failed: {}", e);
        }

        // 初始 selective_attach（含 background_page）
        match selective_attach_with_bg_page(&client, &mut manager).await {
            Ok(result) => {
                for (sid, _) in &result.attached {
                    if let Err(e) = client.enable_network(sid).await {
                        warn!("enable_network failed for session {}: {}", sid.0, e);
                    }
                }
                info!("cdp capture: {} sessions attached", manager.len());
            }
            Err(e) => {
                error!("selective_attach failed: {}", e);
            }
        }

        // 周期校准定时器（60s）
        let mut calibrate_timer = tokio::time::interval(Duration::from_secs(CALIBRATE_INTERVAL_SECS));
        calibrate_timer.tick().await; // 跳过首次立即触发

        // 事件循环
        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    info!("cdp capture: stop signal received, detaching all sessions");
                    detach_all(&client, &mut manager).await;
                    return;
                }
                _ = calibrate_timer.tick() => {
                    info!("cdp capture: periodic recalibration");
                    match selective_attach_with_bg_page(&client, &mut manager).await {
                        Ok(result) => {
                            for (sid, _) in &result.attached {
                                if let Err(e) = client.enable_network(sid).await {
                                    warn!("enable_network failed for session {}: {}", sid.0, e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("recalibration selective_attach failed: {}", e);
                        }
                    }
                }
                evt = client.recv_event() => {
                    let Some(evt) = evt else {
                        warn!("cdp capture: event stream closed, reconnecting");
                        break;
                    };
                    handle_event(&evt, &client, &mut manager, &event_bus).await;
                }
            }
        }

        // 事件流断开 → 退避后重连（select! 让停止信号可中断退避）
        info!("cdp capture: reconnecting in {}s", RECONNECT_BACKOFF_SECS);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(RECONNECT_BACKOFF_SECS)) => {}
            _ = &mut stop_rx => {
                info!("cdp capture: stop signal during reconnect backoff");
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use irtool_browser_forensics::AttributionLevel;
    use serde_json::json;

    fn make_target_info(target_type: &str, url: &str) -> TargetInfo {
        TargetInfo {
            target_id: format!("target-{}", target_type),
            target_type: target_type.to_string(),
            url: url.to_string(),
            title: "Test".to_string(),
        }
    }

    fn make_request_event(url: &str, method: &str, request_id: &str) -> CdpEvent {
        let params = json!({
            "requestId": request_id,
            "request": { "url": url, "method": method },
            "documentURL": "https://example.com/doc",
            "initiator": { "type": "script", "url": "https://example.com/script.js" },
            "type": "Fetch",
            "frameId": "frame-1",
            "loaderId": "loader-1",
            "timestamp": 1000.0
        });
        CdpEvent {
            method: "Network.requestWillBeSent".to_string(),
            params,
            session_id: Some("sess-1".to_string()),
        }
    }

    // ── should_attach_target 测试 ──────────────────────────────────

    #[test]
    fn should_attach_target_page() {
        assert!(should_attach_target("page"));
    }

    #[test]
    fn should_attach_target_service_worker() {
        assert!(should_attach_target("service_worker"));
    }

    #[test]
    fn should_attach_target_background_page() {
        // Edge MV2 扩展用 background_page，需 attach
        assert!(should_attach_target("background_page"));
    }

    #[test]
    fn should_not_attach_target_other_types() {
        assert!(!should_attach_target("iframe"));
        assert!(!should_attach_target("worker"));
        assert!(!should_attach_target("shared_worker"));
        assert!(!should_attach_target("browser"));
        assert!(!should_attach_target(""));
    }

    // ── compute_attribution_status 测试 ────────────────────────────

    #[test]
    fn attribution_status_service_worker_with_extension() {
        let ext_id = Some("abcdefghijklmnopabcdefghijklmnop".to_string());
        assert_eq!(
            compute_attribution_status("service_worker", ext_id.as_ref()),
            "high-confidence"
        );
    }

    #[test]
    fn attribution_status_background_page_with_extension() {
        let ext_id = Some("abcdefghijklmnopabcdefghijklmnop".to_string());
        assert_eq!(
            compute_attribution_status("background_page", ext_id.as_ref()),
            "high-confidence"
        );
    }

    #[test]
    fn attribution_status_service_worker_without_extension() {
        // SW 但 target_url 不是 chrome-extension:// → 无法识别扩展 ID → browser-owned
        assert_eq!(compute_attribution_status("service_worker", None), "browser-owned");
    }

    #[test]
    fn attribution_status_page() {
        assert_eq!(compute_attribution_status("page", None), "page-originated");
    }

    #[test]
    fn attribution_status_other_types() {
        assert_eq!(compute_attribution_status("browser", None), "browser-owned");
        assert_eq!(compute_attribution_status("worker", None), "browser-owned");
    }

    // ── build_payload 测试 ─────────────────────────────────────────

    #[test]
    fn build_payload_extension_service_worker_high_confidence() {
        let ext_id = "abcdefghijklmnopabcdefghijklmnop";
        let target = make_target_info(
            "service_worker",
            &format!("chrome-extension://{}/_generated_background_page.html", ext_id),
        );
        let evt = make_request_event("https://evil.com/exfil", "POST", "req-1");

        let payload = build_payload(&evt, &target).expect("build_payload should succeed");
        assert_eq!(payload.request_id, "req-1");
        assert_eq!(payload.url, "https://evil.com/exfil");
        assert_eq!(payload.method, "POST");
        assert_eq!(payload.resource_type.as_deref(), Some("Fetch"));
        assert_eq!(payload.initiator.as_deref(), Some("https://example.com/script.js"));
        assert_eq!(payload.attribution_status, "high-confidence");
        assert_eq!(payload.extension_id.as_deref(), Some(ext_id));
        // CDP 路径用 target URL 作为源头标识（无法访问 chrome.management API）
        assert_eq!(
            payload.extension_name.as_deref(),
            Some(format!("chrome-extension://{}/_generated_background_page.html", ext_id).as_str())
        );
        // high-confidence → Confirmed
        assert_eq!(payload.level, AttributionLevel::Confirmed);
    }

    #[test]
    fn build_payload_page_target_page_originated() {
        let target = make_target_info("page", "https://example.com/page");
        let evt = make_request_event("https://evil.com/xhr", "GET", "req-2");

        let payload = build_payload(&evt, &target).expect("build_payload should succeed");
        assert_eq!(payload.attribution_status, "page-originated");
        assert_eq!(payload.extension_id, None);
        // page-originated → Possible
        assert_eq!(payload.level, AttributionLevel::Possible);
    }

    #[test]
    fn build_payload_background_page_target_with_extension() {
        let ext_id = "abcdefghijklmnopabcdefghijklmnop";
        let target = make_target_info(
            "background_page",
            &format!("chrome-extension://{}/background.html", ext_id),
        );
        let evt = make_request_event("https://evil.com/bg", "GET", "req-3");

        let payload = build_payload(&evt, &target).expect("build_payload should succeed");
        assert_eq!(payload.attribution_status, "high-confidence");
        assert_eq!(payload.extension_id.as_deref(), Some(ext_id));
    }

    #[test]
    fn build_payload_service_worker_without_extension_id() {
        // SW 但 url 不是 chrome-extension://（例如一些网站注册的 service worker）
        let target = make_target_info("service_worker", "https://example.com/sw.js");
        let evt = make_request_event("https://example.com/x", "GET", "req-4");

        let payload = build_payload(&evt, &target).expect("build_payload should succeed");
        assert_eq!(payload.attribution_status, "browser-owned");
        assert_eq!(payload.extension_id, None);
    }

    #[test]
    fn build_payload_missing_request_id_returns_err() {
        let target = make_target_info("page", "https://example.com");
        let evt = CdpEvent {
            method: "Network.requestWillBeSent".to_string(),
            params: json!({
                "request": { "url": "https://example.com" },
                "initiator": { "type": "parser" }
            }),
            session_id: Some("sess-1".to_string()),
        };
        assert!(build_payload(&evt, &target).is_err());
    }

    #[test]
    fn build_payload_missing_type_field_resource_type_none() {
        // 缺失 type 字段 → resource_type 为 None
        let target = make_target_info("page", "https://example.com");
        let evt = CdpEvent {
            method: "Network.requestWillBeSent".to_string(),
            params: json!({
                "requestId": "req-no-type",
                "request": { "url": "https://example.com", "method": "GET" },
                "initiator": { "type": "parser" }
            }),
            session_id: Some("sess-1".to_string()),
        };
        let payload = build_payload(&evt, &target).expect("build_payload should succeed");
        assert!(payload.resource_type.is_none());
    }

    #[test]
    fn build_payload_timestamp_is_recent() {
        // 验证 timestamp 来自系统当前时间（非 CDP 事件 timestamp）
        let target = make_target_info("page", "https://example.com");
        let evt = make_request_event("https://example.com", "GET", "req-ts");

        let before = chrono::Utc::now().timestamp_millis() as u64;
        let payload = build_payload(&evt, &target).expect("build_payload should succeed");
        let after = chrono::Utc::now().timestamp_millis() as u64;

        assert!(
            payload.timestamp >= before && payload.timestamp <= after,
            "timestamp {} should be in [{}, {}]",
            payload.timestamp,
            before,
            after
        );
    }
}
