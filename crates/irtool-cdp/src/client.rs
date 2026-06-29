//! WebSocket CDP 客户端。
//!
//! 连接 browser-level WebSocket，发送 CDP 命令并接收响应/事件。
//!
//! ## 设计要点
//!
//! - 后台 task 持续读 WebSocket，按 id 匹配响应（oneshot channel）
//! - 事件（无 id 或 method 以 `.` 分隔的非命令响应）推到事件 channel
//! - `Target.attachToTarget` 传 `flatten: true`，使用扁平化 session id
//! - 命令超时保护（默认 10s）

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

use crate::session::{SessionId, SessionManager, TargetInfo};
use crate::CdpError;

/// CDP 命令请求
///
/// `session_id` 为 `None` 时表示 browser-level 命令；
/// 为 `Some(sid)` 时表示对 attached 子 session 发命令（CDP 协议要求 sessionId 在消息顶层）。
/// 序列化时字段名用 camelCase（CDP 协议要求 `sessionId`）。
#[derive(Debug, Serialize)]
struct CdpCommand {
    id: u64,
    method: String,
    params: Value,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sessionId")]
    session_id: Option<String>,
}

/// CDP 响应（带 id 的是命令响应）
#[derive(Debug, Deserialize)]
struct CdpResponse {
    id: u64,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<CdpErrorPayload>,
}

/// CDP 错误负载
#[derive(Debug, Deserialize)]
struct CdpErrorPayload {
    code: i64,
    message: String,
}

/// CDP 事件（无 id，有 method 和 params）
///
/// `session_id` 来自消息 JSON 顶层的 `sessionId` 字段（flatten session 模式下子 session 事件），
/// 由 serde 直接反序列化。browser-level 事件 session_id 为 None。
#[derive(Debug, Clone, Deserialize)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
}

/// 待处理命令的响应 channel 映射（id -> oneshot sender）
type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>>;

/// CDP 客户端
pub struct CdpClient {
    /// WebSocket 写端（通过 channel 发送，避免直接持有写端导致 Send 问题）
    write_tx: mpsc::Sender<Message>,
    /// 待处理命令的响应 channel 映射（id -> oneshot sender）
    pending: PendingMap,
    /// 事件接收 channel（后台 task 推送）
    event_rx: Mutex<mpsc::Receiver<CdpEvent>>,
    /// 下一个命令 id
    next_id: Mutex<u64>,
}

