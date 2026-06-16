use serde::{Deserialize, Serialize};
use specta::Type;

/// 生成唯一 ID（基于时间戳和名称哈希）
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
    /// 规则唯一 ID（用于关联通知配置）
    pub id: String,
    pub name: String,
    /// 匹配目标：域名（支持 *.suffix 通配）or IP or CIDR
    pub targets: Vec<String>,
    /// 只匹配哪些事件类型，空=全部
    pub event_types: Vec<String>,
    /// 是否启用
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotifyAction {
    Popup,
    Feishu { webhook_url: String },
}

/// 通知配置（与告警规则分离）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NotifyConfig {
    /// 触发弹窗的规则 ID 列表
    pub popup_rule_ids: Vec<String>,
    /// 触发飞书的规则 ID 列表
    pub feishu_rule_ids: Vec<String>,
    /// 飞书 Webhook URL
    pub feishu_webhook_url: String,
    /// 弹窗显示时长（秒），0 = 不自动关闭
    #[serde(default = "default_popup_duration")]
    pub popup_duration_secs: u32,
}

fn default_popup_duration() -> u32 {
    10
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            popup_rule_ids: vec![],
            feishu_rule_ids: vec![],
            feishu_webhook_url: String::new(),
            popup_duration_secs: 10,
        }
    }
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
    #[serde(default = "default_true")]
    pub enable_sni: bool,
    /// 启用网络层 DNS 抓包（UDP:53）
    #[serde(default = "default_true")]
    pub enable_dns_pcap: bool,
    /// 指定绑定的适配器 IP，None = 自动检测
    #[serde(default)]
    pub adapter_ip: Option<String>,
    /// 抓包自动停止时长（秒），0 = 不限制
    #[serde(default)]
    pub max_duration_secs: u32,
    /// 每次从数据库加载多少条记录（供前端review使用）
    pub load_limit: u32,
    /// 数据库最大大小（MB），0=不限制
    #[serde(default = "default_max_size_mb")]
    pub max_size_mb: u32,
    /// 通知配置（与告警规则分离）
    #[serde(default)]
    pub notify_config: NotifyConfig,
}

fn default_true() -> bool {
    true
}

fn default_max_size_mb() -> u32 {
    512
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            background_mode: false,
            persist_event_types: vec![],
            retention_days: 7,
            rules: vec![],
            db_path: String::new(),
            enable_sni: true,      // 默认启用 TLS SNI 提取
            enable_dns_pcap: true, // 默认启用网络层 DNS 抓包
            adapter_ip: None,
            max_duration_secs: 0,
            load_limit: 1000,
            max_size_mb: 512,
            notify_config: NotifyConfig::default(),
        }
    }
}

/// 事件分页查询参数
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EventQuery {
    pub source: Option<String>,
    pub event_type: Option<String>,
    pub process_name: Option<String>,
    pub key_field: Option<String>,
    pub is_external: Option<bool>,
    pub search_text: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// 事件分页结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EventPage {
    pub items: Vec<MonitorEvent>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// 运行模式
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum RuntimeMode {
    Foreground,
    Background,
}

/// 运行时遥测信息
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RuntimeTelemetry {
    pub mode: RuntimeMode,
    pub started_at: Option<i64>,
    pub events_written: u64,
    pub events_dropped: u64,
    pub last_event_at: Option<i64>,
    pub last_error: Option<String>,
}

/// 生成唯一 ID（基于时间戳和名称）
pub fn generate_rule_id(name: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{}:{}:{}", duration.as_nanos(), name, std::process::id())
}
