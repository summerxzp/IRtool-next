use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::core::PWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub name: String,
    pub path: Option<PathBuf>,
    pub cmdline: Option<String>,
    pub cached_at: Instant,
}

const CACHE_TTL: Duration = Duration::from_secs(5);
const TOMBSTONE_DEAD: &str = "[已结束]";
const TOMBSTONE_DENIED: &str = "[权限不足]";

#[derive(Debug, Default, Clone)]
pub struct ProcessInfoCache {
    inner: Arc<DashMap<u32, ProcessInfo>>,
}

impl ProcessInfoCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, pid: u32) -> ProcessInfo {
        use dashmap::mapref::entry::Entry;

        match self.inner.entry(pid) {
            Entry::Occupied(mut e) => {
                if e.get().cached_at.elapsed() < CACHE_TTL {
                    return e.get().clone();
                }
                // TTL expired - update in place, but preserve cmdline if already fetched
                let old_cmdline = e.get().cmdline.clone();
                let mut info = lookup_process(pid);
                if info.cmdline.is_none() && old_cmdline.is_some() {
                    info.cmdline = old_cmdline;
                }
                e.insert(info.clone());
                info
            }
            Entry::Vacant(e) => {
                let info = lookup_process(pid);
                e.insert(info.clone());
                info
            }
        }
    }

    pub fn invalidate(&self, pid: u32) {
        self.inner.remove(&pid);
    }

    pub fn clear(&self) {
        self.inner.clear();
    }

    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.inner
            .retain(|_, info| now.duration_since(info.cached_at) < CACHE_TTL * 4);
    }

    /// Update cmdline for a cached entry (called after batch WMI fetch).
    pub fn set_cmdline(&self, pid: u32, cmdline: String) {
        if let Some(mut entry) = self.inner.get_mut(&pid) {
            entry.cmdline = Some(cmdline);
        }
    }
}

use std::collections::HashMap;

/// Result of targeted WMI query for command lines.
/// - `cmdlines`: successfully retrieved command lines (PID -> cmdline)
/// - `exited_pids`: PIDs not found in WMI (process exited)
/// - `failed_pids`: PIDs whose query failed or timed out
/// - `query_failed`: true if the entire WMI query failed (connection error)
pub struct TargetedQueryResult {
    pub cmdlines: HashMap<u32, String>,
    pub exited_pids: Vec<u32>,
    pub failed_pids: Vec<u32>,
    /// PIDs found in WMI but CommandLine is None (protected processes like AV)
    pub no_cmdline_pids: Vec<u32>,
    pub query_failed: bool,
}

/// Targeted WMI query for CommandLine of specific PIDs.
/// Builds a WHERE clause to avoid scanning all processes.
/// Chunks PIDs into groups of 50 with a 1500ms timeout per chunk.
#[cfg(windows)]
pub fn targeted_query_cmdlines(pids: &[u32]) -> Option<TargetedQueryResult> {
    use serde::Deserialize;
    use std::sync::mpsc;
    use std::time::Duration;
    use tracing::{debug, info};
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct Win32Process {
        ProcessId: u32,
        CommandLine: Option<String>,
    }

    info!("targeted_query_cmdlines: starting query for {:?} PIDs", pids.len());

    if pids.is_empty() {
        return Some(TargetedQueryResult {
            cmdlines: HashMap::new(),
            exited_pids: Vec::new(),
            failed_pids: Vec::new(),
            no_cmdline_pids: Vec::new(),
            query_failed: false,
        });
    }

    let mut cmdlines = HashMap::new();
    let mut exited_pids = Vec::new();
    let mut failed_pids = Vec::new();
    let mut no_cmdline_pids = Vec::new();
    let mut any_chunk_succeeded = false;
    let chunk_size = 50;

    for (chunk_idx, chunk) in pids.chunks(chunk_size).enumerate() {
        let conditions: Vec<String> = chunk.iter().map(|p| format!("ProcessId = {}", p)).collect();
        let where_clause = conditions.join(" OR ");
        let query = format!(
            "SELECT ProcessId, CommandLine FROM Win32_Process WHERE {}",
            where_clause
        );

        debug!("targeted_query_cmdlines: chunk {} query started", chunk_idx);

        // Run WMI query on a separate thread with timeout
        let (tx, rx) = mpsc::channel();
        let query_owned = query.clone();
        std::thread::spawn(move || {
            let result = (|| -> Option<Vec<Win32Process>> {
                let wmi = WMIConnection::new().ok()?;
                wmi.raw_query(&query_owned).ok()
            })();
            let _ = tx.send(result);
        });

        match rx.recv_timeout(Duration::from_millis(1500)) {
            Ok(Some(results)) => {
                any_chunk_succeeded = true;
                debug!(
                    "targeted_query_cmdlines: chunk {} succeeded, {} results",
                    chunk_idx,
                    results.len()
                );
                // Track which PIDs in this chunk were found in WMI
                let mut found_pids = std::collections::HashSet::new();
                for proc in results {
                    found_pids.insert(proc.ProcessId);
                    if let Some(cmdline) = proc.CommandLine {
                        cmdlines.insert(proc.ProcessId, cmdline);
                    } else {
                        no_cmdline_pids.push(proc.ProcessId);
                    }
                }
                // PIDs in chunk but not in WMI results: process exited
                for &pid in chunk {
                    if !found_pids.contains(&pid) {
                        exited_pids.push(pid);
                    }
                }
            }
            Ok(None) => {
                // WMI connection/query failed for this chunk
                debug!("targeted_query_cmdlines: chunk {} WMI connection failed", chunk_idx);
                failed_pids.extend(chunk.iter().copied());
            }
            Err(_) => {
                // Timeout - query took too long
                debug!("targeted_query_cmdlines: chunk {} timed out", chunk_idx);
                failed_pids.extend(chunk.iter().copied());
            }
        }
    }

    // If all chunks failed, consider the entire query failed
    let query_failed = !any_chunk_succeeded && !failed_pids.is_empty();

    info!(
        "targeted_query_cmdlines: done, cmdlines={}, exited={}, failed={}, no_cmdline={}, query_failed={}",
        cmdlines.len(),
        exited_pids.len(),
        failed_pids.len(),
        no_cmdline_pids.len(),
        query_failed
    );

    Some(TargetedQueryResult {
        cmdlines,
        exited_pids,
        failed_pids,
        no_cmdline_pids,
        query_failed,
    })
}

