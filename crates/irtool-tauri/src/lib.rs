use tauri::Manager;

#[tauri::command]
fn cmd_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "IRtool",
        "version": env!("CARGO_PKG_VERSION"),
        "build": "alpha",
    })
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![cmd_app_info])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
