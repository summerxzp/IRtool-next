use irtool_net_monitor::{CmdlineStatus, NetConn, RetentionPolicy};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkSnapshotPayload {
    pub items: Vec<NetConn>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicyDto {
    None,
    Seconds(u64),
    Forever,
}

impl From<RetentionPolicyDto> for RetentionPolicy {
    fn from(value: RetentionPolicyDto) -> Self {
        match value {
            RetentionPolicyDto::None => RetentionPolicy::None,
            RetentionPolicyDto::Seconds(s) => RetentionPolicy::Seconds(s),
            RetentionPolicyDto::Forever => RetentionPolicy::Forever,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkPollingControl {
    pub interval_ms: Option<u64>,
    pub paused: Option<bool>,
    pub retention: Option<RetentionPolicyDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NetworkEnrichmentPayload {
    pub pid: u32,
    pub cmdline_status: CmdlineStatus,
    pub process_cmdline: Option<String>,
}