#[cfg(not(windows))]
pub fn targeted_query_cmdlines(_pids: &[u32]) -> Option<TargetedQueryResult> {
    None
}

#[cfg(windows)]
fn lookup_process(pid: u32) -> ProcessInfo {
    if pid == 0 {
        return ProcessInfo {
            name: "System Idle".into(),
            path: None,
            cmdline: None,
            cached_at: Instant::now(),
        };
    }
    if pid == 4 {
        return ProcessInfo {
            name: "System".into(),
            path: None,
            cmdline: None,
            cached_at: Instant::now(),
        };
    }

    unsafe {
        let handle: HANDLE = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(e) => {
                let name = if e.code().0 as u32 == 0x80070005 {
                    TOMBSTONE_DENIED.to_string()
                } else {
                    TOMBSTONE_DEAD.to_string()
                };
                return ProcessInfo {
                    name,
                    path: None,
                    cmdline: None,
                    cached_at: Instant::now(),
                };
            }
        };

        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let path_str =
            if QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut size).is_ok() {
                String::from_utf16_lossy(&buf[..size as usize])
            } else {
                String::new()
            };

        let _ = CloseHandle(handle);

        let path = if !path_str.is_empty() {
            Some(PathBuf::from(&path_str))
        } else {
            None
        };

        let name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("PID {}", pid));

        // cmdline is filled asynchronously via fetch_cmdlines()
        ProcessInfo {
            name,
            path,
            cmdline: None,
            cached_at: Instant::now(),
        }
    }
}

#[cfg(not(windows))]
fn lookup_process(_pid: u32) -> ProcessInfo {
    ProcessInfo {
        name: "[unsupported]".into(),
        path: None,
        cmdline: None,
        cached_at: Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_returns_same_within_ttl() {
        let cache = ProcessInfoCache::new();
        let info1 = cache.get(std::process::id());
        let info2 = cache.get(std::process::id());
        assert_eq!(info1.name, info2.name);
        assert_eq!(info1.cached_at, info2.cached_at);
    }

    #[test]
    fn invalidate_removes_entry() {
        let cache = ProcessInfoCache::new();
        let pid = std::process::id();
        let _ = cache.get(pid);
        cache.invalidate(pid);
        assert!(!cache.inner.contains_key(&pid));
    }

    #[cfg(windows)]
    #[test]
    fn current_process_lookup_has_name() {
        let info = lookup_process(std::process::id());
        assert!(!info.name.is_empty());
        assert!(!info.name.starts_with('['));
    }

    #[cfg(windows)]
    #[test]
    fn current_process_has_cmdline() {
        let cache = ProcessInfoCache::new();
        let pid = std::process::id();
        // First lookup returns cmdline=None
        let info = lookup_process(pid);
        assert!(info.cmdline.is_none(), "cmdline should be None on first lookup");
        // Batch WMI query fills it
        cache.get(pid); // insert into cache
        let result = targeted_query_cmdlines(&[pid]);
        assert!(result.is_some(), "batch WMI query should succeed");
        let query_result = result.unwrap();
        assert!(!query_result.query_failed, "query should not fail");
        let cmdline = query_result.cmdlines.get(&pid).cloned();
        assert!(cmdline.is_some(), "cmdline should be found for current process");
        assert!(!cmdline.as_ref().unwrap().is_empty(), "cmdline should not be empty");
        eprintln!("Current process cmdline: {}", cmdline.unwrap());
    }
}
