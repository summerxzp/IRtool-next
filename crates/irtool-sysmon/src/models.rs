use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

/// Event type discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SysmonEventType {
    ProcessCreate,
    FileCreateTime,
    NetworkConnect,
    ProcessTerminate,
    DriverLoad,
    ImageLoad,
    CreateRemoteThread,
    RawAccessRead,
    ProcessAccess,
    FileCreate,
    RegistryEvent,
    FileCreateStreamHash,
    PipeEvent,
    WmiEvent,
    Dns,
    DnsClient,
    FileDelete,
    ClipboardChange,
    ProcessTampering,
    FileDeleteDetected,
    Unknown,
}

impl SysmonEventType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ProcessCreate => "进程创建",
            Self::FileCreateTime => "文件创建时间修改",
            Self::NetworkConnect => "网络连接",
            Self::ProcessTerminate => "进程终止",
            Self::DriverLoad => "驱动加载",
            Self::ImageLoad => "DLL加载",
            Self::CreateRemoteThread => "远程线程创建",
            Self::RawAccessRead => "原始磁盘访问",
            Self::ProcessAccess => "进程访问",
            Self::FileCreate => "文件创建",
            Self::RegistryEvent => "注册表事件",
            Self::FileCreateStreamHash => "文件流哈希",
            Self::PipeEvent => "管道事件",
            Self::WmiEvent => "WMI事件",
            Self::Dns => "DNS查询",
            Self::DnsClient => "DNS-Client",
            Self::FileDelete => "文件删除",
            Self::ClipboardChange => "剪贴板变化",
            Self::ProcessTampering => "进程篡改",
            Self::FileDeleteDetected => "文件删除检测",
            Self::Unknown => "未知",
        }
    }

    /// Map from Sysmon EventID to event type.
    pub fn from_event_id(event_id: u32) -> Self {
        match event_id {
            1 => Self::ProcessCreate,
            2 => Self::FileCreateTime,
            3 => Self::NetworkConnect,
            5 => Self::ProcessTerminate,
            6 => Self::DriverLoad,
            7 => Self::ImageLoad,
            8 => Self::CreateRemoteThread,
            9 => Self::RawAccessRead,
            10 => Self::ProcessAccess,
            11 => Self::FileCreate,
            12 => Self::RegistryEvent,
            15 => Self::FileCreateStreamHash,
            17 => Self::PipeEvent,
            19 => Self::WmiEvent,
            22 => Self::Dns,
            3008 => Self::DnsClient,
            23 => Self::FileDelete,
            24 => Self::ClipboardChange,
            25 => Self::ProcessTampering,
            26 => Self::FileDeleteDetected,
            _ => Self::Unknown,
        }
    }
}

/// Unified Sysmon event struct. Fields not applicable to a given event type are None/empty.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SysmonEvent {
    // Common fields
    pub event_id: u32,
    pub event_type: SysmonEventType,
    pub timestamp: String,
    pub timestamp_epoch: f64,
    pub timestamp_valid: bool,
    pub record_id: Option<u64>,
    pub raw_data: HashMap<String, String>,

    // DNS (EventID 22) + Network (EventID 3) + FileCreate (EventID 11)
    pub process_id: u32,
    pub process_name: String,
    pub process_path: String,
    pub user: String,
    pub rule_name: String,

    // DNS specific (EventID 22)
    pub query_name: String,
    pub query_results: String,
    pub query_status: u32,

    // Network specific (EventID 3)
    pub source_ip: String,
    pub source_port: u16,
    pub destination_ip: String,
    pub destination_port: u16,
    pub protocol: String,
    pub initiated: bool,
    pub is_external: bool,

    // Remote thread specific (EventID 8)
    pub source_process_id: u32,
    pub source_process_name: String,
    pub source_process_path: String,
    pub target_process_id: u32,
    pub target_process_name: String,
    pub target_process_path: String,
    pub start_address: String,
    pub start_module: String,
    pub start_function: String,
    pub is_suspicious: bool,

    // FileCreate specific (EventID 11)
    pub target_filename: String,
    pub creation_utc_time: String,
}

