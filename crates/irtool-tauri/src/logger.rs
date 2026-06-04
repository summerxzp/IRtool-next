use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

pub struct LoggerGuard {
    _file_guard: WorkerGuard,
}

pub fn init_logger(log_dir: PathBuf) -> LoggerGuard {
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("failed to create rolling file appender");

    let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,irtool=debug,tauri=info"));

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_filter(env_filter);

    let console_layer =
        if cfg!(debug_assertions) {
            Some(
                fmt::layer().with_target(true).with_ansi(true).compact().with_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug,tauri=info")),
                ),
            )
        } else {
            None
        };

    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        log_dir = %log_dir.display(),
        "logger initialized"
    );

    LoggerGuard {
        _file_guard: file_guard,
    }
}
