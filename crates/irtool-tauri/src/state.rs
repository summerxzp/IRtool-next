use irtool_autoruns::{AutorunsScanner, AutorunsStore};
use irtool_core::TaskRegistry;
use irtool_net_monitor::{HistoryStore, RetentionPolicy, WindowsNetCollector};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub tasks: Arc<TaskRegistry>,
    pub net_collector: Arc<WindowsNetCollector>,
    pub net_history: Arc<HistoryStore>,
    pub net_polling: Arc<Mutex<NetworkPollingState>>,
    // --- P2 新增 ---
    pub autoruns_scanner: Arc<Option<AutorunsScanner>>,
    pub autoruns_store: Arc<AutorunsStore>,
    // --- P4 新增 ---
    pub sysmon_reader: Arc<irtool_sysmon::SysmonReader>,
    pub sysmon_config: Arc<irtool_sysmon::SysmonConfigManager>,
}

pub struct NetworkPollingState {
    pub interval_ms: u64,
    pub paused: bool,
    pub retention: RetentionPolicy,
    pub cancel: Option<CancellationToken>,
}

impl Default for NetworkPollingState {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            paused: false,
            retention: RetentionPolicy::default(),
            cancel: None,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        let autoruns_scanner = match AutorunsScanner::new() {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!("autoruns scanner init failed: {} — autoruns features disabled", e);
                None
            }
        };

        Self {
            tasks: Arc::new(TaskRegistry::new()),
            net_collector: Arc::new(WindowsNetCollector::new()),
            net_history: Arc::new(HistoryStore::new()),
            net_polling: Arc::new(Mutex::new(NetworkPollingState::default())),
            autoruns_scanner: Arc::new(autoruns_scanner),
            autoruns_store: Arc::new(AutorunsStore::new()),
            // --- P4 新增 ---
            sysmon_reader: Arc::new(irtool_sysmon::SysmonReader::new()),
            sysmon_config: Arc::new(irtool_sysmon::SysmonConfigManager::new(None, None, &std::path::PathBuf::from("."))),
        }
    }
}
