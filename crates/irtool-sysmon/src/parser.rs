use crate::models::*;
use irtool_core::IrError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;


/// Parse a Sysmon event XML string into a typed event.
pub fn parse_event(xml: &str) -> Result<SysmonEvent, IrError> {
    let (event, _) = parse_event_inner(xml, false)?;
    Ok(event)
}

/// Parse a Sysmon event XML string, also extracting the EventRecordID.
pub fn parse_event_with_record_id(xml: &str) -> Result<(SysmonEvent, Option<u64>), IrError> {
    parse_event_inner(xml, true)
}

fn parse_event_inner(xml: &str, extract_record_id: bool) -> Result<(SysmonEvent, Option<u64>), IrError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut event_id: u32 = 0;
    let mut timestamp_str = String::new();
    let mut record_id: Option<u64> = None;
    let mut event_data: HashMap<String, String> = HashMap::new();

    let mut in_system = false;
    let mut in_event_data = false;
    let mut current_data_name = String::new();
    let mut in_event_id = false;
    let mut in_record_id = false;
    let mut in_data = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = String::from_utf8_lossy(local.as_ref()).into_owned();
                match name.as_ref() {
                    "System" => in_system = true,
                    "EventData" => in_event_data = true,
                    "EventID" if in_system => in_event_id = true,
                    "TimeCreated" if in_system => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"SystemTime" {
                                timestamp_str = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "EventRecordID" if in_system && extract_record_id => in_record_id = true,
                    "Data" if in_event_data => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"Name" {
                                current_data_name = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        in_data = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = String::from_utf8_lossy(local.as_ref()).into_owned();
                if name == "TimeCreated" && in_system {
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == b"SystemTime" {
                            timestamp_str = String::from_utf8_lossy(&attr.value).to_string();
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_event_id {
                    event_id = text.parse().unwrap_or(0);
                } else if in_record_id {
                    record_id = text.parse().ok();
                } else if in_data && !current_data_name.is_empty() {
                    event_data.insert(current_data_name.clone(), text);
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = String::from_utf8_lossy(local.as_ref()).into_owned();
                match name.as_ref() {
                    "System" => in_system = false,
                    "EventData" => in_event_data = false,
                    "EventID" => in_event_id = false,
                    "EventRecordID" => in_record_id = false,
                    "Data" => {
                        in_data = false;
                        current_data_name.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(IrError::Internal(format!("XML parse error: {}", e)));
            }
            _ => {}
        }
        buf.clear();
    }

    let (timestamp, timestamp_epoch, timestamp_valid) = parse_timestamp(&timestamp_str);
    let event = build_typed_event(event_id, timestamp, timestamp_epoch, timestamp_valid, record_id, event_data)?;

    Ok((event, record_id))
}

fn parse_timestamp(time_str: &str) -> (String, f64, bool) {
    if time_str.is_empty() {
        return (String::new(), 0.0, false);
    }

    // Try ISO 8601: 2024-01-01T12:00:00.123Z
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

fn build_typed_event(
    event_id: u32,
    timestamp: String,
    timestamp_epoch: f64,
    timestamp_valid: bool,
    record_id: Option<u64>,
    event_data: HashMap<String, String>,
) -> Result<SysmonEvent, IrError> {
    let get = |key: &str| event_data.get(key).cloned().unwrap_or_default();
    let get_u32 = |key: &str| event_data.get(key).and_then(|v| v.parse().ok()).unwrap_or(0);
    let get_u16 = |key: &str| event_data.get(key).and_then(|v| v.parse().ok()).unwrap_or(0);
    let get_bool = |key: &str| event_data.get(key).map(|v| v.to_lowercase() == "true").unwrap_or(false);

    let raw_data = event_data.clone();

    let (event_type, process_id, process_name, process_path, user, rule_name,
         query_name, query_results, query_status,
         source_ip, source_port, destination_ip, destination_port, protocol, initiated, is_external,
         source_process_id, source_process_name, source_process_path,
         target_process_id, target_process_name, target_process_path,
         start_address, start_module, start_function, is_suspicious,
         target_filename, creation_utc_time) = match event_id {
        22 => {
            // DNS
            let pp = get("Image");
            (SysmonEventType::Dns,
             get_u32("ProcessId"), extract_process_name(&pp), pp, get("User"), get("RuleName"),
             get("QueryName"), get("QueryResults"), get_u32("QueryStatus"),
             String::new(), 0, String::new(), 0, String::new(), false, false,
             0, String::new(), String::new(), 0, String::new(), String::new(),
             String::new(), String::new(), String::new(), false,
             String::new(), String::new())
        }
        3 => {
            // Network connect
            let pp = get("Image");
            let dest_ip = get("DestinationIp");
            let ext = !is_private_ip(&dest_ip);
            (SysmonEventType::NetworkConnect,
             get_u32("ProcessId"), extract_process_name(&pp), pp, get("User"), get("RuleName"),
             String::new(), String::new(), 0,
             get("SourceIp"), get_u16("SourcePort"), dest_ip, get_u16("DestinationPort"), get("Protocol"), get_bool("Initiated"), ext,
             0, String::new(), String::new(), 0, String::new(), String::new(),
             String::new(), String::new(), String::new(), false,
             String::new(), String::new())
        }
        8 => {
            // CreateRemoteThread
            let sp = get("SourceImage");
            let tp = get("TargetImage");
            let sn = extract_process_name(&sp);
            let tn = extract_process_name(&tp);
            let susp = is_suspicious_remote_thread(&sn, &tn);
            (SysmonEventType::CreateRemoteThread,
             0, String::new(), String::new(), get("User"), get("RuleName"),
             String::new(), String::new(), 0,
             String::new(), 0, String::new(), 0, String::new(), false, false,
             get_u32("SourceProcessId"), sn, sp, get_u32("TargetProcessId"), tn, tp,
             get("StartAddress"), get("StartModule"), get("StartFunction"), susp,
             String::new(), String::new())
        }
        11 => {
            // FileCreate
            let pp = get("Image");
            let tf = get("TargetFilename");
            let susp = is_suspicious_path(&tf);
            (SysmonEventType::FileCreate,
             get_u32("ProcessId"), extract_process_name(&pp), pp, get("User"), get("RuleName"),
             String::new(), String::new(), 0,
             String::new(), 0, String::new(), 0, String::new(), false, false,
             0, String::new(), String::new(), 0, String::new(), String::new(),
             String::new(), String::new(), String::new(), susp,
             tf, get("CreationUtcTime"))
        }
        _ => {
            // Unknown event type — just store raw_data
            (SysmonEventType::Unknown,
             0, String::new(), String::new(), String::new(), String::new(),
             String::new(), String::new(), 0,
             String::new(), 0, String::new(), 0, String::new(), false, false,
             0, String::new(), String::new(), 0, String::new(), String::new(),
             String::new(), String::new(), String::new(), false,
             String::new(), String::new())
        }
    };

    Ok(SysmonEvent {
        event_id,
        event_type,
        timestamp,
        timestamp_epoch,
        timestamp_valid,
        record_id,
        raw_data,
        process_id,
        process_name,
        process_path,
        user,
        rule_name,
        query_name,
        query_results,
        query_status,
        source_ip,
        source_port,
        destination_ip,
        destination_port,
        protocol,
        initiated,
        is_external,
        source_process_id,
        source_process_name,
        source_process_path,
        target_process_id,
        target_process_name,
        target_process_path,
        start_address,
        start_module,
        start_function,
        is_suspicious,
        target_filename,
        creation_utc_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dns_event() {
        let xml = r#"<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event">
            <System>
                <EventID>22</EventID>
                <TimeCreated SystemTime="2024-01-15T10:30:00.000Z"/>
                <EventRecordID>12345</EventRecordID>
            </System>
            <EventData>
                <Data Name="Image">C:\Windows\System32\svchost.exe</Data>
                <Data Name="ProcessId">1234</Data>
                <Data Name="QueryName">example.com</Data>
                <Data Name="QueryResults">1.2.3.4</Data>
                <Data Name="User">NT AUTHORITY\SYSTEM</Data>
            </EventData>
        </Event>"#;

        let (event, rid) = parse_event_with_record_id(xml).unwrap();
        assert_eq!(rid, Some(12345));
        assert_eq!(event.event_id, 22);
        assert_eq!(event.event_type, SysmonEventType::Dns);
        assert_eq!(event.process_name, "svchost.exe");
        assert_eq!(event.query_name, "example.com");
    }

    #[test]
    fn test_parse_network_event() {
        let xml = r#"<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event">
            <System>
                <EventID>3</EventID>
                <TimeCreated SystemTime="2024-01-15T10:30:00.000Z"/>
            </System>
            <EventData>
                <Data Name="Image">C:\app\client.exe</Data>
                <Data Name="ProcessId">5678</Data>
                <Data Name="DestinationIp">8.8.8.8</Data>
                <Data Name="DestinationPort">443</Data>
                <Data Name="Protocol">tcp</Data>
                <Data Name="Initiated">true</Data>
            </EventData>
        </Event>"#;

        let event = parse_event(xml).unwrap();
        assert_eq!(event.event_id, 3);
        assert_eq!(event.event_type, SysmonEventType::NetworkConnect);
        assert_eq!(event.destination_ip, "8.8.8.8");
        assert!(event.is_external);
    }

    #[test]
    fn test_parse_remote_thread_event() {
        let xml = r#"<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event">
            <System>
                <EventID>8</EventID>
                <TimeCreated SystemTime="2024-01-15T10:30:00.000Z"/>
            </System>
            <EventData>
                <Data Name="SourceImage">C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe</Data>
                <Data Name="SourceProcessId">100</Data>
                <Data Name="TargetImage">C:\Windows\explorer.exe</Data>
                <Data Name="TargetProcessId">200</Data>
                <Data Name="StartAddress">0x12345678</Data>
            </EventData>
        </Event>"#;

        let event = parse_event(xml).unwrap();
        assert_eq!(event.event_id, 8);
        assert_eq!(event.event_type, SysmonEventType::CreateRemoteThread);
        assert!(event.is_suspicious); // powershell.exe is a suspicious source
    }

    #[test]
    fn test_parse_unknown_event() {
        let xml = r#"<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event">
            <System>
                <EventID>1</EventID>
                <TimeCreated SystemTime="2024-01-15T10:30:00.000Z"/>
            </System>
            <EventData>
                <Data Name="Image">C:\test.exe</Data>
            </EventData>
        </Event>"#;

        let event = parse_event(xml).unwrap();
        assert_eq!(event.event_id, 1);
        assert_eq!(event.event_type, SysmonEventType::Unknown);
    }
}
