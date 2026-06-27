//! 远程 IP → 浏览器进程时序索引。
//!
//! 用于 pcap 域名事件反查浏览器进程：
//! 1. sysmon NetworkConnect / net-monitor 观察到浏览器连接某 IP 时，写入索引
//! 2. pcap 抓到该 IP 的 SNI/DNS 域名时，反查索引判断是否浏览器流量
//! 3. 命中则触发 BrowserMaliciousConnection（domain 来自 pcap，pid 来自索引）

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 远程 IP → 浏览器进程映射条目
#[derive(Debug, Clone)]
pub struct BrowserIpEntry {
    pub pid: u32,
    pub process_name: String,
    pub timestamp: i64, // epoch 毫秒
}

/// IP→浏览器进程时序索引
#[derive(Debug)]
pub struct BrowserIpIndex {
    entries: HashMap<String, BrowserIpEntry>,
    ttl_ms: i64,
    max_entries: usize,
}

impl BrowserIpIndex {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            ttl_ms: 60_000,    // 60s TTL
            max_entries: 1000, // 最多 1000 条
        }
    }

    /// 写入/更新一条 IP→进程映射
    pub fn insert(&mut self, ip: &str, pid: u32, process_name: &str, timestamp_ms: i64) {
        // 容量上限：超限时清理过期条目
        if self.entries.len() >= self.max_entries {
            self.evict_expired(timestamp_ms);
        }
        // 仍超限：跳过（避免无限增长）
        if self.entries.len() >= self.max_entries {
            return;
        }
        self.entries.insert(
            ip.to_string(),
            BrowserIpEntry {
                pid,
                process_name: process_name.to_string(),
                timestamp: timestamp_ms,
            },
        );
    }

    /// 查询 IP 对应的浏览器进程（带 TTL 检查）
    ///
    /// 返回 None 的情况：
    /// - IP 不在索引中
    /// - 条目已过期（timestamp 距 now_ms 超过 ttl_ms）
    pub fn lookup(&self, ip: &str, now_ms: i64) -> Option<&BrowserIpEntry> {
        self.entries.get(ip).filter(|e| now_ms - e.timestamp <= self.ttl_ms)
    }

    /// 清理过期条目
    fn evict_expired(&mut self, now_ms: i64) {
        self.entries.retain(|_, e| now_ms - e.timestamp <= self.ttl_ms);
    }

    /// 当前条目数（测试用）
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for BrowserIpIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// 共享的 IP 索引（Arc<Mutex> 便于跨 task 共享）
pub type SharedBrowserIpIndex = Arc<Mutex<BrowserIpIndex>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_lookup() {
        let mut idx = BrowserIpIndex::new();
        idx.insert("1.2.3.4", 1234, "chrome.exe", 1000);
        let entry = idx.lookup("1.2.3.4", 1000).unwrap();
        assert_eq!(entry.pid, 1234);
        assert_eq!(entry.process_name, "chrome.exe");
    }

    #[test]
    fn lookup_missing_returns_none() {
        let mut idx = BrowserIpIndex::new();
        idx.insert("1.2.3.4", 1234, "chrome.exe", 1000);
        assert!(idx.lookup("9.9.9.9", 1000).is_none());
    }

    #[test]
    fn lookup_expired_returns_none() {
        let mut idx = BrowserIpIndex::new();
        idx.insert("1.2.3.4", 1234, "chrome.exe", 1000);
        // TTL=60s，查询时间 62000ms → 过期
        assert!(idx.lookup("1.2.3.4", 62_001).is_none());
    }

    #[test]
    fn lookup_within_ttl_returns_some() {
        let mut idx = BrowserIpIndex::new();
        idx.insert("1.2.3.4", 1234, "chrome.exe", 1000);
        // 60s 内有效
        assert!(idx.lookup("1.2.3.4", 60_999).is_some());
    }

    #[test]
    fn insert_overwrites() {
        let mut idx = BrowserIpIndex::new();
        idx.insert("1.2.3.4", 1234, "chrome.exe", 1000);
        idx.insert("1.2.3.4", 5678, "msedge.exe", 2000);
        let entry = idx.lookup("1.2.3.4", 2000).unwrap();
        assert_eq!(entry.pid, 5678);
        assert_eq!(entry.process_name, "msedge.exe");
    }

    #[test]
    fn evict_expired_on_capacity() {
        let mut idx = BrowserIpIndex::new();
        idx.max_entries = 3;
        idx.ttl_ms = 1000;
        // 写入 3 条旧数据
        idx.insert("1.1.1.1", 1, "chrome.exe", 0);
        idx.insert("2.2.2.2", 2, "chrome.exe", 0);
        idx.insert("3.3.3.3", 3, "chrome.exe", 0);
        assert_eq!(idx.len(), 3);
        // 第 4 条触发清理（time=2000，旧数据全部过期）
        idx.insert("4.4.4.4", 4, "chrome.exe", 2000);
        assert_eq!(idx.len(), 1);
        assert!(idx.lookup("4.4.4.4", 2000).is_some());
        assert!(idx.lookup("1.1.1.1", 2000).is_none());
    }

    #[test]
    fn insert_skipped_when_full_and_no_expired() {
        let mut idx = BrowserIpIndex::new();
        idx.max_entries = 2;
        idx.insert("1.1.1.1", 1, "chrome.exe", 1000);
        idx.insert("2.2.2.2", 2, "chrome.exe", 1000);
        // 容量满，无过期，第 3 条被跳过
        idx.insert("3.3.3.3", 3, "chrome.exe", 1000);
        assert_eq!(idx.len(), 2);
        assert!(idx.lookup("3.3.3.3", 1000).is_none());
    }
}
