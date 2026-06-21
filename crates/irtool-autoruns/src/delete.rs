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

        // autorunsc 用 \(Default) 后缀表示注册表默认值
        // 统一处理：分离出真正的 subkey，删除默认值
        if subkey.to_lowercase().ends_with(r"\(default)") {
            let real_subkey = &subkey[..subkey.len() - r"\(Default)".len()];
            return delete_registry_default_value(hive, real_subkey);
        }

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
            // entry 是描述名而非 GUID，遍历子键匹配 DllName 值
            let subkey_lower = subkey.to_lowercase();
            let is_winlogon_notify = subkey_lower.ends_with(r"winlogon\notify");
            let is_winlogon_gpext = subkey_lower.ends_with(r"winlogon\gpextensions");
            if is_winlogon_notify || is_winlogon_gpext {
                let search = item.launch_string.as_deref()
                    .or(item.image_path.as_deref());
                if let Some(search) = search {
                    return super::find_and_delete_subkey_by_value(hive, &subkey, "DllName", search);
                }
                return Ok(DeleteResult {
                    success: false,
                    message: "GpExtensions/Notify 项缺少 launch_string 和 image_path，暂不支持自动删除，请手动处理".into(),
                });
            }
            // Credential Provider Filters 等 entry 为空的情况
            if value_name.is_empty() {
                return Ok(DeleteResult {
                    success: false,
                    message: "entry 为空，无法定位删除目标，请手动处理".into(),
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
        //     Catalog_Entries 子键是数字编号（如 000000000001），entry 是描述名
        if category == "winsock" || category == "winsock providers" || category == "network providers" {
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

        // Active Setup\Installed Components — CLSID-based subkey structure
        // autorunsc 的 location 是父键路径，entry 通常是描述名（如 "Microsoft Edge"），
        // CLSID 往往在 launch_string 或 image_path 中
        if subkey.to_lowercase().contains(r"active setup\installed components") {
            if let Some(clsid) = super::extract_clsid_from_item(item) {
                let full_subkey = format!(r"{}\{}", subkey, clsid);
                return delete_registry_key(hive, &full_subkey);
            }
            // 无 CLSID，遍历子键匹配 StubPath 值（与 launch_string 或 image_path 包含匹配）
            let search = item.launch_string.as_deref()
                .or(item.image_path.as_deref());
            if let Some(search) = search {
                return super::find_and_delete_subkey_by_value(hive, &subkey, "StubPath", search);
            }
            return Ok(DeleteResult {
                success: false,
                message: "Active Setup 项缺少 launch_string 和 image_path，暂不支持自动删除，请手动处理".into(),
            });
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
            // schtasks 输出使用系统 ANSI 编码（中文 Windows 为 GBK），用 encoding_rs 解码
            let (stderr, _, _) = encoding_rs::GBK.decode(&output.stderr);
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
        if item.entry.is_empty() {
            return Ok(DeleteResult {
                success: false,
                message: "IFEO 项 entry 为空，无法定位子键，请手动处理".into(),
            });
        }
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
        // 无 CLSID，遍历子键匹配默认值（与 entry 描述名包含匹配）
        super::find_and_delete_subkey_by_value(hive, subkey, "", &item.entry)
    }

    /// Explorer shell extensions: entries may be CLSID-based (context menu handlers,
    /// approved shell extensions) or plain value names.
    fn delete_explorer_addon(item: &AutorunItem, hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        let clsid = super::extract_clsid_from_item(item);
        if let Some(ref clsid) = clsid {
            // For shell extensions, the CLSID is typically a subkey name
            let full_subkey = format!(r"{}\{}", subkey, clsid);
            let result = delete_registry_key(hive, &full_subkey);
            if let Ok(dr) = &result {
                if dr.success {
                    // 子键删除成功，也尝试删除 CLSID 类注册
                    let clsid_path = format!(r"SOFTWARE\Classes\CLSID\{}", clsid);
                    let _ = delete_registry_key(hive, &clsid_path);
                    return Ok(DeleteResult {
                        success: true,
                        message: format!("Explorer 扩展 '{}' 已删除", clsid),
                    });
                }
            }
            // 子键不存在或删除失败，尝试作为值名删除
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
                // ERROR_FILE_NOT_FOUND (2): key doesn't exist — NOT success
                if result.0 == 2 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("注册表项 '{}' 不存在", subkey),
                    });
                }
                // ERROR_ACCESS_DENIED (5): 权限不足
                if result.0 == 5 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("权限不足，无法打开注册表项 '{}'（可能需要 SYSTEM 权限）", subkey),
                    });
                }
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法打开注册表项 '{}' (错误: 0x{:08X})", subkey, result.0),
                });
            }

            let result = RegDeleteValueW(h_key, &value_name_wide);
            let _ = RegCloseKey(h_key);
            if result.is_err() {
                // ERROR_FILE_NOT_FOUND (2): value doesn't exist — NOT success
                if result.0 == 2 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("注册表值 '{}' 不存在于 '{}'", value_name, subkey),
                    });
                }
                Ok(DeleteResult {
                    success: false,
                    message: format!("删除注册表值 '{}' 失败 (错误: 0x{:08X})", value_name, result.0),
                })
            } else {
                Ok(DeleteResult {
                    success: true,
                    message: format!("注册表值 '{}' 已删除", value_name),
                })
            }
        }
    }

    /// 删除注册表项的默认值（autorunsc 用 \(Default) 后缀表示）
    fn delete_registry_default_value(hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        use windows::core::PCWSTR;
        let subkey_wide = HSTRING::from(subkey);

        unsafe {
            let mut h_key = Default::default();
            let result = RegOpenKeyExW(hive, &subkey_wide, None, KEY_SET_VALUE, &mut h_key);

            if result.is_err() {
                if result.0 == 2 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("注册表项 '{}' 不存在", subkey),
                    });
                }
                if result.0 == 5 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("权限不足，无法打开注册表项 '{}'（可能需要 SYSTEM 权限）", subkey),
                    });
                }
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法打开注册表项 '{}' (错误: 0x{:08X})", subkey, result.0),
                });
            }

            // PCWSTR::null() 表示默认值
            let result = RegDeleteValueW(h_key, PCWSTR::null());
            let _ = RegCloseKey(h_key);
            if result.is_err() {
                if result.0 == 2 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("默认值不存在于 '{}'", subkey),
                    });
                }
                Ok(DeleteResult {
                    success: false,
                    message: format!("删除默认值失败 (错误: 0x{:08X})", result.0),
                })
            } else {
                Ok(DeleteResult {
                    success: true,
                    message: format!("默认值已删除于 '{}'", subkey),
                })
            }
        }
    }

    fn delete_registry_key(hive: HKEY, subkey: &str) -> Result<DeleteResult, IrError> {
        // 先检查 key 是否存在，不存在则返回 success=false（防止误报）
        let subkey_wide = HSTRING::from(subkey);
        unsafe {
            let mut h_key = Default::default();
            let open_result = RegOpenKeyExW(hive, &subkey_wide, None, KEY_READ, &mut h_key);
            if open_result.is_err() {
                if open_result.0 == 2 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("注册表项 '{}' 不存在", subkey),
                    });
                }
                if open_result.0 == 5 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("权限不足，无法打开注册表项 '{}'（可能需要 SYSTEM 权限）", subkey),
                    });
                }
                // 其他错误继续尝试删除
            } else {
                let _ = RegCloseKey(h_key);
            }
        }
        // key 存在或不确定，执行删除
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
                if result.0 == 2 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("注册表项 '{}' 不存在", subkey),
                    });
                }
                if result.0 == 5 {
                    return Ok(DeleteResult {
                        success: false,
                        message: format!("权限不足，无法打开注册表项 '{}'（可能需要 SYSTEM 权限）", subkey),
                    });
                }
                return Ok(DeleteResult {
                    success: false,
                    message: format!("无法打开注册表项 '{}' (错误: 0x{:08X})", subkey, result.0),
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

/// Extract CLSID from an AutorunItem — tries `location`, `entry`, `launch_string`, `image_path`.
/// autorunsc 对 Active Setup / BHO 等条目，entry 通常是描述名而非 CLSID，
/// CLSID 往往出现在 launch_string 或 image_path 中。
fn extract_clsid_from_item(item: &AutorunItem) -> Option<String> {
    extract_clsid(&item.location)
        .or_else(|| extract_clsid(&item.entry))
        .or_else(|| item.launch_string.as_deref().and_then(extract_clsid))
        .or_else(|| item.image_path.as_deref().and_then(extract_clsid))
}

/// 遍历父键的子键，读取每个子键的指定值名，与 search 字符串做不区分大小写的包含匹配。
/// 找到匹配的子键后删除整个子键（含所有子键和值）。
///
/// 用于 autorunsc 不输出 CLSID 的情况：
/// - Active Setup: value_name="StubPath", search=launch_string
/// - BHO: value_name="" (默认值), search=entry（描述名）
#[cfg(windows)]
fn find_and_delete_subkey_by_value(
    hive: windows::Win32::System::Registry::HKEY,
    parent_subkey: &str,
    value_name: &str,
    search: &str,
) -> Result<DeleteResult, IrError> {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::System::Registry::*;

    let parent_wide = HSTRING::from(parent_subkey);
    let search_lower = search.to_lowercase();

    // 在循环外创建 value_name 的 HSTRING，避免悬垂指针
    let value_name_hstring: Option<HSTRING> = if value_name.is_empty() {
        None
    } else {
        Some(HSTRING::from(value_name))
    };
    let value_name_pcwstr: PCWSTR = match &value_name_hstring {
        Some(h) => PCWSTR(h.as_ptr()),
        None => PCWSTR::null(),
    };

    unsafe {
        let mut h_parent = Default::default();
        let result = RegOpenKeyExW(
            hive,
            &parent_wide,
            None,
            KEY_READ | KEY_ENUMERATE_SUB_KEYS,
            &mut h_parent,
        );
        if result.is_err() {
            return Ok(DeleteResult {
                success: false,
                message: format!("无法打开注册表项 '{}' (错误: 0x{:08X})", parent_subkey, result.0),
            });
        }

        let mut index: u32 = 0;
        let mut matched_subkey: Option<String> = None;

        loop {
            let mut name_buf = [0u16; 256];
            let mut name_len = name_buf.len() as u32;

            let result = RegEnumKeyExW(
                h_parent,
                index,
                Some(PWSTR(name_buf.as_mut_ptr())),
                &mut name_len,
                None,
                None,
                None,
                None,
            );

            if result.is_err() {
                break;
            }

            let subkey_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            tracing::debug!(
                "find_and_delete_subkey_by_value: checking subkey '{}' under '{}'",
                subkey_name, parent_subkey
            );

            let subkey_wide = HSTRING::from(&subkey_name);
            let mut h_sub = Default::default();
            if RegOpenKeyExW(h_parent, &subkey_wide, None, KEY_READ, &mut h_sub).is_ok() {
                let mut buf = [0u8; 4096];
                let mut buf_len = buf.len() as u32;
                let mut reg_type = REG_NONE;

                let q_result = RegQueryValueExW(
                    h_sub,
                    value_name_pcwstr,
                    None,
                    Some(&mut reg_type),
                    Some(buf.as_mut_ptr()),
                    Some(&mut buf_len),
                );

                if q_result.is_ok() && (reg_type == REG_SZ || reg_type == REG_EXPAND_SZ) {
                    let value_str = String::from_utf16_lossy(
                        &std::slice::from_raw_parts(buf.as_ptr() as *const u16, (buf_len / 2) as usize),
                    );
                    tracing::debug!(
                        "find_and_delete_subkey_by_value: subkey '{}' {}='{}'",
                        subkey_name,
                        if value_name.is_empty() { "(Default)" } else { value_name },
                        value_str
                    );
                    let value_clean = value_str.trim_end_matches('\0');
                    let value_lower = value_clean.to_lowercase();
                    // 精确匹配，或文件名部分匹配（兼容 REG_EXPAND_SZ 未展开的情况）
                    let search_filename = search_lower.rsplit(['\\', '/']).next().unwrap_or(&search_lower);
                    let value_filename = value_lower.rsplit(['\\', '/']).next().unwrap_or(&value_lower);
                    if value_lower == search_lower || value_filename == search_filename {
                        matched_subkey = Some(subkey_name.clone());
                        let _ = RegCloseKey(h_sub);
                        break;
                    }
                }

                let _ = RegCloseKey(h_sub);
            }

            index += 1;
        }

        let _ = RegCloseKey(h_parent);

        if let Some(subkey_name) = matched_subkey {
            let full_path = format!(r"{}\{}", parent_subkey, subkey_name);
            tracing::info!("find_and_delete_subkey_by_value: matched subkey '{}'", full_path);
            match delete_registry_key_fallback(hive, &full_path) {
                Ok(()) => Ok(DeleteResult {
                    success: true,
                    message: format!("注册表项 '{}' 已删除", full_path),
                }),
                Err(e) => Ok(DeleteResult {
                    success: false,
                    message: format!("删除注册表项 '{}' 失败: {}", full_path, e),
                }),
            }
        } else {
            Ok(DeleteResult {
                success: false,
                message: format!(
                    "在 '{}' 下未找到匹配 '{}' 的子键",
                    parent_subkey, search
                ),
            })
        }
    }
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
        // ERROR_FILE_NOT_FOUND (2): key doesn't exist — idempotent success
        if result.0 == 2 {
            return Ok(());
        }

        // Fallback: try simple RegDeleteKeyW (for older Windows versions)
        let result = RegDeleteKeyW(hive, &subkey_wide);
        if result.is_ok() {
            return Ok(());
        }
        if result.0 == 2 {
            return Ok(());
        }
        Err(irtool_core::IrError::Io(format!(
            "删除注册表项失败: 0x{:08X}",
            result.0
        )))
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
