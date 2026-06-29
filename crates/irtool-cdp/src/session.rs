//! CDP 会话管理 Attach/Detach（P2.3 填充实现）。
//!
//! ## 设计要点
//!
//! - `Target.attachToTarget` 时传 `flatten: true`，使用扁平化 session id
//! - 只 Attach page/service_worker，忽略 iframe/worker
//! - 同时 Attach 的 target 数量上限（默认 20），超出只记录不 Attach

use std::collections::HashMap;

/// CDP Session ID（`Target.attachToTarget` 返回的 sessionId）
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

/// CDP Target 信息（`Target.getTargets` 返回的条目子集）
#[derive(Debug, Clone)]
pub struct TargetInfo {
    /// Target ID
    pub target_id: String,
    /// Target 类型（"page" / "service_worker" / "iframe" / "worker" 等）
    pub target_type: String,
    /// Target URL
    pub url: String,
    /// 标题
    pub title: String,
}

/// 会话管理器：维护 sessionId → TargetInfo 映射
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<SessionId, TargetInfo>,
    /// 同时 Attach 的 target 数量上限
    limit: usize,
}

/// 默认会话上限
pub const DEFAULT_SESSION_LIMIT: usize = 20;

impl SessionManager {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_SESSION_LIMIT)
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            limit,
        }
    }

    /// 当前已 Attach 的 session 数量
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// 会话上限
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// 是否已达上限
    pub fn is_full(&self) -> bool {
        self.sessions.len() >= self.limit
    }

    /// 注册一个新会话（不检查上限，调用方应在 attach 前检查 `is_full()`）
    pub fn insert(&mut self, session_id: SessionId, target: TargetInfo) {
        self.sessions.insert(session_id, target);
    }

    /// 移除并返回一个会话
    pub fn remove(&mut self, session_id: &SessionId) -> Option<TargetInfo> {
        self.sessions.remove(session_id)
    }

    /// 查询会话对应的 TargetInfo
    pub fn get(&self, session_id: &SessionId) -> Option<&TargetInfo> {
        self.sessions.get(session_id)
    }

    /// 判断 target 类型是否应该 Attach（只 page/service_worker）
    pub fn should_attach(target_type: &str) -> bool {
        target_type == "page" || target_type == "service_worker"
    }

    /// 清空所有会话
    pub fn clear(&mut self) {
        self.sessions.clear();
    }

    /// 迭代所有会话
    pub fn iter(&self) -> impl Iterator<Item = (&SessionId, &TargetInfo)> {
        self.sessions.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_target(target_type: &str) -> TargetInfo {
        TargetInfo {
            target_id: format!("target-{}", target_type),
            target_type: target_type.to_string(),
            url: "about:blank".to_string(),
            title: "Test".to_string(),
        }
    }

    #[test]
    fn should_attach_page_and_service_worker() {
        assert!(SessionManager::should_attach("page"));
        assert!(SessionManager::should_attach("service_worker"));
    }

    #[test]
    fn should_not_attach_iframe_or_worker() {
        assert!(!SessionManager::should_attach("iframe"));
        assert!(!SessionManager::should_attach("worker"));
        assert!(!SessionManager::should_attach("shared_worker"));
        assert!(!SessionManager::should_attach("browser"));
        assert!(!SessionManager::should_attach(""));
    }

    #[test]
    fn session_manager_insert_remove() {
        let mut sm = SessionManager::new();
        assert!(sm.is_empty());
        assert_eq!(sm.limit(), DEFAULT_SESSION_LIMIT);

        let sid = SessionId("sess-1".to_string());
        sm.insert(sid.clone(), make_target("page"));
        assert_eq!(sm.len(), 1);
        assert!(sm.get(&sid).is_some());

        let removed = sm.remove(&sid);
        assert!(removed.is_some());
        assert!(sm.is_empty());
    }

    #[test]
    fn session_manager_is_full() {
        let mut sm = SessionManager::with_limit(2);
        assert!(!sm.is_full());

        sm.insert(SessionId("s1".to_string()), make_target("page"));
        assert!(!sm.is_full());

        sm.insert(SessionId("s2".to_string()), make_target("page"));
        assert!(sm.is_full());
    }
}
