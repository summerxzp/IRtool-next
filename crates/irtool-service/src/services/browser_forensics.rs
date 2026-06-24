use irtool_browser_forensics::*;
use irtool_core::IrError;

use crate::context::AppContext;
use crate::dto::browser_forensics::*;

pub struct BrowserForensicsService<'a> {
    pub ctx: &'a AppContext,
}

impl<'a> BrowserForensicsService<'a> {
    /// 列出所有浏览器的 Profile
    pub async fn list_profiles(&self) -> Result<Vec<BrowserProfile>, IrError> {
        tokio::task::spawn_blocking(|| Ok(enumerate_all_profiles()))
            .await
            .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描指定 Profile 的扩展
    pub async fn scan_extensions(
        &self,
        browser: BrowserKind,
        profile_name: &str,
    ) -> Result<ExtensionInventory, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(scan_extensions(&profile))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描所有 Profile 的扩展
    pub async fn scan_all_extensions(&self, browser: BrowserKind) -> Result<Vec<ExtensionInventory>, IrError> {
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            Ok(profiles.iter().map(scan_extensions).collect())
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// History 关联
    pub async fn attribute_history(
        &self,
        browser: BrowserKind,
        profile_name: &str,
        target_time: chrono::DateTime<chrono::Utc>,
    ) -> Result<HistoryAttribution, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(attribute_history(&profile, target_time))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描下载记录
    pub async fn scan_downloads(
        &self,
        browser: BrowserKind,
        profile_name: &str,
    ) -> Result<DownloadAttribution, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(irtool_browser_forensics::scan_downloads(&profile))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扫描历史记录
    pub async fn scan_history(&self, browser: BrowserKind, profile_name: &str) -> Result<HistoryList, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(irtool_browser_forensics::scan_history(&profile, 500))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 恢复当前标签页
    pub async fn recover_tabs(
        &self,
        browser: BrowserKind,
        profile_name: &str,
    ) -> Result<SessionRecoveryResult, IrError> {
        let profile_name = profile_name.to_string();
        tokio::task::spawn_blocking(move || {
            let profiles = enumerate_profiles(browser);
            let profile = profiles
                .into_iter()
                .find(|p| p.name == profile_name)
                .ok_or_else(|| IrError::Internal(format!("profile not found: {}", profile_name)))?;
            Ok(recover_tabs(&profile))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// Browser Context Attribution
    pub async fn attribute_browser_context(&self, req: ContextAttributionRequest) -> Result<BrowserContext, IrError> {
        tokio::task::spawn_blocking(move || {
            let timestamp = chrono::DateTime::parse_from_rfc3339(&req.timestamp)
                .map_err(|e| IrError::Internal(format!("invalid timestamp: {}", e)))?
                .to_utc();
            Ok(irtool_browser_forensics::attribute_browser_context(
                &req.domain,
                req.ip.as_deref(),
                &req.process_name,
                req.pid,
                timestamp,
            ))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }

    /// 扩展归因 Layer 1
    pub async fn attribute_extension(
        &self,
        process_name: String,
        pid: u32,
        domain: String,
        cmdline: Option<String>,
    ) -> Result<Option<ExtensionAttribution>, IrError> {
        tokio::task::spawn_blocking(move || {
            Ok(irtool_browser_forensics::attribute_extension(
                &process_name,
                pid,
                &domain,
                cmdline.as_deref(),
            ))
        })
        .await
        .map_err(|e| IrError::Internal(format!("join error: {}", e)))?
    }
}
