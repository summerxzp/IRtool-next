//! SNSS 解析 + 当前标签页恢复
//!
//! 解析 Chromium SNSS 格式的 Session/Tabs 文件，提取当前打开的标签页信息。
//! 支持新版路径 `<Profile>/Sessions/Tabs_<id>` 和旧版路径 `<Profile>/Current Tabs`。

use crate::core::BrowserKind;
use crate::profile::BrowserProfile;
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;

/// Session Recovery 结果
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SessionRecoveryResult {
    pub browser: BrowserKind,
    pub profile: String,
    pub tabs: Vec<RecoveredTab>,
    pub parse_errors: Vec<String>,
}

/// 恢复的标签页
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RecoveredTab {
    pub url: String,
    pub title: String,
    pub active: bool,
    pub tab_index: Option<u32>,
}

/// SNSS 文件 magic
const SNSS_MAGIC: [u8; 4] = [0x53, 0x4E, 0x53, 0x53];

/// SNSS 记录类型：Tab
const COMMAND_TAB: u8 = 6;

/// SNSS 记录类型：Window
const COMMAND_WINDOW: u8 = 1;

/// 恢复指定 Profile 的当前标签页
pub fn recover_tabs(profile: &BrowserProfile) -> SessionRecoveryResult {
    let tabs_path = match find_session_files(&profile.path) {
        Some(p) => p,
        None => {
            return SessionRecoveryResult {
                browser: profile.browser,
                profile: profile.name.clone(),
                tabs: vec![],
                parse_errors: vec!["no Tabs/Session file found".to_string()],
            };
        }
    };

    let data = match std::fs::read(&tabs_path) {
        Ok(d) => d,
        Err(e) => {
            warn!("failed to read Tabs file {:?}: {}", tabs_path, e);
            return SessionRecoveryResult {
                browser: profile.browser,
                profile: profile.name.clone(),
                tabs: vec![],
                parse_errors: vec![format!("failed to read Tabs file: {}", e)],
            };
        }
    };

    parse_snss_tabs(&data, profile.browser, &profile.name)
}

/// 查找 Session/Tabs 文件
///
/// 优先检查 `<Profile>/Sessions/` 目录下最新的 `Tabs_<id>` 文件，
/// 如果不存在则依次检查 Current Tabs、Current Session、Last Session。
fn find_session_files(profile_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let sessions_dir = profile_path.join("Sessions");

    if sessions_dir.is_dir() {
        let latest = find_latest_tabs_in_sessions(&sessions_dir);
        if latest.is_some() {
            return latest;
        }
    }

    // Fallback: 旧版路径
    for name in &["Current Tabs", "Current Session", "Last Session"] {
        let legacy = profile_path.join(name);
        if legacy.is_file() {
            return Some(legacy);
        }
    }

    None
}

/// 在 Sessions 目录中找到数字 ID 最大的 Tabs_<id> 文件
fn find_latest_tabs_in_sessions(sessions_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(sessions_dir).ok()?;

    let mut best: Option<(u64, std::path::PathBuf)> = None;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(id_str) = name_str.strip_prefix("Tabs_") {
            if let Ok(id) = id_str.parse::<u64>() {
                match &best {
                    Some((best_id, _)) if id <= *best_id => {}
                    _ => best = Some((id, entry.path())),
                }
            }
        }
    }

    best.map(|(_, p)| p)
}

