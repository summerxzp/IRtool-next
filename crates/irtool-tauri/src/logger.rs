use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

/// 本地时间计时器（UTC+8），用于日志时间戳格式化。
struct LocalTimer;

impl fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y/%m/%d %H:%M:%S%.3f"))
    }
}

pub struct LoggerGuard {
    _app_guard: WorkerGuard,
    _monitor_guard: WorkerGuard,
    _tools_guard: WorkerGuard,
}

pub fn init_logger(log_dir: PathBuf) -> LoggerGuard {
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    // --- app.log: all logs ---
    let app_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool-app")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .unwrap_or_else(|e| {
            eprintln!(
                "FATAL: failed to create app log appender at {}: {}",
                log_dir.display(),
                e
            );
            std::process::exit(1);
        });

    let (app_nb, app_guard) = tracing_appender::non_blocking(app_appender);

    let app_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,irtool=debug,tauri=info"));

    let app_layer = fmt::layer()
        .with_writer(app_nb)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_timer(LocalTimer)
        .with_filter(app_filter);

    // --- monitor.log: irtool_monitor crate only ---
    let monitor_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool-monitor")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .unwrap_or_else(|e| {
            eprintln!(
                "FATAL: failed to create monitor log appender at {}: {}",
                log_dir.display(),
                e
            );
            std::process::exit(1);
        });

    let (monitor_nb, monitor_guard) = tracing_appender::non_blocking(monitor_appender);

    let monitor_filter = EnvFilter::new("irtool_monitor=debug");

    let monitor_layer = fmt::layer()
        .with_writer(monitor_nb)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_timer(LocalTimer)
        .with_filter(monitor_filter);

    // --- tools.log: irtool_tools crate only ---
    let tools_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool-tools")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .unwrap_or_else(|e| {
            eprintln!(
                "FATAL: failed to create tools log appender at {}: {}",
                log_dir.display(),
                e
            );
            std::process::exit(1);
        });

    let (tools_nb, tools_guard) = tracing_appender::non_blocking(tools_appender);

    let tools_filter = EnvFilter::new("irtool_tools=debug");

    let tools_layer = fmt::layer()
        .with_writer(tools_nb)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_timer(LocalTimer)
        .with_filter(tools_filter);

    // --- console layer (debug builds only) ---
    let console_layer = if cfg!(debug_assertions) {
        Some(fmt::layer().with_target(true).with_ansi(true).with_timer(LocalTimer).compact().with_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug,tauri=info,wmi=warn")),
        ))
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(app_layer)
        .with(monitor_layer)
        .with(tools_layer)
        .with(console_layer)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        log_dir = %log_dir.display(),
        "logger initialized"
    );

    LoggerGuard {
        _app_guard: app_guard,
        _monitor_guard: monitor_guard,
        _tools_guard: tools_guard,
    }
}
