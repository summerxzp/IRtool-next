//! Browser Evidence Engine 统一数据模型。
//!
//! 本模块定义 5 类证据的统一抽象，为评分系统（P0.2）和 EvidenceObject（P0.7）打基础。
//! 阶段 P0 引入 4 类来源（History/Download/Session/Extension），P1/P2 阶段扩展 HelperExtension/Cdp。

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::context_attribution::MaliciousConnection;
use crate::core::BrowserKind;
use crate::download::DownloadInfo;
use crate::history::{NavChainNode, RecentActivity};
use crate::permission_matcher::MatchedExtension;

/// 证据来源类型（当前 P0 四类，未来 P1/P2 扩展）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum BrowserSourceType {
    History,
    Download,
    Session,
    Extension,
    // 未来：Cdp, HelperExtension
}

/// 统一浏览器事件抽象（P0 阶段作为归一化中间结构）
///
/// 各类扫描结果（DownloadInfo/ExtensionInfo/RecoveredTab）作为详情载荷保留特有字段，
/// 通过 source_type 区分，汇总到 EvidenceObject 时组合。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BrowserEvent {
    pub browser: BrowserKind,
    pub profile: String,
    /// RFC3339 时间戳
    pub timestamp: String,
    pub source_type: BrowserSourceType,
    pub url: String,
    pub title: Option<String>,
}

/// 证据置信度等级
///
/// - Confirmed: P1 Helper / P2 CDP 铁证
/// - Probable: P0 高分关联（total >= 70）
/// - Possible: P0 低分关联（total < 70）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum AttributionLevel {
    Confirmed,
    Probable,
    Possible,
}

/// 证据评分（P0.2 填充评分逻辑，P0.1 仅定义结构）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EvidenceScore {
    pub time_score: u32,
    pub domain_score: u32,
    pub chain_score: u32,
    /// ≤ 100
    pub total: u32,
}

impl EvidenceScore {
    /// 根据总分推断置信度等级
    pub fn level(&self) -> AttributionLevel {
        match self.total {
            t if t >= 70 => AttributionLevel::Probable,
            _ => AttributionLevel::Possible,
        }
    }

    /// 构造零分实例（用于无关联场景）
    pub fn zero() -> Self {
        Self {
            time_score: 0,
            domain_score: 0,
            chain_score: 0,
            total: 0,
        }
    }
}

/// 评分权重表（可配置，默认值参考设计方案 +50/+30/+20）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreWeights {
    /// ±5s 时间窗得分
    pub time_immediate: u32,
    /// ±15s 时间窗得分
    pub time_nearby: u32,
    /// ±30s 时间窗得分
    pub time_recent: u32,
    /// 域名精确匹配得分
    pub domain_exact: u32,
    /// 子域名匹配得分
    pub domain_subdomain: u32,
    /// from_visit 链连续得分
    pub chain_continuous: u32,
    /// 链长加分（每深度 +5，封顶 5 层）
    pub chain_length_bonus: u32,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            time_immediate: 50,
            time_nearby: 30,
            time_recent: 10,
            domain_exact: 30,
            domain_subdomain: 20,
            chain_continuous: 20,
            chain_length_bonus: 5,
        }
    }
}

// ── P0.7a EvidenceObject 与子证据 DTO ───────────────────────────

/// 历史关联详情（含评分）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HistoryCorrelation {
    pub confidence: AttributionLevel,
    pub score: EvidenceScore,
    pub recent_activity: Vec<ScoredActivity>,
}

/// 评分后的历史活动条目
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScoredActivity {
    pub activity: RecentActivity,
    pub score: EvidenceScore,
}

/// 扩展归因汇总（P0 阶段 Probable/Possible，P1 可达 Confirmed）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionAttributionSummary {
    pub confidence: AttributionLevel,
    pub matched: Vec<MatchedExtension>,
}

/// Tab 归因（P2 阶段填充，P0 为 None）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TabAttribution {
    pub confidence: AttributionLevel,
    pub url: String,
}

