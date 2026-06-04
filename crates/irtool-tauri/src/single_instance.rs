use tauri::{AppHandle, Manager, Runtime};
use tracing::info;

pub fn handle_second_instance<R: Runtime>(app: &AppHandle<R>, args: Vec<String>, cwd: String) {
    info!(
        ?args,
        ?cwd,
        "second instance attempted; bringing existing window to front"
    );

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.show();
    }
}