/// Event configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EventConfigEntry {
    pub key: String,
    pub name: String,
    pub event_id: u32,
    pub xml_tag: String,
    pub default_enabled: bool,
}

/// Sysmon service status info.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SysmonStatus {
    pub installed: bool,
    pub running: bool,
    pub service_name: Option<String>,
    pub sysmon_exe_exists: bool,
    pub config_exists: bool,
    pub sysmon_exe_path: String,
    pub config_path: String,
    pub started_by_irtool: bool,
    pub config_managed_by_irtool: bool,
}

/// Helper: extract process name from full path.
pub fn extract_process_name(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    path.rsplit('\\').next().unwrap_or(path).to_string()
}

/// Helper: check if an IP is private/loopback/link-local.
pub fn is_private_ip(ip_str: &str) -> bool {
    if ip_str.is_empty() || ip_str == "0.0.0.0" || ip_str == "::" || ip_str == "::ffff:0.0.0.0" || ip_str == "*" {
        return true;
    }
    let ip_lower = ip_str.to_lowercase();
    if ip_lower.starts_with("10.") || ip_lower.starts_with("192.168.") {
        return true;
    }
    if ip_lower.starts_with("172.") {
        if let Some(octet) = ip_lower.strip_prefix("172.").and_then(|s| s.split('.').next()) {
            if let Ok(second) = octet.parse::<u8>() {
                if (16..=31).contains(&second) {
                    return true;
                }
            }
        }
    }
    if ip_lower.starts_with("127.") || ip_lower == "::1" {
        return true;
    }
    if ip_lower.starts_with("169.254.") || ip_lower.starts_with("fe80") {
        return true;
    }
    false
}

/// Helper: check if a file path is in a suspicious (user-writable) location.
pub fn is_suspicious_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    const SUSPICIOUS: &[&str] = &[
        "\\temp\\",
        "\\tmp\\",
        "\\appdata\\local\\temp\\",
        "\\downloads\\",
        "\\programdata\\",
    ];
    SUSPICIOUS.iter().any(|s| lower.contains(s))
}

/// Helper: check if a remote thread event is suspicious.
pub fn is_suspicious_remote_thread(source_name: &str, target_name: &str) -> bool {
    const SUSPICIOUS_SOURCES: &[&str] = &["rundll32.exe", "regsvr32.exe", "mshta.exe", "powershell.exe", "cmd.exe"];
    const SUSPICIOUS_TARGETS: &[&str] = &["lsass.exe", "svchost.exe", "csrss.exe", "services.exe"];
    let src_lower = source_name.to_lowercase();
    let tgt_lower = target_name.to_lowercase();
    SUSPICIOUS_SOURCES.contains(&src_lower.as_str()) || SUSPICIOUS_TARGETS.contains(&tgt_lower.as_str())
}

