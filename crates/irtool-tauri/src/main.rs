#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "egui-fallback")]
use windows::core::PCWSTR;
#[cfg(feature = "egui-fallback")]
use windows::Win32::System::Registry::HKEY;

fn main() {
    #[cfg(feature = "egui-fallback")]
    if !is_webview2_available() {
        tracing::warn!("WebView2 not available, falling back to egui frontend");
        irtool_egui::run(irtool_egui::StartupMode::Fallback);
        return;
    }

    irtool_lib::run();
}

/// Check if WebView2 runtime is available via Windows registry.
///
/// Validates not just key existence but also the `pv` version value,
/// to avoid false positives from leftover registry keys after uninstall.
/// Checks both HKLM (system-wide) and HKCU (per-user) installations.
#[cfg(feature = "egui-fallback")]
fn is_webview2_available() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let paths = [
        w!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        w!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        w!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{2CD8A007-E189-409D-A2C8-9AF4EF3C72AA}"),
    ];

    // Check HKLM first (system-wide install), then HKCU (per-user install)
    for path in &paths {
        if check_registry_key(HKEY_LOCAL_MACHINE, *path) {
            return true;
        }
    }
    for path in &paths {
        if check_registry_key(HKEY_CURRENT_USER, *path) {
            return true;
        }
    }

    false
}

/// Open a registry key under `root` at `path` and validate its `pv` value.
#[cfg(feature = "egui-fallback")]
fn check_registry_key(root: HKEY, path: PCWSTR) -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, KEY_READ, RRF_RT_REG_SZ,
    };

    let mut key = HKEY::default();
    let ok = unsafe { RegOpenKeyExW(root, path, None, KEY_READ, &mut key) };
    if ok.is_err() {
        return false;
    }
    // Read pv value (REG_SZ) and validate
    let mut buf = [0u16; 64];
    let mut len: u32 = (buf.len() * 2) as u32;
    let pv_ok = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            w!("pv"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut len),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }

    if pv_ok.is_ok() && len > 0 {
        let pv = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
        !pv.is_empty() && pv != "0.0.0.0"
    } else {
        false
    }
}
