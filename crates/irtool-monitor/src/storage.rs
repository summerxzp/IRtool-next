use crate::types::{Alert, EventPage, EventQuery, MonitorEvent};
use irtool_core::IrError;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct EventStorage {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

impl EventStorage {
    pub fn open(path: &Path) -> Result<Self, IrError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| IrError::Io(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| IrError::Internal(format!("SQLite 打开失败: {}", e)))?;
        // 性能优化 PRAGMA
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 3000;",
        )
        .map_err(|e| IrError::Internal(format!("设置 PRAGMA 失败: {}", e)))?;
        let storage = Self {
            conn: Mutex::new(conn),
            db_path: path.to_path_buf(),
        };
        storage.init_tables()?;
        storage.migrate_schema()?;
        Ok(storage)
    }

    fn init_tables(&self) -> Result<(), IrError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                event_type TEXT NOT NULL,
                process_name TEXT,
                key_field TEXT,
                raw_json TEXT,
                process_id INTEGER,
                process_path TEXT,
                remote_ip TEXT,
                remote_port INTEGER,
                domain TEXT,
                is_external INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_events_event_type ON events(event_type);

            CREATE TABLE IF NOT EXISTS alerts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                rule_name TEXT NOT NULL,
                event_type TEXT NOT NULL,
                process_name TEXT,
                key_field TEXT,
                action_taken TEXT,
                raw_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_alerts_timestamp ON alerts(timestamp);",
        )
        .map_err(|e| IrError::Internal(format!("建表失败: {}", e)))?;
        Ok(())
    }

    /// 为已有数据库迁移新列（ALTER TABLE ADD COLUMN，列已存在则忽略）
    fn migrate_schema(&self) -> Result<(), IrError> {
        let conn = self.conn.lock().unwrap();
        let new_columns = [
            "process_id INTEGER",
            "process_path TEXT",
            "remote_ip TEXT",
            "remote_port INTEGER",
            "domain TEXT",
            "is_external INTEGER NOT NULL DEFAULT 0",
        ];
        for col_def in &new_columns {
            let col_name = col_def.split_whitespace().next().unwrap();
            let sql = format!("ALTER TABLE events ADD COLUMN {}", col_def);
            // 列已存在时忽略错误
            if let Err(e) = conn.execute_batch(&sql) {
                if !e.to_string().contains("duplicate column name") {
                    return Err(IrError::Internal(format!("迁移列 {} 失败: {}", col_name, e)));
                }
            }
        }
        // 补充新索引（IF NOT EXISTS 保证幂等）
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_events_type_time ON events(event_type, timestamp DESC);
             CREATE INDEX IF NOT EXISTS idx_events_source_time ON events(source, timestamp DESC);
             CREATE INDEX IF NOT EXISTS idx_events_key_time ON events(key_field, timestamp DESC);
             CREATE INDEX IF NOT EXISTS idx_events_external_time ON events(is_external, timestamp DESC);",
        )
        .map_err(|e| IrError::Internal(format!("创建索引失败: {}", e)))?;
        Ok(())
    }

    /// 批量写入事件（事务）
    pub fn insert_events(&self, events: &[MonitorEvent]) -> Result<(), IrError> {
        if events.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| IrError::Internal(format!("事务开始失败: {}", e)))?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO events (timestamp, source, event_type, process_name, key_field, raw_json, process_id, process_path, remote_ip, remote_port, domain, is_external) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
            ).map_err(|e| IrError::Internal(format!("prepare 失败: {}", e)))?;
            for event in events {
                // 从 raw_json 提取新列数据
                let extra = extract_event_extra(&event.raw_json);
                stmt.execute(params![
                    event.timestamp,
                    serde_json::to_string(&event.source).unwrap_or_default(),
                    event.event_type,
                    event.process_name,
                    event.key_field,
                    event.raw_json,
                    extra.process_id,
                    extra.process_path,
                    extra.remote_ip,
                    extra.remote_port,
                    extra.domain,
                    extra.is_external,
                ])
                .map_err(|e| IrError::Internal(format!("插入事件失败: {}", e)))?;
            }
        }
        tx.commit()
            .map_err(|e| IrError::Internal(format!("事务提交失败: {}", e)))?;
        Ok(())
    }

    /// 写入告警
    pub fn insert_alert(&self, alert: &mut Alert) -> Result<(), IrError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO alerts (timestamp, rule_name, event_type, process_name, key_field, action_taken, raw_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                alert.timestamp,
                alert.rule_name,
                alert.event_type,
                alert.process_name,
                alert.key_field,
                alert.action_taken,
                alert.raw_json,
            ],
        ).map_err(|e| IrError::Internal(format!("插入告警失败: {}", e)))?;
        // Get the auto-incremented id that was just inserted
        alert.id = conn.last_insert_rowid();
        Ok(())
    }

    /// 清理过期事件
    pub fn cleanup_old_events(&self, retention_days: u32) -> Result<u64, IrError> {
        if retention_days == 0 {
            return Ok(0);
        }
        let cutoff = chrono::Utc::now().timestamp_millis() - (retention_days as i64 * 24 * 3600 * 1000);
        let conn = self.conn.lock().unwrap();
        let deleted = conn
            .execute("DELETE FROM events WHERE timestamp < ?1", params![cutoff])
            .map_err(|e| IrError::Internal(format!("清理事件失败: {}", e)))?;
        Ok(deleted as u64)
    }

    /// 查询最近告警
    pub fn get_recent_alerts(&self, limit: u32) -> Result<Vec<Alert>, IrError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, rule_name, event_type, process_name, key_field, action_taken, raw_json FROM alerts ORDER BY timestamp DESC LIMIT ?1"
        ).map_err(|e| IrError::Internal(format!("查询告警失败: {}", e)))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(Alert {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    rule_name: row.get(2)?,
                    event_type: row.get(3)?,
                    process_name: row.get(4)?,
                    key_field: row.get(5)?,
                    action_taken: row.get(6)?,
                    raw_json: row.get(7)?,
                })
            })
            .map_err(|e| IrError::Internal(format!("查询告警失败: {}", e)))?;
        let mut alerts = Vec::new();
        for row in rows {
            alerts.push(row.map_err(|e| IrError::Internal(format!("读取告警行失败: {}", e)))?);
        }
        Ok(alerts)
    }

    /// 清除所有告警
    pub fn clear_alerts(&self) -> Result<u64, IrError> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn
            .execute("DELETE FROM alerts", [])
            .map_err(|e| IrError::Internal(format!("清除告警失败: {}", e)))?;
        Ok(deleted as u64)
    }

    /// 查询最近事件
    pub fn get_recent_events(&self, limit: u32) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, source, event_type, process_name, key_field, raw_json FROM events ORDER BY timestamp DESC LIMIT ?1"
        ).map_err(|e| IrError::Internal(format!("查询事件失败: {}", e)))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                let source_str: String = row.get(2)?;
                Ok(crate::types::MonitorEvent {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    source: serde_json::from_str(&source_str).unwrap_or(crate::types::EventSource::Sysmon),
                    event_type: row.get(3)?,
                    process_name: row.get(4)?,
                    key_field: row.get(5)?,
                    raw_json: row.get(6)?,
                })
            })
            .map_err(|e| IrError::Internal(format!("查询事件失败: {}", e)))?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| IrError::Internal(format!("读取事件行失败: {}", e)))?);
        }
        Ok(events)
    }

    /// 清除所有事件
    pub fn clear_events(&self) -> Result<u64, IrError> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn
            .execute("DELETE FROM events", [])
            .map_err(|e| IrError::Internal(format!("清除事件失败: {}", e)))?;
        Ok(deleted as u64)
    }

    /// 查询事件类型统计
    pub fn get_event_type_counts(&self) -> Result<Vec<(String, u64)>, IrError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT event_type, COUNT(*) as cnt FROM events GROUP BY event_type ORDER BY cnt DESC")
            .map_err(|e| IrError::Internal(format!("查询事件类型统计失败: {}", e)))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?)))
            .map_err(|e| IrError::Internal(format!("查询事件类型统计失败: {}", e)))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| IrError::Internal(format!("读取统计失败: {}", e)))?);
        }
        Ok(result)
    }

    /// 获取数据库实际磁盘占用（字节），包含 .db、.db-wal、.db-shm 文件
    pub fn get_db_size(&self) -> Result<u64, IrError> {
        let mut total: u64 = 0;
        for suffix in &["", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{}", self.db_path.display(), suffix));
            if let Ok(meta) = std::fs::metadata(&p) {
                total += meta.len();
            }
        }
        Ok(total)
    }

    /// 查询事件总数
    pub fn get_event_count(&self) -> Result<u64, IrError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .map_err(|e| IrError::Internal(format!("查询事件总数失败: {}", e)))?;
        Ok(count as u64)
    }

    /// 搜索事件，支持多种过滤条件（委托给 query 模块）
    pub fn search_events(&self, query: &EventQuery) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        let page = crate::query::search_events_page(&self.conn, query)?;
        Ok(page.items)
    }

    /// 分页搜索事件，返回总数 + 当前页数据
    pub fn search_events_page(&self, query: &EventQuery) -> Result<EventPage, IrError> {
        crate::query::search_events_page(&self.conn, query)
    }
}

