use crate::csv_parser::{self, RawEntry};
use crate::delete;
use crate::risk::{self, FileInfo};
use crate::sigcheck;
use crate::types::*;
use irtool_core::IrError;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

pub struct AutorunsScanner {
    exe_path: PathBuf,
    next_id: AtomicU64,
}

impl AutorunsScanner {
    pub fn new() -> Result<Self, IrError> {
        let exe_path = find_autorunsc()?;
        tracing::info!("autorunsc found at: {}", exe_path.display());
        Ok(Self {
            exe_path,
            next_id: AtomicU64::new(1),
        })
    }

    pub async fn scan(
        &self,
        options: ScanOptions,
        progress: impl Fn(ScanProgress) + Send + Sync + 'static,
        cancel: CancellationToken,
    ) -> Result<Vec<AutorunItem>, IrError> {
        let start = std::time::Instant::now();

        // 1. Run autorunsc
        progress(ScanProgress {
            task_id: 0,
            phase: ScanPhase::RunningAutorunsc,
            current: 0,
            total: 0,
            message: "正在运行 autorunsc…".into(),
        });

        let output = self.run_autorunsc(&options, &cancel).await?;

        if cancel.is_cancelled() {
            tracing::info!("scan cancelled after autorunsc completed");
            return Err(IrError::Cancelled);
        }

        // 2. Parse CSV
        progress(ScanProgress {
            task_id: 0,
            phase: ScanPhase::ParsingCsv,
            current: 0,
            total: 0,
            message: "正在解析 CSV…".into(),
        });

        let raw_entries = csv_parser::parse(&output)?;
        tracing::info!("parsed {} raw entries from autorunsc CSV", raw_entries.len());

        if cancel.is_cancelled() {
            return Err(IrError::Cancelled);
        }

        // 3. Batch file existence check (rayon parallel)
        progress(ScanProgress {
            task_id: 0,
            phase: ScanPhase::CheckingFiles,
            current: 0,
            total: raw_entries.len(),
            message: format!("正在检查 {} 个文件…", raw_entries.len()),
        });

        let file_info = check_files_batch(&raw_entries);

        // 4. Risk evaluation + build AutorunItem
        progress(ScanProgress {
            task_id: 0,
            phase: ScanPhase::EvaluatingRisk,
            current: 0,
            total: raw_entries.len(),
            message: "正在评估风险…".into(),
        });

        let items: Vec<AutorunItem> = raw_entries
            .into_iter()
            .map(|raw| {
                let info = file_info.get(raw.image_path.as_str());
                let (risk, risk_reasons) = risk::evaluate(&raw, info);
                let signature = risk::parse_signer_status(&raw.signer);
                let enabled_lower = raw.enabled.to_lowercase();
                let enabled = enabled_lower != "disabled"
                    && !raw.entry.to_lowercase().contains("(disabled)")
                    && !raw.location.to_lowercase().contains("(disabled)");
                let (file_exists, file_size) = match info {
                    Some(fi) => (fi.exists, fi.size),
                    None => (true, None),
                };

                let service_name = if raw.category == "Services" || raw.category == "Drivers" {
                    Some(extract_service_name(&raw))
                } else {
                    None
                };

                AutorunItem {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    category: raw.category,
                    entry: raw.entry,
                    enabled,
                    location: raw.location,
                    description: raw.description,
                    publisher: raw.publisher,
                    image_path: if raw.image_path.is_empty() || raw.image_path.eq_ignore_ascii_case("file not found") {
                        None
                    } else {
                        Some(raw.image_path)
                    },
                    launch_string: if raw.launch_string.is_empty() {
                        None
                    } else {
                        Some(raw.launch_string)
                    },
                    timestamp: if raw.timestamp.is_empty() {
                        None
                    } else {
                        Some(raw.timestamp)
                    },
                    file_exists,
                    file_size,
                    file_version: if raw.version.is_empty() {
                        None
                    } else {
                        Some(raw.version)
                    },
                    service_name,
                    md5: if raw.md5.is_empty() { None } else { Some(raw.md5) },
                    sha256: if raw.sha256.is_empty() { None } else { Some(raw.sha256) },
                    risk,
                    risk_reasons,
                    signature,
                }
            })
            .collect();

        // 5. Category filter
        let items = if let Some(ref filter) = options.category_filter {
            items.into_iter().filter(|i| filter.contains(&i.category)).collect()
        } else {
            items
        };

        let elapsed = start.elapsed();
        tracing::info!("scan completed: {} items in {:.1}s", items.len(), elapsed.as_secs_f64());

        progress(ScanProgress {
            task_id: 0,
            phase: ScanPhase::Complete,
            current: items.len(),
            total: items.len(),
            message: format!("扫描完成，共 {} 项，耗时 {:.1}s", items.len(), elapsed.as_secs_f64()),
        });

        Ok(items)
    }

