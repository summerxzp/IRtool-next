use crate::types::{AutorunItem, SignatureStatus};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
pub struct AutorunsStore {
    inner: Arc<DashMap<u64, AutorunItem>>,
}

impl AutorunsStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_and_put(&self, items: Vec<AutorunItem>) {
        self.inner.clear();
        for item in items {
            self.inner.insert(item.id, item);
        }
    }

    pub fn get_all(&self) -> Vec<AutorunItem> {
        self.inner.iter().map(|e| e.value().clone()).collect()
    }

    pub fn get(&self, id: u64) -> Option<AutorunItem> {
        self.inner.get(&id).map(|e| e.value().clone())
    }

    pub fn remove(&self, id: u64) -> Option<AutorunItem> {
        self.inner.remove(&id).map(|(_, v)| v)
    }

    pub fn update_signature(&self, path: &str, status: SignatureStatus) {
        for mut entry in self.inner.iter_mut() {
            if entry.value().image_path.as_deref() == Some(path) {
                entry.value_mut().signature = status.clone();
            }
        }
    }

    pub fn update_hash(&self, id: u64, md5: String, sha256: String) {
        if let Some(mut entry) = self.inner.get_mut(&id) {
            entry.value_mut().md5 = Some(md5);
            entry.value_mut().sha256 = Some(sha256);
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn query_by_path(&self, exe_path: &str) -> Vec<AutorunItem> {
        let normalized = normalize_path(exe_path);
        self.inner
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .image_path
                    .as_ref()
                    .is_some_and(|p| normalize_path(p) == normalized)
            })
            .map(|entry| entry.value().clone())
            .collect()
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskLevel;

    fn make_item(id: u64, path: &str) -> AutorunItem {
        AutorunItem {
            id,
            category: "Logon".into(),
            entry: "test".into(),
            enabled: true,
            location: String::new(),
            description: String::new(),
            publisher: String::new(),
            image_path: Some(path.into()),
            launch_string: None,
            timestamp: None,
            file_exists: true,
            file_size: None,
            file_version: None,
            service_name: None,
            md5: None,
            sha256: None,
            risk: RiskLevel::Safe,
            risk_reasons: vec![],
            signature: SignatureStatus::NotVerified,
        }
    }

    #[test]
    fn put_and_get_all() {
        let store = AutorunsStore::new();
        store.clear_and_put(vec![make_item(1, "a.exe"), make_item(2, "b.exe")]);
        assert_eq!(store.len(), 2);
        let all = store.get_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn remove_by_id() {
        let store = AutorunsStore::new();
        store.clear_and_put(vec![make_item(1, "a.exe"), make_item(2, "b.exe")]);
        store.remove(1);
        assert_eq!(store.len(), 1);
        assert!(store.get(1).is_none());
    }

    #[test]
    fn update_signature_by_path() {
        let store = AutorunsStore::new();
        store.clear_and_put(vec![make_item(1, "a.exe")]);
        store.update_signature(
            "a.exe",
            SignatureStatus::Valid {
                signer: "Microsoft".into(),
            },
        );
        let item = store.get(1).unwrap();
        assert!(matches!(item.signature, SignatureStatus::Valid { .. }));
    }
}
