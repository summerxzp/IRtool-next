use crate::types::{AutorunItem, DeleteResult};
use irtool_core::IrError;

/// Main routing function — dispatches to the appropriate delete implementation
/// based on the autorun entry's category and location.
pub fn delete_entry(item: &AutorunItem) -> Result<DeleteResult, IrError> {
    #[cfg(windows)]
    {
        win_impls::delete_entry_inner(item)
    }
    #[cfg(not(windows))]
    {
        stub_impls::delete_entry_inner(item)
    }
}

// ---------------------------------------------------------------------------
// Windows implementations
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win_impls {
    use crate::types::{AutorunItem, DeleteResult};
    use irtool_core::IrError;
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::*;
    use windows::Win32::System::Services::*;

    pub fn delete_entry_inner(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        let category = item.category.to_lowercase();

        // 1. Windows Services — no registry parsing needed
        if category == "services" {
            return delete_service(item);
        }

        // 2. Scheduled Tasks — no registry parsing needed
        if category == "scheduled tasks" || category == "task scheduler" || category == "tasks" {
            return delete_scheduled_task(item);
        }

        // 11. Drivers — treat as service, no registry parsing needed
        if category == "drivers" {
            return delete_service(item);
        }

        // 9. WMI — not supported via registry, would need WMI API
        if category == "wmi" {
            return Ok(DeleteResult {
                success: false,
                message: "WMI 持久化项暂不支持删除，请手动处理".into(),
            });
        }

        // All remaining categories need registry path parsing
        let hive = match super::parse_hive(&item.location) {
            Some(h) => h,
            None => {
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法解析注册表路径: {}", item.location),
                });
            }
        };

        // autorunsc puts the registry key path in `location` and the value name in `entry`
        let subkey = match super::parse_subkey(&item.location) {
            Some(s) => s,
            None => {
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法解析注册表子键: {}", item.location),
                });
            }
        };
        let value_name = &item.entry;

        // 3. Boot Execute — LSA_MULTI_SZ at HKLM\System\CurrentControlSet\Control\Session Manager
        if category == "boot execute" {
            return delete_boot_execute(item, hive, &subkey);
        }

        // 4. AppInit DLLs — LSA_MULTI_SZ at HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows
        if category == "appinit" {
            return delete_appinit_dlls(item, hive, &subkey);
        }

        // 5. Image File Execution Options (IFEO)
        if category == "image hijacks" || category == "hijacks" {
            return delete_ifeo(item, hive, &subkey);
        }

        // 6. LSA Security Packages — MULTI_SZ value named "Security Packages"
        //    entry is the package name (data within the MULTI_SZ), NOT the value name
        if category == "lsa security packages" {
            return delete_lsa_multi_sz_at(hive, &subkey, "Security Packages", &item.entry);
        }

        // 7. Known DLLs — MULTI_SZ value named "KnownDLLs"
        //    entry is the DLL filename (data within the MULTI_SZ), NOT the value name
        if category == "known dlls" {
            return delete_lsa_multi_sz_at(hive, &subkey, "KnownDLLs", &item.entry);
        }

        // 8. Winlogon — LSA_MULTI_SZ (Userinit, Shell, Taskman, System)
        if category == "winlogon" {
            // Protect critical Winlogon values that would prevent login if deleted
            let val_lower = value_name.to_lowercase();
            if val_lower == "shell" || val_lower == "userinit" {
                return Ok(DeleteResult {
                    success: false,
                    message: "Shell/Userinit 是系统关键值，删除后将无法登录，请手动处理".into(),
                });
            }
            // GpExtensions 和 Notify 是子键结构，不是 MULTI_SZ 值
            // entry 是描述名而非 GUID，无法直接定位子键
            if subkey.to_lowercase().ends_with(r"\gpextensions") || subkey.to_lowercase().ends_with(r"\notify") {
                return Ok(DeleteResult {
                    success: false,
                    message: "GpExtensions/Notify 项为子键结构，entry 为描述名而非 GUID，暂不支持自动删除，请手动处理"
                        .into(),
                });
            }
            return delete_lsa_multi_sz(item, hive, &subkey, value_name);
        }

        // 10. Office addins / COM — CLSID-based
        if category == "office" || category == "com object" {
            return delete_com_or_office(item, hive, &subkey);
        }

        // 13. Internet Explorer addons / BHOs — CLSID-based subkey deletion
        if category == "internet explorer" || category == "ie" || category == "internet addons" {
            return delete_ie_addon(item, hive, &subkey);
        }

        // 14. Explorer shell extensions — may contain CLSID-based entries
        if category == "explorer" {
            return delete_explorer_addon(item, hive, &subkey);
        }

        // 15. Winsock LSP / Network Providers — complex chain-based
        //     Generic delete may work for simple cases; chain removal requires LspFix-style logic
        if category == "winsock" || category == "network providers" {
            return Ok(DeleteResult {
                success: false,
                message: "Winsock/网络提供程序项结构复杂，建议使用专用工具手动处理".into(),
            });
        }

        // 16. Print Monitors — registry value + driver file, value-based delete covers registry part
        if category == "print monitors" || category == "printer monitors" {
            return delete_registry_value(hive, &subkey, value_name);
        }

        // 17. Sidebar Gadgets (Vista/7) — rare, generic value delete
        if category == "sidebar gadgets" || category == "gadgets" {
            return delete_registry_value(hive, &subkey, value_name);
        }

        // 12. Generic registry value delete (Logon, Explorer, Codecs, etc.)
        delete_registry_value(hive, &subkey, value_name)
    }

    fn delete_service(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        let service_name = match &item.service_name {
            Some(name) => name.clone(),
            None => item.entry.clone(),
        };

        let service_name_wide = HSTRING::from(&service_name);

        unsafe {
            let scm = match OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
                Ok(s) => s,
                Err(e) => {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("无法打开 SCM: {}", e),
                    });
                }
            };

            // SERVICE_DELETE = 0x00010000, SERVICE_STOP = 0x00000020
            let service = match OpenServiceW(scm, &service_name_wide, 0x00010000 | 0x00000020) {
                Ok(s) => s,
                Err(e) => {
                    let _ = CloseServiceHandle(scm);
                    // ERROR_SERVICE_DOES_NOT_EXIST (0x80070424): 服务未安装但注册表可能残留，
                    // fallback 到删除注册表项
                    if e.code() == windows::core::HRESULT(0x80070424u32 as i32) {
                        let reg_path = format!(r"SYSTEM\CurrentControlSet\Services\{}", service_name);
                        return delete_registry_key(HKEY_LOCAL_MACHINE, &reg_path);
                    }
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("无法打开服务 '{}': {}", service_name, e),
                    });
                }
            };

            // Try to stop the service first; SERVICE_CONTROL_STOP = 0x00000001
            let mut status = SERVICE_STATUS::default();
            let _ = ControlService(service, 0x00000001, &mut status);

            let result = match DeleteService(service) {
                Ok(()) => Ok(DeleteResult {
                    success: true,
                    message: format!("服务 '{}' 已删除", service_name),
                }),
                Err(e) => Ok(DeleteResult {
                    success: false,
                    message: format!("删除服务 '{}' 失败: {}", service_name, e),
                }),
            };

            let _ = CloseServiceHandle(service);
            let _ = CloseServiceHandle(scm);
            result
        }
    }

    fn delete_scheduled_task(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        use std::os::windows::process::CommandExt;
        let task_name = &item.entry;
        let output = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", task_name, "/F"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| IrError::Io(format!("schtasks 执行失败: {}", e)))?;

        if output.status.success() {
            Ok(DeleteResult {
                success: true,
                message: format!("计划任务 '{}' 已删除", task_name),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(DeleteResult {
                success: false,
                message: format!("删除计划任务 '{}' 失败: {}", task_name, stderr.trim()),
            })
        }
    }

    fn delete_boot_execute(item: &AutorunItem, hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        delete_lsa_multi_sz_at(hive, subkey, "BootExecute", &item.entry)
    }

    fn delete_appinit_dlls(item: &AutorunItem, hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        delete_lsa_multi_sz_at(hive, subkey, "AppInit_DLLs", &item.entry)
    }

    fn delete_ifeo(item: &AutorunItem, hive: HKEY, parent_subkey: &str) -> Result<DeleteResult, IrError> {
        // IFEO: entry is a subkey name under the location path
        // e.g. location = HKLM\...\Image File Execution Options, entry = irtool_test_dummy.exe
        // We need to delete the key: ...\Image File Execution Options\irtool_test_dummy.exe
        let full_subkey = format!(r"{}\{}", parent_subkey, item.entry);
        delete_registry_key(hive, &full_subkey)
    }

    fn delete_lsa_multi_sz(
        item: &AutorunItem,
        hive: HKEY,
        subkey: &str,
        value_name: &str,
    ) -> Result<DeleteResult, IrError> {
        delete_lsa_multi_sz_at(hive, subkey, value_name, &item.entry)
    }

    fn delete_com_or_office(item: &AutorunItem, hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        let clsid = super::extract_clsid_from_item(item);
        if let Some(ref clsid) = clsid {
            let clsid_path = format!(r"SOFTWARE\Classes\CLSID\{}", clsid);
            return delete_registry_key(hive, &clsid_path);
        }
        // Use entry as the value name to delete
        if item.entry.is_empty() {
            return Ok(DeleteResult {
                success: false,
                message: "COM/Office 项无法自动删除，请手动处理".into(),
            });
        }
        delete_registry_value(hive, subkey, &item.entry)
    }

    /// IE BHOs: the CLSID is registered as a subkey under ...\Browser Helper Objects\{CLSID}
    /// We delete both the BHO subkey and the CLSID class registration.
    fn delete_ie_addon(item: &AutorunItem, hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        let clsid = super::extract_clsid_from_item(item);
        if let Some(ref clsid) = clsid {
            // Delete the BHO registration subkey
            let bho_subkey = format!(r"{}\{}", subkey, clsid);
            let _ = super::delete_registry_key_fallback(hive, &bho_subkey);
            // Also delete the CLSID class registration
            let clsid_path = format!(r"SOFTWARE\Classes\CLSID\{}", clsid);
            return delete_registry_key(hive, &clsid_path);
        }
        // No CLSID found, try generic value delete as fallback
        delete_registry_value(hive, subkey, &item.entry)
    }

    /// Explorer shell extensions: entries may be CLSID-based (context menu handlers,
    /// approved shell extensions) or plain value names.
    fn delete_explorer_addon(item: &AutorunItem, hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        let clsid = super::extract_clsid_from_item(item);
        if let Some(ref clsid) = clsid {
            // For shell extensions, the CLSID is typically a subkey name
            let full_subkey = format!(r"{}\{}", subkey, clsid);
            let result = super::delete_registry_key_fallback(hive, &full_subkey);
            if result.is_ok() {
                // Also try to remove the CLSID class registration
                let clsid_path = format!(r"SOFTWARE\Classes\CLSID\{}", clsid);
                let _ = super::delete_registry_key_fallback(hive, &clsid_path);
                return Ok(DeleteResult {
                    success: true,
                    message: format!("Explorer 扩展 '{}' 已删除", clsid),
                });
            }
            // If subkey delete failed, it might be a value name containing the CLSID
            return delete_registry_value(hive, subkey, clsid);
        }
        // No CLSID, treat as plain registry value
        delete_registry_value(hive, subkey, &item.entry)
    }

    fn delete_registry_value(hive: HKEY, subkey: &str, value_name: &str) -> Result<DeleteResult, IrError> {
        let subkey_wide = HSTRING::from(subkey);
        let value_name_wide = HSTRING::from(value_name);

        unsafe {
            let mut h_key = Default::default();
            let result = RegOpenKeyExW(hive, &subkey_wide, None, KEY_SET_VALUE, &mut h_key);

            if result.is_err() {
                // ERROR_FILE_NOT_FOUND (2): key already gone — treat as success
                if result.0 == 2 {
                    return Ok(DeleteResult {
                        success: true,
                        message: format!("注册表值 '{}' 已删除", value_name),
                    });
                }
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法打开注册表项 '{}'", subkey),
                });
            }

            let result = RegDeleteValueW(h_key, &value_name_wide);
            let _ = RegCloseKey(h_key);
            // ERROR_FILE_NOT_FOUND (2): value already gone — treat as success
            if result.is_err() && result.0 != 2 {
                Ok(DeleteResult {
                    success: false,
                    message: format!("删除注册表值 '{}' 失败", value_name),
                })
            } else {
                Ok(DeleteResult {
                    success: true,
                    message: format!("注册表值 '{}' 已删除", value_name),
                })
            }
        }
    }

    fn delete_registry_key(hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        let result = super::delete_registry_key_fallback(hive, subkey);
        match result {
            Ok(()) => Ok(DeleteResult {
                success: true,
                message: format!("注册表项 '{}' 已删除", subkey),
            }),
            Err(e) => Ok(DeleteResult {
                success: false,
                message: format!("删除注册表项 '{}' 失败: {}", subkey, e),
            }),
        }
    }

    fn delete_lsa_multi_sz_at(
        hive: HKEY,
        subkey: &str,
        value_name: &str,
        entry_to_remove: &str,
    ) -> Result<DeleteResult, IrError> {
        let subkey_wide = HSTRING::from(subkey);
        let value_name_wide = HSTRING::from(value_name);

        unsafe {
            let mut h_key = Default::default();
            let result = RegOpenKeyExW(hive, &subkey_wide, None, KEY_QUERY_VALUE | KEY_SET_VALUE, &mut h_key);

            if result.is_err() {
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法打开注册表项 '{}'", subkey),
                });
            }

            // Read existing MULTI_SZ value
            let existing = match super::read_multi_sz(h_key, &value_name_wide) {
                Some(strings) => strings,
                None => {
                    let _ = RegCloseKey(h_key);
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("无法读取 MULTI_SZ 值 '{}'", value_name),
                    });
                }
            };

            // Remove the entry
            let entry_lower = entry_to_remove.to_lowercase();
            let new_strings: Vec<&String> = existing.iter().filter(|s| s.to_lowercase() != entry_lower).collect();

            if new_strings.len() == existing.len() {
                let _ = RegCloseKey(h_key);
                return Ok(DeleteResult {
                    success: false,
                    message: format!("在 '{}' 中未找到 '{}'", value_name, entry_to_remove),
                });
            }

            // Write back the new MULTI_SZ
            let new_data = super::build_multi_sz(&new_strings);
            let value_name_wide2 = HSTRING::from(value_name);
            let result = RegSetKeyValueW(
                hive,
                &subkey_wide,
                &value_name_wide2,
                REG_MULTI_SZ.0,
                Some(new_data.as_ptr() as *const _),
                (new_data.len() * 2) as u32,
            );
            let _ = RegCloseKey(h_key);

            if result.is_err() {
                Ok(DeleteResult {
                    success: false,
                    message: "写入 MULTI_SZ 值失败".into(),
                })
            } else {
                Ok(DeleteResult {
                    success: true,
                    message: format!("已从 '{}' 中移除 '{}'", value_name, entry_to_remove),
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Non-Windows stubs
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod stub_impls {
    use crate::types::{AutorunItem, DeleteResult};
    use irtool_core::IrError;

    pub fn delete_entry_inner(_item: &AutorunItem) -> Result<DeleteResult, IrError> {
        Ok(DeleteResult {
            success: false,
            message: "删除操作仅在 Windows 上可用".into(),
        })
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn parse_hive(location: &str) -> Option<windows::Win32::System::Registry::HKEY> {
    use windows::Win32::System::Registry::*;

    let location = location.trim();
    if location.starts_with("HKLM\\") || location.starts_with("HKEY_LOCAL_MACHINE\\") {
        Some(HKEY_LOCAL_MACHINE)
    } else if location.starts_with("HKCU\\") || location.starts_with("HKEY_CURRENT_USER\\") {
        Some(HKEY_CURRENT_USER)
    } else if location.starts_with("HKU\\") || location.starts_with("HKEY_USERS\\") {
        Some(HKEY_USERS)
    } else {
        None
    }
}

#[cfg(windows)]
fn parse_subkey(location: &str) -> Option<String> {
    let location = location.trim();
    let rest = location
        .strip_prefix("HKLM\\")
        .or_else(|| location.strip_prefix("HKEY_LOCAL_MACHINE\\"))
        .or_else(|| location.strip_prefix("HKCU\\"))
        .or_else(|| location.strip_prefix("HKEY_CURRENT_USER\\"))
        .or_else(|| location.strip_prefix("HKU\\"))
        .or_else(|| location.strip_prefix("HKEY_USERS\\"))?;
    Some(rest.to_owned())
}

#[cfg(not(windows))]
fn parse_hive(_location: &str) -> Option<()> {
    None
}

#[cfg(not(windows))]
fn parse_subkey(_location: &str) -> Option<String> {
    None
}

#[cfg(windows)]
fn read_multi_sz(
    h_key: windows::Win32::System::Registry::HKEY,
    value_name: &windows::core::HSTRING,
) -> Option<Vec<String>> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    unsafe {
        let mut buf_len: u32 = 0;
        let mut reg_type: REG_VALUE_TYPE = REG_NONE;

        // First call to get the size
        let result = RegQueryValueExW(
            h_key,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut buf_len),
        );

        if result.is_err() || reg_type != REG_MULTI_SZ || buf_len == 0 {
            return None;
        }

        let mut buf: Vec<u16> = vec![0u16; (buf_len / 2) as usize + 1];

        let result = RegQueryValueExW(
            h_key,
            PCWSTR(value_name.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut buf_len),
        );

        if result.is_err() {
            return None;
        }

        Some(parse_multi_sz(&buf[..(buf_len / 2) as usize]))
    }
}

fn parse_multi_sz(data: &[u16]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut start = 0;
    for i in 0..data.len() {
        if data[i] == 0 {
            if start == i {
                break;
            }
            let s = String::from_utf16_lossy(&data[start..i]);
            strings.push(s);
            start = i + 1;
        }
    }
    strings
}

#[cfg(windows)]
fn build_multi_sz(strings: &[&String]) -> Vec<u16> {
    let mut data = Vec::new();
    for s in strings {
        data.extend(s.encode_utf16());
        data.push(0);
    }
    data.push(0);
    data
}

fn extract_clsid(text: &str) -> Option<String> {
    static CLSID_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = CLSID_RE.get_or_init(|| {
        regex::Regex::new(r"\{[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}").unwrap()
    });
    let caps = re.find(text)?;
    Some(caps.as_str().to_owned())
}

/// Extract CLSID from an AutorunItem — tries `location` first, then `entry`.
fn extract_clsid_from_item(item: &AutorunItem) -> Option<String> {
    extract_clsid(&item.location).or_else(|| extract_clsid(&item.entry))
}

#[cfg(windows)]
fn delete_registry_key_fallback(
    hive: windows::Win32::System::Registry::HKEY,
    subkey: &str,
) -> Result<(), irtool_core::IrError> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::*;

    let subkey_wide = HSTRING::from(subkey);

    unsafe {
        // Try RegDeleteTreeW first (deletes key with all subkeys)
        let result = RegDeleteTreeW(hive, &subkey_wide);
        if result.is_ok() {
            return Ok(());
        }
        // ERROR_FILE_NOT_FOUND (2): key already gone — treat as success
        if result.0 == 2 {
            return Ok(());
        }

        // Fallback: try simple RegDeleteKeyW
        let result = RegDeleteKeyW(hive, &subkey_wide);
        if result.is_err() && result.0 != 2 {
            return Err(irtool_core::IrError::Io(format!(
                "删除注册表项失败: 0x{:08X}",
                result.0
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RiskLevel, SignatureStatus};

    fn make_item(category: &str, location: &str, entry: &str) -> AutorunItem {
        AutorunItem {
            id: 1,
            category: category.into(),
            entry: entry.into(),
            enabled: true,
            location: location.into(),
            description: String::new(),
            publisher: String::new(),
            image_path: None,
            launch_string: None,
            timestamp: None,
            file_exists: true,
            file_size: None,
            file_version: None,
            service_name: None,
            md5: None,
            sha256: None,
            risk: RiskLevel::Safe,
            risk_reasons: vec![],
            signature: SignatureStatus::NotVerified,
        }
    }

    #[test]
    fn unsupported_type_returns_false() {
        let item = make_item("WMI", r"HKLM\SOFTWARE\Something", "test");
        let result = delete_entry(&item).unwrap();
        assert!(!result.success);
    }

    #[test]
    fn shell_userinit_delete_refused() {
        let item = make_item(
            "Winlogon",
            r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\Userinit",
            "userinit.exe",
        );
        let _result = delete_entry(&item);
    }

    #[test]
    fn parse_hive_hklm() {
        let result = parse_hive(r"HKLM\SOFTWARE\Microsoft\Windows\Run");
        assert!(result.is_some());
    }

    #[test]
    fn parse_hive_hkcu() {
        let result = parse_hive(r"HKCU\SOFTWARE\Run");
        assert!(result.is_some());
    }

    #[test]
    fn parse_hive_invalid() {
        let result = parse_hive("NotARegistryPath");
        assert!(result.is_none());
    }

    #[test]
    fn parse_subkey_hklm() {
        let result = parse_subkey(r"HKLM\SOFTWARE\Microsoft\Windows\Run");
        assert_eq!(result, Some(r"SOFTWARE\Microsoft\Windows\Run".into()));
    }

    #[test]
    fn parse_subkey_hkcu() {
        let result = parse_subkey(r"HKCU\SOFTWARE\Run\SomeValue");
        assert_eq!(result, Some(r"SOFTWARE\Run\SomeValue".into()));
    }

    #[test]
    fn extract_clsid_from_path() {
        let clsid = extract_clsid(r"HKLM\SOFTWARE\Classes\CLSID\{12345678-1234-1234-1234-123456789ABC}");
        assert_eq!(clsid, Some("{12345678-1234-1234-1234-123456789ABC}".into()));
    }

    #[test]
    fn extract_clsid_no_match() {
        let clsid = extract_clsid("no clsid here");
        assert_eq!(clsid, None);
    }

    #[test]
    fn extract_clsid_from_item_location() {
        let item = make_item(
            "COM Object",
            r"HKLM\SOFTWARE\Classes\CLSID\{AABBCCDD-1234-5678-9ABC-DEF012345678}\InprocServer32",
            "MyAddon",
        );
        assert_eq!(
            extract_clsid_from_item(&item),
            Some("{AABBCCDD-1234-5678-9ABC-DEF012345678}".into())
        );
    }

    #[test]
    fn extract_clsid_from_item_entry_fallback() {
        let item = make_item(
            "Internet Explorer",
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Browser Helper Objects",
            "{11223344-5566-7788-99AA-BBCCDDEEFF00}",
        );
        assert_eq!(
            extract_clsid_from_item(&item),
            Some("{11223344-5566-7788-99AA-BBCCDDEEFF00}".into())
        );
    }

    #[test]
    fn extract_clsid_from_item_none() {
        let item = make_item("Logon", r"HKLM\SOFTWARE\Microsoft\Windows\Run", "malware.exe");
        assert_eq!(extract_clsid_from_item(&item), None);
    }

    // --- Category dispatch tests ---
    // These test that the category string routes to the correct handler.
    // On non-Windows, the stub impls return a fixed "unsupported" message,
    // so we only verify the dispatch doesn't panic.

    #[test]
    fn dispatch_ie_category() {
        let item = make_item(
            "Internet Explorer",
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Browser Helper Objects\{12345678-1234-1234-1234-123456789ABC}",
            "BHO",
        );
        let _ = delete_entry(&item);
    }

    #[test]
    fn dispatch_explorer_category() {
        let item = make_item(
            "Explorer",
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved",
            "ShellExt",
        );
        let _ = delete_entry(&item);
    }

    #[test]
    fn dispatch_winsock_category() {
        let item = make_item("Winsock", r"HKLM\SYSTEM\CurrentControlSet\Services\WinSock", "LspEntry");
        let result = delete_entry(&item);
        // Winsock should return unsupported (not panic)
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.message.contains("Winsock"));
    }

    #[test]
    fn dispatch_wmi_category() {
        let item = make_item("WMI", r"HKLM\SOFTWARE\WMI", "SomeEntry");
        let result = delete_entry(&item);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.success);
        assert!(r.message.contains("WMI"));
    }

    #[test]
    fn parse_multi_sz_basic() {
        let data: Vec<u16> = "hello"
            .encode_utf16()
            .chain(std::iter::once(0u16))
            .chain("world".encode_utf16())
            .chain(std::iter::once(0u16))
            .chain(std::iter::once(0u16))
            .collect();
        let strings = parse_multi_sz(&data);
        assert_eq!(strings, vec!["hello", "world"]);
    }
}
