use serde::{Deserialize, Serialize};
use specta::Type;

/// 监控事件的统一轻量模型，从各采集模块转换而来
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MonitorEvent {
    /// 数据库记录 ID（由 SQLite 自动生成），仅查询 DB 时有值，采集时始终为 0
    pub id: i64,
    /// epoch 毫秒
    pub timestamp: i64,
    /// 事件来源
    pub source: EventSource,
    /// 事件类型标识（dns / dns_client / network_connect / ...）
    pub event_type: String,
    /// 进程名
    pub process_name: String,
    /// 规则匹配关键字段：域名 or IP:Port
    pub key_field: String,
    /// 原始数据 JSON
    pub raw_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Sysmon,
    DnsClient,
    NetMonitor,
    Pcap,
}

/// 监控规则
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MonitorRule {
    pub name: String,
    /// 匹配目标：域名（支持 *.suffix 通配）or IP or CIDR
    pub targets: Vec<String>,
    /// 只匹配哪些事件类型，空=全部
    pub event_types: Vec<String>,
    /// 通知方式
    pub actions: Vec<NotifyAction>,
    /// 是否启用
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotifyAction {
    Popup,
    Feishu { webhook_url: String },
}

/// 告警记录
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Alert {
    pub id: i64,
    pub timestamp: i64,
    pub rule_name: String,
    pub event_type: String,
    pub process_name: String,
    pub key_field: String,
    pub action_taken: String,
    pub raw_json: String,
}

/// 后台监控配置
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MonitorConfig {
    /// 是否启用后台监控模式
    pub background_mode: bool,
    /// 后台模式时持久化哪些事件类型，空=全部
    pub persist_event_types: Vec<String>,
    /// SQLite 中事件保留天数，0=永久
    pub retention_days: u32,
    /// 监控规则列表
    pub rules: Vec<MonitorRule>,
    /// 数据库存储路径，空=使用默认路径（可执行文件同目录/data/monitor.db）
    pub db_path: String,
    /// 启用 TLS SNI 提取（网络层抓包 TCP:443）
    pub enable_sni: bool,
    /// 启用网络层 DNS 抓包（UDP:53）
    pub enable_dns_pcap: bool,
    /// 每次从数据库加载多少条记录（供前端review使用）
    pub load_limit: u32,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            background_mode: false,
            persist_event_types: vec![],
            retention_days: 7,
            rules: vec![],
            db_path: String::new(),
            enable_sni: true,
            enable_dns_pcap: true,
            load_limit: 1000,
        }
    }
}
