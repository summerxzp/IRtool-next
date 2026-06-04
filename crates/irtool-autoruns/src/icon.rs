use irtool_core::IrError;
use std::path::Path;

/// Extract icon from an executable file and return as PNG base64 string.
/// Returns None if extraction fails.
pub fn extract_icon_base64(image_path: &str) -> Result<Option<String>, IrError> {
    let path = Path::new(image_path);
    if !path.exists() {
        return Ok(None);
    }

    #[cfg(windows)]
    {
        extract_icon_win32(image_path)
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(None)
    }
}

/// Batch extract icons for multiple paths using rayon parallelism.
/// Returns a map of path -> base64 PNG data URL (None means no icon).
pub fn batch_extract_icons(paths: &[String]) -> Vec<(String, Option<String>)> {
    use rayon::prelude::*;

    // Deduplicate paths
    let unique: Vec<&str> = {
        let mut seen = std::collections::HashSet::new();
        paths
            .iter()
            .filter(|p| !p.is_empty())
            .filter(|p| seen.insert(p.as_str()))
            .map(|p| p.as_str())
            .collect()
    };

    let results: Vec<(String, Option<String>)> = unique
        .par_iter()
        .filter_map(|&path| {
            extract_icon_base64(path).ok().map(|icon| (path.to_owned(), icon))
        })
        .collect();

    results
}

#[cfg(windows)]
fn extract_icon_win32(image_path: &str) -> Result<Option<String>, IrError> {
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;

    let wide_path: Vec<u16> = image_path.encode_utf16().chain(std::iter::once(0)).collect();

    let mut shfi = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            windows::core::PCWSTR(wide_path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };

    if result == 0 || shfi.hIcon.is_invalid() {
        return Ok(None);
    }

    let icon = shfi.hIcon;
    let png_data = unsafe { icon_to_png(icon) };
    unsafe { let _ = DestroyIcon(icon); }

    match png_data {
        Some(data) => {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
            Ok(Some(format!("data:image/png;base64,{}", b64)))
        }
        None => Ok(None),
    }
}

#[cfg(windows)]
unsafe fn icon_to_png(icon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<Vec<u8>> {
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};
    use windows::Win32::Graphics::Gdi::{
        GetDIBits, CreateCompatibleDC, DeleteDC, DeleteObject, GetObjectA,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, BITMAP,
    };

    let mut icon_info = ICONINFO::default();
    GetIconInfo(icon, &mut icon_info).ok()?;

    let has_color = !icon_info.hbmColor.is_invalid();
    let has_mask = !icon_info.hbmMask.is_invalid();

    if !has_color && !has_mask {
        return None;
    }

    let hbm_source = if has_color { icon_info.hbmColor } else { icon_info.hbmMask };

    let mut bm = BITMAP::default();
    let ret = GetObjectA(
        hbm_source.into(),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    );
    if ret == 0 {
        if has_mask { let _ = DeleteObject(icon_info.hbmMask.into()); }
        if has_color { let _ = DeleteObject(icon_info.hbmColor.into()); }
        return None;
    }

    let width = bm.bmWidth.max(1) as u32;
    let height = if has_color {
        bm.bmHeight.max(1) as u32
    } else {
        (bm.bmHeight / 2).max(1) as u32
    };

    let hdc = CreateCompatibleDC(None);
    if hdc.is_invalid() {
        if has_mask { let _ = DeleteObject(icon_info.hbmMask.into()); }
        if has_color { let _ = DeleteObject(icon_info.hbmColor.into()); }
        return None;
    }

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];

    let scan_lines = GetDIBits(
        hdc,
        hbm_source,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    let _ = DeleteDC(hdc);
    if has_mask { let _ = DeleteObject(icon_info.hbmMask.into()); }
    if has_color { let _ = DeleteObject(icon_info.hbmColor.into()); }

    if scan_lines == 0 {
        return None;
    }

    // Convert BGRA to RGBA
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    // For mask-only icons, set alpha from the mask
    if !has_color {
        for chunk in pixels.chunks_exact_mut(4) {
            let gray = chunk[0];
            chunk[3] = if gray > 128 { 255 } else { 0 };
            chunk[0] = gray;
            chunk[1] = gray;
            chunk[2] = gray;
        }
    }

    let mut png_buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
    }

    Some(png_buf)
}
