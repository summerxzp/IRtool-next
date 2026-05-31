use serde::Serialize;
use specta::Type;

#[derive(thiserror::Error, Debug, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum IrError {
    #[error("io: {0}")]
    Io(String),

    #[error("permission denied: requires administrator")]
    PermissionDenied,

    #[error("external tool failed: {tool} exit={code}")]
    ExternalTool { tool: String, code: i32 },

    #[error("parse error: {0}")]
    Parse(String),

    #[error("network: {0}")]
    Network(String),

    #[error("cancelled")]
    Cancelled,

    #[error("feature disabled: {0}")]
    FeatureDisabled(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<std::io::Error> for IrError {
    fn from(value: std::io::Error) -> Self {
        IrError::Io(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_with_kind_tag() {
        let err = IrError::ExternalTool {
            tool: "autorunsc".into(),
            code: 1,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"external_tool\""));
        assert!(json.contains("\"code\":1"));
    }

    #[test]
    fn permission_denied_renders() {
        let err = IrError::PermissionDenied;
        assert_eq!(err.to_string(), "permission denied: requires administrator");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let ir: IrError = io_err.into();
        match ir {
            IrError::Io(msg) => assert!(msg.contains("missing")),
            _ => panic!("expected Io variant"),
        }
    }
}
