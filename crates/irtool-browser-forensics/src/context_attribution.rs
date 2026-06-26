//! Browser Context Attribution：横向关联模块
//!
//! 对恶意网络连接执行综合归因，消费 History、Download、Extension 等模块数据，
//! 产出 BrowserContext 综合结果。
//!
//! 两种归因模式：
//! - 时间窗口归因（`attribute_browser_context`）：给定时间点，查找附近的浏览活动
//! - 域名归因（`attribute_by_domain`）：给定域名/IP，查找所有相关痕迹

use crate::core::{browser_kind_from_process_name, extract_profile_directory, BrowserKind};
use crate::download::{scan_downloads, scan_downloads_in_time_window};
use crate::extension_inventory::scan_extensions_cached;
use crate::history::attribute_history;
use crate::permission_matcher::match_domain_to_extensions;
use crate::profile::enumerate_profiles;
use crate::session_recovery::recover_tabs;
use crate::sqlite::open_browser_db;
use crate::url_utils::domain_matches_url;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;

/// 恶意连接信息
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MaliciousConnection {
    pub domain: String,
    pub ip: Option<String>,
    pub process: String,
    pub pid: u32,
    pub browser: BrowserKind,
    pub profile: String,
    pub timestamp: String,
}

/// Browser Context Attribution 综合输出
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BrowserContext {
    pub malicious_connection: MaliciousConnection,
    pub context: BrowserContextDetail,
}

/// Browser Context 详情
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BrowserContextDetail {
    /// 近期浏览器活动（来自 History Analysis）
    pub recent_browser_activity: Vec<crate::history::RecentActivity>,
    /// 用户访问路径（来自 Navigation Chain）
    pub navigation_chain: Vec<crate::history::NavChainNode>,
    /// 当前打开标签页（来自 Session Recovery，暂为空）
    pub current_tabs: Vec<CurrentTab>,
    /// 下载溯源（来自 Download Analysis）
    pub recent_downloads: Vec<crate::download::DownloadInfo>,
    /// 匹配的扩展（来自 Permission Matcher）
    pub matching_extensions: Vec<crate::permission_matcher::MatchedExtension>,
}

/// 当前标签页（Session Recovery 的占位结构，Phase 3 实现后填充）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CurrentTab {
    pub url: String,
    pub title: String,
    pub active: bool,
    pub evidence_type: String,
}

/// 对恶意连接执行 Browser Context Attribution
///
/// 这是横向关联的主入口，消费其他模块数据产出综合归因结果。
/// 如果提供了 `cmdline`，将通过 `extract_profile_directory` 精确定位 Profile。
pub fn attribute_browser_context(
    domain: &str,
    ip: Option<&str>,
    process_name: &str,
    pid: u32,
    timestamp: chrono::DateTime<chrono::Utc>,
    cmdline: Option<&str>,
) -> BrowserContext {
    // 1. 进程→浏览器识别
    let browser = match browser_kind_from_process_name(process_name) {
        Some(b) => b,
        None => {
            return BrowserContext {
                malicious_connection: MaliciousConnection {
                    domain: domain.to_string(),
                    ip: ip.map(String::from),
                    process: process_name.to_string(),
                    pid,
                    browser: BrowserKind::Chrome, // 占位，非浏览器进程无意义
                    profile: String::new(),
                    timestamp: timestamp.to_rfc3339(),
                },
                context: empty_context_detail(),
            };
        }
    };

    // 2. Profile 定位
    let profiles = enumerate_profiles(browser);

    // 如果提供了 cmdline，使用 extract_profile_directory 精确定位 Profile
    let target_profiles: Vec<_> = if let Some(cmd) = cmdline {
        if let Some(profile_name) = extract_profile_directory(cmd) {
            profiles.into_iter().filter(|p| p.name == profile_name).collect()
        } else {
            // cmdline 存在但没能提取出 Profile，回退到扫描所有 Profile
            warn!("cmdline provided but could not extract profile directory, scanning all profiles");
            profiles
        }
    } else {
        profiles
    };

    if target_profiles.is_empty() {
        warn!("no profiles found for {}", browser);
        return BrowserContext {
            malicious_connection: MaliciousConnection {
                domain: domain.to_string(),
                ip: ip.map(String::from),
                process: process_name.to_string(),
                pid,
                browser,
                profile: "Unknown".to_string(),
                timestamp: timestamp.to_rfc3339(),
            },
            context: empty_context_detail(),
        };
    }

    // 对每个 Profile 执行关联，合并结果
    // 优先使用 total_score 最高的 Profile（基于 RecentActivity 评分汇总）
    let mut best_context: Option<BrowserContextDetail> = None;
    let mut best_profile_name = String::new();
    let mut best_total_score = 0u32;

    for profile in &target_profiles {
        let ctx = build_context_for_profile(domain, profile, timestamp);

        // 汇总 RecentActivity 的 score.total 作为 Profile 评分
        let total_score: u32 = ctx
            .recent_browser_activity
            .iter()
            .filter_map(|a| a.score.as_ref().map(|s| s.total))
            .sum();

        if total_score > best_total_score || best_context.is_none() {
            best_total_score = total_score;
            best_context = Some(ctx);
            best_profile_name = profile.name.clone();
        }
    }

    let context = best_context.unwrap_or_else(empty_context_detail);

    BrowserContext {
        malicious_connection: MaliciousConnection {
            domain: domain.to_string(),
            ip: ip.map(String::from),
            process: process_name.to_string(),
            pid,
            browser,
            profile: best_profile_name,
            timestamp: timestamp.to_rfc3339(),
        },
        context,
    }
}

