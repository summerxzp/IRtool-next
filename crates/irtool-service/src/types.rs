//! Re-exported business types for frontend consumption.
//!
//! Frontend crates (irtool-egui, future irtool-cli) should depend ONLY on
//! irtool-service and import types from here, NOT directly from business crates.
//! This ensures the service layer remains the single dependency boundary.

// ── Network ──
pub use irtool_net_monitor::{CmdlineResult, CmdlineStatus, ConnState, NetConn, Proto, RetentionPolicy};

// ── Autoruns ──
pub use irtool_autoruns::{
    AutorunItem, DeleteResult, RiskLevel, ScanOptions, ScanPhase, ScanProgress, SignatureProgress, SignatureStatus,
};

// ── Sysmon ──
pub use irtool_sysmon::{EventConfigEntry, SysmonEvent, SysmonEventType, SysmonStatus};

// ── Monitor ──
pub use irtool_monitor::{
    Alert, EventPage, EventQuery, EventSource, MonitorConfig, MonitorEvent, MonitorRule, NotifyConfig, RuntimeTelemetry,
};

// ── Pcap ──
pub use irtool_pcap::{AdapterInfo, PcapConfig, PcapCountersSnapshot, PcapEvent, PcapEventKind};

// ── Process ──
pub use irtool_process::{ProcessChain, ProcessNode};

// ── Tools ──
pub use irtool_tools::ToolStatus;
