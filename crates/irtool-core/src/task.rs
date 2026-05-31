use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

pub type TaskId = u64;

#[derive(Default)]
pub struct TaskRegistry {
    next_id: AtomicU64,
    tokens: DashMap<TaskId, CancellationToken>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self) -> (TaskId, CancellationToken) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let token = CancellationToken::new();
        self.tokens.insert(id, token.clone());
        (id, token)
    }

    pub fn cancel(&self, id: TaskId) -> bool {
        if let Some((_, token)) = self.tokens.remove(&id) {
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
