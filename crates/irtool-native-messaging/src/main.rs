//! irtool-native-messaging-host — Chrome Native Messaging Host 二进制入口
//!
//! 由 Chrome/Edge 浏览器通过 Native Messaging 协议启动，
//! 读取 stdin 上的消息并写入队列文件供 IRtool 主进程消费。
//!
//! 此独立二进制是 NMH 的实际部署目标（install_helper.rs 注册的 path 指向它）。
//! 主 exe (irtool.exe) 因 app-manifest 要求 requireAdministrator，无法被非提升的
//! Chrome 进程启动，因此 NMH 功能由本独立二进制承担。

fn main() {
    use irtool_core::AppDirs;

    let app_dirs = AppDirs::detect();
    let log_dir = app_dirs.logs_dir();

    // 队列和 config 仍在 %TEMP%\irtool（NMH 与 service 的约定路径）
    let irtool_temp = std::env::temp_dir().join("irtool");
    let queue_dir = irtool_temp.join("attr-queue");
    let config_dir = irtool_temp.clone();

    let _ = std::fs::create_dir_all(&queue_dir);
    let _ = std::fs::create_dir_all(&log_dir);

    // NMH 日志写独立 rolling file（stdout/stderr 都不可用）
    let nmh_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("nmh")
        .filename_suffix("log")
        .max_log_files(3)
        .build(&log_dir)
        .unwrap_or_else(|e| {
            eprintln!("FATAL: failed to create NMH log appender: {}", e);
            std::process::exit(1);
        });

    let (nmh_nb, guard) = tracing_appender::non_blocking(nmh_appender);

    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_writer(nmh_nb)
        .with_ansi(false)
        .with_target(true)
        .with_timer(NmhTimer)
        .init();

    tracing::info!(
        pid = std::process::id(),
        "Native Messaging Host (standalone binary) started"
    );

    if let Err(e) = irtool_native_messaging::run_event_loop(&queue_dir, &config_dir) {
        tracing::error!("Native Messaging Host error: {:?}", e);
        drop(guard);
        std::process::exit(1);
    }

    drop(guard);
}

/// NMH 日志时间戳格式化（本地时间，与主应用 logger 一致：YYYY/MM/DD HH:MM:SS.mmm）
struct NmhTimer;

impl tracing_subscriber::fmt::time::FormatTime for NmhTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y/%m/%d %H:%M:%S%.3f"))
    }
}
