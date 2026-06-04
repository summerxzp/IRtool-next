use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Safe,
    Suspicious,
    HighRisk,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Suspicious => "suspicious",
            Self::HighRisk => "high_risk",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum SignatureStatus {
    Valid { signer: String },
    Invalid { detail: String },
    Unsigned,
    NotVerified,
}

impl Default for SignatureStatus {
    fn default() -> Self {
        Self::NotVerified
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AutorunItem {
    pub id: u64,
    pub category: String,
    pub entry: String,
    pub enabled: bool,
    pub location: String,
    pub description: String,
    pub publisher: String,
    pub image_path: Option<String>,
    pub launch_string: Option<String>,
    pub timestamp: Option<String>,
    pub file_exists: bool,
    pub file_size: Option<u64>,
    pub file_version: Option<String>,
    pub service_name: Option<String>,
    pub md5: Option<String>,
    pub sha256: Option<String>,
    pub risk: RiskLevel,
    pub risk_reasons: Vec<String>,
    pub signature: SignatureStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScanOptions {
    pub include_hash: bool,
    pub category_filter: Option<Vec<String>>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hash: false,
            category_filter: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    RunningAutorunsc,
    ParsingCsv,
    CheckingFiles,
    EvaluatingRisk,
    VerifyingSignatures,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ScanProgress {
    pub task_id: u64,
    pub phase: ScanPhase,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SignatureProgress {
    pub task_id: u64,
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteResult {
    pub success: bool,
    pub message: String,
}
