use irtool_core::IrError;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError};
#[cfg(windows)]
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

#[cfg(windows)]
pub fn kill_process(pid: u32) -> Result<(), IrError> {
    if pid == 0 || pid == 4 {
        return Err(IrError::Internal(format!(
            "refuse to kill system pid {}",
            pid
        )));
    }
    unsafe {
        match OpenProcess(PROCESS_TERMINATE, false, pid) {
            Ok(handle) => {
                let result = TerminateProcess(handle, 1);
                let _ = CloseHandle(handle);
                if result.is_err() {
                    return Err(IrError::Internal(format!(
                        "TerminateProcess failed for pid {}",
                        pid
                    )));
                }
                Ok(())
            }
            Err(e) => {
                let last = GetLastError().0;
                if e.code().0 as u32 == 0x80070005 || last == ERROR_ACCESS_DENIED.0 {
                    Err(IrError::PermissionDenied)
                } else {
                    Err(IrError::Internal(format!(
                        "OpenProcess failed for pid {}: {}",
                        pid, e
                    )))
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn kill_process(_pid: u32) -> Result<(), IrError> {
    Err(IrError::Internal("kill only supported on Windows".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_system_idle_refused() {
        let r = kill_process(0);
        assert!(r.is_err());
    }

    #[test]
    fn kill_system_refused() {
        let r = kill_process(4);
        assert!(r.is_err());
    }
}
