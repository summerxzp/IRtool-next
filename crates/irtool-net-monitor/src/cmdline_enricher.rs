use crate::types::CmdlineStatus;
use dashmap::DashMap;
use std::time::{Duration, Instant};

const SUCCESS_TTL: Duration = Duration::from_secs(60);
const NEGATIVE_TTL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct CmdlineResult {
    pub cmdline: Option<String>,
    pub status: CmdlineStatus,
    pub cached_at: Instant,
}

pub struct CmdlineEnricher {
    pending_pids: DashMap<u32, Instant>,
    cache: DashMap<u32, CmdlineResult>,
}

impl CmdlineEnricher {
    pub fn new() -> Self {
        Self {
            pending_pids: DashMap::new(),
            cache: DashMap::new(),
        }
    }

    /// Enqueue PIDs that need cmdline enrichment.
    /// Skips PIDs already pending or cached (unless cache is expired).
    pub fn enqueue(&self, pids: &[u32]) {
        let now = Instant::now();
        for &pid in pids {
            if self.pending_pids.contains_key(&pid) {
                continue;
            }
            if let Some(entry) = self.cache.get(&pid) {
                if !is_expired(&entry, now) {
                    continue;
                }
            }
            self.pending_pids.insert(pid, now);
        }
    }

    /// Drain up to `max` pending PIDs for querying.
    pub fn drain_pending(&self, max: usize) -> Vec<u32> {
        let mut result = Vec::with_capacity(max);
        let mut to_remove = Vec::new();

        for entry in self.pending_pids.iter() {
            if result.len() >= max {
                break;
            }
            result.push(*entry.key());
            to_remove.push(*entry.key());
        }

        for pid in &to_remove {
            self.pending_pids.remove(pid);
        }

        result
    }

    /// Store a query result for a PID.
    pub fn update(&self, pid: u32, result: CmdlineResult) {
        self.cache.insert(pid, result);
    }

    /// Look up a cached result for a PID.
    /// Returns None if not cached or expired.
    pub fn get(&self, pid: u32) -> Option<CmdlineResult> {
        let entry = self.cache.get(&pid)?;
        let now = Instant::now();
        if is_expired(&entry, now) {
            return None;
        }
        Some(entry.value().clone())
    }

    /// Remove expired cache entries.
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.cache.retain(|_, result| !is_expired(result, now));
    }
}

impl Default for CmdlineEnricher {
    fn default() -> Self {
        Self::new()
    }
}

fn is_expired(result: &CmdlineResult, now: Instant) -> bool {
    let ttl = match result.status {
        CmdlineStatus::Ready => SUCCESS_TTL,
        _ => NEGATIVE_TTL,
    };
    now.duration_since(result.cached_at) > ttl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_deduplicates() {
        let enricher = CmdlineEnricher::new();
        enricher.enqueue(&[1, 2, 3]);
        enricher.enqueue(&[2, 3, 4]);
        let drained = enricher.drain_pending(100);
        let mut sorted = drained;
        sorted.sort();
        assert_eq!(sorted, vec![1, 2, 3, 4]);
    }

    #[test]
    fn enqueue_skips_cached() {
        let enricher = CmdlineEnricher::new();
        enricher.update(
            1,
            CmdlineResult {
                cmdline: Some("test".into()),
                status: CmdlineStatus::Ready,
                cached_at: Instant::now(),
            },
        );
        enricher.enqueue(&[1, 2]);
        let drained = enricher.drain_pending(100);
        assert_eq!(drained, vec![2]);
    }

    #[test]
    fn get_returns_cached() {
        let enricher = CmdlineEnricher::new();
        enricher.update(
            42,
            CmdlineResult {
                cmdline: Some("cmd".into()),
                status: CmdlineStatus::Ready,
                cached_at: Instant::now(),
            },
        );
        let result = enricher.get(42).unwrap();
        assert_eq!(result.cmdline.as_deref(), Some("cmd"));
        assert_eq!(result.status, CmdlineStatus::Ready);
    }

    #[test]
    fn get_returns_none_for_missing() {
        let enricher = CmdlineEnricher::new();
        assert!(enricher.get(999).is_none());
    }

    #[test]
    fn drain_pending_respects_max() {
        let enricher = CmdlineEnricher::new();
        enricher.enqueue(&[1, 2, 3, 4, 5]);
        let drained = enricher.drain_pending(3);
        assert_eq!(drained.len(), 3);
        // Remaining PIDs still available
        let remaining = enricher.drain_pending(10);
        assert_eq!(remaining.len(), 2);
    }
}