    async fn run_autorunsc(&self, options: &ScanOptions, cancel: &CancellationToken) -> Result<Vec<u8>, IrError> {
        let exe = self.exe_path.clone();
        let include_hash = options.include_hash;
        let cancel_clone = cancel.clone();

        tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(["-accepteula", "-a", "*", "-c", "-s", "-nobanner"]);
            if include_hash {
                cmd.arg("-h");
            }

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                // CREATE_NO_WINDOW
                cmd.creation_flags(0x08000000);
            }

            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| {
                tracing::error!("autorunsc spawn failed: {}", e);
                IrError::Io(format!("autorunsc 启动失败: {}", e))
            })?;

            tracing::info!("autorunsc process started (pid: {:?})", child.id());

            let stdout = child.stdout.take().unwrap();
            use std::io::Read;
            let mut output = Vec::with_capacity(1024 * 1024);
            let mut reader = std::io::BufReader::new(stdout);

            // Read with periodic cancel check
            let mut buf = [0u8; 64 * 1024];
            loop {
                if cancel_clone.is_cancelled() {
                    tracing::info!("cancelling autorunsc, killing process");
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(IrError::Cancelled);
                }
                use std::io::ErrorKind;
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => output.extend_from_slice(&buf[..n]),
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => {
                        tracing::error!("reading autorunsc stdout failed: {}", e);
                        let _ = child.kill();
                        return Err(IrError::Io(format!("读取 autorunsc 输出失败: {}", e)));
                    }
                }
            }

            let status = child.wait().map_err(|e| IrError::Io(e.to_string()))?;
            if !status.success() {
                tracing::error!("autorunsc exited with code: {:?}", status.code());
                return Err(IrError::ExternalTool {
                    tool: "autorunsc".into(),
                    code: status.code().unwrap_or(-1),
                });
            }

            tracing::info!("autorunsc completed, output {} bytes", output.len());
            Ok(output)
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    pub fn verify_signatures_batch(
        &self,
        paths: &[PathBuf],
        progress: impl Fn(usize, usize) + Send + Sync,
    ) -> Vec<(PathBuf, SignatureStatus)> {
        sigcheck::verify_batch(paths, progress)
    }

    pub fn delete_entry(&self, item: &AutorunItem) -> Result<DeleteResult, IrError> {
        delete::delete_entry(item)
    }

    /// Calculate MD5 and SHA256 for a file
    pub fn calculate_hash(path: &Path) -> Result<(String, String), IrError> {
        use sha2::Digest;
        use std::io::Read;
        if !path.exists() {
            return Err(IrError::Io(format!("文件不存在: {}", path.display())));
        }
        let mut file = std::fs::File::open(path).map_err(|e| IrError::Io(format!("无法打开文件: {}", e)))?;
        let mut md5 = md5::Context::new();
        let mut sha256 = sha2::Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .map_err(|e| IrError::Io(format!("读取失败: {}", e)))?;
            if n == 0 {
                break;
            }
            md5.consume(&buf[..n]);
            sha256.update(&buf[..n]);
        }
        let md5_hex = format!("{:x}", md5.compute());
        let sha256_hex = format!("{:x}", sha256.finalize());
        Ok((md5_hex, sha256_hex))
    }

    /// Run sigcheck64.exe on a single file and return the output
    pub fn sigcheck_file(sigcheck_path: &Path, file_path: &Path) -> Result<String, IrError> {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(sigcheck_path);
        cmd.args(["-accepteula", "-nobanner"])
            .arg(file_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let output = cmd
            .output()
            .map_err(|e| IrError::Io(format!("sigcheck 启动失败: {}", e)))?;

        let stdout = crate::csv_parser::decode_bytes(&output.stdout);
        Ok(stdout)
    }
}

