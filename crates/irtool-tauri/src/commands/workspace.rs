use irtool_core::IrError;

/// Execute a system command and return its stdout output.
/// Used for command-template-based disposal operations (attrib, takeown, 7z, etc.).
#[tauri::command]
#[specta::specta]
pub async fn cmd_workspace_run_command(
    program: String,
    args: String,
) -> Result<String, IrError> {
    tracing::info!("workspace run command: {} {}", program, args);

    let output = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new(&program)
                .args(&shell_words_split(&args))
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output()
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new(&program)
                .args(&shell_words_split(&args))
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

/// Simple shell-words splitter: splits on whitespace, respects double-quoted segments.
/// Does not handle escaping — sufficient for our fixed command templates.
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
