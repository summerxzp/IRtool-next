//! History 分层时间窗关联 + Navigation Chain 重建

use crate::core::webkit_timestamp;
use crate::profile::BrowserProfile;
use crate::sqlite::open_browser_db;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{debug, warn};

/// 时间窗层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum TimeTier {
    /// ±5s，极高概率关联
    Immediate,
    /// ±15s，较高概率关联
    Nearby,
    /// ±30s，参考性关联
    Recent,
}

/// 近期浏览器活动记录
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RecentActivity {
    pub url: String,
    pub title: String,
    pub visit_time: String,
    pub tier: TimeTier,
    pub time_distance_ms: i64,
    pub evidence_type: String,
}

/// 历史记录条目（用于 scan_history 返回）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_time: String,
    pub visit_count: i64,
}

/// Navigation Chain 节点
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NavChainNode {
    pub url: String,
    pub title: Option<String>,
    pub transition: Option<String>,
}

/// History 关联结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HistoryAttribution {
    pub browser: crate::core::BrowserKind,
    pub profile: String,
    pub recent_browser_activity: Vec<RecentActivity>,
    pub navigation_chain: Vec<NavChainNode>,
}

/// 将 Chromium transition 位掩码转为可读字符串
///
/// Chromium 使用位掩码：低 5 位为基本类型，高位为限定符。
/// 参考：chromium/src/ui/base/page_transition_types.h
pub fn transition_to_string(raw_transition: i64) -> String {
    let base = raw_transition & 0x1F; // 低 5 位
    match base {
        0 => "LINK",
        1 => "TYPED",
        2 => "AUTO_BOOKMARK",
        3 => "FORM_SUBMIT",
        4 => "REDIRECT",
        5 => "RELOAD",
        _ => "UNKNOWN",
    }
    .to_string()
}

/// 通过 `from_visit` 递归回溯跳转链
///
/// 递归深度限制为 10 层，防止无限循环。
pub fn build_navigation_chain(conn: &Connection, visit_id: i64) -> Vec<NavChainNode> {
    let mut chain = Vec::new();
    let mut current_id = visit_id;
    let max_depth = 10;

    for _ in 0..max_depth {
        let mut stmt = match conn.prepare(
            "SELECT u.url, u.title, v.from_visit, v.transition \
             FROM visits v \
             JOIN urls u ON v.url = u.id \
             WHERE v.id = ?",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to prepare nav chain query: {}", e);
                break;
            }
        };

        let result = stmt.query_row(rusqlite::params![current_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        });

        match result {
            Ok((url, title, from_visit, transition)) => {
                chain.push(NavChainNode {
                    url,
                    title,
                    transition: Some(transition_to_string(transition)),
                });
                if from_visit == 0 {
                    break;
                }
                current_id = from_visit;
            }
            Err(e) => {
                debug!("nav chain backtrack stopped at id={}: {}", current_id, e);
                break;
            }
        }
    }

    chain
}

/// 分层时间窗定义
const TIER_WINDOWS: &[(TimeTier, i64)] = &[
    (TimeTier::Immediate, 5_000_000), // ±5s in microseconds
    (TimeTier::Nearby, 15_000_000),   // ±15s in microseconds
    (TimeTier::Recent, 30_000_000),   // ±30s in microseconds
];

/// 每层最大返回条数
const TIER_LIMIT: i64 = 5;

/// History 列表结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HistoryList {
    pub browser: crate::core::BrowserKind,
    pub profile: String,
    pub entries: Vec<HistoryEntry>,
}

/// 扫描指定 Profile 的最近历史记录
///
/// 返回最近的 N 条历史记录（不限时间窗口），供 UI 表格展示使用。
pub fn scan_history(profile: &BrowserProfile, limit: i64) -> HistoryList {
    let db_path = profile.path.join("History");

    let conn = match open_browser_db(&db_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to open History db for {}: {}", profile.name, e);
            return HistoryList {
                browser: profile.browser,
                profile: profile.name.clone(),
                entries: vec![],
            };
        }
    };

    let sql = "SELECT u.url, u.title, v.visit_time, u.visit_count \
         FROM visits v \
         JOIN urls u ON v.url = u.id \
         ORDER BY v.visit_time DESC \
         LIMIT ?".to_string();

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to prepare history scan query: {}", e);
            return HistoryList {
                browser: profile.browser,
                profile: profile.name.clone(),
                entries: vec![],
            };
        }
    };

    let rows = stmt.query_map([limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    });

    let entries = match rows {
        Ok(mapped_rows) => mapped_rows
            .filter_map(|r| {
                r.ok().map(|(url, title, visit_time, visit_count)| HistoryEntry {
                    url,
                    title,
                    visit_time: webkit_timestamp::from_webkit_micros(visit_time)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default(),
                    visit_count,
                })
            })
            .collect(),
        Err(e) => {
            warn!("failed to execute history scan query: {}", e);
            vec![]
        }
    };

    HistoryList {
        browser: profile.browser,
        profile: profile.name.clone(),
        entries,
    }
}