/// Find sigcheck64.exe using the same search strategy as find_autorunsc
pub fn find_sigcheck() -> Result<PathBuf, IrError> {
    let exe_name = "sigcheck64.exe";

    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let tools = parent.join("tools");
            let managed = tools.join("sigcheck").join(exe_name);
            if managed.exists() {
                return Ok(managed);
            }
            let flat = tools.join(exe_name);
            if flat.exists() {
                return Ok(flat);
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let tools = cwd.join("tools");
        let managed = tools.join("sigcheck").join(exe_name);
        if managed.exists() {
            return Ok(managed);
        }
        let flat = tools.join(exe_name);
        if flat.exists() {
            return Ok(flat);
        }
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut dir = PathBuf::from(&manifest_dir);
        loop {
            let tools = dir.join("tools");
            let managed = tools.join("sigcheck").join(exe_name);
            if managed.exists() {
                return Ok(managed);
            }
            let flat = tools.join(exe_name);
            if flat.exists() {
                return Ok(flat);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd;
        loop {
            let tools = dir.join("tools");
            let managed = tools.join("sigcheck").join(exe_name);
            if managed.exists() {
                return Ok(managed);
            }
            let flat = tools.join(exe_name);
            if flat.exists() {
                return Ok(flat);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    Err(IrError::Io(
        "sigcheck64.exe not found. 请在设置中下载或手动导入工具。".into(),
    ))
}

/// Open file in Windows Explorer (select the file)
pub fn open_in_explorer(path: &str) -> Result<(), IrError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(IrError::Io(format!("路径不存在: {}", path)));
    }
    let mut cmd = std::process::Command::new("explorer");
    cmd.args(["/select,", path]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.spawn()
        .map_err(|e| IrError::Io(format!("无法打开资源管理器: {}", e)))?;
    Ok(())
}

/// Open regedit and navigate to a registry key
pub fn open_regedit(registry_path: &str) -> Result<(), IrError> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    let escaped = registry_path.replace('\\', "\\\\");
    let reg_content = format!(
        "Windows Registry Editor Version 5.00\n\n\
         [HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Applets\\Regedit]\n\
         \"LastKey\"=\"{}\"\n",
        escaped
    );
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("irtool_regedit_nav_{}.reg", std::process::id()));
    std::fs::write(&tmp_path, &reg_content).map_err(|e| IrError::Io(format!("写入临时文件失败: {}", e)))?;
    #[cfg(windows)]
    {
        std::process::Command::new("regedit")
            .args(["/s", tmp_path.to_str().unwrap_or("")])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| IrError::Io(format!("导入注册表失败: {}", e)))?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("regedit")
            .args(["/s", tmp_path.to_str().unwrap_or("")])
            .spawn()
            .map_err(|e| IrError::Io(format!("导入注册表失败: {}", e)))?;
    }
    std::process::Command::new("regedit")
        .spawn()
        .map_err(|e| IrError::Io(format!("打开注册表编辑器失败: {}", e)))?;
    Ok(())
}

/// Open Windows Services manager
pub fn open_services_msc() -> Result<(), IrError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/c", "services.msc"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| IrError::Io(format!("无法打开服务管理器: {}", e)))?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("cmd")
            .args(["/c", "services.msc"])
            .spawn()
            .map_err(|e| IrError::Io(format!("无法打开服务管理器: {}", e)))?;
    }
    Ok(())
}

