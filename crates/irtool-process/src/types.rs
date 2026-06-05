use serde::{Deserialize, Serialize};
use specta::Type;

/// Why a process was flagged as suspicious.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SuspiciousFlag {
    /// A known system process (e.g. svchost.exe) running from a non-standard directory.
    SystemProcessNonStandardPath,
    /// Running from a user-writable directory (temp, appdata, downloads, etc.).
    UserWritablePath,
}

impl SuspiciousFlag {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::SystemProcessNonStandardPath => "系统进程位于非标准路径",
            Self::UserWritablePath => "运行于用户可写目录",
        }
    }
}

/// A single process in a chain, with optional suspicion metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessNode {
    pub pid: u32,
    pub name: String,
    /// Full executable path, if available.
    pub exe: Option<String>,
    /// Command line, if available.
    pub cmdline: Option<String>,
    /// Process creation time as formatted string "HH:MM:SS", if available.
    pub create_time: Option<String>,
    /// Whether this is the target process the chain was requested for.
    pub is_target: bool,
    /// Whether this process was flagged as suspicious.
    pub is_suspicious: bool,
    /// Why the process is suspicious, if flagged.
    pub suspicious_reason: Option<String>,
}

/// A chain from a target process up to the root (PID 0 or 4).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessChain {
    /// Ordered [target, parent, grandparent, ..., root].
    pub nodes: Vec<ProcessNode>,
}

impl ProcessChain {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn target(&self) -> Option<&ProcessNode> {
        self.nodes.first()
    }

    pub fn suspicious_nodes(&self) -> Vec<&ProcessNode> {
        self.nodes.iter().filter(|n| n.is_suspicious).collect()
    }
}

/// A lightweight process entry from a system-wide snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessEntry {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub exe: Option<String>,
}

/// Full process snapshot result.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProcessSnapshot {
    pub processes: Vec<ProcessEntry>,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspicious_flag_serializes() {
        let flag = SuspiciousFlag::UserWritablePath;
        let json = serde_json::to_string(&flag).unwrap();
        assert!(json.contains("user_writable_path"));
    }

    #[test]
    fn process_chain_accessors() {
        let chain = ProcessChain {
            nodes: vec![
                ProcessNode {
                    pid: 100,
                    name: "target.exe".into(),
                    exe: Some(r"C:\temp\target.exe".into()),
                    cmdline: None,
                    create_time: None,
                    is_target: true,
                    is_suspicious: true,
                    suspicious_reason: Some("运行于用户可写目录".into()),
                },
                ProcessNode {
                    pid: 4,
                    name: "System".into(),
                    exe: None,
                    cmdline: None,
                    create_time: None,
                    is_target: false,
                    is_suspicious: false,
                    suspicious_reason: None,
                },
            ],
        };
        assert!(!chain.is_empty());
        assert_eq!(chain.target().unwrap().pid, 100);
        assert_eq!(chain.suspicious_nodes().len(), 1);
    }
}
