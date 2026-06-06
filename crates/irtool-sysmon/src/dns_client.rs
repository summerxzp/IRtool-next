use crate::models::*;
use irtool_core::IrError;
use std::collections::HashMap;
use std::process::Command;
use tracing::{info, warn};

const DNS_CLIENT_CHANNEL: &str = "Microsoft-Windows-DNS-Client/Operational";
const DNS_CLIENT_EVENT_ID: u32 = 3008; // DNS Query event

pub struct DnsClientLogManager {
    was_enabled: bool,
}

impl DnsClientLogManager {
    pub fn new() -> Self {
        Self {
            was_enabled: false,
        }
    }

    #[cfg(windows)]
    pub fn enable(&mut self) -> Result<(), IrError> {
        info!("Checking DNS Client event log state...");
        
        let check_result = Command::new("wevtutil")
            .args(&["get-log", DNS_CLIENT_CHANNEL])
            .output();
        
        match check_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.was_enabled = stdout.contains("enabled: true");
                
                if self.was_enabled {
                    info!("DNS Client event log is already enabled");
                    return Ok(());
                }
            }
            Err(e) => {
                warn!("Failed to check DNS Client log state: {}", e);
            }
        }
        
        info!("Enabling DNS Client event log...");
        
        let result = Command::new("wevtutil")
            .args(&["sl", DNS_CLIENT_CHANNEL, "/e:true"])
            .output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    info!("DNS Client event log enabled successfully");
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    warn!("wevtutil enable command failed: {}", error_msg);
                    // 即使失败也不阻塞，可能日志已经启用
                }
            }
            Err(e) => {
                warn!("Failed to run wevtutil: {}", e);
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn enable(&mut self) -> Result<(), IrError> {
        Ok(())
    }

    #[cfg(windows)]
    pub fn restore(&mut self) -> Result<(), IrError> {
        if self.was_enabled {
            info!("DNS Client event log was already enabled, no need to restore");
            return Ok(());
        }
        
        info!("Restoring DNS Client event log state...");
        
        let result = Command::new("wevtutil")
            .args(&["sl", DNS_CLIENT_CHANNEL, "/e:false"])
            .output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    info!("DNS Client event log restored to disabled state");
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    warn!("Failed to restore DNS Client log: {}", error_msg);
                }
            }
            Err(e) => {
                warn!("Failed to run wevtutil restore: {}", e);
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn restore(&mut self) -> Result<(), IrError> {
        Ok(())
    }
}

impl Default for DnsClientLogManager {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_timestamp(time_str: &str) -> (String, f64, bool) {
    if time_str.is_empty() {
        return (String::new(), 0.0, false);
    }

    let cleaned = time_str.replace('Z', "+00:00");

    match chrono::DateTime::parse_from_rfc3339(&cleaned) {
        Ok(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            let ts = local.format("%Y/%m/%d %H:%M:%S").to_string();
            let epoch = local.timestamp() as f64 + local.timestamp_subsec_nanos() as f64 / 1e9;
            (ts, epoch, true)
        }
        Err(_) => {
            for fmt in &["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, fmt) {
                    let ts = dt.format("%Y/%m/%d %H:%M:%S").to_string();
                    let epoch = dt.and_utc().timestamp() as f64;
                    return (ts, epoch, true);
                }
            }
            (time_str.to_string(), 0.0, false)
        }
    }
}

pub fn parse_dns_client_event(raw_data: &HashMap<String, String>) -> Option<SysmonEvent> {
    let event_id = raw_data.get("EventID").and_then(|s| s.parse::<u32>().ok())?;
    
    if event_id != DNS_CLIENT_EVENT_ID {
        return None;
    }
    
    let get_opt = |key: &str| raw_data.get(key).cloned();
    let get_u32_opt = |key: &str| raw_data.get(key).and_then(|v| v.parse().ok());
    
    let query_name = get_opt("QueryName")
        .or_else(|| get_opt("QNAME"))
        .unwrap_or_default();
    
    let process_path = get_opt("ProcessName")
        .or_else(|| get_opt("Image"))
        .unwrap_or_default();
    let process_name = crate::models::extract_process_name(&process_path);
    
    let timestamp_str = get_opt("TimeCreated")
        .or_else(|| get_opt("UtcTime"))
        .unwrap_or_default();
    
    let (timestamp, timestamp_epoch, timestamp_valid) = parse_timestamp(&timestamp_str);
    
    let mut raw_data_extended = HashMap::new();
    raw_data_extended.insert("QueryName".to_string(), query_name.clone());
    raw_data_extended.extend(raw_data.clone());
    
    Some(SysmonEvent {
        event_id: 3008, // Windows DNS Client event ID, 区别于 Sysmon DnsQuery (22)
        event_type: SysmonEventType::DnsClient,
        timestamp,
        timestamp_epoch,
        timestamp_valid,
        record_id: None,
        raw_data: raw_data_extended,
        process_id: get_u32_opt("ProcessId")
            .or_else(|| get_u32_opt("PID"))
            .unwrap_or(0),
        process_name,
        process_path,
        user: String::new(),
        rule_name: String::new(),
        query_name,
        query_results: String::new(),
        query_status: 0,
        source_ip: String::new(),
        source_port: 0,
        destination_ip: String::new(),
        destination_port: 0,
        protocol: String::new(),
        initiated: false,
        is_external: false,
        source_process_id: 0,
        source_process_name: String::new(),
        source_process_path: String::new(),
        target_process_id: 0,
        target_process_name: String::new(),
        target_process_path: String::new(),
        start_address: String::new(),
        start_module: String::new(),
        start_function: String::new(),
        is_suspicious: false,
        target_filename: String::new(),
        creation_utc_time: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(windows)]
    fn test_dns_client_manager() {
        let manager = DnsClientLogManager::new();
        // 测试不会实际修改系统状态
        assert!(!manager.was_enabled);
    }
    
    #[test]
    fn test_parse_dns_client_event() {
        let mut raw_data = HashMap::new();
        raw_data.insert("EventID".to_string(), "3008".to_string());
        raw_data.insert("QueryName".to_string(), "example.com".to_string());
        raw_data.insert("QueryType".to_string(), "A".to_string());
        raw_data.insert("ProcessId".to_string(), "1234".to_string());
        raw_data.insert("Image".to_string(), "C:\\Windows\\System32\\nslookup.exe".to_string());
        raw_data.insert("TimeCreated".to_string(), "2024-01-15T10:30:00.000Z".to_string());
        
        let event = parse_dns_client_event(&raw_data);
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_id, 3008);
        assert_eq!(event.event_type, SysmonEventType::DnsClient);
        assert_eq!(event.query_name, "example.com");
        assert_eq!(event.process_name, "nslookup.exe");
    }
}
