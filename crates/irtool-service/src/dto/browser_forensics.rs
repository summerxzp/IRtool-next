use serde::{Deserialize, Serialize};
use specta::Type;

use irtool_browser_forensics::BrowserKind;

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
}
