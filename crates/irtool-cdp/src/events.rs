//! CDP 事件解析（P2.4 填充实现）。
//!
//! ## 设计要点
//!
//! - 只关心 `Network.requestWillBeSent` 事件
//! - 不抓 response body/header/cookie（隐私 + 性能）
//! - 提取 request.url/documentURL/initiator/frameId/loaderId/timestamp

use serde::Deserialize;

use crate::CdpError;

/// CDP `Network.requestWillBeSent` 事件的 initiator 字段
#[derive(Debug, Clone, Deserialize)]
pub struct CdpInitiator {
    /// initiator 类型：parser/script/redirect/preload/preflight/other
    #[serde(rename = "type")]
    pub init_type: String,
    /// initiator URL（可选，script/parser 类型常有）
    pub url: Option<String>,
    /// 调用栈（可选，script 类型常有）
    pub stack: Option<serde_json::Value>,
}

/// CDP `Network.requestWillBeSent` 事件提取的关键字段
#[derive(Debug, Clone, Deserialize)]
pub struct CdpRequest {
    /// 请求 ID
    pub request_id: String,
    /// 请求 URL
    pub url: String,
    /// HTTP 方法
    pub method: String,
    /// documentURL（顶层文档 URL，归因铁证）
    pub document_url: String,
    /// initiator
    pub initiator: CdpInitiator,
    /// frame ID
    pub frame_id: String,
    /// loader ID
    pub loader_id: String,
    /// CDP 时间戳（秒，浮点）
    pub timestamp: f64,
}

// ── P2.4 parse_request_will_be_sent ─────────────────────────────

/// 解析 `Network.requestWillBeSent` 事件 params 为 `CdpRequest`。
///
/// CDP 原始结构中 `url`/`method` 嵌套在 `request` 对象内，
/// 本函数提取为扁平的 `CdpRequest`（便于后续归因匹配）。
///
/// 缺失非关键字段时用默认值（空字符串）而非报错，保证容错性。
pub fn parse_request_will_be_sent(params: &serde_json::Value) -> Result<CdpRequest, CdpError> {
    let request_id = params
        .get("requestId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CdpError::Protocol {
            code: -1,
            message: "requestWillBeSent missing requestId".to_string(),
        })?
        .to_string();

    let request_obj = params.get("request").ok_or_else(|| CdpError::Protocol {
        code: -1,
        message: "requestWillBeSent missing request object".to_string(),
    })?;

    let url = request_obj
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let method = request_obj
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let document_url = params
        .get("documentURL")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let initiator: CdpInitiator = params
        .get("initiator")
        .ok_or_else(|| CdpError::Protocol {
            code: -1,
            message: "requestWillBeSent missing initiator".to_string(),
        })?
        .clone()
        .into();

    let frame_id = params.get("frameId").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let loader_id = params
        .get("loaderId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let timestamp = params.get("timestamp").and_then(|v| v.as_f64()).unwrap_or(0.0);

    Ok(CdpRequest {
        request_id,
        url,
        method,
        document_url,
        initiator,
        frame_id,
        loader_id,
        timestamp,
    })
}

/// 从 `serde_json::Value` 提取 `CdpInitiator`（容错：缺失字段用默认值）。
impl From<serde_json::Value> for CdpInitiator {
    fn from(v: serde_json::Value) -> Self {
        let init_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("other").to_string();
        let url = v.get("url").and_then(|u| u.as_str()).map(String::from);
        let stack = v.get("stack").cloned();
        CdpInitiator { init_type, url, stack }
    }
}

// ── P2 session 归属映射 ─────────────────────────────────────────

