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
        if let Some(entry) = self.inner.get(&pid) {
            if entry.cached_at.elapsed() < CACHE_TTL {
                return entry.clone();
            }
        }
        let info = lookup_process(pid);
        self.inner.insert(pid, info.clone());
        info
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
}

#[cfg(windows)]
fn lookup_process(pid: u32) -> ProcessInfo {
    if pid == 0 {
        return ProcessInfo {
            name: "System Idle".into(),
            path: None,
            cached_at: Instant::now(),
        };
    }
    if pid == 4 {
        return ProcessInfo {
            name: "System".into(),
            path: None,
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

        ProcessInfo {
            name,
            path,
            cached_at: Instant::now(),
        }
    }
}

#[cfg(not(windows))]
fn lookup_process(_pid: u32) -> ProcessInfo {
    ProcessInfo {
        name: "[unsupported]".into(),
        path: None,
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
}
