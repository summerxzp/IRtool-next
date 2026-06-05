use crate::models::SysmonStatus;
use irtool_core::IrError;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

const SYSMON_SERVICE_NAME: &str = "Sysmon64";
const SYSMON_SERVICE_NAME_ALT: &str = "Sysmon";

pub struct SysmonConfigManager {
    pub sysmon_exe_path: PathBuf,
    pub config_path: PathBuf,
    marker_file: PathBuf,
}

impl SysmonConfigManager {
    pub fn new(sysmon_exe_path: Option<PathBuf>, config_path: Option<PathBuf>, app_dir: &Path) -> Self {
        let exe = sysmon_exe_path.unwrap_or_else(|| find_sysmon_exe(app_dir));
        let cfg = config_path.unwrap_or_else(|| app_dir.join("tools").join("sysmon_config.xml"));
        let marker = app_dir.join(".sysmon_started_by_irtool");
        Self { sysmon_exe_path: exe, config_path: cfg, marker_file: marker }
    }

    #[cfg(windows)]
    pub fn is_installed(&self) -> bool {
        is_service_installed(SYSMON_SERVICE_NAME) || is_service_installed(SYSMON_SERVICE_NAME_ALT)
    }
    #[cfg(not(windows))]
    pub fn is_installed(&self) -> bool { false }

    #[cfg(windows)]
    pub fn is_running(&self) -> bool {
        is_service_running(SYSMON_SERVICE_NAME) || is_service_running(SYSMON_SERVICE_NAME_ALT)
    }
    #[cfg(not(windows))]
    pub fn is_running(&self) -> bool { false }

    #[cfg(windows)]
    pub fn get_service_name(&self) -> Option<String> {
        if is_service_installed(SYSMON_SERVICE_NAME) { Some(SYSMON_SERVICE_NAME.to_string()) }
        else if is_service_installed(SYSMON_SERVICE_NAME_ALT) { Some(SYSMON_SERVICE_NAME_ALT.to_string()) }
        else { None }
    }
    #[cfg(not(windows))]
    pub fn get_service_name(&self) -> Option<String> { None }

