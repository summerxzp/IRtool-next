use irtool_autoruns::{AutorunsScanner, AutorunsStore};
use irtool_core::{AppDirs, TaskRegistry};
use irtool_net_monitor::{CmdlineEnricher, HistoryStore, RetentionPolicy, WindowsNetCollector};
use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub tasks: Arc<TaskRegistry>,
    pub net_collector: Arc<WindowsNetCollector>,
    pub net_history: Arc<HistoryStore>,
    pub net_polling: Arc<Mutex<NetworkPollingState>>,
    pub net_enricher: Arc<CmdlineEnricher>,
    // --- P2 新增 ---
    pub autoruns_scanner: Arc<Option<AutorunsScanner>>,
    pub autoruns_store: Arc<AutorunsStore>,
    pub autoruns_scanning: Arc<AtomicBool>,
    // --- P4 新增 ---
    pub sysmon_reader: Arc<irtool_sysmon::SysmonReader>,
    pub sysmon_config: Arc<irtool_sysmon::SysmonConfigManager>,
    pub dns_client_manager: Arc<Mutex<irtool_sysmon::DnsClientLogManager>>,
    // --- P5 新增 ---
    pub monitor_engine: Arc<sync::Mutex<irtool_monitor::MonitorEngine>>,
    // --- P6 新增 ---
    pub pcap_collector: Arc<sync::Mutex<irtool_pcap::PcapCollector>>,
    // --- AppDirs ---
    pub app_dirs: Arc<AppDirs>,
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
    pub fn new(app_dirs: AppDirs) -> Self {
        let root = app_dirs.root().to_path_buf();
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
            net_enricher: Arc::new(CmdlineEnricher::new()),
            autoruns_scanner: Arc::new(autoruns_scanner),
            autoruns_store: Arc::new(AutorunsStore::new()),
            autoruns_scanning: Arc::new(AtomicBool::new(false)),
            // --- P4 新增 ---
            sysmon_reader: Arc::new(irtool_sysmon::SysmonReader::new()),
            sysmon_config: Arc::new(irtool_sysmon::SysmonConfigManager::new(None, None, &root)),
            dns_client_manager: Arc::new(Mutex::new(irtool_sysmon::DnsClientLogManager::new())),
            // --- P5 新增 ---
            monitor_engine: Arc::new(sync::Mutex::new(irtool_monitor::MonitorEngine::new(&root))),
            // --- P6 新增 ---
            pcap_collector: Arc::new(sync::Mutex::new(irtool_pcap::PcapCollector::new())),
            // --- AppDirs ---
            app_dirs: Arc::new(app_dirs),
        }
    }
}
