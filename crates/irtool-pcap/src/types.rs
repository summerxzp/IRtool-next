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
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PcapConfig {
    pub enable_sni: bool,
    pub enable_dns_pcap: bool,
}

impl Default for PcapConfig {
    fn default() -> Self {
        Self {
            enable_sni: true,
            enable_dns_pcap: true,
        }
    }
}
