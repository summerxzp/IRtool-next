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
    use windows::Win32::System::Registry::*;
    use windows::Win32::System::Services::*;
    use windows::core::HSTRING;

    pub fn delete_entry_inner(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        let (hive, subkey, value_name) = match super::parse_location(&item.location) {
            Some(parsed) => parsed,
            None => {
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法解析注册表路径: {}", item.location),
                });
            }
        };

        let category = item.category.to_lowercase();

        // 1. Windows Services
        if category == "services" {
            return delete_service(item);
        }

        // 2. Scheduled Tasks
        if category == "scheduled tasks" || category == "task scheduler" {
            return delete_scheduled_task(item);
        }

        // 3. Boot Execute — LSA_MULTI_SZ at HKLM\System\CurrentControlSet\Control\Session Manager
        if category == "boot execute" {
            return delete_boot_execute(item);
        }

        // 4. AppInit DLLs — LSA_MULTI_SZ at HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows
        if category == "appinit" {
            return delete_appinit_dlls(item);
        }

        // 5. Image File Execution Options (IFEO)
        if category == "image hijacks" {
            return delete_ifeo(item);
        }

        // 6. LSA Security Packages — LSA_MULTI_SZ
        if category == "lsa security packages" {
            return delete_lsa_multi_sz(item, hive, &subkey, &value_name);
        }

        // 7. Known DLLs — LSA_MULTI_SZ
        if category == "known dlls" {
            return delete_lsa_multi_sz(item, hive, &subkey, &value_name);
        }

        // 8. Winlogon — LSA_MULTI_SZ (Userinit, Shell, Taskman, System)
        if category == "winlogon" {
            return delete_lsa_multi_sz(item, hive, &subkey, &value_name);
        }

        // 9. WMI — not supported via registry, would need WMI API
        if category == "wmi" {
            return Ok(DeleteResult {
                success: false,
                message: "WMI 持久化项暂不支持删除，请手动处理".into(),
            });
        }

        // 10. Office addins / COM — CLSID-based
        if category == "office" || category == "com object" {
            return delete_com_or_office(item, hive, &subkey);
        }

        // 11. Drivers — treat as service
        if category == "drivers" {
            return delete_service(item);
        }

        // 12. Generic registry value delete (Logon, Explorer, Codecs, etc.)
        delete_registry_value(hive, &subkey, &value_name)
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
            let service = match OpenServiceW(
                scm,
                &service_name_wide,
                0x00010000 | 0x00000020,
            ) {
                Ok(s) => s,
                Err(e) => {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("无法打开服务 '{}': {}", service_name, e),
                    });
                }
            };

            // Try to stop the service first; SERVICE_CONTROL_STOP = 0x00000001
            let mut status = SERVICE_STATUS::default();
            let _ = ControlService(service, 0x00000001, &mut status);

            match DeleteService(service) {
                Ok(()) => Ok(DeleteResult {
                    success: true,
                    message: format!("服务 '{}' 已删除", service_name),
                }),
                Err(e) => Ok(DeleteResult {
                    success: false,
                    message: format!("删除服务 '{}' 失败: {}", service_name, e),
                }),
            }
        }
    }

    fn delete_scheduled_task(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        let task_name = &item.entry;
        let output = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", task_name, "/F"])
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

    fn delete_boot_execute(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        delete_lsa_multi_sz_at(
            HKEY_LOCAL_MACHINE,
            r"System\CurrentControlSet\Control\Session Manager",
            "BootExecute",
            &item.entry,
        )
    }

    fn delete_appinit_dlls(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        delete_lsa_multi_sz_at(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows",
            "AppInit_DLLs",
            &item.entry,
        )
    }

    fn delete_ifeo(item: &AutorunItem) -> Result<DeleteResult, IrError> {
        let (hive, subkey, _value_name) = match super::parse_location(&item.location) {
            Some(parsed) => parsed,
            None => {
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法解析 IFEO 路径: {}", item.location),
                });
            }
        };

        delete_registry_key(hive, &subkey)
    }

    fn delete_lsa_multi_sz(
        item: &AutorunItem,
        hive: HKEY,
        subkey: &str,
        value_name: &str,
    ) -> Result<DeleteResult, IrError> {
        delete_lsa_multi_sz_at(hive, subkey, value_name, &item.entry)
    }

    fn delete_com_or_office(
        item: &AutorunItem,
        hive: HKEY,
        subkey: &str,
    ) -> Result<DeleteResult, IrError> {
        if let Some(clsid) = super::extract_clsid(&item.location) {
            let clsid_path = format!(r"SOFTWARE\Classes\CLSID\{}", clsid);
            return delete_registry_key(hive, &clsid_path);
        }
        if item.entry.is_empty() {
            return Ok(DeleteResult {
                success: false,
                message: "COM/Office 项无法自动删除，请手动处理".into(),
            });
        }
        delete_registry_value(hive, subkey, &item.entry)
    }

    fn delete_registry_value(
        hive: HKEY,
        subkey: &str,
        value_name: &str,
    ) -> Result<DeleteResult, IrError> {
        let subkey_wide = HSTRING::from(subkey);
        let value_name_wide = HSTRING::from(value_name);

        unsafe {
            let mut h_key = Default::default();
            let result = RegOpenKeyExW(
                hive,
                &subkey_wide,
                None,
                KEY_SET_VALUE,
                &mut h_key,
            );

            if result.is_err() {
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法打开注册表项 '{}'", subkey),
                });
            }

            let result = RegDeleteValueW(h_key, &value_name_wide);
            if result.is_err() {
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
            let result = RegOpenKeyExW(
                hive,
                &subkey_wide,
                None,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut h_key,
            );

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
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("无法读取 MULTI_SZ 值 '{}'", value_name),
                    });
                }
            };

            // Remove the entry
            let entry_lower = entry_to_remove.to_lowercase();
            let new_strings: Vec<&String> = existing
                .iter()
                .filter(|s| s.to_lowercase() != entry_lower)
                .collect();

            if new_strings.len() == existing.len() {
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
fn parse_location(location: &str) -> Option<(windows::Win32::System::Registry::HKEY, String, String)> {
    let location = location.trim();

    let (hive_prefix, rest) = if let Some(r) = location.strip_prefix("HKLM\\") {
        ("HKLM", r)
    } else if let Some(r) = location.strip_prefix("HKEY_LOCAL_MACHINE\\") {
        ("HKLM", r)
    } else if let Some(r) = location.strip_prefix("HKCU\\") {
        ("HKCU", r)
    } else if let Some(r) = location.strip_prefix("HKEY_CURRENT_USER\\") {
        ("HKCU", r)
    } else if let Some(r) = location.strip_prefix("HKU\\") {
        ("HKU", r)
    } else if let Some(r) = location.strip_prefix("HKEY_USERS\\") {
        ("HKU", r)
    } else {
        return None;
    };

    use windows::Win32::System::Registry::*;

    let hive = match hive_prefix {
        "HKLM" => HKEY_LOCAL_MACHINE,
        "HKCU" => HKEY_CURRENT_USER,
        "HKU" => HKEY_USERS,
        _ => return None,
    };

    if let Some(pos) = rest.rfind('\\') {
        let subkey = &rest[..pos];
        let value_name = &rest[pos + 1..];
        Some((hive, subkey.to_owned(), value_name.to_owned()))
    } else {
        Some((hive, rest.to_owned(), String::new()))
    }
}

#[cfg(not(windows))]
fn parse_location(_location: &str) -> Option<((), String, String)> {
    None
}

#[cfg(windows)]
fn read_multi_sz(
    h_key: windows::Win32::System::Registry::HKEY,
    value_name: &windows::core::HSTRING,
) -> Option<Vec<String>> {
    use windows::Win32::System::Registry::*;
    use windows::core::PCWSTR;

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
    let re = regex::Regex::new(r"\{[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}").ok()?;
    let caps = re.find(text)?;
    Some(caps.as_str().to_owned())
}

#[cfg(windows)]
fn delete_registry_key_fallback(
    hive: windows::Win32::System::Registry::HKEY,
    subkey: &str,
) -> Result<(), irtool_core::IrError> {
    use windows::Win32::System::Registry::*;
    use windows::core::HSTRING;

    let subkey_wide = HSTRING::from(subkey);

    unsafe {
        // Try RegDeleteTreeW first (deletes key with all subkeys)
        let result = RegDeleteTreeW(hive, &subkey_wide);
        if result.is_ok() {
            return Ok(());
        }

        // Fallback: try simple RegDeleteKeyW
        let result = RegDeleteKeyW(hive, &subkey_wide);
        if result.is_err() {
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
    fn parse_location_hklm() {
        let result = parse_location(r"HKLM\SOFTWARE\Microsoft\Windows\Run\MyApp");
        assert!(result.is_some());
        let (_, subkey, value) = result.unwrap();
        assert_eq!(subkey, r"SOFTWARE\Microsoft\Windows\Run");
        assert_eq!(value, "MyApp");
    }

    #[test]
    fn parse_location_hkcu() {
        let result = parse_location(r"HKCU\SOFTWARE\Run\SomeValue");
        assert!(result.is_some());
        let (_, subkey, value) = result.unwrap();
        assert_eq!(subkey, r"SOFTWARE\Run");
        assert_eq!(value, "SomeValue");
    }

    #[test]
    fn parse_location_invalid() {
        let result = parse_location("NotARegistryPath");
        assert!(result.is_none());
    }

    #[test]
    fn extract_clsid_from_path() {
        let clsid = extract_clsid(
            r"HKLM\SOFTWARE\Classes\CLSID\{12345678-1234-1234-1234-123456789ABC}",
        );
        assert_eq!(
            clsid,
            Some("{12345678-1234-1234-1234-123456789ABC}".into())
        );
    }

    #[test]
    fn extract_clsid_no_match() {
        let clsid = extract_clsid("no clsid here");
        assert_eq!(clsid, None);
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
