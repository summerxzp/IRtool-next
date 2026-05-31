use irtool_core::TaskRegistry;
use irtool_net_monitor::{HistoryStore, RetentionPolicy, WindowsNetCollector};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    pub tasks: Arc<TaskRegistry>,
    pub net_collector: Arc<WindowsNetCollector>,
    pub net_history: Arc<HistoryStore>,
    pub net_polling: Arc<Mutex<NetworkPollingState>>,
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
        Self {
            tasks: Arc::new(TaskRegistry::new()),
            net_collector: Arc::new(WindowsNetCollector::new()),
            net_history: Arc::new(HistoryStore::new()),
            net_polling: Arc::new(Mutex::new(NetworkPollingState::default())),
        }
    }
}
