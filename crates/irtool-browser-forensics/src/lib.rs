//! Browser Forensics: 浏览器扩展归因 + 上下文归因
//!
//! 联合实现《浏览器扩展网络请求归因方案》和《浏览器上下文归因方案》。

pub mod context_attribution;
pub mod core;
pub mod download;
pub mod extension_attribution;
pub mod extension_inventory;
pub mod extension_risk;
pub mod history;
pub mod permission_matcher;
pub mod profile;
pub mod session_recovery;
pub mod sqlite;

pub use context_attribution::{
    attribute_browser_context, BrowserContext, BrowserContextDetail, CurrentTab, MaliciousConnection,
};
pub use core::*;
pub use download::{scan_downloads, scan_downloads_in_time_window, DangerType, DownloadAttribution, DownloadInfo};
pub use extension_attribution::{attribute_extension, ExtensionAttribution};
pub use extension_inventory::{scan_extensions, scan_extensions_cached, ExtensionInfo, ExtensionInventory};
pub use extension_risk::IocMatch;
pub use history::{
    attribute_history, build_navigation_chain, scan_history, transition_to_string, HistoryAttribution,
    HistoryEntry, HistoryList, NavChainNode, RecentActivity, TimeTier,
};
pub use permission_matcher::{match_domain_to_extensions, MatchedExtension, PermissionMatchResult};
pub use profile::{enumerate_all_profiles, enumerate_profiles, BrowserProfile};
pub use session_recovery::{recover_tabs, RecoveredTab, SessionRecoveryResult};