/// 对指定 Profile 执行 History 时间窗关联
pub fn attribute_history(profile: &BrowserProfile, target_time: chrono::DateTime<chrono::Utc>) -> HistoryAttribution {
    let db_path = profile.path.join("History");

    let conn = match open_browser_db(&db_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to open History db for {}: {}", profile.name, e);
            return HistoryAttribution {
                browser: profile.browser,
                profile: profile.name.clone(),
                recent_browser_activity: vec![],
                navigation_chain: vec![],
            };
        }
    };

    let target_webkit = webkit_timestamp::to_webkit_micros(&target_time);

    let mut all_activities = Vec::new();
    let mut best_visit_id: Option<i64> = None;

    for (tier, window_micros) in TIER_WINDOWS {
        let lower = target_webkit - window_micros;
        let upper = target_webkit + window_micros;

        let sql = format!(
            "SELECT u.url, u.title, v.visit_time, v.id \
             FROM visits v \
             JOIN urls u ON v.url = u.id \
             WHERE v.visit_time BETWEEN ? AND ? \
             ORDER BY ABS(v.visit_time - ?) ASC \
             LIMIT {}",
            TIER_LIMIT
        );

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to prepare tier query: {}", e);
                continue;
            }
        };

        let rows = stmt.query_map(rusqlite::params![lower, upper, target_webkit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        });

        match rows {
            Ok(mapped_rows) => {
                for row_result in mapped_rows {
                    match row_result {
                        Ok((url, title, visit_time, visit_id)) => {
                            let distance_micros = (visit_time - target_webkit).abs();
                            let distance_ms = distance_micros / 1_000;

                            all_activities.push(RecentActivity {
                                url,
                                title,
                                visit_time: webkit_timestamp::from_webkit_micros(visit_time)
                                    .map(|dt| dt.to_rfc3339())
                                    .unwrap_or_default(),
                                tier: *tier,
                                time_distance_ms: distance_ms,
                                evidence_type: "recent-visit".to_string(),
                            });

                            // 取最近的 visit_id 作为 navigation chain 起点
                            // 优先从 Tier 1 (Immediate) 取第一条（order by abs 保证最近）；
                            // 若 Tier 1 无命中，则依次 fallback 到 Tier 2 (Nearby)、Tier 3 (Recent)
                            if best_visit_id.is_none() {
                                best_visit_id = Some(visit_id);
                            }
                        }
                        Err(e) => {
                            warn!("failed to read visit row: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("failed to execute tier query: {}", e);
            }
        }
    }

    // 构建 Navigation Chain：从最近（Tier 1 → Tier 2 → Tier 3 依次 fallback）的记录开始回溯
    let navigation_chain = match best_visit_id {
        Some(vid) => build_navigation_chain(&*conn, vid),
        None => vec![],
    };

    HistoryAttribution {
        browser: profile.browser,
        profile: profile.name.clone(),
        recent_browser_activity: all_activities,
        navigation_chain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn transition_link() {
        assert_eq!(transition_to_string(0), "LINK");
    }

    #[test]
    fn transition_typed() {
        assert_eq!(transition_to_string(1), "TYPED");
    }

    #[test]
    fn transition_auto_bookmark() {
        assert_eq!(transition_to_string(2), "AUTO_BOOKMARK");
    }

    #[test]
    fn transition_form_submit() {
        assert_eq!(transition_to_string(3), "FORM_SUBMIT");
    }

    #[test]
    fn transition_redirect() {
        assert_eq!(transition_to_string(4), "REDIRECT");
    }

    #[test]
    fn transition_reload() {
        assert_eq!(transition_to_string(5), "RELOAD");
    }

    #[test]
    fn transition_with_qualifier_bits() {
        // Chromium 使用位掩码，低 5 位为基本类型，高位为限定符
        // 0x00000080 = CLIENT_REDIRECT 限定符，低 5 位仍为 0 (LINK)
        assert_eq!(transition_to_string(0x80), "LINK");
        // 0x00000100 = SERVER_REDIRECT 限定符，低 5 位为 4 (REDIRECT)
        assert_eq!(transition_to_string(0x104), "REDIRECT");
    }

    #[test]
    fn transition_unknown() {
        assert_eq!(transition_to_string(6), "UNKNOWN");
        assert_eq!(transition_to_string(31), "UNKNOWN");
    }

    #[test]
    fn webkit_timestamp_tier_window_calculation() {
        // 验证 WebKit 时间戳与分层窗口的正确计算
        let dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let webkit = webkit_timestamp::to_webkit_micros(&dt);

        // Tier 1: ±5s = ±5_000_000 微秒
        let tier1_lower = webkit - 5_000_000;
        let tier1_upper = webkit + 5_000_000;
        let lower_dt = webkit_timestamp::from_webkit_micros(tier1_lower).unwrap();
        let upper_dt = webkit_timestamp::from_webkit_micros(tier1_upper).unwrap();
        assert_eq!((upper_dt - lower_dt).num_seconds(), 10);

        // Tier 2: ±15s = ±15_000_000 微秒
        let tier2_lower = webkit - 15_000_000;
        let tier2_upper = webkit + 15_000_000;
        let lower_dt2 = webkit_timestamp::from_webkit_micros(tier2_lower).unwrap();
        let upper_dt2 = webkit_timestamp::from_webkit_micros(tier2_upper).unwrap();
        assert_eq!((upper_dt2 - lower_dt2).num_seconds(), 30);

        // Tier 3: ±30s = ±30_000_000 微秒
        let tier3_lower = webkit - 30_000_000;
        let tier3_upper = webkit + 30_000_000;
        let lower_dt3 = webkit_timestamp::from_webkit_micros(tier3_lower).unwrap();
        let upper_dt3 = webkit_timestamp::from_webkit_micros(tier3_upper).unwrap();
        assert_eq!((upper_dt3 - lower_dt3).num_seconds(), 60);
    }

    #[test]
    fn nav_chain_depth_limit() {
        // 使用内存数据库测试递归深度限制
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT); \
             CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER, \
             from_visit INTEGER, transition INTEGER, referrer INTEGER);",
        )
        .unwrap();

        // 插入 15 条链式记录，超过 10 层限制
        for i in 1..=15 {
            conn.execute(
                "INSERT INTO urls (id, url, title) VALUES (?1, ?2, ?3)",
                rusqlite::params![i, format!("https://page{}.com", i), format!("Page {}", i)],
            )
            .unwrap();
            let from_visit = if i > 1 { i - 1 } else { 0 };
            conn.execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![i, i, 1000000 + i * 1000, from_visit, 0],
            )
            .unwrap();
        }

        let chain = build_navigation_chain(&conn, 15);
        // 应该被限制在 10 层
        assert_eq!(chain.len(), 10);
        // 从 visit 15 开始回溯：15 -> 14 -> 13 -> ... -> 6
        assert_eq!(chain[0].url, "https://page15.com");
        assert_eq!(chain[9].url, "https://page6.com");
    }

    #[test]
    fn nav_chain_stops_at_zero_from_visit() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT); \
             CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER, \
             from_visit INTEGER, transition INTEGER, referrer INTEGER);",
        )
        .unwrap();

        // visit 1: from_visit = 0 (链起点)
        conn.execute(
            "INSERT INTO urls (id, url, title) VALUES (1, 'https://start.com', 'Start')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (1, 1, 1000000, 0, 1)",
            [],
        )
        .unwrap();

        // visit 2: from_visit = 1
        conn.execute(
            "INSERT INTO urls (id, url, title) VALUES (2, 'https://next.com', 'Next')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (2, 2, 2000000, 1, 0)",
            [],
        )
        .unwrap();

        let chain = build_navigation_chain(&conn, 2);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].url, "https://next.com");
        assert_eq!(chain[0].transition, Some("LINK".to_string()));
        assert_eq!(chain[1].url, "https://start.com");
        assert_eq!(chain[1].transition, Some("TYPED".to_string()));
    }

    // ── 集成测试：使用内存 SQLite 数据库 ──────────────────────────

    /// 创建包含测试数据的内存 History 数据库
    fn create_test_history_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT, \
             visit_count INTEGER, last_visit_time INTEGER); \
             CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER, \
             from_visit INTEGER, transition INTEGER, referrer INTEGER);",
        )
        .unwrap();

        // 基准时间：2024-06-15 12:00:00 UTC
        let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

        // urls
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (1, 'https://search.com/results', 'Search Results', 5, ?1)",
            rusqlite::params![base_webkit],
        ).unwrap();
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (2, 'https://forum.example.com/thread', 'Forum Thread', 3, ?1)",
            rusqlite::params![base_webkit + 3_000_000], // +3s
        ).unwrap();
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (3, 'https://evil.com/payload', 'Malicious', 1, ?1)",
            rusqlite::params![base_webkit + 4_000_000], // +4s
        ).unwrap();
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (4, 'https://news.example.com', 'News', 10, ?1)",
            rusqlite::params![base_webkit + 12_000_000], // +12s
        ).unwrap();
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (5, 'https://shop.example.com', 'Shop', 2, ?1)",
            rusqlite::params![base_webkit + 25_000_000], // +25s
        ).unwrap();

        // visits
        // visit 1: search at T+0s, from_visit=0 (链起点)
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (1, 1, ?1, 0, 1)",
            rusqlite::params![base_webkit],
        )
        .unwrap();
        // visit 2: forum at T+3s, from visit 1 (点击搜索结果)
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (2, 2, ?1, 1, 0)",
            rusqlite::params![base_webkit + 3_000_000],
        )
        .unwrap();
        // visit 3: evil at T+4s, from visit 2 (页面内脚本)
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (3, 3, ?1, 2, 4)",
            rusqlite::params![base_webkit + 4_000_000],
        )
        .unwrap();
        // visit 4: news at T+12s, from_visit=0
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (4, 4, ?1, 0, 1)",
            rusqlite::params![base_webkit + 12_000_000],
        )
        .unwrap();
        // visit 5: shop at T+25s, from_visit=0
        conn.execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (5, 5, ?1, 0, 1)",
            rusqlite::params![base_webkit + 25_000_000],
        )
        .unwrap();

        conn
    }

    #[test]
    fn integration_tier_assignment() {
        let conn = create_test_history_db();

        let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let target_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

        // 查询 Tier 1 (±5s): 应包含 visit 1 (0s), 2 (3s), 3 (4s)
        let mut stmt = conn
            .prepare(
                "SELECT u.url, v.visit_time \
             FROM visits v \
             JOIN urls u ON v.url = u.id \
             WHERE v.visit_time BETWEEN ? AND ? \
             ORDER BY ABS(v.visit_time - ?) ASC \
             LIMIT 5",
            )
            .unwrap();

        let tier1_lower = target_webkit - 5_000_000;
        let tier1_upper = target_webkit + 5_000_000;
        let rows: Vec<(String, i64)> = stmt
            .query_map(rusqlite::params![tier1_lower, tier1_upper, target_webkit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(rows.len(), 3);

        // 查询 Tier 2 (±15s): 应额外包含 visit 4 (12s)
        let tier2_lower = target_webkit - 15_000_000;
        let tier2_upper = target_webkit + 15_000_000;
        let mut stmt2 = conn
            .prepare(
                "SELECT u.url, v.visit_time \
             FROM visits v \
             JOIN urls u ON v.url = u.id \
             WHERE v.visit_time BETWEEN ? AND ? \
             ORDER BY ABS(v.visit_time - ?) ASC \
             LIMIT 5",
            )
            .unwrap();
        let rows2: Vec<(String, i64)> = stmt2
            .query_map(rusqlite::params![tier2_lower, tier2_upper, target_webkit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(rows2.len(), 4);

        // 查询 Tier 3 (±30s): 应包含全部 5 条
        let tier3_lower = target_webkit - 30_000_000;
        let tier3_upper = target_webkit + 30_000_000;
        let mut stmt3 = conn
            .prepare(
                "SELECT u.url, v.visit_time \
             FROM visits v \
             JOIN urls u ON v.url = u.id \
             WHERE v.visit_time BETWEEN ? AND ? \
             ORDER BY ABS(v.visit_time - ?) ASC \
             LIMIT 5",
            )
            .unwrap();
        let rows3: Vec<(String, i64)> = stmt3
            .query_map(rusqlite::params![tier3_lower, tier3_upper, target_webkit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(rows3.len(), 5);
    }

    #[test]
    fn integration_navigation_chain() {
        let conn = create_test_history_db();

        // 从 visit 3 (evil.com) 回溯
        let chain = build_navigation_chain(&conn, 3);
        assert_eq!(chain.len(), 3);
        // evil.com -> forum.example.com -> search.com
        assert_eq!(chain[0].url, "https://evil.com/payload");
        assert_eq!(chain[0].transition, Some("REDIRECT".to_string()));
        assert_eq!(chain[1].url, "https://forum.example.com/thread");
        assert_eq!(chain[1].transition, Some("LINK".to_string()));
        assert_eq!(chain[2].url, "https://search.com/results");
        assert_eq!(chain[2].transition, Some("TYPED".to_string()));
    }

    #[test]
    fn integration_attribute_history_structure() {
        // 创建临时目录和 History 数据库文件
        let temp_dir = std::env::temp_dir().join("irtool-history-test-structure");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 创建 Preferences 文件以使目录成为有效 Profile
        std::fs::write(temp_dir.join("Preferences"), "{}").unwrap();

        let db_path = temp_dir.join("History");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT, \
                 visit_count INTEGER, last_visit_time INTEGER); \
                 CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER, \
                 from_visit INTEGER, transition INTEGER, referrer INTEGER);",
            )
            .unwrap();

            let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
            let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

            conn.execute(
                "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (1, 'https://example.com', 'Example', 1, ?1)",
                rusqlite::params![base_webkit],
            ).unwrap();
            conn.execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (1, 1, ?1, 0, 1)",
                rusqlite::params![base_webkit],
            )
            .unwrap();
        }

        let profile = BrowserProfile {
            browser: crate::core::BrowserKind::Chrome,
            name: "TestProfile".to_string(),
            path: temp_dir.clone(),
        };

        let target_time = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_history(&profile, target_time);

        assert_eq!(result.browser, crate::core::BrowserKind::Chrome);
        assert_eq!(result.profile, "TestProfile");
        assert!(!result.recent_browser_activity.is_empty());
        assert_eq!(result.recent_browser_activity[0].url, "https://example.com");
        assert_eq!(result.recent_browser_activity[0].tier, TimeTier::Immediate);
        assert_eq!(result.recent_browser_activity[0].evidence_type, "recent-visit");
        assert!(!result.navigation_chain.is_empty());

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn integration_tier_assignment_correctness() {
        // 验证分层窗口的记录归属正确
        let temp_dir = std::env::temp_dir().join("irtool-history-test-tiers");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("Preferences"), "{}").unwrap();

        let db_path = temp_dir.join("History");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT, \
                 visit_count INTEGER, last_visit_time INTEGER); \
                 CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER, \
                 from_visit INTEGER, transition INTEGER, referrer INTEGER);",
            )
            .unwrap();

            let base_dt = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
            let base_webkit = webkit_timestamp::to_webkit_micros(&base_dt);

            // url 1: T+2s → Tier 1 (Immediate)
            conn.execute(
                "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (1, 'https://immediate.com', 'Imm', 1, ?1)",
                rusqlite::params![base_webkit + 2_000_000],
            ).unwrap();
            conn.execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (1, 1, ?1, 0, 0)",
                rusqlite::params![base_webkit + 2_000_000],
            )
            .unwrap();

            // url 2: T+10s → Tier 2 (Nearby)
            conn.execute(
                "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (2, 'https://nearby.com', 'Near', 1, ?1)",
                rusqlite::params![base_webkit + 10_000_000],
            ).unwrap();
            conn.execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (2, 2, ?1, 0, 0)",
                rusqlite::params![base_webkit + 10_000_000],
            )
            .unwrap();

            // url 3: T+20s → Tier 3 (Recent)
            conn.execute(
                "INSERT INTO urls (id, url, title, visit_count, last_visit_time) VALUES (3, 'https://recent.com', 'Rec', 1, ?1)",
                rusqlite::params![base_webkit + 20_000_000],
            ).unwrap();
            conn.execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition) VALUES (3, 3, ?1, 0, 0)",
                rusqlite::params![base_webkit + 20_000_000],
            )
            .unwrap();
        }

        let profile = BrowserProfile {
            browser: crate::core::BrowserKind::Chrome,
            name: "TierTest".to_string(),
            path: temp_dir.clone(),
        };

        let target_time = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_history(&profile, target_time);

        // 验证各记录的 tier 归属
        let imm = result
            .recent_browser_activity
            .iter()
            .find(|a| a.url == "https://immediate.com");
        let near = result
            .recent_browser_activity
            .iter()
            .find(|a| a.url == "https://nearby.com");
        let rec = result
            .recent_browser_activity
            .iter()
            .find(|a| a.url == "https://recent.com");

        assert!(imm.is_some(), "should find immediate entry");
        assert!(near.is_some(), "should find nearby entry");
        assert!(rec.is_some(), "should find recent entry");

        assert_eq!(imm.unwrap().tier, TimeTier::Immediate);
        assert_eq!(near.unwrap().tier, TimeTier::Nearby);
        assert_eq!(rec.unwrap().tier, TimeTier::Recent);

        // 验证时间距离
        assert_eq!(imm.unwrap().time_distance_ms, 2000);
        assert_eq!(near.unwrap().time_distance_ms, 10000);
        assert_eq!(rec.unwrap().time_distance_ms, 20000);

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
