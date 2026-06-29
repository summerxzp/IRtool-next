//! Chrome DevTools Protocol (CDP) 客户端实现。
//!
//! 用于 Browser Evidence Engine P2 阶段：通过 CDP 协议连接本机已开启远程调试的浏览器，
//! 抓取标签页级别的网络请求归因铁证（documentURL/initiator），融合到 EvidenceObject.tab_attribution。
//!
//! ## 模块结构
//!
//! - [`discovery`]：BrowserDiscovery — 端口扫描 + `/json/version` 探测（P2.2）
//! - [`client`]：WebSocket CDP 客户端（P2.3）
//! - [`session`]：CDP 会话管理 Attach/Detach（P2.3）
//! - [`events`]：CDP 事件解析（P2.4）
//! - [`attribution`]：CDP 归因融合（P2.4）
//!
//! ## 设计原则
//!
//! - **不启动浏览器**：只连接已开启 `--remote-debugging-port` 的实例
//! - **选择性 Attach**：只关注 page/service_worker，忽略 iframe/worker
//! - **隐私优先**：只抓 request.url/documentURL/initiator，不抓 body/cookie/header

pub mod attribution;
pub mod client;
pub mod discovery;
pub mod events;
pub mod session;

// ── 公共类型 re-export（方便外部 crate 使用） ──────────────────
pub use attribution::{
    build_match_result, extract_extension_id, find_match, match_cdp_to_malicious, CdpConfidence, CdpMatchResult,
};
pub use client::{selective_attach, AttachResult, CdpClient, CdpEvent};
pub use discovery::{BrowserKind as CdpBrowserKind, CdpTarget, DiscoveryError};
pub use events::{event_session_id, parse_request_will_be_sent, CdpInitiator, CdpRequest};
pub use session::{SessionId, SessionManager};

// ── 错误类型 ─────────────────────────────────────────────────

/// CDP 客户端统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("discovery error: {0}")]
    Discovery(#[from] DiscoveryError),

    #[error("websocket error: {0}")]
    WebSocket(String),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session limit exceeded: attached={attached}, limit={limit}")]
    SessionLimitExceeded { attached: usize, limit: usize },

    #[error("cdp protocol error: {code} {message}")]
    Protocol { code: i64, message: String },
}
