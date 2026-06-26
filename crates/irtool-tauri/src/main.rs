#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "egui-fallback")]
use windows::core::PCWSTR;
#[cfg(feature = "egui-fallback")]
use windows::Win32::System::Registry::HKEY;

fn main() {
    // P1.1: 检测 --native-messaging-host 参数，进入 NMH 模式
    // 这样 install_helper 的 current_exe() 路径正确指向主 exe
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--native-messaging-host" {
        run_native_messaging_host();
        return;
    }

    #[cfg(feature = "egui-fallback")]
    if !is_webview2_available() {
        eprintln!("WebView2 not available, falling back to egui frontend");
        irtool_egui::run(irtool_egui::StartupMode::Fallback);
        return;
    }

    irtool_lib::run();
}

/// P1.1: Native Messaging Host 模式入口
///
/// 由 Chrome 浏览器通过 Native Messaging 协议启动（主 exe + --native-messaging-host 参数），
/// 读取 stdin 上的消息并写入队列文件供 IRtool 主进程消费。
fn run_native_messaging_host() {
    // 队列目录: %TEMP%\irtool\attr-queue
    let queue_dir = std::env::temp_dir().join("irtool").join("attr-queue");
    // 配置目录: %TEMP%\irtool（service 写 config.json 于此）
    let config_dir = std::env::temp_dir().join("irtool");

    // 初始化日志：写入 stderr，避免干扰 stdout 的 Native Messaging 协议
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_writer(std::io::stderr)
        .init();

    // 确保队列目录存在
    let _ = std::fs::create_dir_all(&queue_dir);

    // 运行事件循环（阻塞），直到 stdin 关闭
    if let Err(e) = irtool_native_messaging::run_event_loop(&queue_dir, &config_dir) {
        eprintln!("Native Messaging Host error: {:?}", e);
        std::process::exit(1);
    }
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
    use windows::Win32::System::Registry::{RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, KEY_READ, RRF_RT_REG_SZ};

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
