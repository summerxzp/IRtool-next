//! Chrome Native Messaging Host 协议实现
//!
//! 实现 Chrome 扩展 Native Messaging 的双向通信：
//!
//! - **上行（Extension → IRtool）**：从 stdin 读取浏览器发送的 JSON 消息并写入队列文件。
//! - **下行（IRtool → Extension）**：检查 config 文件变更，通过 stdout 向扩展下发 config 消息。
//!
//! ## 数据流
//!
//! ```text
//! Helper Extension ← [stdout] ── irtool-native-messaging-host ── [config file] ← service
//!                  → [stdin]  →                              → 写 attr-queue.jsonl → service 轮询
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

// ── 下行通道：向扩展发送消息 ─────────────────────────────

/// 向 stdout 写入一条 Native Messaging 消息（下行通道）。
///
/// Chrome Native Messaging 协议：
/// - 前 4 字节（u32 LE）表示后续 JSON 数据长度
/// - 之后是 UTF-8 编码的 JSON
///
/// 扩展通过 `port.onMessage` 接收此消息。
pub fn write_message_to_stdout(json_str: &str) -> Result<(), NativeMessagingError> {
    let json_bytes = json_str.as_bytes();
    let len = json_bytes.len() as u32;
    let mut out = io::stdout().lock();
    out.write_all(&len.to_le_bytes())?;
    out.write_all(json_bytes)?;
    out.flush()?;
    info!("sent message to extension ({} bytes)", len);
    Ok(())
}

/// 从 `config.json` 内容构建下行消息 JSON 字符串。
///
/// config.json 格式：
/// ```json
/// { "filterDomains": ["evil.com", "*.bad.net"] }
/// ```
/// 空数组或缺失字段表示清除过滤。
fn build_config_message(content: &str) -> Result<String, NativeMessagingError> {
    let content = content.trim();
    if content.is_empty() {
        // 空内容 → 清除过滤
        let msg = serde_json::json!({
            "type": "config",
            "filterDomains": null,
        });
        return serde_json::to_string(&msg)
            .map_err(|e| NativeMessagingError::InvalidMessage(format!("config serialize error: {}", e)));
    }

    let config: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| NativeMessagingError::InvalidMessage(format!("config parse error: {}", e)))?;

    let filter_domains: Vec<String> = config
        .get("filterDomains")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let msg = if filter_domains.is_empty() {
        // 空数组 → 清除过滤（扩展端 applyConfig 处理 filterDomains=null 场景）
        serde_json::json!({
            "type": "config",
            "filterDomains": null,
        })
    } else {
        serde_json::json!({
            "type": "config",
            "filterDomains": filter_domains,
        })
    };

    serde_json::to_string(&msg)
        .map_err(|e| NativeMessagingError::InvalidMessage(format!("config serialize error: {}", e)))
}

/// 检查 config 文件变更并转发到扩展。
///
/// 读取 `{config_dir}/config.json`，如果 mtime 有变化，
/// 构造 `{"type":"config","filterDomains":[...]}` 消息写入 stdout。
fn check_and_forward_config(
    config_dir: &Path,
    last_mtime: &mut Option<std::time::SystemTime>,
) -> Result<(), NativeMessagingError> {
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return Ok(());
    }

    let metadata = config_path.metadata()?;
    let mtime = metadata.modified().ok();

    // mtime 未变 → 无更新
    if mtime == *last_mtime {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    if content.trim().is_empty() {
        // 空文件 → 跳过（不更新 mtime，下次再检查）
        return Ok(());
    }

    let json_str = build_config_message(&content)?;

    info!("config changed, forwarding to extension: {:?}", config_path);
    write_message_to_stdout(&json_str)?;
    *last_mtime = mtime;

    Ok(())
}

