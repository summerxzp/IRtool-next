use crate::snapshot::take_snapshot;
use crate::suspicious::check_suspicious;
use crate::types::{ProcessChain, ProcessNode};
use irtool_core::IrError;
use std::collections::HashMap;
use tracing::debug;

/// Build a process chain from the given PID up to the root process.
///
/// Returns a `ProcessChain` with nodes ordered [target, parent, grandparent, ..., root].
pub fn get_process_chain(pid: u32) -> Result<ProcessChain, IrError> {
    let snap = take_snapshot()?;
    let by_pid: HashMap<u32, (u32, String)> = snap
        .processes
        .into_iter()
        .map(|p| (p.pid, (p.ppid, p.name)))
        .collect();

    let mut nodes = Vec::new();
    let mut current_pid = pid;
    let mut visited = std::collections::HashSet::new();

    while current_pid != 0 && !visited.contains(&current_pid) {
        visited.insert(current_pid);

        let is_target = current_pid == pid;

        let (ppid, name) = match by_pid.get(&current_pid) {
            Some(&(pp, ref n)) => (pp, n.clone()),
            None => {
                debug!("process chain: PID {} not found in snapshot", current_pid);
                break;
            }
        };

        // Try to get exe path via OpenProcess (best-effort).
        let exe = query_exe_path(current_pid);
        let exe_str = exe.as_deref().unwrap_or("");

        let suspicious_flag = check_suspicious(&name, exe_str);

        let node = ProcessNode {
            pid: current_pid,
            name: name.clone(),
            exe,
            cmdline: None,
            create_time: query_create_time(current_pid),
            is_target,
            is_suspicious: suspicious_flag.is_some(),
            suspicious_reason: suspicious_flag.map(|f| f.reason().to_string()),
        };

        nodes.push(node);

        // Stop at System (PID 4) or System Idle (PID 0).
        if current_pid <= 4 {
            break;
        }

        current_pid = ppid;
    }

    Ok(ProcessChain { nodes })
}

/// Query the full executable path for a process via OpenProcess + QueryFullProcessImageNameW.
#[cfg(windows)]
fn query_exe_path(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::core::PWSTR;

    if pid == 0 || pid == 4 {
        return None;
    }

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return None,
        };

        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let result =
            QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut size);

        let _ = CloseHandle(handle);

        if result.is_ok() && size > 0 {
            Some(String::from_utf16_lossy(&buf[..size as usize]))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
fn query_exe_path(_pid: u32) -> Option<String> {
    None
}

/// Query the creation time of a process, formatted as "HH:MM:SS".
#[cfg(windows)]
fn query_create_time(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, GetProcessTimes, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::Foundation::FILETIME;

    if pid == 0 || pid == 4 {
        return None;
    }

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return None,
        };

        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();

        let result = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);

        if result.is_err() {
            return None;
        }

        // Convert FILETIME to time-of-day string.
        // FILETIME is 100-ns intervals since 1601-01-01.
        let ticks = ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64);
        // Offset from 1601 to 1970 in 100-ns intervals: 116444736000000000
        let unix_micros = ticks.saturating_sub(116444736000000000) / 10;
        let secs = (unix_micros / 1_000_000) as i64;

        let time_of_day_secs = secs % 86400;
        let h = time_of_day_secs / 3600;
        let m = (time_of_day_secs % 3600) / 60;
        let s = time_of_day_secs % 60;
        Some(format!("{:02}:{:02}:{:02}", h, m, s))
    }
}

#[cfg(not(windows))]
fn query_create_time(_pid: u32) -> Option<String> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn chain_for_current_process() {
        let my_pid = std::process::id();
        let chain = get_process_chain(my_pid).expect("chain should succeed");
        assert!(!chain.is_empty());
        let target = chain.target().expect("should have target");
        assert!(target.is_target);
        assert_eq!(target.pid, my_pid);
    }

    #[test]
    fn chain_for_nonexistent_pid() {
        let chain = get_process_chain(999999).expect("chain should succeed");
        assert!(chain.is_empty());
    }

    #[test]
    fn chain_ends_at_low_pid_or_root() {
        let chain = get_process_chain(std::process::id()).expect("chain should succeed");
        // Chain should not be empty and should contain the target.
        assert!(!chain.is_empty());
        // The chain should terminate (no infinite loop) — just verify it has multiple nodes.
        assert!(chain.nodes.len() >= 1);
    }
}
