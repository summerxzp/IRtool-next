use crate::process_info::ProcessInfoCache;
use crate::tcp::{enumerate_tcp_v4, enumerate_tcp_v6};
use crate::types::NetConn;
use crate::udp::{enumerate_udp_v4, enumerate_udp_v6};
use irtool_core::IrError;
use tracing::warn;

pub trait NetCollector: Send + Sync {
    fn snapshot(&self) -> Result<Vec<NetConn>, IrError>;
}

pub struct WindowsNetCollector {
    process_cache: ProcessInfoCache,
}

impl WindowsNetCollector {
    pub fn new() -> Self {
        Self {
            process_cache: ProcessInfoCache::new(),
        }
    }

    pub fn process_cache(&self) -> &ProcessInfoCache {
        &self.process_cache
    }

    #[cfg(windows)]
    fn batch_fetch_cmdlines(pids: &[u32]) -> Option<std::collections::HashMap<u32, String>> {
        crate::process_info::batch_query_cmdlines(pids)
    }

    #[cfg(not(windows))]
    fn batch_fetch_cmdlines(_pids: &[u32]) -> Option<std::collections::HashMap<u32, String>> {
        None
    }

    fn enrich(&self, mut conns: Vec<NetConn>) -> Vec<NetConn> {
        // Collect unique PIDs that need cmdline
        let mut pids_needing_cmdline: Vec<u32> = Vec::new();
        for c in &mut conns {
            let info = self.process_cache.get(c.pid);
            c.process_name = Some(info.name);
            c.process_path = info.path.map(|p| p.to_string_lossy().into_owned());
            if info.cmdline.is_none() {
                pids_needing_cmdline.push(c.pid);
            }
            c.process_cmdline = info.cmdline;
        }

        // Synchronously batch-fetch cmdlines via WMI (runs in spawn_blocking already)
        if !pids_needing_cmdline.is_empty() {
            if let Some(cmdline_map) = Self::batch_fetch_cmdlines(&pids_needing_cmdline) {
                for c in &mut conns {
                    if c.process_cmdline.is_none() {
                        if let Some(cmdline) = cmdline_map.get(&c.pid) {
                            c.process_cmdline = Some(cmdline.clone());
                            // Also update cache so next poll doesn't need WMI again
                            self.process_cache.set_cmdline(c.pid, cmdline.clone());
                        }
                    }
                }
            }
        }

        self.process_cache.cleanup_expired();
        conns
    }
}

impl Default for WindowsNetCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NetCollector for WindowsNetCollector {
    fn snapshot(&self) -> Result<Vec<NetConn>, IrError> {
        let mut all = Vec::with_capacity(2048);

        match enumerate_tcp_v4() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("tcp v4 failed: {}", e),
        }
        match enumerate_tcp_v6() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("tcp v6 failed: {}", e),
        }
        match enumerate_udp_v4() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("udp v4 failed: {}", e),
        }
        match enumerate_udp_v6() {
            Ok(v) => all.extend(v),
            Err(e) => warn!("udp v6 failed: {}", e),
        }

        Ok(self.enrich(all))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_enriched_conns() {
        let c = WindowsNetCollector::new();
        let conns = c.snapshot().expect("snapshot failed");
        assert!(!conns.is_empty());
        for conn in &conns {
            assert!(conn.process_name.is_some(), "process name should be enriched");
        }
    }
}
