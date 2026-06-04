use crate::csv_parser::RawEntry;
use crate::types::{RiskLevel, SignatureStatus};

const TRUSTED_SYSTEM_PATHS: &[&str] = &[
    r"c:\windows\system32",
    r"c:\windows\syswow64",
    r"c:\program files",
    r"c:\program files (x86)",
];

const HIGH_RISK_PATHS: &[&str] = &[
    r"\appdata\",
    r"\temp\",
    r"\tmp\",
    r"\downloads\",
    r"\desktop\",
    r"\documents\",
];

const TRUSTED_PUBLISHERS: &[&str] = &["microsoft", "windows", "intel", "nvidia", "amd"];

pub struct FileInfo {
    pub exists: bool,
    pub size: Option<u64>,
}

pub fn evaluate(entry: &RawEntry, file_info: Option<&FileInfo>) -> (RiskLevel, Vec<String>) {
    let mut reasons = Vec::new();
    let image_path = &entry.image_path;
    let publisher = &entry.publisher;

    let is_verified = is_verified_signature(&entry.signer);
    let is_system_path = is_trusted_system_path(image_path);
    let is_high_risk_path = is_high_risk_path(image_path);
    let file_exists = file_info.map(|f| f.exists).unwrap_or(true);

    if !file_exists {
        reasons.push("文件不存在".into());
        return (RiskLevel::HighRisk, reasons);
    }
    if !is_verified && !is_system_path && is_high_risk_path {
        reasons.push("无有效签名且不在系统目录".into());
        reasons.push("位于用户可写目录".into());
        return (RiskLevel::HighRisk, reasons);
    }

    if !is_verified && !is_system_path && !is_trusted_publisher(publisher) {
        reasons.push("非系统目录且发布者未知".into());
    }
    if !is_verified && is_system_path {
        reasons.push("系统目录文件但签名验证失败".into());
    }
    if is_verified && is_high_risk_path {
        reasons.push("虽有签名但位于用户可写目录".into());
    }
    if !reasons.is_empty() {
        return (RiskLevel::Suspicious, reasons);
    }

    (RiskLevel::Safe, vec!["无明显风险特征".into()])
}

fn is_verified_signature(signer: &str) -> bool {
    signer.to_lowercase().contains("(verified)")
}

fn is_trusted_system_path(path: &str) -> bool {
    if path.is_empty() || path.eq_ignore_ascii_case("file not found") {
        return false;
    }
    let lower = path.to_lowercase();
    TRUSTED_SYSTEM_PATHS.iter().any(|p| lower.starts_with(p))
}

fn is_high_risk_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let lower = path.to_lowercase();
    HIGH_RISK_PATHS.iter().any(|p| lower.contains(p))
}

fn is_trusted_publisher(publisher: &str) -> bool {
    if publisher.is_empty() {
        return false;
    }
    let lower = publisher.to_lowercase();
    TRUSTED_PUBLISHERS.iter().any(|p| lower.contains(p))
}

pub fn parse_signer_status(raw_signer: &str) -> SignatureStatus {
    if raw_signer.is_empty() {
        return SignatureStatus::Unsigned;
    }
    let lower = raw_signer.to_lowercase();
    if lower.contains("(verified)") {
        let signer = raw_signer
            .replace("(Verified)", "")
            .replace("(verified)", "")
            .trim()
            .to_owned();
        return SignatureStatus::Valid { signer };
    }
    if lower.contains("(error)") {
        let message = raw_signer
            .replace("(Error)", "")
            .replace("(error)", "")
            .trim()
            .to_owned();
        return SignatureStatus::Invalid { message };
    }
    SignatureStatus::Unsigned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(image_path: &str, publisher: &str, signer: &str) -> RawEntry {
        RawEntry {
            location: String::new(),
            entry: String::new(),
            enabled: String::new(),
            category: String::new(),
            description: String::new(),
            publisher: publisher.to_owned(),
            image_path: image_path.to_owned(),
            launch_string: String::new(),
            timestamp: String::new(),
            md5: String::new(),
            sha256: String::new(),
            signer: signer.to_owned(),
            version: String::new(),
        }
    }

    #[test]
    fn file_not_found_is_high_risk() {
        let entry = make_entry("C:\\nonexistent.exe", "", "");
        let info = FileInfo {
            exists: false,
            size: None,
        };
        let (level, _) = evaluate(&entry, Some(&info));
        assert_eq!(level, RiskLevel::HighRisk);
    }

    #[test]
    fn appdata_unsigned_is_high_risk() {
        let entry = make_entry(r"C:\Users\x\AppData\Local\evil.exe", "EvilCo", "");
        let info = FileInfo {
            exists: true,
            size: Some(1024),
        };
        let (level, reasons) = evaluate(&entry, Some(&info));
        assert_eq!(level, RiskLevel::HighRisk);
        assert!(reasons.iter().any(|r| r.contains("用户可写目录")));
    }

    #[test]
    fn system32_unsigned_is_suspicious() {
        let entry = make_entry(r"C:\Windows\System32\foo.dll", "", "");
        let info = FileInfo {
            exists: true,
            size: None,
        };
        let (level, _) = evaluate(&entry, Some(&info));
        assert_eq!(level, RiskLevel::Suspicious);
    }

    #[test]
    fn verified_system32_is_safe() {
        let entry = make_entry(
            r"C:\Windows\System32\svchost.exe",
            "Microsoft",
            "Microsoft Corporation (Verified)",
        );
        let info = FileInfo {
            exists: true,
            size: None,
        };
        let (level, _) = evaluate(&entry, Some(&info));
        assert_eq!(level, RiskLevel::Safe);
    }

    #[test]
    fn parse_signer_verified() {
        let status = parse_signer_status("Microsoft Corporation (Verified)");
        assert!(matches!(status, SignatureStatus::Valid { signer } if signer == "Microsoft Corporation"));
    }

    #[test]
    fn parse_signer_error() {
        let status = parse_signer_status("Revoked (Error)");
        assert!(matches!(status, SignatureStatus::Invalid { message } if message == "Revoked"));
    }

    #[test]
    fn parse_signer_empty_is_unsigned() {
        let status = parse_signer_status("");
        assert_eq!(status, SignatureStatus::Unsigned);
    }
}
