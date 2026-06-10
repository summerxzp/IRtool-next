use std::path::PathBuf;

/// Centralized application directory management.
///
/// Supports two modes:
/// - **Portable**: `portable.flag` exists next to the executable → all data under `<exe_dir>/`
/// - **Installed**: no `portable.flag` → data under `%APPDATA%/IRtool/`
///
/// Directory layout (portable mode):
/// ```text
/// <exe_dir>/
/// ├── IRtool.exe
/// ├── portable.flag          ← triggers portable mode
/// ├── config/
/// │   ├── monitor.toml
/// │   ├── settings.json
/// │   └── sysmon.xml
/// ├── data/
/// │   └── monitor.db
/// ├── logs/
/// │   ├── app.log
/// │   ├── monitor.log
/// │   └── tools.log
/// └── tools/
///     ├── manifest.json
///     ├── autoruns/
///     ├── sigcheck/
///     └── sysmon/
/// ```
#[derive(Debug, Clone)]
pub struct AppDirs {
    root: PathBuf,
}

impl AppDirs {
    /// Detect and create AppDirs based on portable.flag presence.
    pub fn detect() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let portable_flag = exe_dir.join("portable.flag");
        let root = if portable_flag.exists() {
            exe_dir
        } else {
            // Installed mode: use AppData
            dirs::data_dir()
                .map(|d| d.join("IRtool"))
                .unwrap_or(exe_dir)
        };

        let dirs = Self { root };
        dirs.ensure_all();
        dirs
    }

    /// Ensure all subdirectories exist.
    fn ensure_all(&self) {
        for dir in &[&self.config_dir(), &self.data_dir(), &self.logs_dir(), &self.tools_dir()] {
            if !dir.exists() {
                let _ = std::fs::create_dir_all(dir);
            }
        }
    }

    /// Root data directory.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// `<root>/config/`
    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    /// `<root>/config/settings.json`
    pub fn settings_path(&self) -> PathBuf {
        self.config_dir().join("settings.json")
    }

    /// `<root>/config/monitor.toml`
    pub fn monitor_config_path(&self) -> PathBuf {
        self.config_dir().join("monitor.toml")
    }

    /// `<root>/config/sysmon.xml`
    pub fn sysmon_config_path(&self) -> PathBuf {
        self.config_dir().join("sysmon.xml")
    }

    /// `<root>/data/`
    pub fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    /// `<root>/data/monitor.db`
    pub fn monitor_db_path(&self) -> PathBuf {
        self.data_dir().join("monitor.db")
    }

    /// `<root>/logs/`
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// `<root>/tools/`
    pub fn tools_dir(&self) -> PathBuf {
        self.root.join("tools")
    }

    /// Whether running in portable mode.
    pub fn is_portable(&self) -> bool {
        self.root.join("portable.flag").exists()
    }
}