/// 从 raw_json 中提取新列数据
struct EventExtra {
    process_id: Option<i64>,
    process_path: Option<String>,
    remote_ip: Option<String>,
    remote_port: Option<i64>,
    domain: Option<String>,
    is_external: i64,
}

fn extract_event_extra(raw_json: &str) -> EventExtra {
    let val: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(_) => {
            return EventExtra {
                process_id: None,
                process_path: None,
                remote_ip: None,
                remote_port: None,
                domain: None,
                is_external: 0,
            }
        }
    };

    let process_id = val
        .get("process_id")
        .and_then(|v| v.as_i64())
        .or_else(|| val.get("ProcessId").and_then(|v| v.as_i64()));

    let process_path = val
        .get("process_path")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("Image").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let remote_ip = val
        .get("destination_ip")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("DestinationIp").and_then(|v| v.as_str()))
        .or_else(|| val.get("dst_ip").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let remote_port = val
        .get("destination_port")
        .and_then(|v| v.as_u64())
        .or_else(|| val.get("DestinationPort").and_then(|v| v.as_u64()))
        .or_else(|| val.get("dst_port").and_then(|v| v.as_u64()))
        .map(|v| v as i64);

    let domain = val
        .get("query_name")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("QueryName").and_then(|v| v.as_str()))
        .or_else(|| val.get("domain").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    // 判断是否为外部 IP（非私有地址）
    let is_external = if let Some(ref ip_str) = remote_ip {
        ip_str
            .parse::<std::net::IpAddr>()
            .map_or(1, |ip| if ip.is_loopback() || is_private_ip(&ip) { 0 } else { 1 })
    } else {
        0
    };

    EventExtra {
        process_id,
        process_path,
        remote_ip,
        remote_port,
        domain,
        is_external,
    }
}