impl CdpClient {
    /// 连接 browser-level WebSocket。
    ///
    /// 启动后台 task 持续读 WebSocket 并分发响应/事件。
    pub async fn connect(ws_url: &str) -> Result<Self, CdpError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|e| CdpError::WebSocket(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        let (write_tx, mut write_rx) = mpsc::channel::<Message>(32);
        let (event_tx, event_rx) = mpsc::channel::<CdpEvent>(128);
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

        // 写 task：从 write_rx 取消息发到 WebSocket
        tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // 读 task：持续读 WebSocket，分发响应和事件
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                let msg = match msg {
                    Ok(Message::Text(t)) => t,
                    Ok(Message::Close(_)) => {
                        debug!("cdp websocket closed");
                        break;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        warn!("cdp websocket read error: {}", e);
                        break;
                    }
                };

                // 尝试解析为响应（有 id）
                if let Ok(resp) = serde_json::from_str::<CdpResponse>(&msg) {
                    let mut p = pending_clone.lock().await;
                    if let Some(tx) = p.remove(&resp.id) {
                        let result = match resp.error {
                            Some(e) => Err(CdpError::Protocol {
                                code: e.code,
                                message: e.message,
                            }),
                            None => Ok(resp.result.unwrap_or(Value::Null)),
                        };
                        let _ = tx.send(result);
                    }
                    continue;
                }

                // 尝试解析为事件（有 method，无 id）
                // CdpEvent 的 session_id 由 serde 直接从消息 JSON 顶层 sessionId 字段反序列化
                if let Ok(evt) = serde_json::from_str::<CdpEvent>(&msg) {
                    let _ = event_tx.send(evt).await;
                }
            }
        });

        Ok(Self {
            write_tx,
            pending,
            event_rx: Mutex::new(event_rx),
            next_id: Mutex::new(1),
        })
    }

    /// 发送 CDP 命令并等待响应（10s 超时）。
    pub async fn send_command(&self, method: &str, params: Value) -> Result<Value, CdpError> {
        self.send_command_with_timeout(method, params, Duration::from_secs(10))
            .await
    }

    /// 发送 CDP 命令并等待响应（自定义超时）。
    pub async fn send_command_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, CdpError> {
        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let cmd = CdpCommand {
            id,
            method: method.to_string(),
            params,
            session_id: None,
        };
        let msg = serde_json::to_string(&cmd)?;
        let (tx, rx) = oneshot::channel();

        {
            let mut p = self.pending.lock().await;
            p.insert(id, tx);
        }

        self.write_tx
            .send(Message::Text(msg))
            .await
            .map_err(|_| CdpError::WebSocket("write channel closed".to_string()))?;

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.pending.lock().await.remove(&id);
                Err(CdpError::WebSocket("response channel dropped".to_string()))
            }
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(CdpError::WebSocket(format!("command {} timeout", method)))
            }
        }
    }

    /// 接收下一个 CDP 事件（阻塞直到有事件）。
    pub async fn recv_event(&self) -> Option<CdpEvent> {
        self.event_rx.lock().await.recv().await
    }

    /// 调用 `Target.getTargets` 获取所有 target 列表。
    pub async fn get_targets(&self) -> Result<Vec<TargetInfo>, CdpError> {
        let result = self
            .send_command("Target.getTargets", Value::Object(serde_json::Map::new()))
            .await?;
        let target_infos: Vec<RawTargetInfo> = result
            .get("targetInfos")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        Ok(target_infos.into_iter().map(Into::into).collect())
    }

    /// 调用 `Target.attachToTarget`（flatten: true），返回 session id。
    pub async fn attach_to_target(&self, target_id: &str) -> Result<SessionId, CdpError> {
        let mut params = serde_json::Map::new();
        params.insert("targetId".to_string(), Value::String(target_id.to_string()));
        params.insert("flatten".to_string(), Value::Bool(true));
        let result = self
            .send_command("Target.attachToTarget", Value::Object(params))
            .await?;
        let session_id = result
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CdpError::Protocol {
                code: -1,
                message: "attachToTarget missing sessionId".to_string(),
            })?;
        Ok(SessionId(session_id.to_string()))
    }

    /// 调用 `Target.detachFromTarget`。
    pub async fn detach_from_target(&self, session_id: &SessionId) -> Result<(), CdpError> {
        let mut params = serde_json::Map::new();
        params.insert("sessionId".to_string(), Value::String(session_id.0.clone()));
        self.send_command("Target.detachFromTarget", Value::Object(params))
            .await?;
        Ok(())
    }

    /// 对 attached session 发送命令（sessionId 放在消息顶层，符合 CDP 协议）。
    pub async fn send_session_command(
        &self,
        session_id: &SessionId,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpError> {
        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let cmd = CdpCommand {
            id,
            method: method.to_string(),
            params,
            session_id: Some(session_id.0.clone()),
        };
        let msg = serde_json::to_string(&cmd)?;
        let (tx, rx) = oneshot::channel();

        {
            let mut p = self.pending.lock().await;
            p.insert(id, tx);
        }

        self.write_tx
            .send(Message::Text(msg))
            .await
            .map_err(|_| CdpError::WebSocket("write channel closed".to_string()))?;

        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CdpError::WebSocket("response channel closed".to_string())),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(CdpError::WebSocket(format!("session command {} timeout", method)))
            }
        }
    }

    /// 对 attached session 启用 Network domain（只监听 requestWillBeSent）。
    ///
    /// 启用后该 session 的网络事件会通过 `recv_event()` 推送。
    pub async fn enable_network(&self, session_id: &SessionId) -> Result<(), CdpError> {
        self.send_session_command(session_id, "Network.enable", Value::Object(serde_json::Map::new()))
            .await?;
        Ok(())
    }
}

/// `Target.getTargets` 返回的原始 target 信息（CDP 原始字段名）
#[derive(Debug, Deserialize)]
struct RawTargetInfo {
    #[serde(rename = "targetId")]
    target_id: String,
    #[serde(rename = "type")]
    target_type: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    title: String,
}

impl From<RawTargetInfo> for TargetInfo {
    fn from(raw: RawTargetInfo) -> Self {
        TargetInfo {
            target_id: raw.target_id,
            target_type: raw.target_type,
            url: raw.url,
            title: raw.title,
        }
    }
}

// ── Attach 编排：选择性 Attach page/service_worker ──────────────

/// 选择性 Attach 结果
#[derive(Debug)]
pub struct AttachResult {
    /// 成功 attach 的 session 列表
    pub attached: Vec<(SessionId, TargetInfo)>,
    /// 因上限跳过的 target 列表
    pub skipped: Vec<TargetInfo>,
    /// attach 失败的 target 列表（附错误）
    pub failed: Vec<(TargetInfo, CdpError)>,
}