/// 解析 SNSS 格式的 Tabs 文件
fn parse_snss_tabs(data: &[u8], browser: BrowserKind, profile: &str) -> SessionRecoveryResult {
    let mut errors = Vec::new();
    let mut tabs = Vec::new();
    let mut window_current_index: Option<u32> = None;

    // 验证文件头
    if data.len() < 8 {
        return SessionRecoveryResult {
            browser,
            profile: profile.to_string(),
            tabs: vec![],
            parse_errors: vec!["file too short for SNSS header".to_string()],
        };
    }

    if data[0..4] != SNSS_MAGIC {
        return SessionRecoveryResult {
            browser,
            profile: profile.to_string(),
            tabs: vec![],
            parse_errors: vec!["invalid SNSS magic".to_string()],
        };
    }

    let _version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // 第一遍：提取所有 Window 记录的 current_index
    let mut offset = 8usize;
    while offset + 5 <= data.len() {
        let command_type = data[offset];
        let record_len =
            u32::from_le_bytes([data[offset + 1], data[offset + 2], data[offset + 3], data[offset + 4]]) as usize;

        offset += 5;

        if offset + record_len > data.len() {
            break;
        }

        if command_type == COMMAND_WINDOW {
            let record_data = &data[offset..offset + record_len];
            if let Some(idx) = parse_window_record(record_data) {
                window_current_index = Some(idx);
            }
        }

        offset += record_len;
    }

    // 第二遍：解析 Tab 记录，匹配 active 状态
    let mut offset = 8usize;
    while offset + 5 <= data.len() {
        let command_type = data[offset];
        let record_len =
            u32::from_le_bytes([data[offset + 1], data[offset + 2], data[offset + 3], data[offset + 4]]) as usize;

        offset += 5;

        if offset + record_len > data.len() {
            errors.push(format!(
                "record extends beyond file: type={} len={} offset={}",
                command_type, record_len, offset
            ));
            break;
        }

        let record_data = &data[offset..offset + record_len];

        if command_type == COMMAND_TAB {
            match parse_tab_record(record_data, window_current_index) {
                Some(tab) => tabs.push(tab),
                None => {
                    errors.push(format!("failed to parse tab record at offset {}", offset));
                }
            }
        }

        offset += record_len;
    }

    SessionRecoveryResult {
        browser,
        profile: profile.to_string(),
        tabs,
        parse_errors: errors,
    }
}

/// 解析 Window 记录，提取 current_index (field 4, varint)
fn parse_window_record(data: &[u8]) -> Option<u32> {
    let mut offset = 0usize;
    while offset < data.len() {
        let (tag, tag_len) = parse_varint(&data[offset..])?;
        offset += tag_len;

        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match wire_type {
            0 => {
                // varint
                let (val, val_len) = parse_varint(&data[offset..])?;
                if field_number == 4 {
                    return Some(val as u32);
                }
                offset += val_len;
            }
            2 => {
                // length-delimited - 跳过
                let (len, len_size) = parse_varint(&data[offset..])?;
                offset += len_size;
                offset += len as usize;
            }
            _ => {
                // 未知 wire type，无法跳过
                return None;
            }
        }
    }
    None
}

/// 解析 protobuf 编码的 Tab 记录
///
/// 关键字段：
/// - field 7 (length-delimited): current_url
/// - field 9 (length-delimited): title
/// - field 15 (varint): tab_index
/// - `window_current_index`: 如果 tab_index 匹配，则 active = true
fn parse_tab_record(data: &[u8], window_current_index: Option<u32>) -> Option<RecoveredTab> {
    let mut url = None;
    let mut title = None;
    let mut tab_index = None;

    let mut offset = 0usize;
    while offset < data.len() {
        let (tag, tag_len) = parse_varint(&data[offset..])?;
        offset += tag_len;

        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        match wire_type {
            0 => {
                // varint
                let (_, val_len) = parse_varint(&data[offset..])?;
                if field_number == 15 {
                    let (val, _) = parse_varint(&data[offset..])?;
                    tab_index = Some(val as u32);
                }
                offset += val_len;
            }
            2 => {
                // length-delimited
                let (len, len_size) = parse_varint(&data[offset..])?;
                offset += len_size;
                let len = len as usize;
                if offset + len > data.len() {
                    return None;
                }
                let bytes = &data[offset..offset + len];
                match field_number {
                    7 => {
                        url = String::from_utf8(bytes.to_vec()).ok();
                    }
                    9 => {
                        title = String::from_utf8(bytes.to_vec()).ok();
                    }
                    _ => {}
                }
                offset += len;
            }
            _ => {
                // 未知 wire type，无法跳过，终止解析
                return None;
            }
        }
    }

    let url = url.unwrap_or_default();
    let title = title.unwrap_or_default();

    // 跳过空 URL 的标签页（如 chrome://newtab）
    if url.is_empty() {
        return None;
    }

    Some(RecoveredTab {
        url,
        title,
        active: tab_index.is_some() && Some(tab_index.unwrap()) == window_current_index,
        tab_index,
    })
}

