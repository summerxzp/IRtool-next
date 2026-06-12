use serde::{Deserialize, Serialize};
use specta::Type;

/// pcap 捕获的事件
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PcapEvent {
    pub timestamp: i64,
    pub event_kind: PcapEventKind,
    pub domain: String,
    pub src_ip: String,
    pub src_port: u16,
    pub dst_ip: String,
    pub dst_port: u16,
    /// DNS 查询类型（仅 DNS 事件）
    pub query_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PcapEventKind {
    /// TLS ClientHello 中的 SNI
    TlsSni,
    /// 网络层 DNS 查询
    DnsQuery,
}

/// pcap 配置
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct PcapConfig {
    pub enable_sni: bool,
    pub enable_dns_pcap: bool,
    /// Optional specific adapter IP to bind to. None = auto-detect.
    pub adapter_ip: Option<String>,
    /// Auto-stop after this many seconds. 0 = no limit.
    #[serde(default)]
    pub max_duration_secs: u32,
}

/// 适配器信息
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AdapterInfo {
    pub name: String,
    pub ip: String,
    pub description: String,
}

/// 捕获计数器快照
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PcapCountersSnapshot {
    pub packets_seen: u64,
    pub events_extracted: u64,
    pub parse_errors: u64,
    pub dropped_events: u64,
}
