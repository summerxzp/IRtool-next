mod manifest;

use std::path::{Path, PathBuf};

use irtool_core::IrError;
pub use manifest::{write_installed_manifest, ToolManifest, ToolManifests, VerifyMethod};

/// Status of a single tool
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ToolStatus {
    pub id: String,
    pub installed: bool,
    pub version: Option<String>,
    pub files: Vec<String>,
    pub missing_files: Vec<String>,
    pub optional: bool,
}

/// Check which tools are missing from the tools directory.
pub fn check_tools(tools_dir: &Path) -> Vec<ToolStatus> {
    let manifests = ToolManifests::load();
    manifests.check(tools_dir)
}

/// Resolve the tools directory: <exe_dir>/tools/
pub fn tools_dir() -> Result<PathBuf, IrError> {
    let exe_dir = std::env::current_exe().map_err(|e| IrError::Internal(format!("无法获取可执行文件路径: {}", e)))?;
    let dir = exe_dir
        .parent()
        .ok_or_else(|| IrError::Internal("无法获取可执行文件目录".into()))?
        .join("tools");
    Ok(dir)
}

/// Ensure the tools directory exists.
pub fn ensure_tools_dir() -> Result<PathBuf, IrError> {
    let dir = tools_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| IrError::Internal(format!("创建 tools 目录失败: {}", e)))?;
    }
    Ok(dir)
}

/// Download a tool's zip, verify, extract to tools/<id>/.
/// `on_progress` is called with (downloaded_bytes, total_bytes).
pub async fn download_tool(tool_id: &str, tools_dir: &Path, on_progress: impl Fn(u64, u64)) -> Result<(), IrError> {
    let manifests = ToolManifests::load();
    let manifest = manifests
        .get(tool_id)
        .ok_or_else(|| IrError::Internal(format!("未知工具: {}", tool_id)))?;

    let tool_dir = tools_dir.join(tool_id);
    if !tool_dir.exists() {
        std::fs::create_dir_all(&tool_dir).map_err(|e| IrError::Internal(format!("创建目录失败: {}", e)))?;
    }

    // Download zip to temp file
    let temp_dir = std::env::temp_dir();
    let zip_path = temp_dir.join(format!("irtool-{}-download.zip", tool_id));

    tracing::info!("下载工具 {} 从 {}", tool_id, manifest.url);
    download_file_with_progress(&manifest.url, &zip_path, on_progress).await?;

    // Verify based on manifest configuration
    verify_tool(&zip_path, manifest)?;

    // Extract
    tracing::info!("解压 {} 到 {:?}", tool_id, tool_dir);
    extract_zip(&zip_path, &tool_dir, &manifest.files)?;

    // Verify extracted executables; clean up on failure
    if let Err(e) = verify_extracted_exes(&tool_dir, manifest) {
        cleanup_tool_files(&tool_dir, &manifest.files);
        return Err(e);
    }

    // Clean up temp zip
    let _ = std::fs::remove_file(&zip_path);

    // Write installed manifest
    if let Err(e) = write_installed_manifest(tools_dir, tool_id, &manifest.version) {
        tracing::warn!("写入 manifest.json 失败: {}", e);
    }

    tracing::info!("工具 {} 安装完成", tool_id);
    Ok(())
}

/// Import a tool from a local zip file.
pub fn import_tool_zip(tool_id: &str, tools_dir: &Path, zip_path: &Path) -> Result<(), IrError> {
    let manifests = ToolManifests::load();
    let manifest = manifests
        .get(tool_id)
        .ok_or_else(|| IrError::Internal(format!("未知工具: {}", tool_id)))?;

    let tool_dir = tools_dir.join(tool_id);
    if !tool_dir.exists() {
        std::fs::create_dir_all(&tool_dir).map_err(|e| IrError::Internal(format!("创建目录失败: {}", e)))?;
    }

    // Verify zip if configured
    verify_tool(zip_path, manifest)?;

    // Extract
    extract_zip(zip_path, &tool_dir, &manifest.files)?;

    // Verify extracted executables; clean up on failure
    if let Err(e) = verify_extracted_exes(&tool_dir, manifest) {
        cleanup_tool_files(&tool_dir, &manifest.files);
        return Err(e);
    }

    // Write installed manifest
    if let Err(e) = write_installed_manifest(tools_dir, tool_id, &manifest.version) {
        tracing::warn!("写入 manifest.json 失败: {}", e);
    }

    tracing::info!("工具 {} 从本地 ZIP 导入完成", tool_id);
    Ok(())
}

