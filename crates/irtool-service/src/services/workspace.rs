use irtool_core::IrError;

use crate::context::AppContext;

pub struct WorkspaceService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> WorkspaceService<'a> {
    pub async fn run_command(program: String, args: String) -> Result<String, IrError> {
        #[cfg(not(debug_assertions))]
        {
            let _ = (program, args);
            return Err(IrError::Internal(
                "通用命令执行已在生产构建中禁用，请使用类型化命令".to_string(),
            ));
        }

        #[cfg(debug_assertions)]
        {
            tracing::info!("workspace run command: {} {}", program, args);

            let output = tokio::task::spawn_blocking(move || {
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    std::process::Command::new(&program)
                        .args(shell_words_split(&args))
                        .creation_flags(0x08000000)
                        .output()
                }
                #[cfg(not(windows))]
                {
                    std::process::Command::new(&program)
                        .args(shell_words_split(&args))
                        .output()
                }
            })
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))??;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                Ok(stdout)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(IrError::Internal(format!(
                    "command failed (exit {}): {}",
                    output.status.code().unwrap_or(-1),
                    stderr
                )))
            }
        }
    }

    pub fn unhide_path(path: String) -> Result<String, IrError> {
        if !std::path::Path::new(&path).exists() {
            return Err(IrError::Internal(format!("路径不存在: {}", path)));
        }
        run_typed_command("attrib", format!("-H -S \"{}\"", path))
    }

    pub fn take_ownership(path: String) -> Result<String, IrError> {
        if !std::path::Path::new(&path).exists() {
            return Err(IrError::Internal(format!("路径不存在: {}", path)));
        }
        run_typed_command("takeown", format!("/f \"{}\"", path))
    }

    pub fn sample_path(path: String, output_dir: String, password: String) -> Result<String, IrError> {
        if !std::path::Path::new(&path).exists() {
            return Err(IrError::Internal(format!("路径不存在: {}", path)));
        }
        if !std::path::Path::new(&output_dir).exists() {
            std::fs::create_dir_all(&output_dir).map_err(|e| IrError::Io(e.to_string()))?;
        }
        let filename = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "sample".to_string());
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let output_path = std::path::Path::new(&output_dir).join(format!("{}_{}.7z", filename, timestamp));
        run_typed_command(
            "7z",
            format!("a -p\"{}\" \"{}\" \"{}\"", password, output_path.display(), path),
        )
    }

    pub fn open_path(path: String) -> Result<String, IrError> {
        if !std::path::Path::new(&path).exists() {
            return Err(IrError::Internal(format!("路径不存在: {}", path)));
        }
        run_typed_command("explorer", format!("\"{}\"", path))
    }
}

fn run_typed_command(program: &str, args: String) -> Result<String, IrError> {
    tracing::info!("typed command: {} {}", program, args);
    let program_owned = program.to_string();
    let output = std::thread::spawn(move || {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new(&program_owned)
                .args(shell_words_split(&args))
                .creation_flags(0x08000000)
                .output()
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new(&program_owned)
                .args(shell_words_split(&args))
                .output()
        }
    })
    .join()
    .map_err(|e| IrError::Internal(format!("thread join error: {:?}", e)))??;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(IrError::Internal(format!(
            "command failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        )))
    }
}

/// Simple shell-words splitter: splits on whitespace, respects double-quoted segments.
fn shell_words_split(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in s.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}
