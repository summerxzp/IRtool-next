//! 下载溯源：扫描浏览器下载记录

use crate::core::webkit_timestamp;
use crate::profile::BrowserProfile;
use crate::sqlite::open_browser_db;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;

/// 下载溯源结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadAttribution {
    pub browser: crate::core::BrowserKind,
    pub profile: String,
    pub downloads: Vec<DownloadInfo>,
}

/// 单个下载记录
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadInfo {
    /// 下载文件名
    pub filename: String,
    /// 下载文件本地路径
    pub local_path: String,
    /// 下载源 URL
    pub download_url: String,
    /// 来源页面 URL
    pub referrer: Option<String>,
    /// 下载开始时间
    pub start_time: Option<String>,
    /// 下载完成时间
    pub end_time: Option<String>,
    /// 文件大小（字节）
    pub total_bytes: Option<i64>,
    /// Chrome 安全判定
    pub danger_type: DangerType,
    /// 用户是否打开了文件
    pub opened: bool,
    /// 中断原因
    pub interrupt_reason: Option<String>,
    /// 证据类型
    pub evidence_type: String,
    /// 完整重定向链（按 chain_index ASC 排序）
    pub url_chain: Vec<String>,
    /// 发起下载的标签页 URL（新版 schema 字段）
    pub tab_url: Option<String>,
    /// 标签页 referrer（新版 schema 字段）
    pub tab_referrer_url: Option<String>,
}

/// Chrome 对下载文件的安全判定值
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DangerType {
    NotDangerous,
    DangerousUrl,
    DangerousContent,
    DangerousHost,
    UncommonUrl,
    PotentiallyUnwanted,
    AllowlistedByPolicy,
    Unknown,
}

/// 将 danger_type 整数映射为枚举
pub fn danger_type_from_int(value: i64) -> DangerType {
    match value {
        0 => DangerType::NotDangerous,
        1 => DangerType::DangerousUrl,
        2 => DangerType::DangerousContent,
        3 => DangerType::DangerousHost,
        4 => DangerType::UncommonUrl,
        5 => DangerType::PotentiallyUnwanted,
        7 => DangerType::AllowlistedByPolicy,
        _ => DangerType::Unknown,
    }
}

/// Map Chromium interrupt_reason integer to human-readable string
/// Reference: chromium/src/components/download/public/common/download_interrupt_reasons.h
pub fn interrupt_reason_to_string(reason: Option<i64>) -> Option<String> {
    match reason {
        None => None,
        Some(0) => Some("NONE".to_string()),
        Some(1) => Some("FILE_FAILED".to_string()),
        Some(2) => Some("FILE_ACCESS_DENIED".to_string()),
        Some(3) => Some("FILE_NO_SPACE".to_string()),
        Some(4) => Some("FILE_NAME_TOO_LONG".to_string()),
        Some(5) => Some("FILE_TOO_LARGE".to_string()),
        Some(6) => Some("FILE_VIRUS_INFECTED".to_string()),
        Some(7) => Some("FILE_TRANSIENT_ERROR".to_string()),
        Some(8) => Some("FILE_BLOCKED".to_string()),
        Some(9) => Some("FILE_SECURITY_CHECK_FAILED".to_string()),
        Some(10) => Some("FILE_TOO_SHORT".to_string()),
        Some(11) => Some("FILE_HASH_MISMATCH".to_string()),
        Some(20) => Some("NETWORK_FAILED".to_string()),
        Some(21) => Some("NETWORK_TIMEOUT".to_string()),
        Some(22) => Some("NETWORK_DISCONNECTED".to_string()),
        Some(23) => Some("NETWORK_SERVER_DOWN".to_string()),
        Some(24) => Some("NETWORK_INVALID_REQUEST".to_string()),
        Some(30) => Some("SERVER_FAILED".to_string()),
        Some(31) => Some("SERVER_NO_RANGE".to_string()),
        Some(32) => Some("SERVER_BAD_CONTENT".to_string()),
        Some(33) => Some("SERVER_UNAUTHORIZED".to_string()),
        Some(34) => Some("SERVER_CERT_PROBLEM".to_string()),
        Some(35) => Some("SERVER_FORBIDDEN".to_string()),
        Some(36) => Some("SERVER_UNREACHABLE".to_string()),
        Some(40) => Some("USER_CANCELED".to_string()),
        Some(41) => Some("USER_SHUTDOWN".to_string()),
        Some(50) => Some("CRASH".to_string()),
        Some(val) => Some(format!("UNKNOWN({})", val)),
    }
}