/// 统一证据对象（设计方案目标 JSON 结构的 Rust 对应）
///
/// `attribute_browser_context` 的返回值，组合 5 类子证据并通过 `overall_score`/`overall_confidence`
/// 给出综合归因结论。P0 阶段 `alert_id`/`tab_attribution` 暂为 None。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EvidenceObject {
    pub domain: String,
    pub process: String,
    pub pid: u32,
    pub alert_id: Option<String>,

    pub malicious_connection: MaliciousConnection,
    pub history_correlation: Option<HistoryCorrelation>,
    pub downloads: Vec<DownloadInfo>,
    pub navigation_chain: Vec<NavChainNode>,
    pub extension_attribution: Option<ExtensionAttributionSummary>,
    pub tab_attribution: Option<TabAttribution>,

    pub overall_confidence: AttributionLevel,
    pub overall_score: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_type_serde_lowercase() {
        let s = serde_json::to_string(&BrowserSourceType::Download).unwrap();
        assert_eq!(s, "\"download\"");
        let h: BrowserSourceType = serde_json::from_str("\"history\"").unwrap();
        assert_eq!(h, BrowserSourceType::History);
    }

    #[test]
    fn attribution_level_serde_lowercase() {
        let s = serde_json::to_string(&AttributionLevel::Confirmed).unwrap();
        assert_eq!(s, "\"confirmed\"");
    }

    #[test]
    fn score_level_probable_at_70() {
        let score = EvidenceScore {
            time_score: 50,
            domain_score: 20,
            chain_score: 0,
            total: 70,
        };
        assert_eq!(score.level(), AttributionLevel::Probable);
    }

    #[test]
    fn score_level_possible_below_70() {
        let score = EvidenceScore {
            time_score: 30,
            domain_score: 0,
            chain_score: 0,
            total: 30,
        };
        assert_eq!(score.level(), AttributionLevel::Possible);
    }

    #[test]
    fn score_zero_helper() {
        let z = EvidenceScore::zero();
        assert_eq!(z.total, 0);
        assert_eq!(z.level(), AttributionLevel::Possible);
    }

    #[test]
    fn default_weights_match_design_doc() {
        let w = ScoreWeights::default();
        assert_eq!(w.time_immediate, 50);
        assert_eq!(w.time_nearby, 30);
        assert_eq!(w.time_recent, 10);
        assert_eq!(w.domain_exact, 30);
        assert_eq!(w.domain_subdomain, 20);
        assert_eq!(w.chain_continuous, 20);
        assert_eq!(w.chain_length_bonus, 5);
    }

    #[test]
    fn browser_event_serialization_roundtrip() {
        let evt = BrowserEvent {
            browser: BrowserKind::Chrome,
            profile: "Default".to_string(),
            timestamp: "2026-06-26T10:00:00Z".to_string(),
            source_type: BrowserSourceType::History,
            url: "https://example.com/".to_string(),
            title: Some("Example".to_string()),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: BrowserEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.browser, BrowserKind::Chrome);
        assert_eq!(back.source_type, BrowserSourceType::History);
        assert_eq!(back.title, Some("Example".to_string()));
    }

    // ── P0.7a EvidenceObject 测试 ───────────────────────────────

    #[test]
    fn evidence_object_construction() {
        use crate::context_attribution::MaliciousConnection;
        use crate::download::DownloadInfo;
        use crate::history::{NavChainNode, RecentActivity, TimeTier};
        use crate::permission_matcher::MatchedExtension;

        let mc = MaliciousConnection {
            domain: "evil.com".to_string(),
            ip: Some("1.2.3.4".to_string()),
            process: "chrome.exe".to_string(),
            pid: 1234,
            browser: BrowserKind::Chrome,
            profile: "Default".to_string(),
            timestamp: "2024-06-15T12:00:00Z".to_string(),
        };

        let activity = RecentActivity {
            url: "https://evil.com/payload".to_string(),
            title: "Evil".to_string(),
            visit_time: "2024-06-15T12:00:00Z".to_string(),
            tier: TimeTier::Immediate,
            time_distance_ms: 1000,
            evidence_type: "recent-visit".to_string(),
            score: Some(EvidenceScore {
                time_score: 50,
                domain_score: 30,
                chain_score: 20,
                total: 100,
            }),
        };

        let scored = ScoredActivity {
            score: activity.score.clone().unwrap_or_else(EvidenceScore::zero),
            activity: activity.clone(),
        };

        let hc_score = EvidenceScore {
            time_score: 50,
            domain_score: 30,
            chain_score: 20,
            total: 100,
        };
        let hc = HistoryCorrelation {
            confidence: AttributionLevel::Probable,
            score: hc_score.clone(),
            recent_activity: vec![scored],
        };

        let matched_ext = MatchedExtension {
            id: "ext1".to_string(),
            name: "Ad Blocker".to_string(),
            version: "1.0".to_string(),
            risk_flags: vec!["broad_host_access".to_string()],
            matched_patterns: vec!["<all_urls>".to_string()],
            has_sensitive_permissions: true,
        };
        let eas = ExtensionAttributionSummary {
            confidence: AttributionLevel::Possible,
            matched: vec![matched_ext.clone()],
        };

        let nav_node = NavChainNode {
            url: "https://evil.com".to_string(),
            title: Some("Evil".to_string()),
            transition: Some("LINK".to_string()),
            qualifiers: vec![],
            referrer: None,
        };

        let download = DownloadInfo {
            filename: "payload.exe".to_string(),
            local_path: r"C:\Downloads\payload.exe".to_string(),
            download_url: "https://evil.com/payload.exe".to_string(),
            referrer: Some("https://evil.com/page".to_string()),
            start_time: Some("2024-06-15T12:00:00Z".to_string()),
            end_time: None,
            total_bytes: Some(2048),
            danger_type: crate::download::DangerType::DangerousContent,
            opened: false,
            interrupt_reason: None,
            evidence_type: "download".to_string(),
            url_chain: vec!["https://evil.com/payload.exe".to_string()],
            tab_url: None,
            tab_referrer_url: None,
        };

        let obj = EvidenceObject {
            domain: "evil.com".to_string(),
            process: "chrome.exe".to_string(),
            pid: 1234,
            alert_id: None,
            malicious_connection: mc,
            history_correlation: Some(hc),
            downloads: vec![download],
            navigation_chain: vec![nav_node],
            extension_attribution: Some(eas),
            tab_attribution: None,
            overall_confidence: AttributionLevel::Probable,
            overall_score: 100,
        };

        // 顶层字段
        assert_eq!(obj.domain, "evil.com");
        assert_eq!(obj.process, "chrome.exe");
        assert_eq!(obj.pid, 1234);
        assert!(obj.alert_id.is_none());

        // malicious_connection
        assert_eq!(obj.malicious_connection.domain, "evil.com");
        assert_eq!(obj.malicious_connection.ip.as_deref(), Some("1.2.3.4"));
        assert_eq!(obj.malicious_connection.profile, "Default");

        // history_correlation
        let hc_ref = obj
            .history_correlation
            .as_ref()
            .expect("history_correlation should be Some");
        assert_eq!(hc_ref.confidence, AttributionLevel::Probable);
        assert_eq!(hc_ref.score.total, 100);
        assert_eq!(hc_ref.recent_activity.len(), 1);
        assert_eq!(hc_ref.recent_activity[0].activity.url, "https://evil.com/payload");
        assert_eq!(hc_ref.recent_activity[0].score.total, 100);

        // downloads
        assert_eq!(obj.downloads.len(), 1);
        assert_eq!(obj.downloads[0].filename, "payload.exe");

        // navigation_chain
        assert_eq!(obj.navigation_chain.len(), 1);
        assert_eq!(obj.navigation_chain[0].url, "https://evil.com");

        // extension_attribution
        let eas_ref = obj
            .extension_attribution
            .as_ref()
            .expect("extension_attribution should be Some");
        assert_eq!(eas_ref.confidence, AttributionLevel::Possible);
        assert_eq!(eas_ref.matched.len(), 1);
        assert_eq!(eas_ref.matched[0].id, "ext1");

        // tab_attribution (P0 为 None)
        assert!(obj.tab_attribution.is_none());

        // 评分汇总
        assert_eq!(obj.overall_confidence, AttributionLevel::Probable);
        assert_eq!(obj.overall_score, 100);
    }

    #[test]
    fn evidence_object_serialization_roundtrip() {
        use crate::context_attribution::MaliciousConnection;

        let mc = MaliciousConnection {
            domain: "evil.com".to_string(),
            ip: None,
            process: "chrome.exe".to_string(),
            pid: 1234,
            browser: BrowserKind::Chrome,
            profile: "Default".to_string(),
            timestamp: "2024-06-15T12:00:00Z".to_string(),
        };

        let obj = EvidenceObject {
            domain: "evil.com".to_string(),
            process: "chrome.exe".to_string(),
            pid: 1234,
            alert_id: Some("alert-001".to_string()),
            malicious_connection: mc,
            history_correlation: None,
            downloads: vec![],
            navigation_chain: vec![],
            extension_attribution: None,
            tab_attribution: Some(TabAttribution {
                confidence: AttributionLevel::Possible,
                url: "https://evil.com/tab".to_string(),
            }),
            overall_confidence: AttributionLevel::Possible,
            overall_score: 30,
        };

        let json = serde_json::to_string(&obj).unwrap();
        let back: EvidenceObject = serde_json::from_str(&json).unwrap();
        assert_eq!(back.domain, "evil.com");
        assert_eq!(back.pid, 1234);
        assert_eq!(back.alert_id.as_deref(), Some("alert-001"));
        assert!(back.history_correlation.is_none());
        assert!(back.extension_attribution.is_none());
        assert!(back.tab_attribution.is_some());
        assert_eq!(back.tab_attribution.unwrap().url, "https://evil.com/tab");
        assert_eq!(back.overall_score, 30);
    }
}
