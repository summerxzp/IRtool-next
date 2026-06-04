use crate::types::SignatureStatus;
use irtool_core::IrError;
use std::path::Path;
use std::path::PathBuf;

#[cfg(windows)]
pub fn verify(path: &Path) -> Result<SignatureStatus, IrError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Security::WinTrust::*;

    if !path.exists() {
        return Ok(SignatureStatus::Unsigned);
    }

    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();

    unsafe {
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(wide.as_ptr()),
            ..Default::default()
        };

        let mut trust_data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            ..Default::default()
        };
        trust_data.Anonymous.pFile = &mut file_info;

        let wintrust_action = WINTRUST_ACTION_GENERIC_VERIFY_V2;

        let result = WinVerifyTrust(
            HWND(std::ptr::null_mut()),
            &wintrust_action as *const _ as *mut _,
            &mut trust_data as *mut _ as *mut _,
        );

        const TRUST_E_NOSIGNATURE: i32 = 0x800B0100u32 as i32;
        match result {
            0 => Ok(SignatureStatus::Valid {
                signer: "Verified".into(),
            }),
            TRUST_E_NOSIGNATURE => Ok(SignatureStatus::Unsigned),
            code => {
                let message = format!("WinVerifyTrust error: 0x{:08X}", code as u32);
                Ok(SignatureStatus::Invalid { message })
            }
        }
    }
}

#[cfg(not(windows))]
pub fn verify(_path: &Path) -> Result<SignatureStatus, IrError> {
    Ok(SignatureStatus::NotVerified)
}

pub fn verify_batch(
    paths: &[PathBuf],
    progress: impl Fn(usize, usize) + Send + Sync,
) -> Vec<(PathBuf, SignatureStatus)> {
    use rayon::prelude::*;

    paths
        .par_iter()
        .enumerate()
        .map(|(i, path)| {
            let status = verify(path).unwrap_or(SignatureStatus::Invalid {
                message: "verify error".into(),
            });
            progress(i + 1, paths.len());
            (path.clone(), status)
        })
        .collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn verify_nonexistent_is_unsigned() {
        let path = PathBuf::from(r"C:\nonexistent_file_12345.exe");
        let status = verify(&path).unwrap();
        assert_eq!(status, SignatureStatus::Unsigned);
    }

    #[test]
    fn verify_system_file_has_signature() {
        let path = PathBuf::from(r"C:\Windows\System32\ntdll.dll");
        if !path.exists() {
            return;
        }
        let status = verify(&path).unwrap();
        match status {
            SignatureStatus::Valid { .. } => {}
            SignatureStatus::NotVerified => {}
            other => panic!("expected Valid or NotVerified, got {:?}", other),
        }
    }
}
