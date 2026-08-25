// i18n（P4）：编译期嵌入 crate 根 locales/{zh-CN,en-US}.json；
// en 缺键回退 zh-CN。键名与 React ui/src/locales 一致（scripts/i18n-egui-sync.mjs 同步）。
rust_i18n::i18n!("locales", fallback = "zh-CN");

pub mod design;

mod app;
mod event_bridge;
mod icon_cache;
mod layout;
mod nav;
// pub：供集成测试（tests/p5_network_table.rs kittest 交互验证）访问页面状态类型
pub mod pages;
mod single_instance;
mod theme;
mod widgets;

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
    let log_guard = init_logger(app_dirs.logs_dir());

    info!("============================================");
    info!("IRtool v{} starting (egui frontend)", env!("CARGO_PKG_VERSION"));
    info!("Root: {}", app_dirs.root().display());
    info!("Startup mode: {:?}", mode);

    // Check for existing instance (guard must be held for process lifetime)
    let single_instance_guard = match single_instance::check_and_acquire() {
        Some(g) => g,
        None => std::process::exit(1),
    };

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
    let app = app::IrtoolApp::new(app_ctx, bridge, rt, mode, log_guard, single_instance_guard);

    // Launch eframe
    let icon = load_icon();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false) // 自绘标题栏（顶栏即标题栏，对齐 React 版）
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native("IRtool", native_options, Box::new(|_cc| Ok(Box::new(app)))).unwrap_or_else(|e| {
        tracing::error!("eframe error: {}", e);
        std::process::exit(1);
    });
}

fn init_logger(log_dir: std::path::PathBuf) -> tracing_appender::non_blocking::WorkerGuard {
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

    let (app_nb, guard) = tracing_appender::non_blocking(app_appender);

    let app_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,irtool=debug"));

    let app_layer = fmt::layer()
        .with_writer(app_nb)
        .with_ansi(false)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .with_timer(LocalTimer)
        .with_filter(app_filter);

    let console_layer = if cfg!(debug_assertions) {
        Some(
            fmt::layer()
                .with_target(true)
                .with_ansi(true)
                .with_timer(LocalTimer)
                .compact()
                .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug,wmi=warn"))),
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(app_layer)
        .with(console_layer)
        .init();

    guard
}

/// 加载窗口图标：编译时嵌入 icon.png，运行时解码为 RGBA 像素数据。
fn load_icon() -> egui::IconData {
    const ICON_BYTES: &[u8] = include_bytes!("../../irtool-tauri/icons/icon.png");
    match image::load_from_memory(ICON_BYTES) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            }
        }
        Err(e) => {
            tracing::warn!("failed to load icon: {}", e);
            egui::IconData::default()
        }
    }
}
