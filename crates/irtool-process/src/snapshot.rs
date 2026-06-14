use crate::types::{ProcessEntry, ProcessSnapshot};
use irtool_core::IrError;

/// Take a snapshot of all running processes.
#[cfg(windows)]
pub fn take_snapshot() -> Result<ProcessSnapshot, IrError> {
    use std::time::SystemTime;
    use windows::Win32::System::Diagnostics::ToolHelp::*;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| IrError::Internal(format!("CreateToolhelp32Snapshot failed: {}", e)))?;

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut processes = Vec::with_capacity(512);

        if Process32FirstW(snap, &mut entry).is_err() {
            return Ok(ProcessSnapshot { processes, timestamp });
        }

        loop {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len())],
            );

            processes.push(ProcessEntry {
                pid: entry.th32ProcessID,
                ppid: entry.th32ParentProcessID,
                name,
                exe: None,
            });

            if Process32NextW(snap, &mut entry).is_err() {
                break;
            }
        }

        Ok(ProcessSnapshot { processes, timestamp })
    }
}

#[cfg(not(windows))]
pub fn take_snapshot() -> Result<ProcessSnapshot, IrError> {
    Err(IrError::FeatureDisabled("process snapshot requires Windows".into()))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_processes() {
        let snap = take_snapshot().expect("snapshot should succeed");
        assert!(!snap.processes.is_empty());
        let my_pid = std::process::id();
        assert!(snap.processes.iter().any(|p| p.pid == my_pid));
    }

    #[test]
    fn snapshot_has_timestamp() {
        let snap = take_snapshot().expect("snapshot should succeed");
        assert!(snap.timestamp > 0);
    }
}
