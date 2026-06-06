use crate::types::{now_epoch_secs, NetConn, NetConnKey};
use dashmap::DashMap;
use std::sync::Arc;

const MAX_HISTORY_RECORDS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionPolicy {
    None,
    Seconds(u64),
    Forever,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::Seconds(600)
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryStore {
    inner: Arc<DashMap<NetConnKey, NetConn>>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge(&self, current_snapshot: Vec<NetConn>, retention: RetentionPolicy) -> Vec<NetConn> {
        let now = now_epoch_secs();

        let mut current_keys = Vec::with_capacity(current_snapshot.len());
        for mut conn in current_snapshot {
            let key = conn.key();
            current_keys.push(key.clone());
            if let Some(mut existing) = self.inner.get_mut(&key) {
                existing.last_seen = now;
                existing.state = conn.state;
                existing.is_current = true;
                existing.process_name = conn.process_name.clone();
                existing.process_path = conn.process_path.clone();
            } else {
                conn.first_seen = now;
                conn.last_seen = now;
                conn.is_current = true;
                self.inner.insert(key, conn);
            }
        }

        let current_set: std::collections::HashSet<_> = current_keys.into_iter().collect();
        self.inner.iter_mut().for_each(|mut e| {
            let key = e.key().clone();
            if !current_set.contains(&key) && e.is_current {
                e.is_current = false;
            }
        });

        match retention {
            RetentionPolicy::None => {
                self.inner.retain(|_, v| v.is_current);
            }
            RetentionPolicy::Seconds(secs) => {
                self.inner
                    .retain(|_, v| v.is_current || now.saturating_sub(v.last_seen) <= secs);
            }
            RetentionPolicy::Forever => {}
        }

        // Trim oldest entries if exceeding max capacity
        if self.inner.len() > MAX_HISTORY_RECORDS {
            let excess = self.inner.len() - MAX_HISTORY_RECORDS;
            let keys_to_remove: Vec<_> = self.inner.iter().take(excess).map(|e| e.key().clone()).collect();
            for key in keys_to_remove {
                self.inner.remove(&key);
            }
        }

        self.inner.iter().map(|e| e.value().clone()).collect()
    }

    pub fn clear_history(&self) {
        self.inner.retain(|_, v| v.is_current);
    }

    pub fn clear_all(&self) {
        self.inner.clear();
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConnState, Family, NetEndpoint, Proto};

    fn mk_conn(pid: u32, port: u16, state: ConnState) -> NetConn {
        NetConn {
            proto: Proto::Tcp,
            family: Family::V4,
            local: NetEndpoint {
                addr: "127.0.0.1".into(),
                port,
            },
            remote: NetEndpoint {
                addr: "1.1.1.1".into(),
                port: 443,
            },
            state,
            pid,
            process_name: Some("test.exe".into()),
            process_path: None,
            process_cmdline: None,
            first_seen: 0,
            last_seen: 0,
            is_current: true,
        }
    }

    #[test]
    fn first_snapshot_marks_all_current() {
        let store = HistoryStore::new();
        let snap = vec![
            mk_conn(100, 8080, ConnState::Established),
            mk_conn(101, 9090, ConnState::Listen),
        ];
        let merged = store.merge(snap, RetentionPolicy::Forever);
        assert_eq!(merged.len(), 2);
        for c in &merged {
            assert!(c.is_current);
        }
    }

    #[test]
    fn second_snapshot_drops_missing_marks_historical() {
        let store = HistoryStore::new();
        store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
        let merged = store.merge(vec![], RetentionPolicy::Forever);
        assert_eq!(merged.len(), 1);
        assert!(!merged[0].is_current);
    }

    #[test]
    fn retention_none_drops_historical() {
        let store = HistoryStore::new();
        store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        let merged = store.merge(vec![], RetentionPolicy::None);
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn first_seen_preserved_across_merges() {
        let store = HistoryStore::new();
        let merged1 = store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        let first_seen = merged1[0].first_seen;
        std::thread::sleep(std::time::Duration::from_secs(1));
        let merged2 = store.merge(
            vec![mk_conn(100, 8080, ConnState::Established)],
            RetentionPolicy::Forever,
        );
        assert_eq!(merged2[0].first_seen, first_seen);
        assert!(merged2[0].last_seen >= first_seen);
    }
}
