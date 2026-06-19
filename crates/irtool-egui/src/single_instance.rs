/// Cross-process single instance guard using a named Windows mutex.
///
/// Holds the mutex handle alive for the process lifetime. Drops when the
/// app exits, automatically releasing the mutex.

#[cfg(windows)]
pub struct SingleInstanceGuard {
    _handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
pub fn check_and_acquire() -> Option<SingleInstanceGuard> {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::w;

    let mutex_name = w!("Global\\IRtool-SingleInstance");
    let handle = match unsafe { CreateMutexW(None, true, mutex_name) } {
        Ok(h) => h,
        Err(_) => {
            tracing::warn!("failed to create single-instance mutex, proceeding anyway");
            return Some(SingleInstanceGuard { _handle: HANDLE(std::ptr::null_mut()) });
        }
    };
    if handle.0.is_null() {
        tracing::warn!("failed to create single-instance mutex, proceeding anyway");
        return Some(SingleInstanceGuard { _handle: handle });
    }
    let err = unsafe { windows::Win32::Foundation::GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        tracing::error!("another IRtool instance is already running. Exiting.");
        eprintln!("错误：IRtool 已有实例在运行，请勿同时启动多个实例。");
        return None;
    }
    Some(SingleInstanceGuard { _handle: handle })
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard;

#[cfg(not(windows))]
pub fn check_and_acquire() -> Option<SingleInstanceGuard> {
    Some(SingleInstanceGuard)
}
