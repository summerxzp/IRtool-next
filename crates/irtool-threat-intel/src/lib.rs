//! v2.0 仅 trait + NoopProvider；v2.1+ 接入 Weibu/VirusTotal Provider
//! 见设计文档 §4.6 与 §14 路线图

use async_trait::async_trait;
use irtool_core::IrError;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IntelResult {
    Disabled { reason: String },
    Clean,
    Suspicious { score: u32, sources: Vec<String> },
    Malicious { score: u32, sources: Vec<String> },
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum IocQuery {
    Hash(String),
    Ip(String),
    Domain(String),
}

#[async_trait]
pub trait ThreatIntelProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn query(&self, query: &IocQuery) -> Result<IntelResult, IrError>;
}

pub struct NoopProvider;

#[async_trait]
impl ThreatIntelProvider for NoopProvider {
    fn name(&self) -> &str {
        "noop"
    }

    async fn query(&self, _query: &IocQuery) -> Result<IntelResult, IrError> {
        Ok(IntelResult::Disabled {
            reason: "v2.0 not implemented; planned for v2.1+".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_provider_returns_disabled() {
        let provider = NoopProvider;
        let result = provider.query(&IocQuery::Hash("abc".into())).await.unwrap();
        match result {
            IntelResult::Disabled { reason } => assert!(reason.contains("v2.0")),
            _ => panic!("expected Disabled"),
        }
    }
}
