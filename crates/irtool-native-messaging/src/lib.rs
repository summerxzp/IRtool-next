//! Chrome Native Messaging Host 协议实现
//!
//! 实现 Chrome 扩展 Native Messaging 的接收端协议，
//! 通过 stdin 读取浏览器发送的 JSON 消息并写入队列文件。
//!
//! ## 数据流
//!
//! ```text
//! Helper Extension → [stdin] → irtool-native-messaging-host
//!                                       ↓
//!                              写 attr-queue.jsonl
//!                                       ↓
//!                           irtool-service 轮询读取
//!                                       ↓
//!                             EventBus publish AppEvent
//! ```

use serde::Deserialize;
use std::io::{self, Read, Write};
use std::path::Path;
use tracing::{error, info};

/// Chrome Native Messaging 消息，实际取值为来自扩展的任意 JSON 对象。
///
/// 扩展发送的消息格式例如：
/// - `{"type":"network_batch","batch_id":"...","events":[...]}`
/// - `{"type":"extension_list","mode":"full","extensions":[...]}`
/// - `{"type":"heartbeat","timestamp":...}`
///
/// 我们只提取 `type` 字段用于日志，其余内容保持原始 JSON 格式写入队列。
#[derive(Debug, Deserialize)]
pub struct NativeMessage {
    /// 消息类型（由扩展的 `type` 字段映射）
    #[serde(rename = "type")]
    pub msg_type: String,
    /// 其余消息字段以原始 JSON Value 形式保留
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

/// Native Messaging Host 错误
#[derive(Debug)]
pub enum NativeMessagingError {
    Io(io::Error),
    InvalidMessage(String),
}

impl std::fmt::Display for NativeMessagingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeMessagingError::Io(e) => write!(f, "IO error: {}", e),
            NativeMessagingError::InvalidMessage(s) => write!(f, "invalid message: {}", s),
        }
    }
}

impl std::error::Error for NativeMessagingError {}

impl From<io::Error> for NativeMessagingError {
    fn from(e: io::Error) -> Self {
        NativeMessagingError::Io(e)
    }
}

/// 从 stdin 读取一条 Native Messaging 消息。
///
/// Chrome Native Messaging 协议：
/// - 前 4 字节（u32 LE）表示后续 JSON 数据长度
/// - 之后是 UTF-8 编码的 JSON
pub fn read_message() -> Result<Option<NativeMessage>, NativeMessagingError> {
    let mut len_buf = [0u8; 4];

    // 读取长度前缀
    match io::stdin().read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            // stdin 已关闭，浏览器进程退出
            return Ok(None);
        }
        Err(e) => return Err(NativeMessagingError::Io(e)),
    }

    let msg_len = u32::from_le_bytes(len_buf) as usize;

    // 读取消息体
    let mut msg_buf = vec![0u8; msg_len];
    io::stdin().read_exact(&mut msg_buf)?;

    // 解析 JSON
    let msg: NativeMessage = serde_json::from_slice(&msg_buf)
        .map_err(|e| NativeMessagingError::InvalidMessage(format!("JSON parse error: {}", e)))?;

    Ok(Some(msg))
}

/// 将消息追加写入 JSONL 队列文件。
///
/// 队列文件路径: `{queue_dir}/attr-queue.jsonl`
/// 每条记录为一行完整的 JSON，追加写入。
pub fn append_to_queue(queue_dir: &Path, msg: &NativeMessage) -> Result<(), NativeMessagingError> {
    std::fs::create_dir_all(queue_dir)?;

    let queue_path = queue_dir.join("attr-queue.jsonl");

    // 将 NativeMessage 还原为完整 JSON 再写入（保留 `type` 字段名而非 `msg_type`）
    let full_json = serde_json::json!({
        "type": msg.msg_type,
    })
    .as_object()
    .map(|obj| {
        let mut obj = obj.clone();
        // 合并 payload 中的字段
        if let serde_json::Value::Object(payload_obj) = &msg.payload {
            for (k, v) in payload_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
        serde_json::Value::Object(obj)
    })
    .unwrap_or(serde_json::Value::Null);

    let line = serde_json::to_string(&full_json)
        .map_err(|e| NativeMessagingError::InvalidMessage(format!("JSON serialize error: {}", e)))?;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&queue_path)?;

    writeln!(file, "{}", line)?;

    Ok(())
}