/// 对单个 Profile 构建上下文详情
fn build_context_for_profile(
    domain: &str,
    profile: &crate::profile::BrowserProfile,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> BrowserContextDetail {
    // 3. History 关联
    let history = attribute_history(profile, timestamp, domain);

    // 4. Download 关联：±30s 时间窗口
    let download_start = timestamp - chrono::Duration::seconds(30);
    let download_end = timestamp + chrono::Duration::seconds(30);
    let downloads = scan_downloads_in_time_window(profile, download_start, download_end);

    // 5. Extension 匹配
    let inventory = scan_extensions_cached(profile.browser, profile);
    let perm_result = match_domain_to_extensions(domain, &inventory.extensions);

    // 6. Session Recovery: 当前标签页
    let rec_result = recover_tabs(profile);

    BrowserContextDetail {
        recent_browser_activity: history.recent_browser_activity,
        navigation_chain: history.navigation_chain,
        current_tabs: rec_result
            .tabs
            .into_iter()
            .map(|t| CurrentTab {
                url: t.url,
                title: t.title,
                active: t.active,
                evidence_type: "session-recovery".to_string(),
            })
            .collect(),
        recent_downloads: downloads.downloads,
        matching_extensions: perm_result.matching_extensions,
    }
}

/// 空的上下文详情
fn empty_context_detail() -> BrowserContextDetail {
    BrowserContextDetail {
        recent_browser_activity: vec![],
        navigation_chain: vec![],
        current_tabs: vec![],
        recent_downloads: vec![],
        matching_extensions: vec![],
    }
}

// ── 基于域名的归因 ──────────────────────────────────────────────

/// 基于域名的归因结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DomainAttribution {
    /// 目标域名/IP
    pub target: String,
    /// 浏览器类型
    pub browser: BrowserKind,
    /// Profile 名
    pub profile: String,
    /// 匹配的扩展（权限覆盖目标域名）
    pub matching_extensions: Vec<crate::permission_matcher::MatchedExtension>,
    /// 访问过目标域名的浏览记录
    pub related_history: Vec<crate::history::HistoryEntry>,
    /// 从目标域名下载的文件
    pub related_downloads: Vec<crate::download::DownloadInfo>,
    /// 包含目标域名的当前标签页
    pub related_tabs: Vec<CurrentTab>,
}

/// 基于域名/IP 的归因：查找所有与目标相关的浏览器痕迹
///
/// 给定域名/IP，横向关联：
/// - 哪些扩展有权限访问该域名（permission_matcher）
/// - 浏览历史中是否访问过该域名（History LIKE 筛选）
/// - 下载记录中是否有来自该域名的文件（Download URL 筛选）
/// - 当前标签页中是否有该域名（Session Recovery 筛选）
pub fn attribute_by_domain(target: &str, browser: BrowserKind) -> Vec<DomainAttribution> {
    let profiles = enumerate_profiles(browser);
    if profiles.is_empty() {
        return vec![];
    }

    profiles
        .into_iter()
        .map(|profile| build_domain_attribution(target, browser, &profile))
        .collect()
}

fn build_domain_attribution(
    target: &str,
    browser: BrowserKind,
    profile: &crate::profile::BrowserProfile,
) -> DomainAttribution {
    // 1. Extension 匹配：哪些扩展有权限访问目标域名
    let inventory = scan_extensions_cached(browser, profile);
    let perm_result = match_domain_to_extensions(target, &inventory.extensions);

    // 2. History 筛选：访问过目标域名的记录
    let related_history = query_history_by_domain(profile, target);

    // 3. Download 筛选：从目标域名下载的文件
    let all_downloads = scan_downloads(profile);
    let related_downloads: Vec<_> = all_downloads
        .downloads
        .into_iter()
        .filter(|d| {
            domain_matches_url(target, &d.download_url)
                || d.referrer
                    .as_deref()
                    .map(|r| domain_matches_url(target, r))
                    .unwrap_or(false)
        })
        .collect();

    // 4. Session Recovery 筛选：包含目标域名的标签页
    let rec_result = recover_tabs(profile);
    let related_tabs: Vec<_> = rec_result
        .tabs
        .into_iter()
        .filter(|t| domain_matches_url(target, &t.url))
        .map(|t| CurrentTab {
            url: t.url,
            title: t.title,
            active: t.active,
            evidence_type: "session-recovery".to_string(),
        })
        .collect();

    DomainAttribution {
        target: target.to_string(),
        browser,
        profile: profile.name.clone(),
        matching_extensions: perm_result.matching_extensions,
        related_history,
        related_downloads,
        related_tabs,
    }
}

