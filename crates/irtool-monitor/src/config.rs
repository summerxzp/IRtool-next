use crate::types::MonitorConfig;
use irtool_core::IrError;
use std::path::Path;

pub fn load_config(path: &Path) -> Result<MonitorConfig, IrError> {
    if !path.exists() {
        return Ok(MonitorConfig::default());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| IrError::Io(e.to_string()))?;
    let config: MonitorConfig = toml::from_str(&content)
        .map_err(|e| IrError::Parse(format!("监控配置解析失败: {}", e)))?;
    Ok(config)
}

pub fn save_config(path: &Path, config: &MonitorConfig) -> Result<(), IrError> {
    let content = toml::to_string_pretty(config)
        .map_err(|e| IrError::Internal(format!("序列化配置失败: {}", e)))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| IrError::Io(e.to_string()))?;
    }
    std::fs::write(path, content)
        .map_err(|e| IrError::Io(e.to_string()))?;
    Ok(())
}
