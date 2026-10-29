//! Cached Windows Shell file-association icons. Extraction runs on scan workers,
//! never during row construction, wheel events, or rasterization.
use iced::widget::image::Handle;
pub fn get(name: &str, directory: bool) -> Option<Handle> {
    #[cfg(windows)]
    {
        windows::get(name, directory)
    }
    #[cfg(not(windows))]
    {
        let _ = (name, directory);
        None
    }
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };
    use windows_sys::Win32::{
        Graphics::Gdi::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    static ICONS: OnceLock<Mutex<HashMap<String, Option<Handle>>>> = OnceLock::new();
    pub fn get(name: &str, directory: bool) -> Option<Handle> {
        let key = if directory {
            "<folder>".into()
        } else {
            std::path::Path::new(name)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                .unwrap_or_default()
        };
        let cache = ICONS.get_or_init(Default::default);
        if let Some(icon) = cache.lock().ok()?.get(&key) {
            return icon.clone();
        }
        let icon = extract(&key, directory);
        let mut guard = cache.lock().ok()?;
        if guard.len() < 1024 {
            guard.insert(key, icon.clone());
        }
        icon
    }
    fn extract(extension: &str, directory: bool) -> Option<Handle> {
        unsafe {
            use windows_sys::Win32::System::Com::{
                COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize,
            };
            struct Apartment(bool);
            impl Drop for Apartment {
                fn drop(&mut self) {
                    if self.0 {
                        unsafe {
                            CoUninitialize();
                        }
                    }
                }
            }
            let _apartment =
                Apartment(CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) >= 0);
            let path: Vec<u16> = (if directory {
                "folder".to_owned()
            } else {
                format!("file{extension}")
            })
            .encode_utf16()
            .chain(Some(0))
            .collect();
            let mut info: SHFILEINFOW = std::mem::zeroed();
            if SHGetFileInfoW(
                path.as_ptr(),
                if directory { 0x10 } else { 0x80 },
                &mut info,
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
            ) == 0
            {
                return None;
            }
            let result = render(info.hIcon);
            DestroyIcon(info.hIcon);
            result
        }
    }
    unsafe fn render(icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON) -> Option<Handle> {
        unsafe {
            let n = 40usize;
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut bitmap: BITMAPINFO = std::mem::zeroed();
            bitmap.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bitmap.bmiHeader.biWidth = n as i32;
            bitmap.bmiHeader.biHeight = -(n as i32);
            bitmap.bmiHeader.biPlanes = 1;
            bitmap.bmiHeader.biBitCount = 32;
            let mut bits = std::ptr::null_mut();
            let dib = CreateDIBSection(
                dc,
                &bitmap,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if dib.is_null() {
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, dib);
            let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, n * n * 4);
            pixels.fill(0);
            let ok = DrawIconEx(
                dc,
                0,
                0,
                icon,
                n as i32,
                n as i32,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            ) != 0;
            GdiFlush();
            let black = pixels.to_vec();
            pixels.fill(255);
            let ok = ok
                && DrawIconEx(
                    dc,
                    0,
                    0,
                    icon,
                    n as i32,
                    n as i32,
                    0,
                    std::ptr::null_mut(),
                    DI_NORMAL,
                ) != 0;
            GdiFlush();
            let mut rgba = vec![0; n * n * 4];
            for ((b, w), out) in black
                .chunks_exact(4)
                .zip(pixels.chunks_exact(4))
                .zip(rgba.chunks_exact_mut(4))
            {
                let alpha = 255
                    - w[0]
                        .saturating_sub(b[0])
                        .max(w[1].saturating_sub(b[1]))
                        .max(w[2].saturating_sub(b[2]));
                if alpha > 0 {
                    for c in 0..3 {
                        out[c] = ((b[2 - c] as u32 * 255) / alpha as u32).min(255) as u8;
                    }
                }
                out[3] = alpha;
            }
            SelectObject(dc, old);
            DeleteObject(dib);
            DeleteDC(dc);
            ok.then(|| Handle::from_rgba(n as u32, n as u32, rgba))
        }
    }
}
