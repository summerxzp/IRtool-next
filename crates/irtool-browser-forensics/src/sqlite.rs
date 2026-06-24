//! SQLite 读取策略：WAL 优先 + 复制 fallback
//!
//! 浏览器运行时 SQLite 数据库可能被锁定，需要策略性读取。
//! Chromium 默认使用 WAL 模式，允许并发读取。

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// SQLite 读取错误
#[derive(thiserror::Error, Debug)]
pub enum SqliteReadError {
    #[error("failed to copy database: {0}")]
    CopyFailed(String),

    #[error("failed to open database: {0}")]
    OpenFailed(String),

    #[error("database file not found: {0}")]
    NotFound(PathBuf),
}

/// 打开浏览器 SQLite 数据库，使用 WAL 优先 + 复制 fallback 策略
///
/// 1. 先尝试直接打开（WAL 模式下可并发读取）
/// 2. 如果直接打开失败（锁定），复制 db + -wal + -shm 到临时目录后打开
pub fn open_browser_db(db_path: &Path) -> Result<Connection, SqliteReadError> {
    if !db_path.exists() {
        return Err(SqliteReadError::NotFound(db_path.to_path_buf()));
    }

    // 策略 1：直接打开
    match Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => {
            debug!("opened browser db directly: {}", db_path.display());
            let _ = conn.execute_batch("PRAGMA query_only = ON;");
            return Ok(conn);
        }
        Err(e) => {
            debug!(
                "direct open failed (expected if browser is running): {}, falling back to copy",
                e
            );
        }
    }

    // 策略 2：复制后打开
    open_copied_db(db_path)
}

/// 复制数据库文件到临时目录后打开
fn open_copied_db(db_path: &Path) -> Result<Connection, SqliteReadError> {
    let temp_dir = std::env::temp_dir().join("irtool-browser-forensics");
    std::fs::create_dir_all(&temp_dir).map_err(|e| SqliteReadError::CopyFailed(format!("create temp dir: {}", e)))?;

    // 生成唯一临时文件名，避免并发冲突
    let file_stem = db_path.file_stem().and_then(|s| s.to_str()).unwrap_or("db");
    let suffix = db_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let unique_name = format!("{}-{}-{}", file_stem, std::process::id(), suffix);
    let copied_path = temp_dir.join(&unique_name);

    // 复制主数据库文件
    std::fs::copy(db_path, &copied_path).map_err(|e| SqliteReadError::CopyFailed(format!("copy db: {}", e)))?;

    // 复制 WAL 和 SHM 文件（如果存在）
    for ext in &["-wal", "-shm"] {
        let src = db_path.with_extension(format!("{}{}", suffix, ext));
        if src.exists() {
            let dst = copied_path.with_extension(format!("{}{}", suffix, ext));
            if let Err(e) = std::fs::copy(&src, &dst) {
                warn!("failed to copy {} file: {}", ext, e);
                // 不中断，WAL/SHM 缺失可能导致数据不完整但不影响打开
            }
        }
    }

    let conn = Connection::open_with_flags(&copied_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| SqliteReadError::OpenFailed(format!("open copied db: {}", e)))?;

    debug!("opened browser db via copy: {}", copied_path.display());

    let _ = conn.execute_batch("PRAGMA query_only = ON;");

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_nonexistent_db() {
        let result = open_browser_db(Path::new("nonexistent.db"));
        assert!(result.is_err());
    }

    #[test]
    fn open_empty_db() {
        let temp_dir = std::env::temp_dir().join("irtool-browser-forensics-test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test-open-empty.db");

        // 清理残留
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

        // 创建一个空数据库
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY);")
                .unwrap();
        }

        let result = open_browser_db(&db_path);
        assert!(result.is_ok(), "should be able to open the test database");

        // 清理
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
