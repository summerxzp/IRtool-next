use crate::storage::EventStorage;
use crate::types::MonitorEvent;
use irtool_core::IrError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// 批量摄入队列：异步收集事件，定时批量写入 SQLite
pub struct EventIngestQueue {
    tx: mpsc::Sender<MonitorEvent>,
    /// 遥测计数器
    events_written: Arc<AtomicU64>,
    events_dropped: Arc<AtomicU64>,
}

/// 队列容量
const CHANNEL_CAPACITY: usize = 10_000;
/// 刷新阈值：累积 200 条或每 500ms 刷一次
const FLUSH_BATCH_SIZE: usize = 200;
const FLUSH_INTERVAL_MS: u64 = 500;

impl EventIngestQueue {
    /// 创建队列并启动后台 worker。
    /// 必须在 Tokio 运行时上下文中调用。
    pub fn start(storage: Arc<EventStorage>) -> Self {
        let (tx, rx) = mpsc::channel::<MonitorEvent>(CHANNEL_CAPACITY);
        let events_written = Arc::new(AtomicU64::new(0));
        let events_dropped = Arc::new(AtomicU64::new(0));
        let written_clone = events_written.clone();
        let dropped_clone = events_dropped.clone();

        tokio::spawn(async move {
            ingest_worker(rx, storage, written_clone, dropped_clone).await;
        });

        Self {
            tx,
            events_written,
            events_dropped,
        }
    }

    /// 推送一条事件到队列（非阻塞）
    pub fn push(&self, event: MonitorEvent) -> Result<(), IrError> {
        match self.tx.try_send(event) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.events_dropped.fetch_add(1, Ordering::Relaxed);
                warn!("摄入队列已满，丢弃事件");
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(IrError::Internal("摄入队列已关闭".to_string())),
        }
    }

    /// 获取已写入事件数
    pub fn events_written(&self) -> u64 {
        self.events_written.load(Ordering::Relaxed)
    }

    /// 获取已丢弃事件数
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }
}

async fn ingest_worker(
    mut rx: mpsc::Receiver<MonitorEvent>,
    storage: Arc<EventStorage>,
    events_written: Arc<AtomicU64>,
    events_dropped: Arc<AtomicU64>,
) {
    let mut buffer: Vec<MonitorEvent> = Vec::with_capacity(FLUSH_BATCH_SIZE);
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(FLUSH_INTERVAL_MS));

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                buffer.push(event);
                if buffer.len() >= FLUSH_BATCH_SIZE {
                    flush_buffer(&storage, &mut buffer, &events_written, &events_dropped);
                }
            }
            _ = interval.tick() => {
                if !buffer.is_empty() {
                    flush_buffer(&storage, &mut buffer, &events_written, &events_dropped);
                }
            }
            else => {
                // 通道关闭，刷完剩余
                if !buffer.is_empty() {
                    flush_buffer(&storage, &mut buffer, &events_written, &events_dropped);
                }
                info!("摄入工作线程退出");
                break;
            }
        }
    }
}

fn flush_buffer(
    storage: &EventStorage,
    buffer: &mut Vec<MonitorEvent>,
    events_written: &Arc<AtomicU64>,
    events_dropped: &Arc<AtomicU64>,
) {
    if buffer.is_empty() {
        return;
    }
    let count = buffer.len() as u64;
    match storage.insert_events(buffer) {
        Ok(()) => {
            events_written.fetch_add(count, Ordering::Relaxed);
        }
        Err(e) => {
            events_dropped.fetch_add(count, Ordering::Relaxed);
            warn!("批量写入事件失败: {}", e);
        }
    }
    buffer.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventSource;

    fn make_test_event(id: i64) -> MonitorEvent {
        MonitorEvent {
            id,
            timestamp: id,
            source: EventSource::Sysmon,
            event_type: "dns".to_string(),
            process_name: "test.exe".to_string(),
            key_field: "example.com".to_string(),
            raw_json: "{}".to_string(),
        }
    }

    fn create_test_storage() -> (Arc<EventStorage>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = Arc::new(EventStorage::open(&db_path).unwrap());
        (storage, dir)
    }

    #[tokio::test]
    async fn events_written_initially_zero() {
        let (storage, _dir) = create_test_storage();
        let queue = EventIngestQueue::start(storage);
        assert_eq!(queue.events_written(), 0);
        assert_eq!(queue.events_dropped(), 0);
    }

    #[tokio::test]
    async fn push_normal_path_ok() {
        let (storage, _dir) = create_test_storage();
        let queue = EventIngestQueue::start(storage);

        let result = queue.push(make_test_event(1));
        assert!(result.is_ok());

        // 等待 flush（FLUSH_INTERVAL_MS = 500ms）
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

        assert!(
            queue.events_written() >= 1,
            "events_written should be >= 1 after flush, got {}",
            queue.events_written()
        );
        assert_eq!(queue.events_dropped(), 0);
    }

    #[tokio::test]
    async fn push_multiple_events_counted() {
        let (storage, _dir) = create_test_storage();
        let queue = EventIngestQueue::start(storage);

        let count: usize = 5;
        for i in 0..count {
            assert!(queue.push(make_test_event(i as i64)).is_ok());
        }

        // 等待 flush
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

        assert_eq!(
            queue.events_written(),
            count as u64,
            "events_written should equal {} after flush",
            count
        );
        assert_eq!(queue.events_dropped(), 0);
    }

    #[tokio::test]
    async fn events_dropped_increments_on_full() {
        let (storage, _dir) = create_test_storage();
        let queue = EventIngestQueue::start(storage);

        // 在 current_thread 运行时中，不 yield 时 worker 不会被调度，
        // 通道会填满，后续 push 触发 Full 分支
        for i in 0..(CHANNEL_CAPACITY + 100) {
            let _ = queue.push(make_test_event(i as i64));
        }

        assert!(
            queue.events_dropped() > 0,
            "events_dropped should be > 0 when channel overflows, got {}",
            queue.events_dropped()
        );
    }
}