/// 从路径中提取文件名
pub fn extract_filename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

/// 检测 downloads 表是否包含 url 列。
///
/// 新版 Chromium（约 M118+）将 url 字段从 downloads 表移除，
/// URL 信息改存于 downloads_url_chains 表（通过 chain_id = downloads.id 关联）。
/// 老版本和测试 fixture 仍保留 url 列，需兼容两种 schema。
fn downloads_has_url_column(conn: &Connection) -> bool {
    downloads_has_column(conn, "url")
}

/// 检测 downloads 表是否包含指定列。
fn downloads_has_column(conn: &Connection, column: &str) -> bool {
    let Ok(mut stmt) = conn.prepare("PRAGMA table_info(downloads)") else {
        return false;
    };
    let rows = stmt.query_map([], |row| row.get::<_, String>(1));
    match rows {
        Ok(names) => names.filter_map(|r| r.ok()).any(|n| n == column),
        Err(_) => false,
    }
}

/// 检测 downloads 表是否包含 tab_url 列（新版 Chromium schema）。
fn downloads_has_tab_url_column(conn: &Connection) -> bool {
    downloads_has_column(conn, "tab_url")
}

/// 检测 downloads 表是否包含 tab_referrer_url 列（新版 Chromium schema）。
fn downloads_has_tab_referrer_url_column(conn: &Connection) -> bool {
    downloads_has_column(conn, "tab_referrer_url")
}

/// 构建 downloads 查询 SQL。
///
/// `has_url=true`：直接 SELECT url（老 schema / 测试 fixture）。
/// `has_url=false`：用子查询从 downloads_url_chains 取最终 URL
///   （取 url_index 最大的那条，即重定向后的最终地址）。
///
/// `has_tab_url`/`has_tab_referrer_url`：新版 Chromium schema 在 downloads 表
///   新增 tab_url/tab_referrer_url 列；老 schema 用 NULL 保持兼容。
///
/// 始终 SELECT d.id 以便后续读取完整 url_chain。
///
/// `where_clause` 为空表示全量扫描；非空时需使用 `d.` 前缀引用别名。
fn build_downloads_sql(has_url: bool, has_tab_url: bool, has_tab_referrer_url: bool, where_clause: &str) -> String {
    let url_expr = if has_url {
        "d.url".to_string()
    } else {
        // 新版 Chromium schema：URL 在 downloads_url_chains 表中
        // 列名：id（对应 downloads.id）、chain_index（URL 在链中的序号）、url
        "(SELECT u.url FROM downloads_url_chains u \
         WHERE u.id = d.id \
         ORDER BY u.chain_index DESC LIMIT 1) AS url"
            .to_string()
    };
    let tab_url_expr = if has_tab_url {
        "d.tab_url".to_string()
    } else {
        "NULL AS tab_url".to_string()
    };
    let tab_referrer_url_expr = if has_tab_referrer_url {
        "d.tab_referrer_url".to_string()
    } else {
        "NULL AS tab_referrer_url".to_string()
    };
    // 始终使用别名 d，以便 WHERE 子句统一使用 d. 前缀
    let from = "downloads d";
    let where_ = if where_clause.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_clause)
    };
    format!(
        "SELECT d.id, target_path, {}, referrer, start_time, end_time, \
         total_bytes, danger_type, interrupt_reason, opened, {}, {} \
         FROM {}{}",
        url_expr, tab_url_expr, tab_referrer_url_expr, from, where_
    )
}

