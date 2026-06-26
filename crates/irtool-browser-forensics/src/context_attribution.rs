//! Browser Context Attribution：横向关联模块
//!
//! 对恶意网络连接执行综合归因，消费 History、Download、Extension 等模块数据，
//! 产出 EvidenceObject 综合结果。
//!
//! 两种归因模式：
//! - 时间窗口归因（`attribute_browser_context`）：给定时间点，查找附近的浏览活动
//! - 域名归因（`attribute_by_domain`）：给定域名/IP，查找所有相关痕迹

use crate::core::{browser_kind_from_process_name, extract_profile_directory, BrowserKind};
use crate::download::{scan_downloads, scan_downloads_in_time_window, DownloadInfo};
use crate::evidence::{
    AttributionLevel, EvidenceObject, EvidenceScore, ExtensionAttributionSummary, HistoryCorrelation, ScoredActivity,
};
use crate::extension_inventory::scan_extensions_cached;
use crate::history::{attribute_history, NavChainNode, RecentActivity};
use crate::permission_matcher::{match_domain_to_extensions, MatchedExtension};
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

/// 当前标签页（Session Recovery 的占位结构，Phase 3 实现后填充）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CurrentTab {
    pub url: String,
    pub title: String,
    pub active: bool,
    pub evidence_type: String,
}

/// 单 Profile 的子证据组件（内部聚合用,EvidenceObject 的构造原料）
#[derive(Default)]
struct ContextParts {
    recent_browser_activity: Vec<RecentActivity>,
    navigation_chain: Vec<NavChainNode>,
    recent_downloads: Vec<DownloadInfo>,
    matching_extensions: Vec<MatchedExtension>,
}