fn find_autorunsc() -> Result<PathBuf, IrError> {
    let exe_name = "autorunsc64.exe";

    // Search paths in priority order:
    // 1. <exe_dir>/tools/autoruns/autorunsc64.exe  (new managed layout)
    // 2. <exe_dir>/tools/autorunsc64.exe           (legacy flat layout)
    // 3. Walk up from CARGO_MANIFEST_DIR
    // 4. Walk up from CWD

    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let tools = parent.join("tools");
            let managed = tools.join("autoruns").join(exe_name);
            if managed.exists() {
                return Ok(managed);
            }
            let flat = tools.join(exe_name);
            if flat.exists() {
                return Ok(flat);
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let tools = cwd.join("tools");
        let managed = tools.join("autoruns").join(exe_name);
        if managed.exists() {
            return Ok(managed);
        }
        let flat = tools.join(exe_name);
        if flat.exists() {
            return Ok(flat);
        }
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut dir = PathBuf::from(&manifest_dir);
        loop {
            let tools = dir.join("tools");
            let managed = tools.join("autoruns").join(exe_name);
            if managed.exists() {
                return Ok(managed);
            }
            let flat = tools.join(exe_name);
            if flat.exists() {
                return Ok(flat);
            }
            if !dir.pop() {
                break;
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd;
        loop {
            let tools = dir.join("tools");
            let managed = tools.join("autoruns").join(exe_name);
            if managed.exists() {
                return Ok(managed);
            }
            let flat = tools.join(exe_name);
            if flat.exists() {
                return Ok(flat);
            }
            if !dir.pop() {
                break;
            }
        }
    }

    Err(IrError::Io(
        "autorunsc64.exe not found. 请在设置中下载或手动导入工具。".into(),
    ))
}

fn check_files_batch(entries: &[RawEntry]) -> std::collections::HashMap<String, FileInfo> {
    use rayon::prelude::*;

    let unique_paths: Vec<&str> = {
        let mut seen = std::collections::HashSet::new();
        entries
            .iter()
            .filter_map(|e| {
                if e.image_path.is_empty() || e.image_path.eq_ignore_ascii_case("file not found") {
                    None
                } else {
                    Some(e.image_path.as_str())
                }
            })
            .filter(|p| seen.insert(*p))
            .collect()
    };

    unique_paths
        .par_iter()
        .map(|&path| {
            let p = Path::new(path);
            let exists = p.exists();
            let size = if exists {
                std::fs::metadata(path).ok().map(|m| m.len())
            } else {
                None
            };
            (path.to_owned(), FileInfo { exists, size })
        })
        .collect()
}

fn extract_service_name(raw: &RawEntry) -> String {
    // Primary: extract from registry location (authoritative Windows service key name)
    // e.g. HKLM\System\CurrentControlSet\Services\BitCometService → "BitCometService"
    static SERVICE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    if let Some(caps) = SERVICE_RE
        .get_or_init(|| regex::Regex::new(r"(?i)Services\\([^\\]+)").unwrap())
        .captures(&raw.location)
    {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_owned();
        }
    }

    // Fallback: extract from launch_string binary path
    if !raw.launch_string.is_empty() {
        if let Some(name) = extract_service_name_from_path(&raw.launch_string) {
            return name;
        }
    }

    raw.entry.clone()
}

fn extract_service_name_from_path(path: &str) -> Option<String> {
    // Strip surrounding quotes and anything after the executable extension
    // e.g. '"C:\Program Files\BitComet\BitCometService.exe" -service' → 'BitCometService'
    let cleaned = path.trim().trim_matches('"');
    // Find the .exe/.sys/.dll portion and extract the filename before it
    static EXT_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = EXT_RE.get_or_init(|| regex::Regex::new(r#"(?:^|.*[\\/])([^\\/"\s]+?)(?:\.(?:exe|sys|dll))"#).unwrap());
    if let Some(caps) = re.captures(cleaned) {
        if let Some(m) = caps.get(1) {
            let name = m.as_str().to_owned();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    // Fallback: last path component without extension
    static PATH_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re2 = PATH_RE.get_or_init(|| regex::Regex::new(r"\\([^\\]+)\s*$").unwrap());
    let m = re2.captures(path)?.get(1)?;
    let mut name = m.as_str().to_owned();
    for ext in &[".sys", ".dll", ".exe"] {
        if name.to_lowercase().ends_with(ext) {
            name = name[..name.len() - ext.len()].to_owned();
            break;
        }
    }
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_service_name_from_sys_path() {
        let name = extract_service_name_from_path(r"C:\Windows\System32\drivers\ndis.sys");
        assert_eq!(name, Some("ndis".into()));
    }

    #[test]
    fn extract_service_name_from_exe_path() {
        let name = extract_service_name_from_path(r"C:\Program Files\MyApp\service.exe");
        assert_eq!(name, Some("service".into()));
    }

    #[test]
    fn extract_service_name_empty_path() {
        let name = extract_service_name_from_path("");
        assert_eq!(name, None);
    }

    #[test]
    fn extract_service_name_quoted_with_args() {
        let name = extract_service_name_from_path(r#""C:\Program Files\BitComet\BitCometService.exe" -service"#);
        assert_eq!(name, Some("BitCometService".into()));
    }

    #[test]
    fn extract_service_name_unquoted_with_args() {
        let name = extract_service_name_from_path(r"C:\Program Files\MyApp\service.exe --daemon");
        assert_eq!(name, Some("service".into()));
    }
}
