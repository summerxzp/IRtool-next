use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

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
        .expect("failed to create app rolling file appender");

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
        .with_filter(app_filter);

    // --- monitor.log: irtool_monitor crate only ---
    let monitor_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool-monitor")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("failed to create monitor rolling file appender");

    let (monitor_nb, monitor_guard) = tracing_appender::non_blocking(monitor_appender);

    let monitor_filter = EnvFilter::new("irtool_monitor=debug");

    let monitor_layer = fmt::layer()
        .with_writer(monitor_nb)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_filter(monitor_filter);

    // --- tools.log: irtool_tools crate only ---
    let tools_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool-tools")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("failed to create tools rolling file appender");

    let (tools_nb, tools_guard) = tracing_appender::non_blocking(tools_appender);

    let tools_filter = EnvFilter::new("irtool_tools=debug");

    let tools_layer = fmt::layer()
        .with_writer(tools_nb)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_filter(tools_filter);

    // --- console layer (debug builds only) ---
    let console_layer = if cfg!(debug_assertions) {
        Some(
            fmt::layer().with_target(true).with_ansi(true).compact().with_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug,tauri=info")),
            ),
        )
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