/// 判断是否为私有 IP 地址
fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 10.0.0.0/8
            octets[0] == 10
            // 172.16.0.0/12
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
            // 192.168.0.0/16
            || (octets[0] == 192 && octets[1] == 168)
            // 127.0.0.0/8 (loopback)
            || octets[0] == 127
            // 169.254.0.0/16 (link-local)
            || (octets[0] == 169 && octets[1] == 254)
        }
        std::net::IpAddr::V6(v6) => v6.is_loopback() || is_ula_v6(v6),
    }
}

/// 判断 IPv6 是否为唯一本地地址 (fc00::/7)
fn is_ula_v6(v6: &std::net::Ipv6Addr) -> bool {
    let segments = v6.segments();
    (segments[0] & 0xfe00) == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EventSource;

    fn test_storage() -> EventStorage {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        EventStorage::open(&db_path).unwrap()
    }

    fn make_event(timestamp: i64, event_type: &str, source: EventSource, raw_json: &str) -> MonitorEvent {
        MonitorEvent {
            id: 0,
            timestamp,
            source,
            event_type: event_type.to_string(),
            process_name: "test.exe".to_string(),
            key_field: "example.com".to_string(),
            raw_json: raw_json.to_string(),
        }
    }

    #[test]
    fn migrate_from_old_schema() {
        // 模拟旧数据库：只有基础列，没有新列
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("old.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                source TEXT NOT NULL,
                event_type TEXT NOT NULL,
                process_name TEXT,
                key_field TEXT,
                raw_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_events_event_type ON events(event_type);",
        )
        .unwrap();
        // 插入一条旧数据
        conn.execute(
            "INSERT INTO events (timestamp, source, event_type, process_name, key_field, raw_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![1000i64, "\"Sysmon\"", "dns", "test.exe", "example.com", "{}"],
        ).unwrap();
        drop(conn);

        // 用 EventStorage::open 打开旧数据库，应自动迁移
        let storage = EventStorage::open(&db_path).unwrap();
        // 验证迁移后可以正常写入
        let event = make_event(
            2000,
            "network_connect",
            EventSource::Sysmon,
            r#"{"destination_ip":"8.8.8.8"}"#,
        );
        storage.insert_events(&[event]).unwrap();
        let count = storage.get_event_count().unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn insert_events_batch_persists_all_rows() {
        let storage = test_storage();
        let events: Vec<MonitorEvent> = (0..5)
            .map(|i| make_event(1000 + i, "dns", EventSource::Sysmon, r#"{"process_id":123}"#))
            .collect();
        storage.insert_events(&events).unwrap();

        let count = storage.get_event_count().unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn search_events_uses_limit_offset() {
        let storage = test_storage();
        let events: Vec<MonitorEvent> = (0..10)
            .map(|i| make_event(1000 + i, "dns", EventSource::Sysmon, "{}"))
            .collect();
        storage.insert_events(&events).unwrap();

        let query1 = EventQuery {
            source: None,
            event_type: None,
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: None,
            limit: 3,
            offset: 0,
        };
        let page1 = storage.search_events(&query1).unwrap();
        assert_eq!(page1.len(), 3);

        // 第二页
        let query2 = EventQuery {
            source: None,
            event_type: None,
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: None,
            limit: 3,
            offset: 3,
        };
        let page2 = storage.search_events(&query2).unwrap();
        assert_eq!(page2.len(), 3);

        // 超出范围
        let query3 = EventQuery {
            source: None,
            event_type: None,
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: None,
            limit: 3,
            offset: 9,
        };
        let page3 = storage.search_events(&query3).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn search_events_page_total_matches_filters() {
        let storage = test_storage();
        let dns_events: Vec<MonitorEvent> = (0..4)
            .map(|i| make_event(1000 + i, "dns", EventSource::Sysmon, "{}"))
            .collect();
        let net_events: Vec<MonitorEvent> = (0..3)
            .map(|i| make_event(2000 + i, "network_connect", EventSource::Sysmon, "{}"))
            .collect();
        storage.insert_events(&dns_events).unwrap();
        storage.insert_events(&net_events).unwrap();

        // 无过滤
        let query = EventQuery {
            source: None,
            event_type: None,
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: None,
            limit: 10,
            offset: 0,
        };
        let page = storage.search_events_page(&query).unwrap();
        assert_eq!(page.total, 7);
        assert_eq!(page.items.len(), 7);

        // 过滤 event_type = dns
        let query = EventQuery {
            event_type: Some("dns".to_string()),
            ..query.clone()
        };
        let page = storage.search_events_page(&query).unwrap();
        assert_eq!(page.total, 4);
        assert_eq!(page.items.len(), 4);

        // 过滤 event_type = network_connect
        let query = EventQuery {
            event_type: Some("network_connect".to_string()),
            ..query.clone()
        };
        let page = storage.search_events_page(&query).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 3);
    }

    #[test]
    fn extract_event_extra_parses_raw_json() {
        let raw = r#"{"process_id":42,"destination_ip":"8.8.8.8","destination_port":443,"query_name":"example.com"}"#;
        let extra = extract_event_extra(raw);
        assert_eq!(extra.process_id, Some(42));
        assert_eq!(extra.remote_ip.as_deref(), Some("8.8.8.8"));
        assert_eq!(extra.remote_port, Some(443));
        assert_eq!(extra.domain.as_deref(), Some("example.com"));
        assert_eq!(extra.is_external, 1); // 8.8.8.8 is external

        let raw_private = r#"{"destination_ip":"192.168.1.1"}"#;
        let extra_private = extract_event_extra(raw_private);
        assert_eq!(extra_private.remote_ip.as_deref(), Some("192.168.1.1"));
        assert_eq!(extra_private.is_external, 0); // private IP
    }

    #[test]
    fn open_creates_db_file() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("new_db.db");
        assert!(!db_path.exists());
        EventStorage::open(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn init_tables_idempotent() {
        let storage = test_storage();
        // init_tables already called by open(); calling again should not error
        storage.init_tables().unwrap();
        storage.init_tables().unwrap();
    }

    #[test]
    fn insert_events_empty_array_ok() {
        let storage = test_storage();
        let result = storage.insert_events(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn get_event_count_matches_inserted() {
        let storage = test_storage();
        assert_eq!(storage.get_event_count().unwrap(), 0);
        let events: Vec<MonitorEvent> = (0..7)
            .map(|i| make_event(1000 + i, "dns", EventSource::Sysmon, "{}"))
            .collect();
        storage.insert_events(&events).unwrap();
        assert_eq!(storage.get_event_count().unwrap(), 7);
    }

    #[test]
    fn get_recent_events_ordered_by_timestamp_desc() {
        let storage = test_storage();
        let events: Vec<MonitorEvent> = (0..5)
            .map(|i| make_event(1000 + i * 100, "dns", EventSource::Sysmon, "{}"))
            .collect();
        storage.insert_events(&events).unwrap();
        let recent = storage.get_recent_events(10).unwrap();
        assert_eq!(recent.len(), 5);
        for w in recent.windows(2) {
            assert!(w[0].timestamp >= w[1].timestamp);
        }
    }

    #[test]
    fn cleanup_old_events_removes_expired() {
        let storage = test_storage();
        let now_ms = chrono::Utc::now().timestamp_millis();
        // 4 天前的事件，超过 3 天保留期
        let old_event = make_event(now_ms - 4 * 24 * 3600 * 1000, "dns", EventSource::Sysmon, "{}");
        // 刚发生的事件
        let recent_event = make_event(now_ms - 1000, "dns", EventSource::Sysmon, "{}");
        storage.insert_events(&[old_event, recent_event]).unwrap();
        assert_eq!(storage.get_event_count().unwrap(), 2);

        let deleted = storage.cleanup_old_events(3).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(storage.get_event_count().unwrap(), 1);
    }

    #[test]
    fn insert_alert_and_get_recent() {
        let storage = test_storage();
        let mut alert = Alert {
            id: 0,
            timestamp: 5000,
            rule_name: "test_rule".to_string(),
            event_type: "dns".to_string(),
            process_name: "test.exe".to_string(),
            key_field: "evil.com".to_string(),
            action_taken: "popup".to_string(),
            raw_json: "{}".to_string(),
        };
        storage.insert_alert(&mut alert).unwrap();
        assert!(alert.id > 0);

        let alerts = storage.get_recent_alerts(10).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_name, "test_rule");
        assert_eq!(alerts[0].key_field, "evil.com");
    }

    #[test]
    fn clear_alerts_removes_all() {
        let storage = test_storage();
        let mut alert = Alert {
            id: 0,
            timestamp: 5000,
            rule_name: "test_rule".to_string(),
            event_type: "dns".to_string(),
            process_name: "test.exe".to_string(),
            key_field: "evil.com".to_string(),
            action_taken: "popup".to_string(),
            raw_json: "{}".to_string(),
        };
        storage.insert_alert(&mut alert).unwrap();
        assert_eq!(storage.get_recent_alerts(10).unwrap().len(), 1);

        let deleted = storage.clear_alerts().unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(storage.get_recent_alerts(10).unwrap().len(), 0);
    }

    #[test]
    fn clear_events_removes_all() {
        let storage = test_storage();
        let events: Vec<MonitorEvent> = (0..3)
            .map(|i| make_event(1000 + i, "dns", EventSource::Sysmon, "{}"))
            .collect();
        storage.insert_events(&events).unwrap();
        assert_eq!(storage.get_event_count().unwrap(), 3);

        let deleted = storage.clear_events().unwrap();
        assert_eq!(deleted, 3);
        assert_eq!(storage.get_event_count().unwrap(), 0);
    }

    #[test]
    fn search_events_with_source_filter() {
        let storage = test_storage();
        let sysmon_event = make_event(1000, "dns", EventSource::Sysmon, "{}");
        let dns_event = make_event(1001, "dns", EventSource::DnsClient, "{}");
        storage.insert_events(&[sysmon_event, dns_event]).unwrap();

        let query = EventQuery {
            source: Some("sysmon".to_string()),
            event_type: None,
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: None,
            limit: 10,
            offset: 0,
        };
        let results = storage.search_events(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, EventSource::Sysmon);
    }

    #[test]
    fn search_events_with_event_type_filter() {
        let storage = test_storage();
        let dns_event = make_event(1000, "dns", EventSource::Sysmon, "{}");
        let net_event = make_event(1001, "network_connect", EventSource::Sysmon, "{}");
        storage.insert_events(&[dns_event, net_event]).unwrap();

        let query = EventQuery {
            source: None,
            event_type: Some("dns".to_string()),
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: None,
            limit: 10,
            offset: 0,
        };
        let results = storage.search_events(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_type, "dns");
    }

    #[test]
    fn search_events_with_search_text() {
        let storage = test_storage();
        let event1 = MonitorEvent {
            id: 0,
            timestamp: 1000,
            source: EventSource::Sysmon,
            event_type: "dns".to_string(),
            process_name: "test.exe".to_string(),
            key_field: "evil.com".to_string(),
            raw_json: "{}".to_string(),
        };
        let event2 = MonitorEvent {
            id: 0,
            timestamp: 1001,
            source: EventSource::Sysmon,
            event_type: "dns".to_string(),
            process_name: "test.exe".to_string(),
            key_field: "safe.com".to_string(),
            raw_json: "{}".to_string(),
        };
        storage.insert_events(&[event1, event2]).unwrap();

        let query = EventQuery {
            source: None,
            event_type: None,
            process_name: None,
            key_field: None,
            is_external: None,
            search_text: Some("evil".to_string()),
            limit: 10,
            offset: 0,
        };
        let results = storage.search_events(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key_field, "evil.com");
    }
}