/// 读取指定 download id 的完整 URL 重定向链（按 chain_index ASC 排序）。
///
/// 老版 schema 无 downloads_url_chains 表时返回空 Vec。
fn read_url_chain_for_download(conn: &Connection, download_id: i64) -> Vec<String> {
    let mut chain = Vec::new();
    let sql = "SELECT url FROM downloads_url_chains WHERE id = ? ORDER BY chain_index ASC";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return chain;
    };
    let rows = stmt.query_map([download_id], |row| {
        let url: String = row.get(0)?;
        Ok(url)
    });
    if let Ok(row_iter) = rows {
        for url in row_iter.flatten() {
            chain.push(url);
        }
    }
    chain
}

/// 从 Connection 读取 downloads 表
fn read_downloads_from_conn(conn: &Connection) -> Vec<DownloadInfo> {
    let has_url = downloads_has_url_column(conn);
    let has_tab_url = downloads_has_tab_url_column(conn);
    let has_tab_referrer_url = downloads_has_tab_referrer_url_column(conn);
    let sql = build_downloads_sql(has_url, has_tab_url, has_tab_referrer_url, "");
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to prepare downloads query: {}", e);
            return vec![];
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, Option<String>>(11)?,
        ))
    });

    // 先收集原始行数据，避免在 stmt 借用 conn 期间嵌套查询 url_chain
    let raw_rows: Vec<_> = match rows {
        Ok(mapped_rows) => mapped_rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            warn!("failed to execute downloads query: {}", e);
            return vec![];
        }
    };

    raw_rows
        .into_iter()
        .map(
            |(
                id,
                target_path,
                url,
                referrer,
                start_time,
                end_time,
                total_bytes,
                danger_type,
                interrupt_reason,
                opened,
                tab_url,
                tab_referrer_url,
            )| {
                let download_url = url.unwrap_or_default();
                // 读取完整重定向链；老 schema 无 downloads_url_chains 表时回退到 [download_url]
                let mut url_chain = read_url_chain_for_download(conn, id);
                if url_chain.is_empty() {
                    url_chain = vec![download_url.clone()];
                }
                DownloadInfo {
                    filename: extract_filename(&target_path),
                    local_path: target_path,
                    download_url,
                    referrer,
                    start_time: start_time
                        .and_then(webkit_timestamp::from_webkit_micros)
                        .map(|dt| dt.to_rfc3339()),
                    end_time: end_time
                        .and_then(webkit_timestamp::from_webkit_micros)
                        .map(|dt| dt.to_rfc3339()),
                    total_bytes,
                    danger_type: danger_type_from_int(danger_type),
                    opened: opened != 0,
                    interrupt_reason: interrupt_reason_to_string(interrupt_reason),
                    evidence_type: "download".to_string(),
                    url_chain,
                    tab_url,
                    tab_referrer_url,
                }
            },
        )
        .collect()
}

/// 扫描指定 Profile 的下载记录
pub fn scan_downloads(profile: &BrowserProfile) -> DownloadAttribution {
    let db_path = profile.path.join("History");

    let conn = match open_browser_db(&db_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to open History db for {}: {}", profile.name, e);
            return DownloadAttribution {
                browser: profile.browser,
                profile: profile.name.clone(),
                downloads: vec![],
            };
        }
    };

    let downloads = read_downloads_from_conn(&conn);

    DownloadAttribution {
        browser: profile.browser,
        profile: profile.name.clone(),
        downloads,
    }
}

