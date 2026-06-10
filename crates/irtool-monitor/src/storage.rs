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

    /// 查询最近事件
    pub fn get_recent_events(&self, limit: u32) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, source, event_type, process_name, key_field, raw_json FROM events ORDER BY timestamp DESC LIMIT ?1"
        ).map_err(|e| IrError::Internal(format!("查询事件失败: {}", e)))?;
        let rows = stmt.query_map(params![limit], |row| {
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
        }).map_err(|e| IrError::Internal(format!("查询事件失败: {}", e)))?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| IrError::Internal(format!("读取事件行失败: {}", e)))?);
        }
        Ok(events)
    }

    /// 清除所有事件
    pub fn clear_events(&self) -> Result<u64, IrError> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM events", [])
            .map_err(|e| IrError::Internal(format!("清除事件失败: {}", e)))?;
        Ok(deleted as u64)
    }

    /// 查询事件类型统计
    pub fn get_event_type_counts(&self) -> Result<Vec<(String, u64)>, IrError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT event_type, COUNT(*) as cnt FROM events GROUP BY event_type ORDER BY cnt DESC"
        ).map_err(|e| IrError::Internal(format!("查询事件类型统计失败: {}", e)))?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        }).map_err(|e| IrError::Internal(format!("查询事件类型统计失败: {}", e)))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| IrError::Internal(format!("读取统计失败: {}", e)))?);
        }
        Ok(result)
    }

    /// 查询事件总数
    pub fn get_event_count(&self) -> Result<u64, IrError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .map_err(|e| IrError::Internal(format!("查询事件总数失败: {}", e)))?;
        Ok(count as u64)
    }

    /// 搜索事件，支持多种过滤条件
    pub fn search_events(
        &self,
        source: Option<&str>,
        event_type: Option<&str>,
        process_name: Option<&str>,
        key_field: Option<&str>,
        search_text: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::types::MonitorEvent>, IrError> {
        let conn = self.conn.lock().unwrap();
        
        // 构建 WHERE 子句
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        let mut param_index = 1;

        if let Some(s) = source {
            conditions.push(format!("source = ?{}", param_index));
            params.push(s.to_string());
            param_index += 1;
        }
        if let Some(et) = event_type {
            conditions.push(format!("event_type = ?{}", param_index));
            params.push(et.to_string());
            param_index += 1;
        }
        if let Some(pn) = process_name {
            conditions.push(format!("process_name LIKE ?{}", param_index));
            params.push(format!("%{}%", pn));
            param_index += 1;
        }
        if let Some(kf) = key_field {
            conditions.push(format!("key_field LIKE ?{}", param_index));
            params.push(format!("%{}%", kf));
            param_index += 1;
        }
        if let Some(st) = search_text {
            conditions.push(format!("(process_name LIKE ?{} OR key_field LIKE ?{} OR raw_json LIKE ?{})", param_index, param_index + 1, param_index + 2));
            params.push(format!("%{}%", st));
            params.push(format!("%{}%", st));
            params.push(format!("%{}%", st));
            param_index += 3;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, timestamp, source, event_type, process_name, key_field, raw_json FROM events {} ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            where_clause, param_index, param_index + 1
        );

        params.push(limit.to_string());
        params.push(offset.to_string());

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| IrError::Internal(format!("准备搜索失败: {}", e)))?;
        
        // 创建params引用
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
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
        }).map_err(|e| IrError::Internal(format!("搜索事件失败: {}", e)))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| IrError::Internal(format!("读取搜索行失败: {}", e)))?);
        }
        Ok(events)
    }
}