/// Verify a downloaded/imported zip based on manifest config.
fn verify_tool(zip_path: &Path, manifest: &ToolManifest) -> Result<(), IrError> {
    match &manifest.verify {
        VerifyMethod::None => {
            tracing::warn!(
                "工具 {} 未配置校验，跳过",
                manifest.files.first().unwrap_or(&String::new())
            );
        }
        VerifyMethod::Sha256 => {
            if let Some(expected) = &manifest.sha256 {
                tracing::info!("SHA256 校验: {}", expected);
                verify_sha256(zip_path, expected)?;
            } else {
                tracing::warn!("工具配置了 Sha256 校验但未提供 sha256 值，跳过");
            }
        }
        VerifyMethod::Authenticode => {
            // Authenticode is verified on extracted EXEs, not on the zip
            tracing::info!("将在解压后验证 Authenticode 签名");
        }
    }
    Ok(())
}

/// Verify extracted executables using Authenticode or other per-file methods.
fn verify_extracted_exes(tool_dir: &Path, manifest: &ToolManifest) -> Result<(), IrError> {
    if matches!(manifest.verify, VerifyMethod::Authenticode) {
        let expected_signer = manifest.expected_signer.as_deref().unwrap_or("");
        for file_name in &manifest.files {
            let exe_path = tool_dir.join(file_name);
            if !exe_path.exists() {
                continue;
            }
            verify_authenticode(&exe_path, expected_signer)?;
        }
    }
    Ok(())
}

/// Remove extracted tool files (used when verification fails after extraction).
fn cleanup_tool_files(tool_dir: &Path, files: &[String]) {
    for file_name in files {
        let path = tool_dir.join(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            tracing::info!("已清理: {}", file_name);
        }
    }
}

/// Verify Authenticode signature of a PE file using WinVerifyTrust.
#[cfg(windows)]
fn verify_authenticode(path: &Path, expected_signer: &str) -> Result<(), IrError> {
    use windows::core::PCWSTR;
    use windows::Win32::Security::WinTrust::*;

    let path_wide: Vec<u16> = path
        .to_str()
        .ok_or_else(|| IrError::Internal(format!("路径转宽字符失败: {:?}", path)))?
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut trust_file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(path_wide.as_ptr()),
        hFile: Default::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };

    let trust_data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        pPolicyCallbackData: std::ptr::null_mut(),
        pSIPClientData: std::ptr::null_mut(),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WINTRUST_DATA_REVOCATION_CHECKS(0),
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut trust_file },
        dwStateAction: WINTRUST_DATA_STATE_ACTION(0),
        hWVTStateData: Default::default(),
        pwszURLReference: windows::core::PWSTR::null(),
        dwProvFlags: WINTRUST_DATA_PROVIDER_FLAGS(0),
        dwUIContext: WINTRUST_DATA_UICONTEXT(0),
        pSignatureSettings: std::ptr::null_mut(),
    };

    let mut action_id = WINTRUST_ACTION_GENERIC_VERIFY_V2;

    let result = unsafe { WinVerifyTrust(None, &mut action_id, &trust_data as *const _ as *mut _) };

    // WinVerifyTrust returns 0 on success
    if result != 0 {
        return Err(IrError::Internal(format!(
            "Authenticode 签名验证失败: {:?} — 文件可能被篡改或签名无效 (错误码: {})",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
            result
        )));
    }

    // Verify publisher if expected_signer is specified
    if !expected_signer.is_empty() {
        verify_publisher(path, expected_signer)?;
    }

    tracing::info!(
        "Authenticode 验证通过: {} (签名者: {})",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        if expected_signer.is_empty() {
            "未指定"
        } else {
            expected_signer
        }
    );
    Ok(())
}

/// Verify the publisher of a signed PE file by searching for the expected signer
/// in the certificate store embedded in the Authenticode signature.
#[cfg(windows)]
fn verify_publisher(path: &Path, expected_signer: &str) -> Result<(), IrError> {
    use windows::Win32::Security::Cryptography::*;

    let path_wide: Vec<u16> = path
        .to_str()
        .ok_or_else(|| IrError::Internal(format!("路径转宽字符失败: {:?}", path)))?
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut cert_store: HCERTSTORE = HCERTSTORE::default();
    let mut msg_handle: *mut core::ffi::c_void = std::ptr::null_mut();

    let result = unsafe {
        CryptQueryObject(
            CERT_QUERY_OBJECT_FILE,
            path_wide.as_ptr() as *const core::ffi::c_void,
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
            CERT_QUERY_FORMAT_FLAG_BINARY,
            0,
            None,
            None,
            None,
            Some(&mut cert_store),
            Some(&mut msg_handle),
            None,
        )
    };

    if let Err(e) = result {
        return Err(IrError::Internal(format!("提取签名证书失败: {} — 文件可能未签名", e)));
    }

    // Search for a certificate whose subject contains the expected signer
    let signer_wide: Vec<u16> = expected_signer.encode_utf16().chain(std::iter::once(0)).collect();

    let cert_ctx = unsafe {
        CertFindCertificateInStore(
            cert_store,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            0,
            CERT_FIND_SUBJECT_STR,
            Some(signer_wide.as_ptr() as *const core::ffi::c_void),
            None,
        )
    };

    // Cleanup
    unsafe {
        if !cert_store.is_invalid() {
            let _ = CertCloseStore(cert_store, 0);
        }
        if !msg_handle.is_null() {
            let _ = CryptMsgClose(Some(msg_handle as *const _));
        }
    }

    if cert_ctx.is_null() {
        return Err(IrError::Internal(format!(
            "签名者不匹配: 期望包含 '{}'",
            expected_signer
        )));
    }

    tracing::info!(
        "发布者验证通过: {} (签名者匹配: {})",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        expected_signer
    );
    Ok(())
}

