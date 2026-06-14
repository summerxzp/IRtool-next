use crate::types::SuspiciousFlag;

/// Known system processes and their expected directories.
const SYSTEM_PROC_EXPECTED: &[(&str, &[&str])] = &[
    ("svchost.exe", &["\\system32\\", "\\syswow64\\"]),
    ("lsass.exe", &["\\system32\\"]),
    ("csrss.exe", &["\\system32\\"]),
    ("winlogon.exe", &["\\system32\\"]),
    ("services.exe", &["\\system32\\"]),
    ("smss.exe", &["\\system32\\"]),
    ("wininit.exe", &["\\system32\\"]),
    ("spoolsv.exe", &["\\system32\\"]),
];

/// Path fragments that indicate user-writable directories.
const SUSPICIOUS_PATH_FRAGMENTS: &[&str] = &[
    "\\temp\\",
    "\\tmp\\",
    "\\appdata\\roaming\\",
    "\\appdata\\local\\temp\\",
    "\\downloads\\",
    "\\desktop\\",
    "\\public\\",
    "\\recycle",
];

/// Check if a process name + exe path is suspicious.
/// Returns the first matching flag, or None.
pub fn check_suspicious(name: &str, exe: &str) -> Option<SuspiciousFlag> {
    if exe.is_empty() {
        return None;
    }

    let exe_lower = exe.to_lowercase();
    let name_lower = name.to_lowercase();

    // Check if a known system process is running from a non-standard path.
    for (proc_name, expected_dirs) in SYSTEM_PROC_EXPECTED {
        if name_lower == *proc_name {
            let in_expected = expected_dirs.iter().any(|d| exe_lower.contains(d));
            if !in_expected {
                return Some(SuspiciousFlag::SystemProcessNonStandardPath);
            }
        }
    }

    // Check if running from a user-writable directory.
    for fragment in SUSPICIOUS_PATH_FRAGMENTS {
        if exe_lower.contains(fragment) {
            return Some(SuspiciousFlag::UserWritablePath);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svchost_in_system32_is_ok() {
        assert_eq!(
            check_suspicious("svchost.exe", r"C:\Windows\System32\svchost.exe"),
            None
        );
    }

    #[test]
    fn svchost_in_temp_is_suspicious() {
        let result = check_suspicious("svchost.exe", r"C:\Temp\svchost.exe");
        assert!(matches!(result, Some(SuspiciousFlag::SystemProcessNonStandardPath)));
    }

    #[test]
    fn random_exe_in_temp_is_user_writable() {
        let result = check_suspicious("payload.exe", r"C:\Temp\payload.exe");
        assert!(matches!(result, Some(SuspiciousFlag::UserWritablePath)));
    }

    #[test]
    fn normal_path_is_clean() {
        assert_eq!(check_suspicious("notepad.exe", r"C:\Windows\notepad.exe"), None);
    }

    #[test]
    fn empty_exe_is_clean() {
        assert_eq!(check_suspicious("something.exe", ""), None);
    }

    #[test]
    fn appdata_roaming_is_suspicious() {
        let result = check_suspicious("agent.exe", r"C:\Users\user\AppData\Roaming\agent.exe");
        assert!(matches!(result, Some(SuspiciousFlag::UserWritablePath)));
    }
}
