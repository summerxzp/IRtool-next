//! Re-exported business types for frontend consumption.
//!
//! Frontend crates (irtool-egui, future irtool-cli) should depend ONLY on
//! irtool-service and import types from here, NOT directly from business crates.
//! This ensures the service layer remains the single dependency boundary.

// ── Network ──
pub use irtool_net_monitor::{
    NetConn, ConnState, Proto, CmdlineStatus, CmdlineResult, RetentionPolicy,
};

// ── Autoruns ──
pub use irtool_autoruns::{
    AutorunItem, ScanOptions, ScanProgress, ScanPhase,
    SignatureProgress, DeleteResult,
};

// ── Sysmon ──
pub use irtool_sysmon::{SysmonEvent};

// ── Monitor ──
pub use irtool_monitor::{
    Alert, MonitorEvent, MonitorConfig, EventQuery, EventPage,
    EventSource, RuntimeTelemetry,
};

// ── Pcap ──
pub use irtool_pcap::{PcapEvent};