/// 按时间窗口筛选下载记录
pub fn scan_downloads_in_time_window(
    profile: &BrowserProfile,
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
) -> DownloadAttribution {
    let db_path = profile.path.join("History");

    let conn = match open_browser_db(&db_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to open History db for {}: {}", profile.name, e);
            return DownloadAttribution {
                browser: profile.browser,
                profile: profile.name.clone(),
                downloads: vec![],
            };
        }
    };

    let start_webkit = webkit_timestamp::to_webkit_micros(&start);
    let end_webkit = webkit_timestamp::to_webkit_micros(&end);

    let has_url = downloads_has_url_column(&conn);
    let has_tab_url = downloads_has_tab_url_column(&conn);
    let has_tab_referrer_url = downloads_has_tab_referrer_url_column(&conn);
    // 注意：WHERE 子句使用 d. 前缀以兼容带别名的 SQL
    let sql = build_downloads_sql(
        has_url,
        has_tab_url,
        has_tab_referrer_url,
        "d.start_time BETWEEN ? AND ?",
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to prepare downloads time window query: {}", e);
            return DownloadAttribution {
                browser: profile.browser,
                profile: profile.name.clone(),
                downloads: vec![],
            };
        }
    };

    let rows = stmt.query_map(rusqlite::params![start_webkit, end_webkit], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, Option<String>>(11)?,
        ))
    });

    // 先收集原始行数据，避免在 stmt 借用 conn 期间嵌套查询 url_chain
    let raw_rows: Vec<_> = match rows {
        Ok(mapped_rows) => mapped_rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            warn!("failed to execute downloads time window query: {}", e);
            return DownloadAttribution {
                browser: profile.browser,
                profile: profile.name.clone(),
                downloads: vec![],
            };
        }
    };

    let downloads = raw_rows
        .into_iter()
        .map(
            |(
                id,
                target_path,
                url,
                referrer,
                start_time,
                end_time,
                total_bytes,
                danger_type,
                interrupt_reason,
                opened,
                tab_url,
                tab_referrer_url,
            )| {
                let download_url = url.unwrap_or_default();
                let mut url_chain = read_url_chain_for_download(&conn, id);
                if url_chain.is_empty() {
                    url_chain = vec![download_url.clone()];
                }
                DownloadInfo {
                    filename: extract_filename(&target_path),
                    local_path: target_path,
                    download_url,
                    referrer,
                    start_time: start_time
                        .and_then(webkit_timestamp::from_webkit_micros)
                        .map(|dt| dt.to_rfc3339()),
                    end_time: end_time
                        .and_then(webkit_timestamp::from_webkit_micros)
                        .map(|dt| dt.to_rfc3339()),
                    total_bytes,
                    danger_type: danger_type_from_int(danger_type),
                    opened: opened != 0,
                    interrupt_reason: interrupt_reason_to_string(interrupt_reason),
                    evidence_type: "download".to_string(),
                    url_chain,
                    tab_url,
                    tab_referrer_url,
                }
            },
        )
        .collect();

    DownloadAttribution {
        browser: profile.browser,
        profile: profile.name.clone(),
        downloads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn danger_type_mapping() {
        assert_eq!(danger_type_from_int(0), DangerType::NotDangerous);
        assert_eq!(danger_type_from_int(1), DangerType::DangerousUrl);
        assert_eq!(danger_type_from_int(2), DangerType::DangerousContent);
        assert_eq!(danger_type_from_int(3), DangerType::DangerousHost);
        assert_eq!(danger_type_from_int(4), DangerType::UncommonUrl);
        assert_eq!(danger_type_from_int(5), DangerType::PotentiallyUnwanted);
        assert_eq!(danger_type_from_int(7), DangerType::AllowlistedByPolicy);
    }

    #[test]
    fn danger_type_unknown() {
        assert_eq!(danger_type_from_int(6), DangerType::Unknown);
        assert_eq!(danger_type_from_int(8), DangerType::Unknown);
        assert_eq!(danger_type_from_int(-1), DangerType::Unknown);
        assert_eq!(danger_type_from_int(99), DangerType::Unknown);
    }

    #[test]
    fn interrupt_reason_mapping() {
        assert_eq!(interrupt_reason_to_string(Some(0)), Some("NONE".to_string()));
        assert_eq!(interrupt_reason_to_string(Some(1)), Some("FILE_FAILED".to_string()));
        assert_eq!(interrupt_reason_to_string(Some(20)), Some("NETWORK_FAILED".to_string()));
        assert_eq!(interrupt_reason_to_string(Some(40)), Some("USER_CANCELED".to_string()));
        assert_eq!(interrupt_reason_to_string(Some(99)), Some("UNKNOWN(99)".to_string()));
        assert_eq!(interrupt_reason_to_string(None), None);
    }

    #[test]
    fn extract_filename_simple() {
        assert_eq!(extract_filename(r"C:\Users\test\Downloads\file.exe"), "file.exe");
    }

    #[test]
    fn extract_filename_unix_path() {
        assert_eq!(extract_filename("/home/user/Downloads/archive.zip"), "archive.zip");
    }

    #[test]
    fn extract_filename_no_directory() {
        assert_eq!(extract_filename("file.exe"), "file.exe");
    }

    #[test]
    fn extract_filename_trailing_slash() {
        // 路径以 / 结尾时，file_name 返回 None（Windows）或最后一个目录名
        // 行为因平台而异，只验证不 panic 且返回非空字符串
        let result = extract_filename("/some/dir/");
        assert!(!result.is_empty());
    }

    // ── 集成测试：使用内存 SQLite 数据库 ──────────────────────

    /// 创建包含测试数据的内存 downloads 数据库
    fn create_test_downloads_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE downloads (\
             id INTEGER PRIMARY KEY, \
             target_path TEXT, \
             url TEXT, \
             referrer TEXT, \
             start_time INTEGER, \
             end_time INTEGER, \
             total_bytes INTEGER, \
             danger_type INTEGER, \
             interrupt_reason INTEGER, \
             opened INTEGER\
             );",
        )
        .unwrap();

        // 基准时间：2024-06-15 12:00:00 UTC
        let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

        // 下载 1: 正常文件，T+0s
        conn.execute(
            "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
             VALUES (1, 'C:\\Users\\test\\Downloads\\report.pdf', 'https://example.com/report.pdf', 'https://example.com/docs', ?1, ?2, 1024000, 0, NULL, 1)",
            rusqlite::params![base_webkit, base_webkit + 5_000_000],
        ).unwrap();

        // 下载 2: 危险文件，T+10s
        conn.execute(
            "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
             VALUES (2, 'C:\\Users\\test\\Downloads\\malware.exe', 'https://evil.com/payload.exe', 'https://evil.com/page', ?1, ?2, 2048000, 2, NULL, 0)",
            rusqlite::params![base_webkit + 10_000_000, base_webkit + 15_000_000],
        ).unwrap();

        // 下载 3: 不常见 URL，T+60s
        conn.execute(
            "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
             VALUES (3, 'C:\\Users\\test\\Downloads\\tool.zip', 'https://uncommon.com/tool.zip', NULL, ?1, NULL, 512000, 4, 1, 0)",
            rusqlite::params![base_webkit + 60_000_000],
        ).unwrap();

        conn
    }

    #[test]
    fn integration_read_downloads_from_conn() {
        let conn = create_test_downloads_db();
        let downloads = read_downloads_from_conn(&conn);

        assert_eq!(downloads.len(), 3);

        // 验证下载 1
        let d1 = &downloads[0];
        assert_eq!(d1.filename, "report.pdf");
        assert_eq!(d1.local_path, r"C:\Users\test\Downloads\report.pdf");
        assert_eq!(d1.download_url, "https://example.com/report.pdf");
        assert_eq!(d1.referrer, Some("https://example.com/docs".to_string()));
        assert!(d1.start_time.is_some());
        assert!(d1.end_time.is_some());
        assert_eq!(d1.total_bytes, Some(1024000));
        assert_eq!(d1.danger_type, DangerType::NotDangerous);
        assert!(d1.opened);
        assert!(d1.interrupt_reason.is_none());
        assert_eq!(d1.evidence_type, "download");

        // 验证下载 2
        let d2 = &downloads[1];
        assert_eq!(d2.filename, "malware.exe");
        assert_eq!(d2.danger_type, DangerType::DangerousContent);
        assert!(!d2.opened);

        // 验证下载 3
        let d3 = &downloads[2];
        assert_eq!(d3.filename, "tool.zip");
        assert_eq!(d3.danger_type, DangerType::UncommonUrl);
        assert!(d3.end_time.is_none());
        assert_eq!(d3.referrer, None);
        assert_eq!(d3.interrupt_reason, Some("FILE_FAILED".to_string()));
    }

    #[test]
    fn integration_scan_downloads_structure() {
        // 创建临时目录和 History 数据库文件
        let temp_dir = std::env::temp_dir().join("irtool-download-test-structure");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 创建 Preferences 文件以使目录成为有效 Profile
        std::fs::write(temp_dir.join("Preferences"), "{}").unwrap();

        let db_path = temp_dir.join("History");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE downloads (\
                 id INTEGER PRIMARY KEY, \
                 target_path TEXT, \
                 url TEXT, \
                 referrer TEXT, \
                 start_time INTEGER, \
                 end_time INTEGER, \
                 total_bytes INTEGER, \
                 danger_type INTEGER, \
                 interrupt_reason INTEGER, \
                 opened INTEGER\
                 );",
            )
            .unwrap();

            let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
            let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

            conn.execute(
                "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
                 VALUES (1, 'C:\\Downloads\\test.exe', 'https://example.com/test.exe', 'https://example.com', ?1, ?2, 5000, 0, NULL, 0)",
                rusqlite::params![base_webkit, base_webkit + 1_000_000],
            ).unwrap();
        }

        let profile = BrowserProfile {
            browser: crate::core::BrowserKind::Chrome,
            name: "TestProfile".to_string(),
            display_name: None,
            path: temp_dir.clone(),
        };

        let result = scan_downloads(&profile);

        assert_eq!(result.browser, crate::core::BrowserKind::Chrome);
        assert_eq!(result.profile, "TestProfile");
        assert_eq!(result.downloads.len(), 1);
        assert_eq!(result.downloads[0].filename, "test.exe");
        assert_eq!(result.downloads[0].download_url, "https://example.com/test.exe");
        assert_eq!(result.downloads[0].evidence_type, "download");

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn integration_time_window_filtering() {
        // 创建临时目录和 History 数据库文件
        let temp_dir = std::env::temp_dir().join("irtool-download-test-window");
        // 清理上次可能残留的目录，避免 CREATE TABLE already exists
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("Preferences"), "{}").unwrap();

        let db_path = temp_dir.join("History");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE downloads (\
                 id INTEGER PRIMARY KEY, \
                 target_path TEXT, \
                 url TEXT, \
                 referrer TEXT, \
                 start_time INTEGER, \
                 end_time INTEGER, \
                 total_bytes INTEGER, \
                 danger_type INTEGER, \
                 interrupt_reason INTEGER, \
                 opened INTEGER\
                 );",
            )
            .unwrap();

            let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
            let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

            // 下载 1: T+0s
            conn.execute(
                "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
                 VALUES (1, 'C:\\Downloads\\a.pdf', 'https://a.com/a.pdf', NULL, ?1, NULL, 1000, 0, NULL, 0)",
                rusqlite::params![base_webkit],
            ).unwrap();

            // 下载 2: T+30s
            conn.execute(
                "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
                 VALUES (2, 'C:\\Downloads\\b.exe', 'https://b.com/b.exe', NULL, ?1, NULL, 2000, 0, NULL, 0)",
                rusqlite::params![base_webkit + 30_000_000],
            ).unwrap();

            // 下载 3: T+120s
            conn.execute(
                "INSERT INTO downloads (id, target_path, url, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
                 VALUES (3, 'C:\\Downloads\\c.zip', 'https://c.com/c.zip', NULL, ?1, NULL, 3000, 0, NULL, 0)",
                rusqlite::params![base_webkit + 120_000_000],
            ).unwrap();
        }

        let profile = BrowserProfile {
            browser: crate::core::BrowserKind::Chrome,
            name: "WindowTest".to_string(),
            display_name: None,
            path: temp_dir.clone(),
        };

        // 时间窗口：T-10s ~ T+60s，应包含下载 1 和 2
        let start = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 11, 59, 50).unwrap();
        let end = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 1, 0).unwrap();

        let result = scan_downloads_in_time_window(&profile, start, end);

        assert_eq!(result.downloads.len(), 2);
        let urls: Vec<&str> = result.downloads.iter().map(|d| d.download_url.as_str()).collect();
        assert!(urls.contains(&"https://a.com/a.pdf"));
        assert!(urls.contains(&"https://b.com/b.exe"));
        assert!(!urls.contains(&"https://c.com/c.zip"));

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn integration_empty_downloads_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE downloads (\
             id INTEGER PRIMARY KEY, \
             target_path TEXT, \
             url TEXT, \
             referrer TEXT, \
             start_time INTEGER, \
             end_time INTEGER, \
             total_bytes INTEGER, \
             danger_type INTEGER, \
             interrupt_reason INTEGER, \
             opened INTEGER\
             );",
        )
        .unwrap();

        let downloads = read_downloads_from_conn(&conn);
        assert!(downloads.is_empty());
    }

    #[test]
    fn integration_missing_downloads_table() {
        let conn = Connection::open_in_memory().unwrap();
        // 不创建 downloads 表
        let downloads = read_downloads_from_conn(&conn);
        assert!(downloads.is_empty());
    }

    /// 验证新版 Chromium schema（downloads 表无 url 列，URL 在 downloads_url_chains）
    #[test]
    fn integration_new_schema_url_in_chains() {
        let conn = Connection::open_in_memory().unwrap();
        // 新版 schema：downloads 表无 url 列
        conn.execute_batch(
            "CREATE TABLE downloads (\
             id INTEGER PRIMARY KEY, \
             target_path TEXT, \
             referrer TEXT, \
             start_time INTEGER, \
             end_time INTEGER, \
             total_bytes INTEGER, \
             danger_type INTEGER, \
             interrupt_reason INTEGER, \
             opened INTEGER\
             );\
             CREATE TABLE downloads_url_chains (\
             id INTEGER NOT NULL, \
             chain_index INTEGER NOT NULL, \
             url TEXT NOT NULL, \
             PRIMARY KEY (id, chain_index)\
             );",
        )
        .unwrap();

        let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

        // 下载 1：带重定向链（chain_index 0 是原始 URL，chain_index 1 是最终 URL）
        conn.execute(
            "INSERT INTO downloads (id, target_path, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
             VALUES (1, 'C:\\Users\\test\\Downloads\\file.pdf', 'https://example.com/docs', ?1, ?2, 1024, 0, NULL, 1)",
            rusqlite::params![base_webkit, base_webkit + 1_000_000],
        ).unwrap();
        conn.execute(
            "INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (1, 0, 'https://redirect.example.com/file.pdf')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (1, 1, 'https://final.example.com/file.pdf')",
            [],
        ).unwrap();

        // 验证 url 列检测
        assert!(!downloads_has_url_column(&conn));

        let downloads = read_downloads_from_conn(&conn);
        assert_eq!(downloads.len(), 1);
        let d = &downloads[0];
        assert_eq!(d.filename, "file.pdf");
        // 应取 url_index 最大的最终 URL
        assert_eq!(d.download_url, "https://final.example.com/file.pdf");
        assert_eq!(d.referrer, Some("https://example.com/docs".to_string()));
        assert!(d.opened);
        // url_chain 应包含完整跳转链（chain_index 0 和 1）
        assert_eq!(d.url_chain.len(), 2);
        assert_eq!(d.url_chain[0], "https://redirect.example.com/file.pdf");
        assert_eq!(d.url_chain[1], "https://final.example.com/file.pdf");
    }

    /// 创建包含 downloads_url_chains 表的测试数据库（新版 schema，无 url 列）
    fn create_test_url_chains_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE downloads (\
             id INTEGER PRIMARY KEY, \
             target_path TEXT, \
             referrer TEXT, \
             start_time INTEGER, \
             end_time INTEGER, \
             total_bytes INTEGER, \
             danger_type INTEGER, \
             interrupt_reason INTEGER, \
             opened INTEGER\
             );\
             CREATE TABLE downloads_url_chains (\
             id INTEGER NOT NULL, \
             chain_index INTEGER NOT NULL, \
             url TEXT NOT NULL, \
             PRIMARY KEY (id, chain_index)\
             );",
        )
        .unwrap();

        let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

        // 下载 1：3 段重定向链（chain_index 0/1/2）
        conn.execute(
            "INSERT INTO downloads (id, target_path, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened) \
             VALUES (1, 'C:\\Users\\test\\Downloads\\file.zip', NULL, ?1, ?2, 1024, 0, NULL, 0)",
            rusqlite::params![base_webkit, base_webkit + 1_000_000],
        ).unwrap();
        conn.execute(
            "INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (1, 0, 'https://start.example.com/file.zip')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (1, 1, 'https://redirect.example.com/file.zip')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (1, 2, 'https://final.example.com/file.zip')",
            [],
        ).unwrap();

        conn
    }

    #[test]
    fn read_url_chain_returns_complete_chain() {
        let conn = create_test_url_chains_db();
        let chain = read_url_chain_for_download(&conn, 1);
        assert_eq!(chain.len(), 3);
        // 按 chain_index ASC 排序
        assert_eq!(chain[0], "https://start.example.com/file.zip");
        assert_eq!(chain[1], "https://redirect.example.com/file.zip");
        assert_eq!(chain[2], "https://final.example.com/file.zip");
    }

    #[test]
    fn read_url_chain_empty_for_missing_id() {
        let conn = create_test_url_chains_db();
        // 不存在的 download id 返回空 vec
        let chain = read_url_chain_for_download(&conn, 999);
        assert!(chain.is_empty());
    }

    /// 创建包含 tab_url/tab_referrer_url 列的测试数据库（新版 schema）
    fn create_test_downloads_db_with_tab_columns() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE downloads (\
             id INTEGER PRIMARY KEY, \
             target_path TEXT, \
             referrer TEXT, \
             start_time INTEGER, \
             end_time INTEGER, \
             total_bytes INTEGER, \
             danger_type INTEGER, \
             interrupt_reason INTEGER, \
             opened INTEGER, \
             tab_url TEXT, \
             tab_referrer_url TEXT\
             );\
             CREATE TABLE downloads_url_chains (\
             id INTEGER NOT NULL, \
             chain_index INTEGER NOT NULL, \
             url TEXT NOT NULL, \
             PRIMARY KEY (id, chain_index)\
             );",
        )
        .unwrap();

        let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

        conn.execute(
            "INSERT INTO downloads (id, target_path, referrer, start_time, end_time, total_bytes, danger_type, interrupt_reason, opened, tab_url, tab_referrer_url) \
             VALUES (1, 'C:\\Users\\test\\Downloads\\doc.pdf', NULL, ?1, ?2, 1024, 0, NULL, 0, 'https://tab.example.com/page', 'https://ref.example.com/prev')",
            rusqlite::params![base_webkit, base_webkit + 1_000_000],
        ).unwrap();
        // 单段链（无重定向）
        conn.execute(
            "INSERT INTO downloads_url_chains (id, chain_index, url) VALUES (1, 0, 'https://example.com/doc.pdf')",
            [],
        )
        .unwrap();

        conn
    }

    #[test]
    fn download_info_has_tab_url_when_column_exists() {
        let conn = create_test_downloads_db_with_tab_columns();
        let downloads = read_downloads_from_conn(&conn);
        assert_eq!(downloads.len(), 1);
        let d = &downloads[0];
        assert_eq!(d.tab_url, Some("https://tab.example.com/page".to_string()));
        assert_eq!(d.tab_referrer_url, Some("https://ref.example.com/prev".to_string()));
    }

    #[test]
    fn download_info_tab_url_none_when_column_missing() {
        // 老版 schema（create_test_downloads_db 无 tab_url/tab_referrer_url 列）
        let conn = create_test_downloads_db();
        let downloads = read_downloads_from_conn(&conn);
        assert_eq!(downloads.len(), 3);
        for d in &downloads {
            assert!(d.tab_url.is_none(), "tab_url 应为 None（老 schema）");
            assert!(d.tab_referrer_url.is_none(), "tab_referrer_url 应为 None（老 schema）");
        }
    }
}