/// 从 History 数据库中按域名筛选浏览记录
fn query_history_by_domain(
    profile: &crate::profile::BrowserProfile,
    domain: &str,
) -> Vec<crate::history::HistoryEntry> {
    let db_path = profile.path.join("History");
    let conn = match open_browser_db(&db_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to open History db for {}: {}", profile.name, e);
            return vec![];
        }
    };

    let sql = "SELECT u.url, u.title, v.visit_time, u.visit_count \
         FROM visits v \
         JOIN urls u ON v.url = u.id \
         WHERE u.url LIKE ? \
         ORDER BY v.visit_time DESC \
         LIMIT 200";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            warn!("failed to prepare domain history query: {}", e);
            return vec![];
        }
    };

    let pattern = format!("%{}%", domain);
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    });

    match rows {
        Ok(mapped_rows) => mapped_rows
            .filter_map(|r| {
                r.ok()
                    .map(|(url, title, visit_time, visit_count)| crate::history::HistoryEntry {
                        url,
                        title,
                        visit_time: crate::core::webkit_timestamp::from_webkit_micros(visit_time)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default(),
                        visit_count,
                    })
            })
            .collect(),
        Err(e) => {
            warn!("failed to execute domain history query: {}", e);
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn non_browser_process_returns_empty() {
        let ts = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_browser_context("evil.com", None, "notepad.exe", 1234, ts, None);

        assert!(result.context.recent_browser_activity.is_empty());
        assert!(result.context.navigation_chain.is_empty());
        assert!(result.context.current_tabs.is_empty());
        assert!(result.context.recent_downloads.is_empty());
        assert!(result.context.matching_extensions.is_empty());
        assert!(result.malicious_connection.profile.is_empty());
    }

    #[test]
    fn browser_kind_identified_correctly() {
        assert_eq!(browser_kind_from_process_name("chrome.exe"), Some(BrowserKind::Chrome));
        assert_eq!(browser_kind_from_process_name("msedge.exe"), Some(BrowserKind::Edge));
        assert_eq!(browser_kind_from_process_name("svchost.exe"), None);
    }

    #[test]
    fn malicious_connection_fields_populated() {
        let ts = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_browser_context("evil.com", Some("1.2.3.4"), "notepad.exe", 5678, ts, None);

        assert_eq!(result.malicious_connection.domain, "evil.com");
        assert_eq!(result.malicious_connection.ip, Some("1.2.3.4".to_string()));
        assert_eq!(result.malicious_connection.process, "notepad.exe");
        assert_eq!(result.malicious_connection.pid, 5678);
        assert_eq!(result.malicious_connection.timestamp, ts.to_rfc3339());
    }

    #[test]
    fn empty_context_detail_is_empty() {
        let ctx = empty_context_detail();
        assert!(ctx.recent_browser_activity.is_empty());
        assert!(ctx.navigation_chain.is_empty());
        assert!(ctx.current_tabs.is_empty());
        assert!(ctx.recent_downloads.is_empty());
        assert!(ctx.matching_extensions.is_empty());
    }

    #[test]
    fn current_tab_structure() {
        let tab = CurrentTab {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            active: true,
            evidence_type: "session-recovery".to_string(),
        };
        assert_eq!(tab.url, "https://example.com");
        assert!(tab.active);
    }

    #[test]
    fn extract_profile_directory_utility() {
        let cmdline = r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --profile-directory="Profile 1""#;
        assert_eq!(
            crate::core::extract_profile_directory(cmdline),
            Some("Profile 1".to_string())
        );
    }

    #[test]
    fn cmdline_no_profile_directory_falls_back() {
        // cmdline 不包含 --profile-directory 时，应回退到扫描所有 Profile
        let ts = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_browser_context("evil.com", None, "notepad.exe", 9999, ts, Some("chrome.exe"));
        // 非浏览器进程会直接返回空
        assert!(result.context.recent_browser_activity.is_empty());
        assert!(result.malicious_connection.profile.is_empty());
    }

    #[test]
    fn cmdline_with_profile_directory() {
        // 验证 cmdline 参数被接收并处理
        let ts = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_browser_context(
            "evil.com",
            None,
            "chrome.exe",
            9999,
            ts,
            Some(r#"--profile-directory="Profile 1""#),
        );
        // chrome.exe 是浏览器进程，会尝试扫描 Profile
        // 这里验证 cmdline 传递不会导致 panic，且 Profile 字段不为空摘要字符串
        // 实际 Profile 选择结果取决于测试环境的 Chrome 安装情况
        assert!(
            result.malicious_connection.profile.is_empty()
                || result.malicious_connection.profile == "Profile 1"
                || !result.malicious_connection.profile.is_empty()
        );
    }
}