/// Default event configurations. DNS Client + DNS + Network are enabled by default.
pub fn default_event_configs() -> Vec<EventConfigEntry> {
    vec![
        EventConfigEntry {
            key: "dns_client".into(),
            name: "DNS-Client".into(),
            event_id: 3008,
            xml_tag: "DnsClient".into(),
            default_enabled: true,
        },
        EventConfigEntry {
            key: "dns".into(),
            name: "DNS查询".into(),
            event_id: 22,
            xml_tag: "DnsQuery".into(),
            default_enabled: true,
        },
        EventConfigEntry {
            key: "network_connect".into(),
            name: "网络连接".into(),
            event_id: 3,
            xml_tag: "NetworkConnect".into(),
            default_enabled: true,
        },
        EventConfigEntry {
            key: "process_create".into(),
            name: "进程创建".into(),
            event_id: 1,
            xml_tag: "ProcessCreate".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "file_create_time".into(),
            name: "文件创建时间修改".into(),
            event_id: 2,
            xml_tag: "FileCreateTime".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "process_terminate".into(),
            name: "进程终止".into(),
            event_id: 5,
            xml_tag: "ProcessTerminate".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "driver_load".into(),
            name: "驱动加载".into(),
            event_id: 6,
            xml_tag: "DriverLoad".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "image_load".into(),
            name: "DLL加载".into(),
            event_id: 7,
            xml_tag: "ImageLoad".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "remote_thread".into(),
            name: "远程线程创建".into(),
            event_id: 8,
            xml_tag: "CreateRemoteThread".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "raw_access_read".into(),
            name: "原始磁盘访问".into(),
            event_id: 9,
            xml_tag: "RawAccessRead".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "process_access".into(),
            name: "进程访问".into(),
            event_id: 10,
            xml_tag: "ProcessAccess".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "file_create".into(),
            name: "文件创建".into(),
            event_id: 11,
            xml_tag: "FileCreate".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "registry_event".into(),
            name: "注册表事件".into(),
            event_id: 12,
            xml_tag: "RegistryEvent".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "file_create_stream_hash".into(),
            name: "文件流哈希".into(),
            event_id: 15,
            xml_tag: "FileCreateStreamHash".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "pipe_event".into(),
            name: "管道事件".into(),
            event_id: 17,
            xml_tag: "PipeEvent".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "wmi_event".into(),
            name: "WMI事件".into(),
            event_id: 19,
            xml_tag: "WmiEvent".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "file_delete".into(),
            name: "文件删除".into(),
            event_id: 23,
            xml_tag: "FileDelete".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "clipboard_change".into(),
            name: "剪贴板变化".into(),
            event_id: 24,
            xml_tag: "ClipboardChange".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "process_tampering".into(),
            name: "进程篡改".into(),
            event_id: 25,
            xml_tag: "ProcessTampering".into(),
            default_enabled: false,
        },
        EventConfigEntry {
            key: "file_delete_detected".into(),
            name: "文件删除检测".into(),
            event_id: 26,
            xml_tag: "FileDeleteDetected".into(),
            default_enabled: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_process_name() {
        assert_eq!(extract_process_name(r"C:\Windows\System32\svchost.exe"), "svchost.exe");
        assert_eq!(extract_process_name(""), "");
        assert_eq!(extract_process_name("cmd.exe"), "cmd.exe");
    }

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip("10.0.0.1"));
        assert!(is_private_ip("192.168.1.1"));
        assert!(is_private_ip("172.16.0.1"));
        assert!(is_private_ip("127.0.0.1"));
        assert!(!is_private_ip("8.8.8.8"));
        assert!(!is_private_ip("1.2.3.4"));
    }

    #[test]
    fn test_is_suspicious_path() {
        assert!(is_suspicious_path(r"C:\Temp\malware.exe"));
        assert!(is_suspicious_path(r"C:\Users\x\AppData\Local\Temp\x.exe"));
        assert!(!is_suspicious_path(r"C:\Windows\System32\svchost.exe"));
    }

    #[test]
    fn test_is_suspicious_remote_thread() {
        assert!(is_suspicious_remote_thread("powershell.exe", "explorer.exe"));
        assert!(is_suspicious_remote_thread("something.exe", "lsass.exe"));
        assert!(!is_suspicious_remote_thread("explorer.exe", "notepad.exe"));
    }

    #[test]
    fn test_default_event_configs() {
        let configs = default_event_configs();
        assert!(configs.len() >= 20);
        let enabled: Vec<_> = configs.iter().filter(|c| c.default_enabled).collect();
        assert_eq!(enabled.len(), 3); // DNS Client + DNS + Network
        assert!(enabled.iter().any(|c| c.key == "dns_client"));
        assert!(enabled.iter().any(|c| c.key == "dns"));
        assert!(enabled.iter().any(|c| c.key == "network_connect"));
    }
}
