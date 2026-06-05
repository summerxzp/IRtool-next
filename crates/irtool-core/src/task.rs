use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

pub type TaskId = u64;

const STALE_THRESHOLD_SECS: u64 = 3600; // 1 hour

#[derive(Default)]
pub struct TaskRegistry {
    next_id: AtomicU64,
    tokens: DashMap<TaskId, (CancellationToken, Instant)>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self) -> (TaskId, CancellationToken) {
        // Periodically clean up stale entries
        self.cleanup_stale();

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let token = CancellationToken::new();
        self.tokens.insert(id, (token.clone(), Instant::now()));
        (id, token)
    }

    pub fn cancel(&self, id: TaskId) -> bool {
        if let Some((_, (token, _))) = self.tokens.remove(&id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn finish(&self, id: TaskId) {
        self.tokens.remove(&id);
    }

    pub fn is_active(&self, id: TaskId) -> bool {
        self.tokens.contains_key(&id)
    }

    fn cleanup_stale(&self) {
        let now = Instant::now();
        self.tokens.retain(|_, (_, registered)| {
            now.duration_since(*registered).as_secs() < STALE_THRESHOLD_SECS
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_propagates_to_token() {
        let reg = TaskRegistry::new();
        let (id, token) = reg.register();
        assert!(!token.is_cancelled());
        let cancelled = reg.cancel(id);
        assert!(cancelled);
        assert!(token.is_cancelled());
    }

    #[test]
    fn finish_removes_without_cancel() {
        let reg = TaskRegistry::new();
        let (id, token) = reg.register();
        reg.finish(id);
        assert!(!reg.is_active(id));
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_unknown_returns_false() {
        let reg = TaskRegistry::new();
        assert!(!reg.cancel(999));
    }
}
