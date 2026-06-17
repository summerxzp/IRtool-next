use serde::Serialize;
use specta::Type;
use tokio::sync::broadcast;

use irtool_autoruns::{ScanProgress, SignatureProgress};
use irtool_monitor::Alert;
use irtool_pcap::PcapEvent;
use irtool_sysmon::SysmonEvent;

use crate::dto::network::{NetworkEnrichmentPayload, NetworkSnapshotPayload};

/// Unified application event enum, published via [`EventBus`].
///
/// Both Tauri and egui frontends subscribe to this enum and bridge
/// events into their respective UI update mechanisms.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(tag = "type", content = "payload")]
pub enum AppEvent {
    // Network
    NetworkSnapshot(NetworkSnapshotPayload),
    NetworkError(String),
    NetworkEnrichment(NetworkEnrichmentPayload),

    // Autoruns
    AutorunsProgress(ScanProgress),
    AutorunsSignatureProgress(SignatureProgress),
    AutorunsHashProgress(SignatureProgress),
    AutorunsScanComplete { count: usize },
    AutorunsScanCancelled(u64),
    AutorunsScanFailed { task_id: u64, error: String },

    // Sysmon
    SysmonEvent(SysmonEvent),

    // Monitor
    MonitorAlert(Alert),

    // Pcap
    PcapEvent(PcapEvent),

    // Tools
    ToolsDownloadProgress { tool_id: String, downloaded: u64, total: u64 },
    ToolsDownloadError { tool_id: String, error: String },
    ToolsDownloadComplete { errors: usize },

    // Window
    CloseRequested,
}

/// Broadcast-based event bus shared across all services and frontends.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Publish an event to all subscribers. Silently drops if there are no receivers.
    pub fn publish(&self, event: AppEvent) {
        let _ = self.tx.send(event);
    }

    /// Create a new subscriber. Events published after this call will be received.
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