/// 对 page/service_worker target 执行选择性 Attach。
///
/// 流程：
/// 1. 调 `Target.getTargets` 获取所有 target
/// 2. 过滤：只 `page` / `service_worker`
/// 3. 检查 `SessionManager` 上限（is_full），超出记录到 skipped
/// 4. 调 `attachToTarget`（flatten: true），成功则 `SessionManager::insert`
/// 5. 失败记录到 failed
///
/// 返回 `AttachResult` 汇总。
pub async fn selective_attach(client: &CdpClient, manager: &mut SessionManager) -> Result<AttachResult, CdpError> {
    let targets = client.get_targets().await?;
    let mut attached = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for target in targets {
        // 只 attach page/service_worker
        if !SessionManager::should_attach(&target.target_type) {
            continue;
        }

        // 上限保护
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
            "skipped {} targets due to session limit ({})",
            skipped.len(),
            manager.limit()
        );
    }
    if !failed.is_empty() {
        warn!("failed to attach {} targets", failed.len());
    }
    info!(
        "selective_attach: attached={}, skipped={}, failed={}",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_target_info_deserialize() {
        let json = r#"{"targetId":"tid-1","type":"page","url":"https://example.com","title":"Example"}"#;
        let raw: RawTargetInfo = serde_json::from_str(json).unwrap();
        assert_eq!(raw.target_id, "tid-1");
        assert_eq!(raw.target_type, "page");
        assert_eq!(raw.url, "https://example.com");
        assert_eq!(raw.title, "Example");
    }

    #[test]
    fn raw_target_info_minimal() {
        // url/title 可缺失（serde default）
        let json = r#"{"targetId":"tid-2","type":"service_worker"}"#;
        let raw: RawTargetInfo = serde_json::from_str(json).unwrap();
        assert_eq!(raw.target_id, "tid-2");
        assert_eq!(raw.target_type, "service_worker");
        assert_eq!(raw.url, "");
        assert_eq!(raw.title, "");
    }

    #[test]
    fn raw_target_info_convert_to_target_info() {
        let raw = RawTargetInfo {
            target_id: "tid-3".to_string(),
            target_type: "page".to_string(),
            url: "https://test.com".to_string(),
            title: "Test".to_string(),
        };
        let info: TargetInfo = raw.into();
        assert_eq!(info.target_id, "tid-3");
        assert_eq!(info.target_type, "page");
        assert_eq!(info.url, "https://test.com");
        assert_eq!(info.title, "Test");
    }

    #[test]
    fn cdp_event_deserialize() {
        let json = r#"{"method":"Network.requestWillBeSent","params":{"requestId":"r1"}}"#;
        let evt: CdpEvent = serde_json::from_str(json).unwrap();
        assert_eq!(evt.method, "Network.requestWillBeSent");
        assert_eq!(evt.params["requestId"], "r1");
        // browser-level 事件，session_id 为 None
        assert_eq!(evt.session_id, None);
    }

    #[test]
    fn cdp_event_session_id_from_top_level() {
        // flatten session 模式：sessionId 在消息 JSON 顶层（与 method/params 平级）
        let json = r#"{"method":"Network.requestWillBeSent","params":{"requestId":"r1"},"sessionId":"sess-abc-123"}"#;
        let evt: CdpEvent = serde_json::from_str(json).unwrap();
        // serde 直接从顶层 sessionId 字段反序列化
        assert_eq!(evt.session_id.as_deref(), Some("sess-abc-123"));
        // params 不含 sessionId（因为本来就不在 params 里）
        assert!(evt.params.get("sessionId").is_none());
        // params 其他字段保留
        assert_eq!(evt.params["requestId"], "r1");
    }

    #[test]
    fn cdp_event_no_session_id() {
        // browser-level 事件：顶层没有 sessionId
        let json = r#"{"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"t1"}}}"#;
        let evt: CdpEvent = serde_json::from_str(json).unwrap();
        assert_eq!(evt.session_id, None);
        assert!(evt.params.get("targetInfo").is_some());
    }

    #[test]
    fn cdp_response_deserialize_success() {
        let json = r#"{"id":1,"result":{"sessionId":"sess-1"}}"#;
        let resp: CdpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, 1);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["sessionId"], "sess-1");
    }

    #[test]
    fn cdp_response_deserialize_error() {
        let json = r#"{"id":2,"error":{"code":-32000,"message":"Target not found"}}"#;
        let resp: CdpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, 2);
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32000);
    }
}
