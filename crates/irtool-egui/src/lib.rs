mod app;
mod event_bridge;
mod layout;
mod nav;
mod pages;
mod theme;
mod widgets;
mod single_instance;

use irtool_core::AppDirs;
use irtool_service::context::AppContext;
use irtool_service::services::network::NetworkService;
use tracing::info;

/// 启动模式：区分独立运行与 WebView2 缺失时的 fallback。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartupMode {
    /// 独立运行（irtool-egui 二进制直接启动）
    Normal,
    /// 作为 WebView2 缺失的 fallback 启动
    Fallback,
}

/// Entry point for the egui frontend.
/// Called directly from `main()` or from `irtool-tauri` as a WebView2 fallback.
pub fn run(mode: StartupMode) {
    let app_dirs = AppDirs::detect();

    // Initialize logger (reuse irtool-tauri's logger pattern)
    init_logger(app_dirs.logs_dir());

    info!("============================================");
    info!("IRtool v{} starting (egui frontend)", env!("CARGO_PKG_VERSION"));
    info!("Root: {}", app_dirs.root().display());
    info!("Startup mode: {:?}", mode);

    // Check for existing instance
    if single_instance::check_and_acquire().is_none() {
        std::process::exit(1);
    }

    info!("============================================");

    // Create tokio runtime in a dedicated thread
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .expect("failed to create tokio runtime");

    // Create AppContext
    let ctx = AppContext::new(app_dirs);

    // Start default network polling
    NetworkService { ctx: &ctx }.start_default_polling(rt.handle().clone());

    // Create EventBus bridge
    let bridge = event_bridge::EventBridge::new(&ctx, rt.handle());

    // Build egui app
    let app_ctx = ctx.clone();
    let app = app::IrtoolApp::new(app_ctx, bridge, rt, mode);

    // Launch eframe
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "IRtool",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .unwrap_or_else(|e| {
        tracing::error!("eframe error: {}", e);
        std::process::exit(1);
    });
}

fn init_logger(log_dir: std::path::PathBuf) {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter, Layer};

    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    let app_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("irtool-egui-app")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .unwrap_or_else(|e| {
            eprintln!("FATAL: failed to create log appender: {}", e);
            std::process::exit(1);
        });

    let (app_nb, _guard) = tracing_appender::non_blocking(app_appender);

    let app_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,irtool=debug"));

    let app_layer = fmt::layer()
        .with_writer(app_nb)
        .with_ansi(false)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .with_filter(app_filter);

    let console_layer = if cfg!(debug_assertions) {
        Some(
            fmt::layer()
                .with_target(true)
                .with_ansi(true)
                .compact()
                .with_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("debug,wmi=warn")),
                ),
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(app_layer)
        .with(console_layer)
        .init();

    // Leak the guard so the logger stays alive for the app lifetime
    std::mem::forget(_guard);
}
