use crate::models::*;
use crate::parser;
use irtool_core::IrError;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, warn};
use tokio::sync::mpsc;

const SYSMON_CHANNEL: &str = "Microsoft-Windows-Sysmon/Operational";
const BATCH_SIZE: usize = 64;

pub struct SysmonReader {
    last_record_id: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
}

impl Default for SysmonReader {
    fn default() -> Self {
        Self::new()
    }
}

impl SysmonReader {
    pub fn new() -> Self {
        Self {
            last_record_id: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if the Sysmon event channel is available.
    #[cfg(windows)]
    pub fn is_channel_available(&self) -> bool {
        use windows::core::HSTRING;
        use windows::Win32::System::EventLog::{EvtQuery, EvtQueryChannelPath};

        let channel = HSTRING::from(SYSMON_CHANNEL);
        let query = HSTRING::from("*");
        unsafe {
            EvtQuery(None, &channel, &query, EvtQueryChannelPath.0).is_ok()
        }
    }

    #[cfg(not(windows))]
    pub fn is_channel_available(&self) -> bool {
        false
    }

    /// Read existing (historical) events from the channel.
    #[cfg(windows)]
    pub fn get_existing_events(
        &self,
        limit: u32,
        enabled_event_ids: &[u32],
    ) -> Result<Vec<SysmonEvent>, IrError> {
        use windows::core::HSTRING;
        use windows::Win32::System::EventLog::{EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection};

        let xpath = build_xpath_query(enabled_event_ids, None);
        let channel = HSTRING::from(SYSMON_CHANNEL);
        let query_str = HSTRING::from(&xpath);

        let result_set = unsafe {
            EvtQuery(
                None,
                &channel,
                &query_str,
                EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
            )
            .map_err(|e| IrError::Internal(format!("EvtQuery failed: {}", e)))?
        };

        let events = read_events_from_result_set(&result_set, limit as usize)?;
        // Reverse to chronological order since we queried in reverse direction
        let mut events = events;
        events.reverse();
        Ok(events)
    }

    #[cfg(not(windows))]
    pub fn get_existing_events(
        &self,
        _limit: u32,
        _enabled_event_ids: &[u32],
    ) -> Result<Vec<SysmonEvent>, IrError> {
        Err(IrError::FeatureDisabled("sysmon requires Windows".into()))
    }

    /// Initialize the last record ID to skip existing events.
    #[cfg(windows)]
    pub fn init_last_record_id(&self, enabled_event_ids: &[u32]) -> Result<(), IrError> {
        use windows::core::HSTRING;
        use windows::Win32::System::EventLog::{EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection};

        let xpath = build_xpath_query(enabled_event_ids, None);
        let channel = HSTRING::from(SYSMON_CHANNEL);
        let query_str = HSTRING::from(&xpath);

        let result_set = unsafe {
            EvtQuery(
                None,
                &channel,
                &query_str,
                EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
            )
            .map_err(|e| IrError::Internal(format!("EvtQuery failed: {}", e)))?
        };

        // Read just the first (most recent) event to get its RecordID
        let events = read_events_from_result_set(&result_set, 1)?;
        if let Some(event) = events.first() {
            if let Some(rid) = event.record_id {
                self.last_record_id.store(rid, Ordering::SeqCst);
                info!("Initialized last_record_id to {}", rid);
            }
        } else {
            info!("No existing Sysmon events found, last_record_id stays at 0");
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn init_last_record_id(&self, _enabled_event_ids: &[u32]) -> Result<(), IrError> {
        Ok(())
    }

    /// Poll for new events since last_record_id.
    #[cfg(windows)]
    pub fn poll_new_events(
        &self,
        enabled_event_ids: &[u32],
    ) -> Result<Vec<SysmonEvent>, IrError> {
        use windows::core::HSTRING;
        use windows::Win32::System::EventLog::{EvtQuery, EvtQueryChannelPath, EvtQueryForwardDirection};

        let last_id = self.last_record_id.load(Ordering::SeqCst);
        let xpath = build_xpath_query(enabled_event_ids, if last_id > 0 { Some(last_id) } else { None });
        let channel = HSTRING::from(SYSMON_CHANNEL);
        let query_str = HSTRING::from(&xpath);

        let result_set = unsafe {
            EvtQuery(
                None,
                &channel,
                &query_str,
                EvtQueryChannelPath.0 | EvtQueryForwardDirection.0,
            )
            .map_err(|e| IrError::Internal(format!("EvtQuery failed: {}", e)))?
        };

        let events = read_events_from_result_set(&result_set, usize::MAX)?;

        // Update last_record_id to the highest record ID we've seen
        if let Some(max_id) = events.iter().filter_map(|e| e.record_id).max() {
            self.last_record_id.store(max_id, Ordering::SeqCst);
        }

        Ok(events)
    }

    #[cfg(not(windows))]
    pub fn poll_new_events(
        &self,
        _enabled_event_ids: &[u32],
    ) -> Result<Vec<SysmonEvent>, IrError> {
        Err(IrError::FeatureDisabled("sysmon requires Windows".into()))
    }

    /// Start a background polling loop.
    pub fn start_polling(
        &self,
        enabled_event_ids: Vec<u32>,
        poll_interval_ms: u64,
        tx: mpsc::UnboundedSender<SysmonEvent>,
    ) {
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let last_record_id = self.last_record_id.clone();
        std::thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                let reader = SysmonReader {
                    last_record_id: last_record_id.clone(),
                    running: running.clone(),
                };
                match reader.poll_new_events(&enabled_event_ids) {
                    Ok(events) => {
                        for event in events {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => warn!("Poll error: {}", e),
                }
                std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
            }
        });
    }

    pub fn stop_polling(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_polling(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Read events from an EvtQuery result set handle, up to `max_events`.
#[cfg(windows)]
fn read_events_from_result_set(
    result_set: &windows::Win32::System::EventLog::EVT_HANDLE,
    max_events: usize,
) -> Result<Vec<SysmonEvent>, IrError> {
    use windows::Win32::System::EventLog::EvtNext;

    let mut all_events = Vec::new();
    let mut event_handles: Vec<isize> = vec![0; BATCH_SIZE];

    loop {
        let mut returned: u32 = 0;
        let result = unsafe {
            EvtNext(*result_set, &mut event_handles, 0, 0, &mut returned)
        };

        if returned == 0 {
            break;
        }

        for &raw_handle in event_handles.iter().take(returned as usize) {
            if all_events.len() >= max_events {
                return Ok(all_events);
            }

            let handle = windows::Win32::System::EventLog::EVT_HANDLE(raw_handle);
            if handle.is_invalid() {
                continue;
            }

            match render_event_xml(&handle) {
                Ok(xml) => {
                    match parser::parse_event_with_record_id(&xml) {
                        Ok((event, _record_id)) => {
                            all_events.push(event);
                        }
                        Err(e) => {
                            warn!("Failed to parse event XML: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to render event: {}", e);
                }
            }

            // Close the event handle explicitly
            unsafe {
                let _ = windows::Win32::System::EventLog::EvtClose(handle);
            }
        }

        if result.is_err() {
            // No more events available
            break;
        }
    }

    Ok(all_events)
}

/// Render a single event handle to its XML representation.
#[cfg(windows)]
fn render_event_xml(
    event_handle: &windows::Win32::System::EventLog::EVT_HANDLE,
) -> Result<String, IrError> {
    use windows::Win32::System::EventLog::{EvtRender, EvtRenderEventXml};

    let mut buffer_used: u32 = 0;
    let mut property_count: u32 = 0;

    // First call: determine required buffer size
    let _probe_result = unsafe {
        EvtRender(
            None,
            *event_handle,
            EvtRenderEventXml.0,
            0,
            None,
            &mut buffer_used,
            &mut property_count,
        )
    };

    // Expected to fail with ERROR_INSUFFICIENT_BUFFER
    if buffer_used == 0 {
        return Err(IrError::Internal("EvtRender returned zero buffer size".into()));
    }

    // Allocate buffer (buffer_used is in bytes; we need u16 units for the wide string)
    let buffer_len = (buffer_used / 2) as usize + 1;
    let mut buffer: Vec<u16> = vec![0u16; buffer_len];
    let mut new_buffer_used: u32 = 0;
    let mut new_property_count: u32 = 0;

    unsafe {
        EvtRender(
            None,
            *event_handle,
            EvtRenderEventXml.0,
            buffer_used,
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            &mut new_buffer_used,
            &mut new_property_count,
        )
        .map_err(|e| IrError::Internal(format!("EvtRender failed: {}", e)))?;
    }

    // Convert from wide string to Rust String
    let xml = String::from_utf16_lossy(
        &buffer[..(new_buffer_used / 2) as usize],
    );
    Ok(xml.trim_end_matches('\0').to_string())
}

fn build_xpath_query(event_ids: &[u32], after_record_id: Option<u64>) -> String {
    if event_ids.is_empty() {
        return "*".to_string();
    }
    let ids_expr = event_ids
        .iter()
        .map(|id| format!("EventID={}", id))
        .collect::<Vec<_>>()
        .join(" or ");
    match after_record_id {
        Some(rid) => format!("*[System[({}) and (EventRecordID > {})]]", ids_expr, rid),
        None => format!("*[System[{}]]", ids_expr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_xpath_query_with_ids() {
        let query = build_xpath_query(&[22, 3], None);
        assert!(query.contains("EventID=22 or EventID=3"));
        assert!(!query.contains("EventRecordID"));
    }

    #[test]
    fn test_build_xpath_query_with_record_id() {
        let query = build_xpath_query(&[22, 3], Some(100));
        assert!(query.contains("EventID=22 or EventID=3"));
        assert!(query.contains("EventRecordID > 100"));
    }

    #[test]
    fn test_build_xpath_query_empty_ids() {
        let query = build_xpath_query(&[], None);
        assert_eq!(query, "*");
    }

    #[test]
    fn test_sysmon_reader_new() {
        let reader = SysmonReader::new();
        assert!(!reader.is_polling());
        assert_eq!(reader.last_record_id.load(Ordering::SeqCst), 0);
    }
}
