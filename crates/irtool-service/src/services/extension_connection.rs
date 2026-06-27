//! Helper Extension 连接状态追踪
//!
//! 通过 AtomicI64 无锁存储最后心跳/事件时间戳，
//! 供前端轮询判断扩展是否在线。

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// 扩展心跳间隔（与 service_worker.js 的 HEARTBEAT_INTERVAL_MS 一致）
const HEARTBEAT_INTERVAL_MS: i64 = 30_000;

/// 连接超时阈值（心跳间隔的 2 倍，容错）
const CONNECTION_TIMEOUT_MS: i64 = HEARTBEAT_INTERVAL_MS * 2;

/// Helper Extension 连接状态（无锁，高频更新）
pub struct ExtensionConnectionState {
    last_heartbeat_ms: AtomicI64,
    last_event_ms: AtomicI64,
    /// 上一次 status() 调用时的 connected 状态，用于检测状态转换并打日志
    last_reported_connected: AtomicBool,
}

impl Default for ExtensionConnectionState {
    fn default() -> Self {
        Self {
            last_heartbeat_ms: AtomicI64::new(0),
            last_event_ms: AtomicI64::new(0),
            last_reported_connected: AtomicBool::new(false),
        }
    }
}

impl ExtensionConnectionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录收到心跳
    pub fn record_heartbeat(&self, timestamp_ms: i64) {
        self.last_heartbeat_ms.store(timestamp_ms, Ordering::Relaxed);
    }

    /// 记录收到事件（network_batch / extension_list）
    pub fn record_event(&self, timestamp_ms: i64) {
        self.last_event_ms.store(timestamp_ms, Ordering::Relaxed);
    }

    /// 查询连接状态
    ///
    /// connected = 最后心跳在 CONNECTION_TIMEOUT_MS 内
    ///
    /// 当连接状态发生转换（connected → disconnected 或反之）时打日志，
    /// 便于排查连接问题。
    pub fn status(&self, now_ms: i64) -> ExtensionConnectionStatus {
        let last_hb = self.last_heartbeat_ms.load(Ordering::Relaxed);
        let last_evt = self.last_event_ms.load(Ordering::Relaxed);
        let connected = last_hb > 0 && (now_ms - last_hb) < CONNECTION_TIMEOUT_MS;

        // 检测状态转换并打日志
        let prev_connected = self.last_reported_connected.swap(connected, Ordering::Relaxed);
        if prev_connected != connected {
            if connected {
                tracing::info!(
                    last_heartbeat_ms = last_hb,
                    last_event_ms = last_evt,
                    "extension connection state: disconnected → connected"
                );
            } else {
                tracing::warn!(
                    last_heartbeat_ms = last_hb,
                    last_event_ms = last_evt,
                    now_ms = now_ms,
                    gap_ms = now_ms - last_hb,
                    "extension connection state: connected → disconnected (heartbeat timeout)"
                );
            }
        }

        ExtensionConnectionStatus {
            connected,
            last_heartbeat_ms: last_hb,
            last_event_ms: last_evt,
        }
    }
}

/// 扩展连接状态快照
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ExtensionConnectionStatus {
    pub connected: bool,
    pub last_heartbeat_ms: i64,
    pub last_event_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_disconnected_when_no_heartbeat() {
        let state = ExtensionConnectionState::new();
        let s = state.status(100_000);
        assert!(!s.connected);
        assert_eq!(s.last_heartbeat_ms, 0);
    }

    #[test]
    fn status_connected_when_heartbeat_recent() {
        let state = ExtensionConnectionState::new();
        state.record_heartbeat(100_000);
        let s = state.status(130_000); // 30s 后
        assert!(s.connected);
    }

    #[test]
    fn status_disconnected_when_heartbeat_stale() {
        let state = ExtensionConnectionState::new();
        state.record_heartbeat(100_000);
        let s = state.status(200_000); // 100s 后，超过 60s 超时
        assert!(!s.connected);
    }

    #[test]
    fn record_event_updates_timestamp() {
        let state = ExtensionConnectionState::new();
        state.record_event(150_000);
        let s = state.status(150_000);
        assert_eq!(s.last_event_ms, 150_000);
    }
}