    /// Install Sysmon. If already installed, falls back to update_config.
    pub fn install(&self, accept_eula: bool) -> Result<(bool, String), IrError> {
        if self.is_installed() {
            info!("Sysmon already installed, updating config instead");
            return self.update_config();
        }
        if !self.sysmon_exe_path.exists() {
            return Ok((false, format!("找不到 Sysmon: {}", self.sysmon_exe_path.display())));
        }
        if !self.config_path.exists() {
            return Ok((false, format!("找不到配置文件: {}", self.config_path.display())));
        }

        let mut cmd = std::process::Command::new(&self.sysmon_exe_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if accept_eula { cmd.arg("-accepteula"); }
        cmd.arg("-i").arg(&self.config_path);

        info!("Installing Sysmon: {:?}", cmd);

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    self.mark_started_by_irtool();
                    Ok((true, "Sysmon 安装成功".to_string()))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // "Usage" means already installed
                    if stdout.contains("Usage") || stderr.contains("Usage") {
                        return self.update_config();
                    }
                    // "wevtutil"/"manifest" error → retry once
                    if stdout.contains("wevtutil") || stderr.contains("wevtutil")
                        || stdout.to_lowercase().contains("manifest") || stderr.to_lowercase().contains("manifest") {
                        warn!("Sysmon install hit wevtutil/manifest error, retrying after delay...");
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        return self.run_install_cmd(accept_eula);
                    }
                    let msg = if stderr.is_empty() { stdout.to_string() } else { stderr.to_string() };
                    Ok((false, format!("安装失败: {}", msg.trim())))
                }
            }
            Err(e) => Ok((false, format!("安装异常: {}", e))),
        }
    }

    fn run_install_cmd(&self, accept_eula: bool) -> Result<(bool, String), IrError> {
        let mut cmd = std::process::Command::new(&self.sysmon_exe_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        if accept_eula { cmd.arg("-accepteula"); }
        cmd.arg("-i").arg(&self.config_path);

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    self.mark_started_by_irtool();
                    Ok((true, "Sysmon 安装成功".to_string()))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("Usage") || stderr.contains("Usage") {
                        return self.update_config();
                    }
                    let msg = if stderr.is_empty() { stdout.to_string() } else { stderr.to_string() };
                    Ok((false, format!("安装失败: {}", msg.trim())))
                }
            }
            Err(e) => Ok((false, format!("安装异常: {}", e))),
        }
    }

    /// Uninstall Sysmon.
    pub fn uninstall(&self) -> Result<(bool, String), IrError> {
        if !self.is_installed() {
            return Ok((true, "Sysmon 未安装".to_string()));
        }
        let mut cmd = std::process::Command::new(&self.sysmon_exe_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd.arg("-accepteula").arg("-u");

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    self.clear_started_marker();
                    Ok((true, "Sysmon 卸载成功".to_string()))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Ok((false, format!("卸载失败: {}", stderr.trim())))
                }
            }
            Err(e) => Ok((false, format!("卸载异常: {}", e))),
        }
    }

    /// Update Sysmon configuration.
    pub fn update_config(&self) -> Result<(bool, String), IrError> {
        if !self.config_path.exists() {
            return Ok((false, format!("配置文件不存在: {}", self.config_path.display())));
        }
        if !self.is_installed() {
            return Ok((false, "Sysmon 未安装，请先安装".to_string()));
        }

        let mut cmd = std::process::Command::new(&self.sysmon_exe_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd.arg("-c").arg(&self.config_path);

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    Ok((true, "配置更新成功".to_string()))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Ok((false, format!("配置更新失败: {}", stderr.trim())))
                }
            }
            Err(e) => Ok((false, format!("配置更新异常: {}", e))),
        }
    }

    /// Get full status info.
    pub fn get_status_info(&self) -> SysmonStatus {
        let installed = self.is_installed();
        let running = if installed { self.is_running() } else { false };
        SysmonStatus {
            installed,
            running,
            service_name: self.get_service_name(),
            sysmon_exe_exists: self.sysmon_exe_path.exists(),
            config_exists: self.config_path.exists(),
            sysmon_exe_path: self.sysmon_exe_path.to_string_lossy().to_string(),
            config_path: self.config_path.to_string_lossy().to_string(),
            started_by_irtool: self.was_started_by_irtool(),
            config_managed_by_irtool: self.is_current_config_managed_by_irtool(),
        }
    }

    /// Generate Sysmon XML config from enabled events list.
    pub fn generate_config(enabled_events: &[String]) -> String {
        let all_tags = [
            ("ProcessCreate", "进程创建"), ("FileCreateTime", "文件创建时间修改"),
            ("NetworkConnect", "网络连接"), ("ProcessTerminate", "进程终止"),
            ("DriverLoad", "驱动加载"), ("ImageLoad", "DLL加载"),
            ("CreateRemoteThread", "远程线程创建"), ("RawAccessRead", "原始磁盘访问"),
            ("ProcessAccess", "进程访问"), ("FileCreate", "文件创建"),
            ("RegistryEvent", "注册表事件"), ("FileCreateStreamHash", "文件流哈希"),
            ("PipeEvent", "管道事件"), ("WmiEvent", "WMI事件"),
            ("DnsQuery", "DNS查询"), ("FileDelete", "文件删除"),
            ("ClipboardChange", "剪贴板变化"), ("ProcessTampering", "进程篡改"),
            ("FileDeleteDetected", "文件删除检测"),
        ];

        let key_to_xml: &[(&str, &str)] = &[
            ("network", "NetworkConnect"), ("dns", "DnsQuery"),
            ("remote_thread", "CreateRemoteThread"), ("process_create", "ProcessCreate"),
            ("process_terminate", "ProcessTerminate"), ("file_create", "FileCreate"),
            ("file_create_dll", "FileCreate"), ("registry_event", "RegistryEvent"),
            ("process_access", "ProcessAccess"), ("driver_load", "DriverLoad"),
            ("image_load", "ImageLoad"), ("raw_access_read", "RawAccessRead"),
            ("file_create_stream_hash", "FileCreateStreamHash"), ("pipe_event", "PipeEvent"),
            ("wmi_event", "WmiEvent"), ("file_delete", "FileDelete"),
            ("clipboard_change", "ClipboardChange"), ("process_tampering", "ProcessTampering"),
            ("file_delete_detected", "FileDeleteDetected"), ("file_create_time", "FileCreateTime"),
        ];

        let mut enabled_tags = std::collections::HashSet::new();
        for key in enabled_events {
            if let Some((_, tag)) = key_to_xml.iter().find(|(k, _)| *k == key.as_str()) {
                enabled_tags.insert(*tag);
            }
        }

        let mut lines = vec![r#"<Sysmon schemaversion="4.90">"#.to_string()];
        lines.push("  <!-- IRtool: Managed by IRtool -->".to_string());
        lines.push("  <EventFiltering>".to_string());

        for (tag, desc) in &all_tags {
            if enabled_tags.contains(tag) {
                if *tag == "FileCreate" && enabled_events.iter().any(|e| e == "file_create_dll") {
                    lines.push(format!("    <!-- {} - 仅收集 DLL 文件 -->", desc));
                    lines.push(format!(r#"    <{} onmatch="include">"#, tag));
                    lines.push(r#"      <TargetFilename condition="end with">.dll</TargetFilename>"#.to_string());
                    lines.push(format!("    </{}>", tag));
                } else {
                    lines.push(format!("    <!-- {} - 启用 -->", desc));
                    lines.push(format!(r#"    <{} onmatch="exclude"/>"#, tag));
                }
            } else {
                lines.push(format!("    <!-- {} - 禁用 -->", desc));
                lines.push(format!(r#"    <{} onmatch="include"/>"#, tag));
            }
        }

        lines.push("  </EventFiltering>".to_string());
        lines.push("</Sysmon>".to_string());
        lines.join("\n")
    }

    fn mark_started_by_irtool(&self) { let _ = std::fs::write(&self.marker_file, std::process::id().to_string()); }
    fn clear_started_marker(&self) { let _ = std::fs::remove_file(&self.marker_file); }
    fn was_started_by_irtool(&self) -> bool { self.marker_file.exists() }
    fn is_current_config_managed_by_irtool(&self) -> bool {
        std::fs::read_to_string(&self.config_path)
            .map(|c| c.contains("<!-- IRtool: Managed by IRtool")).unwrap_or(false)
    }
}

fn find_sysmon_exe(app_dir: &Path) -> PathBuf {
    let possible_names = ["sysmon64.exe", "Sysmon64.exe", "sysmon.exe", "Sysmon.exe"];
    for name in &possible_names {
        let path = app_dir.join("tools").join(name);
        if path.exists() { return path; }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        for name in &possible_names {
            if let Ok(output) = std::process::Command::new("where").arg(name).creation_flags(0x08000000).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout);
                    if let Some(first) = path.lines().next() {
                        let trimmed = first.trim();
                        if !trimmed.is_empty() { return PathBuf::from(trimmed); }
                    }
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = &possible_names;
    }
    app_dir.join("tools").join("sysmon64.exe")
}

#[cfg(windows)]
fn is_service_installed(name: &str) -> bool {
    use windows::Win32::System::Services::*;
    use windows::core::*;
    unsafe {
        let manager = match OpenSCManagerW(None, None, SC_MANAGER_CONNECT) { Ok(m) => m, Err(_) => return false };
        let result = OpenServiceW(manager, &HSTRING::from(name), SERVICE_QUERY_STATUS);
        let _ = CloseServiceHandle(manager);
        result.is_ok()
    }
}

#[cfg(windows)]
fn is_service_running(name: &str) -> bool {
    use windows::Win32::System::Services::*;
    use windows::core::*;
    unsafe {
        let manager = match OpenSCManagerW(None, None, SC_MANAGER_CONNECT) { Ok(m) => m, Err(_) => return false };
        let service = match OpenServiceW(manager, &HSTRING::from(name), SERVICE_QUERY_STATUS) {
            Ok(s) => s, Err(_) => { let _ = CloseServiceHandle(manager); return false }
        };
        let mut status = SERVICE_STATUS::default();
        let result = QueryServiceStatus(service, &mut status);
        let _ = CloseServiceHandle(service);
        let _ = CloseServiceHandle(manager);
        result.is_ok() && status.dwCurrentState == SERVICE_RUNNING
    }
}