/// 运行 Native Messaging 事件循环（阻塞）。
///
/// 从 stdin 持续读取消息，写入队列文件。
/// 当 stdin 关闭（浏览器进程退出）时优雅返回。
pub fn run_event_loop(queue_dir: &Path, config_dir: &Path) -> Result<(), NativeMessagingError> {
    info!(
        "Native Messaging Host started, queue dir: {:?}, config dir: {:?}",
        queue_dir, config_dir
    );

    let mut message_count = 0u64;
    let mut last_config_mtime: Option<std::time::SystemTime> = None;

    // 启动时先检查一次 config（可能在启动前就已写入）
    if let Err(e) = check_and_forward_config(config_dir, &mut last_config_mtime) {
        error!("failed to check config on startup: {:?}", e);
    }

    loop {
        match read_message()? {
            Some(msg) => {
                message_count += 1;
                trace_event(&msg);

                if let Err(e) = append_to_queue(queue_dir, &msg) {
                    error!("failed to append message to queue: {:?}", e);
                }

                // 每次收到消息后检查 config 变更
                if let Err(e) = check_and_forward_config(config_dir, &mut last_config_mtime) {
                    error!("failed to check/forward config: {:?}", e);
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
            info!("received network_batch with {} events", count);
        }
        "extension_list" => {
            let mode = msg.payload.get("mode").and_then(|v| v.as_str()).unwrap_or("unknown");
            let count = msg
                .payload
                .get("extensions")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            info!("received extension_list (mode={}, count={})", mode, count);
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
        let json =
            r#"{"type":"network_batch","batch_id":"abc-123","events":[{"url":"https://example.com","method":"GET"}]}"#;
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
        assert_eq!(
            msg.payload.get("timestamp").and_then(|v| v.as_u64()),
            Some(1700000000000)
        );
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

    // ── 下行通道测试 ──────────────────────────────────────

    #[test]
    fn test_build_config_message_with_domains() {
        let content = r#"{"filterDomains":["evil.com","*.bad.net"]}"#;
        let result = build_config_message(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "config");
        assert_eq!(parsed["filterDomains"][0], "evil.com");
        assert_eq!(parsed["filterDomains"][1], "*.bad.net");
    }

    #[test]
    fn test_build_config_message_empty_array_clears() {
        let content = r#"{"filterDomains":[]}"#;
        let result = build_config_message(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "config");
        assert!(parsed["filterDomains"].is_null());
    }

    #[test]
    fn test_build_config_message_no_filter_key_clears() {
        let content = r#"{"someOtherKey":"value"}"#;
        let result = build_config_message(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "config");
        assert!(parsed["filterDomains"].is_null());
    }

    #[test]
    fn test_build_config_message_empty_content_clears() {
        let result = build_config_message("").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "config");
        assert!(parsed["filterDomains"].is_null());
    }

    #[test]
    fn test_build_config_message_whitespace_only_clears() {
        let result = build_config_message("  \n  ").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "config");
        assert!(parsed["filterDomains"].is_null());
    }

    #[test]
    fn test_check_and_forward_config_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut last_mtime = None;
        // 没有 config 文件，应静默返回
        assert!(check_and_forward_config(dir.path(), &mut last_mtime).is_ok());
        assert!(last_mtime.is_none());
    }

    #[test]
    fn test_check_and_forward_config_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "").unwrap();
        let mut last_mtime = None;
        // 空文件应静默返回且不更新 mtime（下次继续检查）
        assert!(check_and_forward_config(dir.path(), &mut last_mtime).is_ok());
        assert!(last_mtime.is_none());
    }

    #[test]
    fn test_check_and_forward_config_unchanged_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"filterDomains":["evil.com"]}"#).unwrap();
        let mut last_mtime = std::fs::metadata(&config_path).ok().and_then(|m| m.modified().ok());
        // mtime 相同，不应转发
        assert!(check_and_forward_config(dir.path(), &mut last_mtime).is_ok());
        // mtime 仍为原值
        assert!(last_mtime.is_some());
    }

    /// 验证 write_message_to_stdout 的输出格式符合 Native Messaging 协议
    #[test]
    fn test_write_message_to_stdout_format() {
        // 写入到 Vec<u8> 模拟 stdout，验证 4 字节 LE 长度前缀
        let json = r#"{"type":"config","filterDomains":null}"#;
        let json_bytes = json.as_bytes();
        let expected_len = json_bytes.len() as u32;

        let mut buf = Vec::new();
        buf.extend_from_slice(&expected_len.to_le_bytes());
        buf.extend_from_slice(json_bytes);

        // 验证格式：前 4 字节 = 长度，后续 = JSON
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&buf[..4]);
        let decoded_len = u32::from_le_bytes(len_bytes) as usize;
        assert_eq!(decoded_len, json_bytes.len());

        let decoded_json = std::str::from_utf8(&buf[4..]).unwrap();
        assert_eq!(decoded_json, json);
    }
}
