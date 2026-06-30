use serde::{Deserialize, Serialize};
use specta::Type;

use irtool_browser_forensics::{AttributionLevel, BrowserKind};

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
///
/// JS 端（service_worker.js）使用 camelCase 上报：
/// `requestId` / `attribution.extensionId` / `attribution.extensionName`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributedWebRequest {
    pub timestamp: u64,
    pub request_id: String,
    pub url: String,
    pub method: String,
    /// CDP Network.requestWillBeSent 的 type 字段（Document/XHR/Fetch/WebSocket/...）。
    /// 旧事件（webRequest 时代）无该字段，用 Option + default 兼容。
    #[serde(default)]
    pub resource_type: Option<String>,
    pub initiator: Option<String>,
    pub attribution: AttributionInfo,
    /// 请求来源 target 信息（page 或 service_worker）。
    /// 旧事件无该字段，用 Option + default 兼容。
    #[serde(default)]
    pub source_target: Option<SourceTarget>,
}

/// 归因信息
///
/// JS 端上报字段为 camelCase：`extensionId` / `extensionName`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionInfo {
    pub status: String,
    pub extension_id: Option<String>,
    pub extension_name: Option<String>,
}

/// 请求来源 target 信息（debugger API 抓包时填充）
///
/// JS 端上报字段为 camelCase：`targetId` / `extensionId`
/// 注意 `type` 字段用 serde rename 保留关键字
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTarget {
    #[serde(rename = "type")]
    pub target_type: String,
    pub target_id: String,
    #[serde(default)]
    pub extension_id: Option<String>,
}

/// 扩展清单条目
///
/// JS 端（chrome.management.ExtensionInfo）使用 camelCase：
/// `installType` / `hostPermissions`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    /// CDP 资源类型（Document/XHR/Fetch/WebSocket/...）。旧事件可能为 None。
    #[serde(default)]
    pub resource_type: Option<String>,
    pub initiator: Option<String>,
    pub attribution_status: String,
    pub extension_id: Option<String>,
    pub extension_name: Option<String>,
    /// P1.2: Helper Extension 自报归因置信度（matched → Confirmed）
    pub level: AttributionLevel,
    /// CDP 路径独有：发起请求的 target 类型（page/service_worker/background_page）。
    /// webRequest 路径无此信息，为 None。
    #[serde(default)]
    pub target_type: Option<String>,
    /// CDP 路径独有：target 标题（页面标题或扩展名）。
    /// webRequest 路径无此信息，为 None。
    #[serde(default)]
    pub target_title: Option<String>,
    /// CDP initiator 类型（parser/script/redirect/preload/preflight/other）。
    /// webRequest 路径无此字段，为 None。
    #[serde(default)]
    pub initiator_type: Option<String>,
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
