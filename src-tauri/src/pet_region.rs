use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct HitRect { x: f64, y: f64, width: f64, height: f64 }

#[tauri::command]
pub fn pet_set_region(window: tauri::WebviewWindow, rects: Option<Vec<HitRect>>) -> Result<(), String> {
    if window.label() != "main" { return Err("Solo disponible para Neeko".into()); }
    if let Some(rects) = &rects {
        if rects.len() > 4096 || rects.iter().any(|r| [r.x, r.y, r.width, r.height].iter().any(|v| !v.is_finite() || v.abs() > 32768.0) || r.width < 0.0 || r.height < 0.0) {
            return Err("Región inválida".into());
        }
    }
    #[cfg(windows)]
    {
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as windows_sys::Win32::Foundation::HWND;
        // The main window has no native border: CSS client coordinates share its origin.
        unsafe { apply(hwnd, rects.as_deref(), scale) }
    }
    #[cfg(not(windows))]
    { let _ = rects; Ok(()) }
}

#[cfg(windows)]
unsafe fn apply(hwnd: windows_sys::Win32::Foundation::HWND, rects: Option<&[HitRect]>, scale: f64) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::*;
    let Some(rects) = rects else {
        return if SetWindowRgn(hwnd, std::ptr::null_mut(), 1) != 0 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) };
    };
    let region = CreateRectRgn(0, 0, 0, 0);
    if region.is_null() { return Err("No se pudo crear la región".into()); }
    for r in rects {
        let part = CreateRectRgn((r.x * scale).floor() as i32, (r.y * scale).floor() as i32,
            ((r.x + r.width) * scale).ceil() as i32, ((r.y + r.height) * scale).ceil() as i32);
        if part.is_null() {
            DeleteObject(region);
            return Err("No se pudo crear el contorno".into());
        }
        let result = CombineRgn(region, region, part, RGN_OR);
        DeleteObject(part);
        if result == 0 {
            DeleteObject(region);
            return Err("No se pudo unir el contorno".into());
        }
    }
    if SetWindowRgn(hwnd, region, 1) == 0 {
        DeleteObject(region);
        return Err(std::io::Error::last_os_error().to_string());
    }
    // On success Windows owns region and frees it when replaced/reset.
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};

    #[test]
    fn native_region_excludes_empty_space_and_restores_window() {
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let hwnd = CreateWindowExW(0, class.as_ptr(), std::ptr::null(), WS_POPUP, 0, 0, 400, 600,
                std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
            assert!(!hwnd.is_null());
            let rects = [HitRect { x: 100.0, y: 100.0, width: 80.0, height: 200.0 },
                HitRect { x: 10.0, y: 10.0, width: 60.0, height: 40.0 }];
            apply(hwnd, Some(&rects), 1.5).unwrap();
            let region = CreateRectRgn(0, 0, 0, 0);
            assert_ne!(GetWindowRgn(hwnd, region), 0);
            assert_eq!(PtInRegion(region, 90, 200), 0); // transparent gap
            assert_ne!(PtInRegion(region, 170, 200), 0); // character at 150% DPI
            assert_ne!(PtInRegion(region, 20, 20), 0); // controls
            apply(hwnd, None, 1.5).unwrap();
            assert_eq!(GetWindowRgn(hwnd, region), 0); // rectangular default restored
            DeleteObject(region);
            DestroyWindow(hwnd);
        }
    }
}
