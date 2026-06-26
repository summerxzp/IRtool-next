use serde::{Deserialize, Serialize};
use specta::Type;

use irtool_browser_forensics::BrowserKind;

// ── Native Messaging 事件 DTO ─────────────────────────────────

/// Native Messaging 队列中的原始消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeQueueMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

/// 从扩展上报的网络请求归因事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributedWebRequest {
    pub timestamp: u64,
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub initiator: Option<String>,
    pub attribution: AttributionInfo,
}

/// 归因信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionInfo {
    pub status: String,
    pub extension_id: Option<String>,
    pub extension_name: Option<String>,
}

/// 扩展清单条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionListEntry {
    pub id: String,
    pub name: Option<String>,
    pub enabled: bool,
    pub install_type: String,
    pub version: Option<String>,
    pub host_permissions: Option<Vec<String>>,
}

/// 浏览器恶意连接事件负载
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BrowserMaliciousConnectionPayload {
    pub domain: String,
    pub ip: String,
    pub process_name: String,
    pub pid: u32,
    pub cmdline: Option<String>,
    pub alert_id: String,
}

/// Helper Extension 上报的归因网络请求事件（发布到 EventBus）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionAttributionPayload {
    pub timestamp: u64,
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub initiator: Option<String>,
    pub attribution_status: String,
    pub extension_id: Option<String>,
    pub extension_name: Option<String>,
}

/// 扫描请求参数
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BrowserForensicsScanRequest {
    pub browser: BrowserKind,
    pub profile_name: Option<String>,
}

/// History 关联请求参数
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HistoryAttributionRequest {
    pub browser: BrowserKind,
    pub profile_name: String,
    pub target_time: String, // RFC3339 格式
}

/// Browser Context Attribution 请求参数
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContextAttributionRequest {
    pub domain: String,
    pub ip: Option<String>,
    pub process_name: String,
    pub pid: u32,
    pub timestamp: String, // RFC3339 格式
    pub cmdline: Option<String>,
}
