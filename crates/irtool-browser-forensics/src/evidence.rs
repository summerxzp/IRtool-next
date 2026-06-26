//! Browser Evidence Engine 统一数据模型。
//!
//! 本模块定义 5 类证据的统一抽象，为评分系统（P0.2）和 EvidenceObject（P0.7）打基础。
//! 阶段 P0 引入 4 类来源（History/Download/Session/Extension），P1/P2 阶段扩展 HelperExtension/Cdp。

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::BrowserKind;

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
}