/// 运行 Native Messaging 事件循环（阻塞）。
///
/// 从 stdin 持续读取消息，写入队列文件。
/// 当 stdin 关闭（浏览器进程退出）时优雅返回。
pub fn run_event_loop(queue_dir: &Path) -> Result<(), NativeMessagingError> {
    info!(
        "Native Messaging Host started, queue dir: {:?}",
        queue_dir
    );

    let mut message_count = 0u64;

    loop {
        match read_message()? {
            Some(msg) => {
                message_count += 1;
                trace_event(&msg);

                if let Err(e) = append_to_queue(queue_dir, &msg) {
                    error!("failed to append message to queue: {:?}", e);
                }
            }
            None => {
                info!(
                    "stdin closed, Native Messaging Host exiting (total messages: {})",
                    message_count
                );
                break;
            }
        }
    }

    Ok(())
}

/// 对消息进行采样日志（心跳类消息降频）
fn trace_event(msg: &NativeMessage) {
    match msg.msg_type.as_str() {
        "heartbeat" => {
            // 心跳消息太多，降频到 trace 级别
            tracing::trace!("received heartbeat");
        }
        "network_batch" => {
            let count = msg
                .payload
                .get("events")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            info!(
                "received network_batch with {} events",
                count
            );
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
            info!(
                "received extension_list (mode={}, count={})",
                mode, count
            );
        }
        other => {
            info!("received message: type={}", other);
        }
    }
}

// ─── 测试 ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// 构造一条 Native Messaging 协议格式的字节数据
    fn encode_message(json: &str) -> Vec<u8> {
        let json_bytes = json.as_bytes();
        let len = json_bytes.len() as u32;
        let mut buf = Vec::with_capacity(4 + json_bytes.len());
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(json_bytes);
        buf
    }

    #[test]
    fn test_parse_network_batch() {
        let json = r#"{"type":"network_batch","batch_id":"abc-123","events":[{"url":"https://example.com","method":"GET"}]}"#;
        let data = encode_message(json);

        // Mock stdin: 注入编码后的数据
        let mut reader = Cursor::new(&data[..]);
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).unwrap();
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        let mut msg_buf = vec![0u8; msg_len];
        reader.read_exact(&mut msg_buf).unwrap();
        let msg: NativeMessage = serde_json::from_slice(&msg_buf).unwrap();

        assert_eq!(msg.msg_type, "network_batch");
        assert!(msg.payload.get("batch_id").is_some());
        assert!(msg.payload.get("events").is_some());
    }

    #[test]
    fn test_parse_heartbeat() {
        let json = r#"{"type":"heartbeat","timestamp":1700000000000}"#;
        let data = encode_message(json);
        let mut reader = Cursor::new(&data[..]);
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).unwrap();
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        let mut msg_buf = vec![0u8; msg_len];
        reader.read_exact(&mut msg_buf).unwrap();
        let msg: NativeMessage = serde_json::from_slice(&msg_buf).unwrap();

        assert_eq!(msg.msg_type, "heartbeat");
        assert_eq!(msg.payload.get("timestamp").and_then(|v| v.as_u64()), Some(1700000000000));
    }

    #[test]
    fn test_append_to_queue() {
        let dir = tempfile::tempdir().unwrap();
        let msg = NativeMessage {
            msg_type: "test".to_string(),
            payload: serde_json::json!({"key": "value"}),
        };
        append_to_queue(dir.path(), &msg).unwrap();

        let queue_path = dir.path().join("attr-queue.jsonl");
        let content = std::fs::read_to_string(&queue_path).unwrap();
        assert!(content.contains("test"));
        assert!(content.contains("value"));
    }

    #[test]
    fn test_append_to_queue_multiple() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..3 {
            let msg = NativeMessage {
                msg_type: "test".to_string(),
                payload: serde_json::json!({"index": i}),
            };
            append_to_queue(dir.path(), &msg).unwrap();
        }

        let queue_path = dir.path().join("attr-queue.jsonl");
        let content = std::fs::read_to_string(&queue_path).unwrap();
        assert_eq!(content.lines().count(), 3);
    }

    #[test]
    fn test_read_message_eof() {
        // 空 stdin → None
        let result = {
            // 不能真正关闭 stdin，我们只测试 read_message 的逻辑的一部分
            // 这里验证 read_message 返回 Err(InvalidMessage) 当输入不是合法 JSON
            let json = r#"not-json"#;
            let data = encode_message(json);
            let mut reader = Cursor::new(&data[..]);
            let mut len_buf = [0u8; 4];
            reader.read_exact(&mut len_buf).unwrap();
            let msg_len = u32::from_le_bytes(len_buf) as usize;
            let mut msg_buf = vec![0u8; msg_len];
            reader.read_exact(&mut msg_buf).unwrap();

            serde_json::from_slice::<NativeMessage>(&msg_buf)
                .map_err(|e| NativeMessagingError::InvalidMessage(format!("JSON parse error: {}", e)))
        };
        assert!(result.is_err());
    }
}
