use crate::types::{Alert, MonitorEvent};
use irtool_core::IrError;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct EventStorage {
    conn: Mutex<Connection>,
}

impl EventStorage {
    pub fn open(path: &Path) -> Result<Self, IrError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| IrError::Io(e.to_string()))?;
        }
        let conn = Connection::open(path)
            .map_err(|e| IrError::Internal(format!("SQLite 打开失败: {}", e)))?;
        let storage = Self { conn: Mutex::new(conn) };
        storage.init_tables()?;
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
                raw_json TEXT
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
            CREATE INDEX IF NOT EXISTS idx_alerts_timestamp ON alerts(timestamp);"
        ).map_err(|e| IrError::Internal(format!("建表失败: {}", e)))?;
        Ok(())
    }

    /// 批量写入事件（事务）
    pub fn insert_events(&self, events: &[MonitorEvent]) -> Result<(), IrError> {
        if events.is_empty() { return Ok(()); }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()
            .map_err(|e| IrError::Internal(format!("事务开始失败: {}", e)))?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO events (timestamp, source, event_type, process_name, key_field, raw_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ).map_err(|e| IrError::Internal(format!("prepare 失败: {}", e)))?;
            for event in events {
                stmt.execute(params![
                    event.timestamp,
                    serde_json::to_string(&event.source).unwrap_or_default(),
                    event.event_type,
                    event.process_name,
                    event.key_field,
                    event.raw_json,
                ]).map_err(|e| IrError::Internal(format!("插入事件失败: {}", e)))?;
            }
        }
        tx.commit().map_err(|e| IrError::Internal(format!("事务提交失败: {}", e)))?;
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
        if retention_days == 0 { return Ok(0); }
        let cutoff = chrono::Utc::now().timestamp_millis()
            - (retention_days as i64 * 24 * 3600 * 1000);
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM events WHERE timestamp < ?1",
            params![cutoff],
        ).map_err(|e| IrError::Internal(format!("清理事件失败: {}", e)))?;
        Ok(deleted as u64)
    }

    /// 查询最近告警
    pub fn get_recent_alerts(&self, limit: u32) -> Result<Vec<Alert>, IrError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, rule_name, event_type, process_name, key_field, action_taken, raw_json FROM alerts ORDER BY timestamp DESC LIMIT ?1"
        ).map_err(|e| IrError::Internal(format!("查询告警失败: {}", e)))?;
        let rows = stmt.query_map(params![limit], |row| {
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
        }).map_err(|e| IrError::Internal(format!("查询告警失败: {}", e)))?;
        let mut alerts = Vec::new();
        for row in rows {
            alerts.push(row.map_err(|e| IrError::Internal(format!("读取告警行失败: {}", e)))?);
        }
        Ok(alerts)
    }

    /// 清除所有告警
    pub fn clear_alerts(&self) -> Result<u64, IrError> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM alerts", [])
            .map_err(|e| IrError::Internal(format!("清除告警失败: {}", e)))?;
        Ok(deleted as u64)
    }
}