#[cfg(not(windows))]
fn verify_authenticode(_path: &Path, _expected_signer: &str) -> Result<(), IrError> {
    // Authenticode is Windows-only; skip on other platforms
    tracing::warn!("Authenticode 验证仅在 Windows 上可用，跳过");
    Ok(())
}

#[cfg(not(windows))]
fn verify_publisher(_path: &Path, _expected_signer: &str) -> Result<(), IrError> {
    Ok(())
}

/// Accept EULA for a tool by running it with -accepteula flag.
#[cfg(windows)]
pub async fn accept_eula(tool_id: &str, tools_dir: &Path) -> Result<(), IrError> {
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let manifests = ToolManifests::load();
    let manifest = manifests
        .get(tool_id)
        .ok_or_else(|| IrError::Internal(format!("未知工具: {}", tool_id)))?;

    let tool_dir = tools_dir.join(tool_id);
    for file_name in &manifest.files {
        let exe_path = tool_dir.join(file_name);
        if !exe_path.exists() {
            continue;
        }
        tracing::info!("接受 EULA: {:?}", exe_path);
        let output = tokio::process::Command::new(&exe_path)
            .arg("-accepteula")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .await
            .map_err(|e| IrError::Internal(format!("运行 {} 失败: {}", file_name, e)))?;
        tracing::info!("{} -accepteula 退出码: {:?}", file_name, output.status.code());
    }
    Ok(())
}

#[cfg(not(windows))]
pub async fn accept_eula(_tool_id: &str, _tools_dir: &Path) -> Result<(), IrError> {
    Ok(())
}

/// Download a file with progress reporting.
async fn download_file_with_progress(url: &str, dest: &Path, on_progress: impl Fn(u64, u64)) -> Result<(), IrError> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| IrError::Internal(format!("下载失败: {}", e)))?;

    if !response.status().is_success() {
        return Err(IrError::Internal(format!("下载失败: HTTP {}", response.status())));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| IrError::Internal(format!("创建临时文件失败: {}", e)))?;

    use tokio::io::AsyncWriteExt;
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| IrError::Internal(format!("下载数据读取失败: {}", e)))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| IrError::Internal(format!("写入临时文件失败: {}", e)))?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    file.flush()
        .await
        .map_err(|e| IrError::Internal(format!("刷新文件失败: {}", e)))?;

    Ok(())
}

/// Verify SHA256 hash of a file.
fn verify_sha256(path: &Path, expected: &str) -> Result<(), IrError> {
    use sha2::{Digest, Sha256};

    let data = std::fs::read(path).map_err(|e| IrError::Internal(format!("读取文件失败: {}", e)))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    let actual = format!("{:x}", result);

    if actual != expected {
        return Err(IrError::Internal(format!(
            "SHA256 校验失败: 期望 {} 实际 {}",
            expected, actual
        )));
    }
    Ok(())
}

/// Extract specific files from a zip archive to a target directory.
fn extract_zip(zip_path: &Path, dest_dir: &Path, wanted_files: &[String]) -> Result<(), IrError> {
    let file = std::fs::File::open(zip_path).map_err(|e| IrError::Internal(format!("打开 ZIP 失败: {}", e)))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| IrError::Internal(format!("解析 ZIP 失败: {}", e)))?;

    let wanted_set: std::collections::HashSet<&str> = wanted_files.iter().map(|s| s.as_str()).collect();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| IrError::Internal(format!("读取 ZIP 条目失败: {}", e)))?;

        let name = entry.name().to_string();
        let file_name = Path::new(&name).file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !wanted_set.contains(file_name) {
            continue;
        }

        let out_path = dest_dir.join(file_name);
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| IrError::Internal(format!("创建文件 {:?} 失败: {}", out_path, e)))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| IrError::Internal(format!("解压 {:?} 失败: {}", file_name, e)))?;

        tracing::info!("已解压: {}", file_name);
    }

    Ok(())
}