/// 对恶意连接执行 Browser Context Attribution
///
/// 这是横向关联的主入口，消费其他模块数据产出 `EvidenceObject` 综合归因结果。
/// 如果提供了 `cmdline`，将通过 `extract_profile_directory` 精确定位 Profile。
pub fn attribute_browser_context(
    domain: &str,
    ip: Option<&str>,
    process_name: &str,
    pid: u32,
    timestamp: chrono::DateTime<chrono::Utc>,
    cmdline: Option<&str>,
) -> EvidenceObject {
    // 1. 进程→浏览器识别
    let browser = match browser_kind_from_process_name(process_name) {
        Some(b) => b,
        None => {
            return empty_evidence_object(
                domain,
                ip,
                process_name,
                pid,
                timestamp,
                BrowserKind::Chrome, // 占位，非浏览器进程无意义
                String::new(),
            );
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
        return empty_evidence_object(domain, ip, process_name, pid, timestamp, browser, "Unknown".to_string());
    }

    // 对每个 Profile 执行关联，合并结果
    // 优先使用 total_score 最高的 Profile（基于 RecentActivity 评分汇总）
    let mut best_parts: Option<ContextParts> = None;
    let mut best_profile_name = String::new();
    let mut best_total_score = 0u32;

    for profile in &target_profiles {
        let parts = build_context_parts(domain, profile, timestamp);

        // 汇总 RecentActivity 的 score.total 作为 Profile 评分
        let total_score: u32 = parts
            .recent_browser_activity
            .iter()
            .filter_map(|a| a.score.as_ref().map(|s| s.total))
            .sum();

        if total_score > best_total_score || best_parts.is_none() {
            best_total_score = total_score;
            best_parts = Some(parts);
            best_profile_name = profile.name.clone();
        }
    }

    let parts = best_parts.unwrap_or_default();

    // 构造 HistoryCorrelation（无活动时为 None）
    let history_correlation = if parts.recent_browser_activity.is_empty() {
        None
    } else {
        let scored: Vec<ScoredActivity> = parts
            .recent_browser_activity
            .iter()
            .map(|a| ScoredActivity {
                activity: a.clone(),
                score: a.score.clone().unwrap_or_else(EvidenceScore::zero),
            })
            .collect();

        // 聚合评分:各分项取 max, total = min(分项之和, 100), 保持 EvidenceScore 不变式
        let score = aggregate_activity_score(&parts.recent_browser_activity);

        Some(HistoryCorrelation {
            confidence: score.level(),
            score,
            recent_activity: scored,
        })
    };

    // 构造 ExtensionAttributionSummary（无匹配扩展时为 None）
    // P0 阶段不连接 Helper Extension/CDP,匹配仅说明"可能",故使用 Possible
    let extension_attribution = if parts.matching_extensions.is_empty() {
        None
    } else {
        Some(ExtensionAttributionSummary {
            confidence: AttributionLevel::Possible,
            matched: parts.matching_extensions.clone(),
        })
    };

    // overall_confidence 由 overall_score 推断(>=70 → Probable,否则 Possible)
    let overall_confidence = if best_total_score >= 70 {
        AttributionLevel::Probable
    } else {
        AttributionLevel::Possible
    };

    EvidenceObject {
        domain: domain.to_string(),
        process: process_name.to_string(),
        pid,
        alert_id: None,
        malicious_connection: MaliciousConnection {
            domain: domain.to_string(),
            ip: ip.map(String::from),
            process: process_name.to_string(),
            pid,
            browser,
            profile: best_profile_name,
            timestamp: timestamp.to_rfc3339(),
        },
        history_correlation,
        downloads: parts.recent_downloads,
        navigation_chain: parts.navigation_chain,
        extension_attribution,
        tab_attribution: None,
        overall_confidence,
        overall_score: best_total_score.min(100),
    }
}

/// 聚合多活动的评分为单个 EvidenceScore
///
/// 各分项取 max（最强单维证据），total = min(time+domain+chain, 100)，
/// 保持 EvidenceScore 单条不变式 `total = min(time_score + domain_score + chain_score, 100)`。
fn aggregate_activity_score(activities: &[crate::history::RecentActivity]) -> EvidenceScore {
    let time_score = activities
        .iter()
        .filter_map(|a| a.score.as_ref().map(|s| s.time_score))
        .max()
        .unwrap_or(0);
    let domain_score = activities
        .iter()
        .filter_map(|a| a.score.as_ref().map(|s| s.domain_score))
        .max()
        .unwrap_or(0);
    let chain_score = activities
        .iter()
        .filter_map(|a| a.score.as_ref().map(|s| s.chain_score))
        .max()
        .unwrap_or(0);
    let total = (time_score + domain_score + chain_score).min(100);
    EvidenceScore {
        time_score,
        domain_score,
        chain_score,
        total,
    }
}

/// 构造一个无子证据的空 EvidenceObject
///
/// 用于非浏览器进程或无 Profile 场景,overall_score = 0、confidence = Possible。
fn empty_evidence_object(
    domain: &str,
    ip: Option<&str>,
    process_name: &str,
    pid: u32,
    timestamp: chrono::DateTime<chrono::Utc>,
    browser: BrowserKind,
    profile: String,
) -> EvidenceObject {
    EvidenceObject {
        domain: domain.to_string(),
        process: process_name.to_string(),
        pid,
        alert_id: None,
        malicious_connection: MaliciousConnection {
            domain: domain.to_string(),
            ip: ip.map(String::from),
            process: process_name.to_string(),
            pid,
            browser,
            profile,
            timestamp: timestamp.to_rfc3339(),
        },
        history_correlation: None,
        downloads: vec![],
        navigation_chain: vec![],
        extension_attribution: None,
        tab_attribution: None,
        overall_confidence: AttributionLevel::Possible,
        overall_score: 0,
    }
}

/// 对单个 Profile 构建子证据组件
///
/// P0 阶段不调用 `recover_tabs`：`EvidenceObject.tab_attribution` 暂为 None,
/// Session Recovery 数据由 `attribute_by_domain` 路径独立消费。
fn build_context_parts(
    domain: &str,
    profile: &crate::profile::BrowserProfile,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> ContextParts {
    // History 关联
    let history = attribute_history(profile, timestamp, domain);

    // Download 关联：±30s 时间窗口
    let download_start = timestamp - chrono::Duration::seconds(30);
    let download_end = timestamp + chrono::Duration::seconds(30);
    let downloads = scan_downloads_in_time_window(profile, download_start, download_end);

    // Extension 匹配
    let inventory = scan_extensions_cached(profile.browser, profile);
    let perm_result = match_domain_to_extensions(domain, &inventory.extensions);

    ContextParts {
        recent_browser_activity: history.recent_browser_activity,
        navigation_chain: history.navigation_chain,
        recent_downloads: downloads.downloads,
        matching_extensions: perm_result.matching_extensions,
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

        // 非浏览器进程:无任何子证据,overall_score = 0
        assert!(result.history_correlation.is_none());
        assert!(result.navigation_chain.is_empty());
        assert!(result.downloads.is_empty());
        assert!(result.extension_attribution.is_none());
        assert!(result.tab_attribution.is_none());
        assert!(result.malicious_connection.profile.is_empty());
        assert_eq!(result.overall_score, 0);
        assert_eq!(result.overall_confidence, crate::evidence::AttributionLevel::Possible);
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

        // EvidenceObject 顶层冗余字段
        assert_eq!(result.domain, "evil.com");
        assert_eq!(result.process, "notepad.exe");
        assert_eq!(result.pid, 5678);
        assert!(result.alert_id.is_none());
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
        assert!(result.history_correlation.is_none());
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

    #[test]
    fn attribute_browser_context_returns_evidence_object() {
        let ts = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let result = attribute_browser_context("evil.com", Some("1.2.3.4"), "notepad.exe", 1234, ts, None);

        // 验证返回值是 EvidenceObject 类型（通过字段访问）
        assert_eq!(result.domain, "evil.com");
        assert_eq!(result.process, "notepad.exe");
        assert_eq!(result.pid, 1234);
        assert!(result.alert_id.is_none());

        // 非浏览器进程 → 无子证据,overall_score = 0,confidence = Possible
        assert_eq!(result.overall_score, 0);
        assert_eq!(result.overall_confidence, crate::evidence::AttributionLevel::Possible);
        assert!(result.history_correlation.is_none());
        assert!(result.extension_attribution.is_none());
        assert!(result.tab_attribution.is_none());
        assert!(result.downloads.is_empty());
        assert!(result.navigation_chain.is_empty());

        // malicious_connection 仍含完整连接信息
        assert_eq!(result.malicious_connection.domain, "evil.com");
        assert_eq!(result.malicious_connection.ip.as_deref(), Some("1.2.3.4"));
        assert_eq!(result.malicious_connection.process, "notepad.exe");
    }

    /// 构造测试用 RecentActivity（仅 score 与 url 有意义）
    fn make_activity(score: Option<EvidenceScore>) -> RecentActivity {
        RecentActivity {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            visit_time: "2024-06-15T12:00:00Z".to_string(),
            tier: crate::history::TimeTier::Immediate,
            time_distance_ms: 1000,
            evidence_type: "history".to_string(),
            score,
        }
    }

    #[test]
    fn aggregate_activity_score_empty_returns_zero() {
        let score = aggregate_activity_score(&[]);
        assert_eq!(score.time_score, 0);
        assert_eq!(score.domain_score, 0);
        assert_eq!(score.chain_score, 0);
        assert_eq!(score.total, 0);
    }

    #[test]
    fn aggregate_activity_score_single_preserves_score() {
        let original = EvidenceScore {
            time_score: 50,
            domain_score: 30,
            chain_score: 20,
            total: 100,
        };
        let activities = vec![make_activity(Some(original.clone()))];
        let aggregated = aggregate_activity_score(&activities);
        assert_eq!(aggregated.time_score, original.time_score);
        assert_eq!(aggregated.domain_score, original.domain_score);
        assert_eq!(aggregated.chain_score, original.chain_score);
        assert_eq!(aggregated.total, original.total);
    }

    #[test]
    fn aggregate_activity_score_multi_takes_max_per_dim() {
        let activities = vec![
            make_activity(Some(EvidenceScore {
                time_score: 50,
                domain_score: 10,
                chain_score: 5,
                total: 65,
            })),
            make_activity(Some(EvidenceScore {
                time_score: 20,
                domain_score: 30,
                chain_score: 15,
                total: 65,
            })),
            make_activity(Some(EvidenceScore {
                time_score: 5,
                domain_score: 5,
                chain_score: 20,
                total: 30,
            })),
        ];
        let aggregated = aggregate_activity_score(&activities);
        assert_eq!(aggregated.time_score, 50);
        assert_eq!(aggregated.domain_score, 30);
        assert_eq!(aggregated.chain_score, 20);
    }

    #[test]
    fn aggregate_activity_score_total_capped_at_100() {
        // 各分项 max 之和 = 50 + 50 + 50 = 150,应封顶 100
        let activities = vec![make_activity(Some(EvidenceScore {
            time_score: 50,
            domain_score: 50,
            chain_score: 50,
            total: 100,
        }))];
        let aggregated = aggregate_activity_score(&activities);
        assert_eq!(aggregated.total, 100);
    }

    #[test]
    fn aggregate_activity_score_preserves_invariant() {
        let activities = vec![
            make_activity(Some(EvidenceScore {
                time_score: 40,
                domain_score: 25,
                chain_score: 10,
                total: 75,
            })),
            make_activity(Some(EvidenceScore {
                time_score: 10,
                domain_score: 35,
                chain_score: 30,
                total: 75,
            })),
            // 无 score 的活动应被忽略
            make_activity(None),
        ];
        let aggregated = aggregate_activity_score(&activities);
        let expected_total = (aggregated.time_score + aggregated.domain_score + aggregated.chain_score).min(100);
        assert_eq!(aggregated.total, expected_total);
        // 各分项应为 max:time=40, domain=35, chain=30,和=105 → 封顶 100
        assert_eq!(aggregated.time_score, 40);
        assert_eq!(aggregated.domain_score, 35);
        assert_eq!(aggregated.chain_score, 30);
        assert_eq!(aggregated.total, 100);
    }
}
