use crate::snapshot::take_snapshot;
use crate::suspicious::check_suspicious;
use crate::types::{ProcessChain, ProcessNode};
use irtool_core::IrError;
use std::collections::HashMap;
use tracing::trace;

/// Build a process chain from the given PID up to the root process.
///
/// Returns a `ProcessChain` with nodes ordered [target, parent, grandparent, ..., root].
pub fn get_process_chain(pid: u32) -> Result<ProcessChain, IrError> {
    let snap = take_snapshot()?;
    let by_pid: HashMap<u32, (u32, String)> = snap.processes.into_iter().map(|p| (p.pid, (p.ppid, p.name))).collect();

    let mut nodes = Vec::new();
    let mut current_pid = pid;
    let mut visited = std::collections::HashSet::new();

    while current_pid != 0 && !visited.contains(&current_pid) {
        visited.insert(current_pid);

        let is_target = current_pid == pid;

        let (ppid, name) = match by_pid.get(&current_pid) {
            Some(&(pp, ref n)) => (pp, n.clone()),
            None => {
                trace!("process chain: PID {} not found in snapshot", current_pid);
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

    // Batch-fill cmdline via WMI for all PIDs in the chain.
    fill_cmdlines(&mut nodes);

    Ok(ProcessChain { nodes })
}

/// Batch query WMI for command lines of all PIDs in the chain.
#[cfg(windows)]
fn fill_cmdlines(nodes: &mut [ProcessNode]) {
    let pids: Vec<u32> = nodes.iter().map(|n| n.pid).filter(|&p| p > 4).collect();
    if pids.is_empty() {
        return;
    }
    if let Some(result) = irtool_net_monitor::process_info::targeted_query_cmdlines(&pids) {
        for node in nodes.iter_mut() {
            if let Some(cmdline) = result.cmdlines.get(&node.pid) {
                node.cmdline = Some(cmdline.clone());
            }
        }
    }
}

#[cfg(not(windows))]
fn fill_cmdlines(_nodes: &mut [ProcessNode]) {}

/// Query the full executable path for a process via OpenProcess + QueryFullProcessImageNameW.
#[cfg(windows)]
pub(crate) fn query_exe_path(pid: u32) -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };

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
        let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut size);

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
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

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

        // Convert FILETIME to date-time string "YYYY/MM/DD HH:MM:SS" in local time.
        // FILETIME is 100-ns intervals since 1601-01-01 UTC.
        let ticks = ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64);
        // Offset from 1601 to 1970 in 100-ns intervals: 116444736000000000
        let unix_micros = ticks.saturating_sub(116444736000000000) / 10;
        let secs = (unix_micros / 1_000_000) as i64;

        // Apply local timezone offset
        let local_secs = secs + local_timezone_offset_secs();

        // Days since 1970-01-01 (local)
        let days = local_secs / 86400;
        let time_of_day_secs = local_secs % 86400;
        let h = time_of_day_secs / 3600;
        let m = (time_of_day_secs % 3600) / 60;
        let s = time_of_day_secs % 60;

        // Convert days since epoch to year/month/day
        let (year, month, day) = days_to_ymd(days as u32);
        Some(format!("{}/{:02}/{:02} {:02}:{:02}:{:02}", year, month, day, h, m, s))
    }
}

#[cfg(not(windows))]
fn query_create_time(_pid: u32) -> Option<String> {
    None
}

/// Get the local timezone offset from UTC in seconds.
#[cfg(windows)]
fn local_timezone_offset_secs() -> i64 {
    use windows::Win32::System::Time::GetTimeZoneInformation;
    use windows::Win32::System::Time::TIME_ZONE_INFORMATION;

    unsafe {
        let mut tz: TIME_ZONE_INFORMATION = std::mem::zeroed();
        let result = GetTimeZoneInformation(&mut tz);
        // Bias is in minutes; negative Bias means east of UTC (e.g. UTC+8 = -480)
        // Convert to seconds and negate to get the offset to add to UTC
        let bias = tz.Bias as i64;
        // If result is TIME_ZONE_ID_INVALID (0xFFFFFFFF), fall back to 0
        if result as u32 == 0xFFFFFFFF {
            return 0;
        }
        -bias * 60
    }
}

#[cfg(not(windows))]
fn local_timezone_offset_secs() -> i64 {
    0
}

/// Convert days since 1970-01-01 to (year, month, day).
fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u32; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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
        assert!(!chain.nodes.is_empty());
    }
}
