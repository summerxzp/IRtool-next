use crate::types::{EventPage, EventQuery, MonitorEvent};
use irtool_core::IrError;
use rusqlite::Connection;
use std::sync::Mutex;

/// 构建 WHERE 子句和参数（供 search_events_page 和 search_events 复用）
fn build_where_clause(query: &EventQuery) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref s) = query.source {
        conditions.push("source = ?".to_string());
        params.push(Box::new(serde_json::to_string(s).unwrap_or_default()));
    }
    if let Some(ref et) = query.event_type {
        conditions.push("event_type = ?".to_string());
        params.push(Box::new(et.clone()));
    }
    if let Some(ref pn) = query.process_name {
        conditions.push("process_name LIKE ?".to_string());
        params.push(Box::new(format!("%{}%", pn)));
    }
    if let Some(ref kf) = query.key_field {
        conditions.push("key_field LIKE ?".to_string());
        params.push(Box::new(format!("%{}%", kf)));
    }
    if let Some(is_ext) = query.is_external {
        conditions.push("is_external = ?".to_string());
        params.push(Box::new(if is_ext { 1i64 } else { 0i64 }));
    }
    if let Some(ref st) = query.search_text {
        conditions.push("(process_name LIKE ? OR key_field LIKE ? OR raw_json LIKE ?)".to_string());
        let pattern = format!("%{}%", st);
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, params)
}

/// 从行中读取 MonitorEvent（包含新列）
fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<MonitorEvent> {
    let source_str: String = row.get(2)?;
    Ok(MonitorEvent {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        source: serde_json::from_str(&source_str).unwrap_or(crate::types::EventSource::Sysmon),
        event_type: row.get(3)?,
        process_name: row.get(4)?,
        key_field: row.get(5)?,
        raw_json: row.get(6)?,
    })
}

/// 分页搜索事件，返回总数 + 当前页数据
pub fn search_events_page(
    conn: &Mutex<Connection>,
    query: &EventQuery,
) -> Result<EventPage, IrError> {
    let conn = conn.lock().unwrap();
    let (where_clause, params) = build_where_clause(query);

    // 查询总数
    let count_sql = format!("SELECT COUNT(*) FROM events {}", where_clause);
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: u64 = conn
        .query_row(&count_sql, rusqlite::params_from_iter(param_refs.iter().copied()), |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v as u64)
        .map_err(|e| IrError::Internal(format!("查询事件总数失败: {}", e)))?;

    // 查询当前页数据
    let data_sql = format!(
        "SELECT id, timestamp, source, event_type, process_name, key_field, raw_json FROM events {} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = params;
    all_params.push(Box::new(query.limit));
    all_params.push(Box::new(query.offset));
    let param_refs: Vec<&dyn rusqlite::ToSql> = all_params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn
        .prepare(&data_sql)
        .map_err(|e| IrError::Internal(format!("准备搜索失败: {}", e)))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(param_refs.iter().copied()), |row| {
            row_to_event(row)
        })
        .map_err(|e| IrError::Internal(format!("搜索事件失败: {}", e)))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| IrError::Internal(format!("读取搜索行失败: {}", e)))?);
    }

    Ok(EventPage {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    })
}