/// 解析 protobuf varint
///
/// 返回 (值, 消耗的字节数)
fn parse_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;

    for (i, &byte) in data.iter().enumerate() {
        if i >= 10 {
            // varint 最多 10 字节
            return None;
        }
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_varint_single_byte() {
        assert_eq!(parse_varint(&[0x01]), Some((1, 1)));
        assert_eq!(parse_varint(&[0x7F]), Some((127, 1)));
        assert_eq!(parse_varint(&[0x00]), Some((0, 1)));
    }

    #[test]
    fn parse_varint_multi_byte() {
        // 128 = 0x80 => varint: 0x80 0x01
        assert_eq!(parse_varint(&[0x80, 0x01]), Some((128, 2)));
        // 300 = 0x12C => varint: 0xAC 0x02
        assert_eq!(parse_varint(&[0xAC, 0x02]), Some((300, 2)));
        // 16384 = 0x4000 => varint: 0x80 0x80 0x01
        assert_eq!(parse_varint(&[0x80, 0x80, 0x01]), Some((16384, 3)));
    }

    #[test]
    fn parse_varint_empty() {
        assert_eq!(parse_varint(&[]), None);
    }

    #[test]
    fn parse_varint_truncated() {
        // 缺少终止字节
        assert_eq!(parse_varint(&[0x80]), None);
    }

    /// 构造 protobuf 编码的 Tab 记录
    fn build_tab_protobuf(url: &str, title: &str, tab_index: u32) -> Vec<u8> {
        let mut buf = Vec::new();

        // field 7 (URL): wire_type=2, tag = 7<<3 | 2 = 58
        let url_bytes = url.as_bytes();
        buf.push(58);
        append_varint(&mut buf, url_bytes.len() as u64);
        buf.extend_from_slice(url_bytes);

        // field 9 (title): wire_type=2, tag = 9<<3 | 2 = 74
        let title_bytes = title.as_bytes();
        buf.push(74);
        append_varint(&mut buf, title_bytes.len() as u64);
        buf.extend_from_slice(title_bytes);

        // field 15 (tab_index): wire_type=0, tag = 15<<3 | 0 = 120
        buf.push(120);
        append_varint(&mut buf, tab_index as u64);

        buf
    }

    fn append_varint(buf: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                buf.push(byte);
                break;
            }
            buf.push(byte | 0x80);
        }
    }

    #[test]
    fn parse_tab_record_basic() {
        let data = build_tab_protobuf("https://example.com", "Example", 0);
        let tab = parse_tab_record(&data, None).unwrap();
        assert_eq!(tab.url, "https://example.com");
        assert_eq!(tab.title, "Example");
        assert_eq!(tab.tab_index, Some(0));
        assert!(!tab.active);
    }

    #[test]
    fn parse_tab_record_no_url() {
        // 只有 title，没有 URL → 应返回 None
        let mut buf = Vec::new();
        let title_bytes = "Some Title".as_bytes();
        buf.push(74); // field 9, wire_type 2
        append_varint(&mut buf, title_bytes.len() as u64);
        buf.extend_from_slice(title_bytes);

        assert!(parse_tab_record(&buf, None).is_none());
    }

    #[test]
    fn parse_tab_record_url_only() {
        // 只有 URL，没有 title
        let mut buf = Vec::new();
        let url_bytes = "https://example.com".as_bytes();
        buf.push(58); // field 7, wire_type 2
        append_varint(&mut buf, url_bytes.len() as u64);
        buf.extend_from_slice(url_bytes);

        let tab = parse_tab_record(&buf, None).unwrap();
        assert_eq!(tab.url, "https://example.com");
        assert_eq!(tab.title, "");
    }

    #[test]
    fn parse_tab_record_with_large_tab_index() {
        let data = build_tab_protobuf("https://test.com", "Test", 42);
        let tab = parse_tab_record(&data, None).unwrap();
        assert_eq!(tab.tab_index, Some(42));
    }

    #[test]
    fn snss_header_validation() {
        // 有效的 SNSS 头 + 空 body
        let mut data = Vec::new();
        data.extend_from_slice(&SNSS_MAGIC);
        data.extend_from_slice(&1u32.to_le_bytes());

        let result = parse_snss_tabs(&data, BrowserKind::Chrome, "Test");
        assert!(result.tabs.is_empty());
        assert!(result.parse_errors.is_empty());
    }

    #[test]
    fn snss_invalid_magic() {
        let data = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let result = parse_snss_tabs(&data, BrowserKind::Chrome, "Test");
        assert!(result.parse_errors.iter().any(|e| e.contains("invalid SNSS magic")));
    }

    #[test]
    fn snss_too_short() {
        let data = vec![0x53, 0x4E]; // 只有部分 magic
        let result = parse_snss_tabs(&data, BrowserKind::Chrome, "Test");
        assert!(result.parse_errors.iter().any(|e| e.contains("too short")));
    }

    #[test]
    fn snss_with_tab_record() {
        let tab_data = build_tab_protobuf("https://example.com", "Example", 0);

        let mut data = Vec::new();
        data.extend_from_slice(&SNSS_MAGIC);
        data.extend_from_slice(&1u32.to_le_bytes());

        // 写入 Tab 记录
        data.push(COMMAND_TAB);
        data.extend_from_slice(&(tab_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&tab_data);

        let result = parse_snss_tabs(&data, BrowserKind::Chrome, "Test");
        assert_eq!(result.tabs.len(), 1);
        assert_eq!(result.tabs[0].url, "https://example.com");
        assert_eq!(result.tabs[0].title, "Example");
    }

    #[test]
    fn snss_multiple_tab_records() {
        let mut data = Vec::new();
        data.extend_from_slice(&SNSS_MAGIC);
        data.extend_from_slice(&1u32.to_le_bytes());

        for i in 0..3 {
            let tab_data = build_tab_protobuf(&format!("https://page{}.com", i), &format!("Page {}", i), i);
            data.push(COMMAND_TAB);
            data.extend_from_slice(&(tab_data.len() as u32).to_le_bytes());
            data.extend_from_slice(&tab_data);
        }

        let result = parse_snss_tabs(&data, BrowserKind::Edge, "Default");
        assert_eq!(result.tabs.len(), 3);
        assert_eq!(result.tabs[0].url, "https://page0.com");
        assert_eq!(result.tabs[2].url, "https://page2.com");
        assert_eq!(result.browser, BrowserKind::Edge);
    }

    #[test]
    fn snss_skips_non_tab_records() {
        let tab_data = build_tab_protobuf("https://example.com", "Example", 0);

        let mut data = Vec::new();
        data.extend_from_slice(&SNSS_MAGIC);
        data.extend_from_slice(&1u32.to_le_bytes());

        // Window 记录 (type=1)，应被跳过
        data.push(1);
        let window_payload = vec![0x08, 0x01]; // field 1 = 1
        data.extend_from_slice(&(window_payload.len() as u32).to_le_bytes());
        data.extend_from_slice(&window_payload);

        // Tab 记录
        data.push(COMMAND_TAB);
        data.extend_from_slice(&(tab_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&tab_data);

        let result = parse_snss_tabs(&data, BrowserKind::Chrome, "Test");
        assert_eq!(result.tabs.len(), 1);
    }

    /// 构造 protobuf 编码的 Window 记录，包含 field 4 (current_index)
    fn build_window_protobuf(current_index: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        // field 4 (current_index): wire_type=0, tag = 4<<3 | 0 = 32
        buf.push(32);
        append_varint(&mut buf, current_index as u64);
        buf
    }

    #[test]
    fn snss_with_window_and_active_tab() {
        // 构造 SNSS: Window (current_index=1) + Tab (index=0) + Tab (index=1, should be active) + Tab (index=2)
        let tab0 = build_tab_protobuf("https://page0.com", "Page 0", 0);
        let tab1 = build_tab_protobuf("https://page1.com", "Page 1", 1);
        let tab2 = build_tab_protobuf("https://page2.com", "Page 2", 2);
        let window = build_window_protobuf(1);

        let mut data = Vec::new();
        data.extend_from_slice(&SNSS_MAGIC);
        data.extend_from_slice(&1u32.to_le_bytes());

        // Window 记录
        data.push(COMMAND_WINDOW);
        data.extend_from_slice(&(window.len() as u32).to_le_bytes());
        data.extend_from_slice(&window);

        // Tab 记录 (index=0)
        data.push(COMMAND_TAB);
        data.extend_from_slice(&(tab0.len() as u32).to_le_bytes());
        data.extend_from_slice(&tab0);

        // Tab 记录 (index=1) - 应该标记为 active
        data.push(COMMAND_TAB);
        data.extend_from_slice(&(tab1.len() as u32).to_le_bytes());
        data.extend_from_slice(&tab1);

        // Tab 记录 (index=2)
        data.push(COMMAND_TAB);
        data.extend_from_slice(&(tab2.len() as u32).to_le_bytes());
        data.extend_from_slice(&tab2);

        let result = parse_snss_tabs(&data, BrowserKind::Chrome, "Test");
        assert_eq!(result.tabs.len(), 3);
        assert!(!result.tabs[0].active, "tab 0 should not be active");
        assert!(result.tabs[1].active, "tab 1 should be active (matches window current_index)");
        assert!(!result.tabs[2].active, "tab 2 should not be active");
    }

    #[test]
    fn find_session_files_fallback_legacy() {
        let temp_dir = tempfile::tempdir().unwrap();
        let profile_path = temp_dir.path();

        // 旧版路径
        let legacy = profile_path.join("Current Tabs");
        std::fs::write(&legacy, "dummy").unwrap();

        let result = find_session_files(profile_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Current Tabs");
    }

    #[test]
    fn find_session_files_prefers_sessions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let profile_path = temp_dir.path();

        let sessions_dir = profile_path.join("Sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // 新版路径
        std::fs::write(sessions_dir.join("Tabs_100"), "dummy").unwrap();
        std::fs::write(sessions_dir.join("Tabs_200"), "dummy").unwrap();

        // 旧版路径也存在
        std::fs::write(profile_path.join("Current Tabs"), "dummy").unwrap();

        let result = find_session_files(profile_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap().file_name().unwrap(), "Tabs_200");
    }

    #[test]
    fn find_session_files_no_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = find_session_files(temp_dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn recover_tabs_no_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let profile = BrowserProfile {
            browser: BrowserKind::Chrome,
            name: "Test".to_string(),
            path: temp_dir.path().to_path_buf(),
        };

        let result = recover_tabs(&profile);
        assert!(result.tabs.is_empty());
        assert!(result.parse_errors.iter().any(|e| e.contains("no Tabs/Session file found")));
    }

    #[test]
    fn recover_tabs_with_valid_snss() {
        let temp_dir = tempfile::tempdir().unwrap();
        let profile_path = temp_dir.path();

        // 创建 Sessions 目录和 Tabs 文件
        let sessions_dir = profile_path.join("Sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let tab_data = build_tab_protobuf("https://example.com", "Example", 0);
        let mut snss = Vec::new();
        snss.extend_from_slice(&SNSS_MAGIC);
        snss.extend_from_slice(&1u32.to_le_bytes());
        snss.push(COMMAND_TAB);
        snss.extend_from_slice(&(tab_data.len() as u32).to_le_bytes());
        snss.extend_from_slice(&tab_data);

        std::fs::write(sessions_dir.join("Tabs_100"), &snss).unwrap();

        let profile = BrowserProfile {
            browser: BrowserKind::Chrome,
            name: "Test".to_string(),
            path: profile_path.to_path_buf(),
        };

        let result = recover_tabs(&profile);
        assert_eq!(result.tabs.len(), 1);
        assert_eq!(result.tabs[0].url, "https://example.com");
    }

    // ── 集成测试：读取本机浏览器 Tabs 文件（条件跳过）──────────

    #[test]
    fn integration_read_local_edge_tabs() {
        let profiles = crate::profile::enumerate_profiles(BrowserKind::Edge);
        if profiles.is_empty() {
            eprintln!("skipping: no Edge profiles found");
            return;
        }

        for profile in &profiles {
            let tabs_path = find_session_files(&profile.path);
            if tabs_path.is_none() {
                eprintln!("skipping: no Tabs file for Edge profile {}", profile.name);
                continue;
            }

            let result = recover_tabs(profile);
            eprintln!(
                "Edge/{}: {} tabs, {} errors",
                profile.name,
                result.tabs.len(),
                result.parse_errors.len()
            );
            // 至少能解析出一些标签页（或至少不 panic）
            if !result.tabs.is_empty() {
                for tab in &result.tabs {
                    assert!(!tab.url.is_empty());
                }
            }
        }
    }

    #[test]
    fn integration_read_local_chrome_tabs() {
        let profiles = crate::profile::enumerate_profiles(BrowserKind::Chrome);
        if profiles.is_empty() {
            eprintln!("skipping: no Chrome profiles found");
            return;
        }

        for profile in &profiles {
            let tabs_path = find_session_files(&profile.path);
            if tabs_path.is_none() {
                eprintln!("skipping: no Tabs file for Chrome profile {}", profile.name);
                continue;
            }

            let result = recover_tabs(profile);
            eprintln!(
                "Chrome/{}: {} tabs, {} errors",
                profile.name,
                result.tabs.len(),
                result.parse_errors.len()
            );
            if !result.tabs.is_empty() {
                for tab in &result.tabs {
                    assert!(!tab.url.is_empty());
                }
            }
        }
    }
}