/// 从 CDP 事件 params 顶层提取 `sessionId` 字段。
///
/// CDP flatten session 模式下，子 session 事件的 params 顶层会带 `sessionId` 字段，
/// 用于标识事件来自哪个 target。本函数提取该字段，便于 `CdpEvent` 归属映射。
///
/// 返回 `None` 表示 params 中没有 `sessionId`（browser-level 事件）或字段类型非字符串。
pub fn event_session_id(params: &serde_json::Value) -> Option<String> {
    params.get("sessionId").and_then(|v| v.as_str()).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdp_initiator_deserialize_script() {
        let json = r#"{"type":"script","url":"https://example.com/script.js","stack":{"callFrames":[]}}"#;
        let init: CdpInitiator = serde_json::from_str(json).unwrap();
        assert_eq!(init.init_type, "script");
        assert_eq!(init.url.as_deref(), Some("https://example.com/script.js"));
        assert!(init.stack.is_some());
    }

    #[test]
    fn cdp_initiator_deserialize_parser_minimal() {
        let json = r#"{"type":"parser"}"#;
        let init: CdpInitiator = serde_json::from_str(json).unwrap();
        assert_eq!(init.init_type, "parser");
        assert!(init.url.is_none());
        assert!(init.stack.is_none());
    }

    #[test]
    fn cdp_request_deserialize() {
        // 注意：CDP 原始事件 request 字段是嵌套对象，这里测试简化结构
        // 实际 P2.4 实现需要从嵌套 request 对象提取 url/method
        let _cdp_raw_event = r#"{
            "requestId":"req-1",
            "request":{"url":"https://evil.com/payload","method":"POST"},
            "documentURL":"https://malicious.example.com/landing.html",
            "initiator":{"type":"script","url":"https://evil.com/inject.js"},
            "frameId":"frame-1",
            "loaderId":"loader-1",
            "timestamp":1700000000.5
        }"#;
        let simplified = serde_json::json!({
            "request_id": "req-1",
            "url": "https://evil.com/payload",
            "method": "POST",
            "document_url": "https://malicious.example.com/landing.html",
            "initiator": {"type":"script","url":"https://evil.com/inject.js"},
            "frame_id": "frame-1",
            "loader_id": "loader-1",
            "timestamp": 1700000000.5
        });
        let req: CdpRequest = serde_json::from_value(simplified).unwrap();
        assert_eq!(req.request_id, "req-1");
        assert_eq!(req.url, "https://evil.com/payload");
        assert_eq!(req.method, "POST");
        assert_eq!(req.document_url, "https://malicious.example.com/landing.html");
        assert_eq!(req.initiator.init_type, "script");
        assert_eq!(req.frame_id, "frame-1");
        assert_eq!(req.loader_id, "loader-1");
        assert!((req.timestamp - 1700000000.5).abs() < 1e-9);
    }

    #[test]
    fn parse_request_will_be_sent_full() {
        let params = serde_json::json!({
            "requestId": "req-1",
            "request": { "url": "https://evil.com/payload", "method": "POST" },
            "documentURL": "https://malicious.example.com/landing.html",
            "initiator": { "type": "script", "url": "https://evil.com/inject.js" },
            "frameId": "frame-1",
            "loaderId": "loader-1",
            "timestamp": 1700000000.5
        });
        let req = parse_request_will_be_sent(&params).unwrap();
        assert_eq!(req.request_id, "req-1");
        assert_eq!(req.url, "https://evil.com/payload");
        assert_eq!(req.method, "POST");
        assert_eq!(req.document_url, "https://malicious.example.com/landing.html");
        assert_eq!(req.initiator.init_type, "script");
        assert_eq!(req.initiator.url.as_deref(), Some("https://evil.com/inject.js"));
        assert_eq!(req.frame_id, "frame-1");
        assert_eq!(req.loader_id, "loader-1");
        assert!((req.timestamp - 1700000000.5).abs() < 1e-9);
    }

    #[test]
    fn parse_request_will_be_sent_missing_optional_fields() {
        // 缺 documentURL/frameId/loaderId/timestamp → 用默认值
        let params = serde_json::json!({
            "requestId": "req-2",
            "request": { "url": "https://example.com" },
            "initiator": { "type": "parser" }
        });
        let req = parse_request_will_be_sent(&params).unwrap();
        assert_eq!(req.request_id, "req-2");
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.method, "");
        assert_eq!(req.document_url, "");
        assert_eq!(req.initiator.init_type, "parser");
        assert!(req.initiator.url.is_none());
        assert_eq!(req.frame_id, "");
        assert_eq!(req.loader_id, "");
        assert_eq!(req.timestamp, 0.0);
    }

    #[test]
    fn parse_request_will_be_sent_missing_request_id_errors() {
        let params = serde_json::json!({
            "request": { "url": "https://example.com" },
            "initiator": { "type": "parser" }
        });
        assert!(parse_request_will_be_sent(&params).is_err());
    }

    #[test]
    fn parse_request_will_be_sent_missing_request_object_errors() {
        let params = serde_json::json!({
            "requestId": "req-3",
            "initiator": { "type": "parser" }
        });
        assert!(parse_request_will_be_sent(&params).is_err());
    }

    #[test]
    fn parse_request_will_be_sent_missing_initiator_errors() {
        let params = serde_json::json!({
            "requestId": "req-4",
            "request": { "url": "https://example.com" }
        });
        assert!(parse_request_will_be_sent(&params).is_err());
    }

    #[test]
    fn cdp_initiator_from_value_minimal() {
        let v = serde_json::json!({ "type": "parser" });
        let init: CdpInitiator = v.into();
        assert_eq!(init.init_type, "parser");
        assert!(init.url.is_none());
        assert!(init.stack.is_none());
    }

    #[test]
    fn cdp_initiator_from_value_missing_type_defaults_other() {
        let v = serde_json::json!({ "url": "https://example.com" });
        let init: CdpInitiator = v.into();
        assert_eq!(init.init_type, "other");
        assert_eq!(init.url.as_deref(), Some("https://example.com"));
    }

    // ── event_session_id 测试 ──────────────────────────────────────

    #[test]
    fn event_session_id_present() {
        // flatten session 模式：params 顶层带 sessionId
        let params = serde_json::json!({"sessionId": "sess-abc-123", "requestId": "r1"});
        assert_eq!(event_session_id(&params), Some("sess-abc-123".to_string()));
    }

    #[test]
    fn event_session_id_absent() {
        // browser-level 事件：params 中没有 sessionId
        let params = serde_json::json!({"requestId": "r1"});
        assert_eq!(event_session_id(&params), None);
    }

    #[test]
    fn event_session_id_not_string_returns_none() {
        // sessionId 字段非字符串 → None
        let params = serde_json::json!({"sessionId": 123});
        assert_eq!(event_session_id(&params), None);
    }

    #[test]
    fn event_session_id_empty_string() {
        // sessionId 为空字符串 → Some("")（保留原值，由调用方判断是否有效）
        let params = serde_json::json!({"sessionId": ""});
        assert_eq!(event_session_id(&params), Some("".to_string()));
    }

    #[test]
    fn event_session_id_null_value() {
        // sessionId 为 null → None
        let params = serde_json::json!({"sessionId": null});
        assert_eq!(event_session_id(&params), None);
    }
}
