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

#[cfg(windows)]
use std::collections::HashMap;

/// Batch query WMI for CommandLine of all given PIDs in a single WMI connection.
/// Returns a map of PID -> CommandLine.
#[cfg(windows)]
pub(crate) fn batch_query_cmdlines(pids: &[u32]) -> Option<HashMap<u32, String>> {
    use serde::Deserialize;
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct Win32Process {
        ProcessId: u32,
        CommandLine: Option<String>,
    }

    let wmi = WMIConnection::new().ok()?;
    // Single query for all processes, then filter by PID
    let results: Vec<Win32Process> = wmi
        .raw_query("SELECT ProcessId, CommandLine FROM Win32_Process")
        .ok()?;

    let pid_set: std::collections::HashSet<u32> = pids.iter().copied().collect();
    let mut map = HashMap::new();
    for proc in results {
        if pid_set.contains(&proc.ProcessId) {
            if let Some(cmdline) = proc.CommandLine {
                map.insert(proc.ProcessId, cmdline);
            }
        }
    }
    Some(map)
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
        let cmdlines = batch_query_cmdlines(&[pid]);
        assert!(cmdlines.is_some(), "batch WMI query should succeed");
        let cmdline = cmdlines.unwrap().get(&pid).cloned();
        assert!(cmdline.is_some(), "cmdline should be found for current process");
        assert!(!cmdline.as_ref().unwrap().is_empty(), "cmdline should not be empty");
        eprintln!("Current process cmdline: {}", cmdline.unwrap());
    }
}
