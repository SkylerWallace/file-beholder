// SPDX-License-Identifier: MIT
//! Native caption styling. Windows owns caption controls,
//! Snap, dragging, resizing and accessibility; older Windows keeps native chrome.
static CAPTION_EVENTS: std::sync::Mutex<
    Option<iced::futures::channel::mpsc::UnboundedSender<crate::Message>>,
> = std::sync::Mutex::new(None);
pub fn events() -> impl iced::futures::Stream<Item = crate::Message> {
    let (sender, receiver) = iced::futures::channel::mpsc::unbounded();
    *CAPTION_EVENTS.lock().unwrap() = Some(sender);
    receiver
}
#[cfg(windows)]
unsafe extern "system" fn caption_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    _: usize,
    _: usize,
) -> isize {
    use windows_sys::Win32::UI::{Shell::DefSubclassProc, WindowsAndMessaging::*};
    if msg == WM_SETTINGCHANGE {
        if let Some(sender) = CAPTION_EVENTS.lock().unwrap().as_ref() {
            let _ = sender.unbounded_send(crate::Message::MotionSettingsChanged);
        }
    }
    if msg == WM_NCLBUTTONDOWN || (msg == WM_ACTIVATE && wp & 0xffff == 0) {
        if let Some(sender) = CAPTION_EVENTS.lock().unwrap().as_ref() {
            let _ = sender.unbounded_send(crate::Message::WindowUnfocused);
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}
pub fn apply(window: &dyn iced::window::Window, t: crate::theme::Tokens, dark: bool) {
    #[cfg(windows)]
    if let Ok(handle) = window.window_handle() {
        if let iced::window::raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() {
            unsafe {
                use windows_sys::Win32::{
                    Foundation::FreeLibrary,
                    System::LibraryLoader::*,
                    UI::{
                        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
                        WindowsAndMessaging::*,
                    },
                };
                let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
                // Suppress caption icons while retaining the large taskbar icon.
                let extended = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                if extended & WS_EX_DLGMODALFRAME as isize == 0 {
                    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, extended | WS_EX_DLGMODALFRAME as isize);
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                }
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, 0);
                // Settings notifications are needed even when DWM is unavailable.
                windows_sys::Win32::UI::Shell::SetWindowSubclass(
                    hwnd,
                    Some(caption_proc),
                    0x4642,
                    0,
                );
                let name: Vec<u16> = "dwmapi.dll".encode_utf16().chain(Some(0)).collect();
                let library = LoadLibraryExW(
                    name.as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_LIBRARY_SEARCH_SYSTEM32,
                );
                if library.is_null() {
                    return;
                }
                if let Some(raw) = GetProcAddress(library, c"DwmSetWindowAttribute".as_ptr().cast())
                {
                    type Set = unsafe extern "system" fn(
                        windows_sys::Win32::Foundation::HWND,
                        u32,
                        *const std::ffi::c_void,
                        u32,
                    ) -> i32;
                    let set: Set = std::mem::transmute(raw);
                    let mut contrast: HIGHCONTRASTW = std::mem::zeroed();
                    contrast.cbSize = std::mem::size_of_val(&contrast) as u32;
                    let high = SystemParametersInfoW(
                        SPI_GETHIGHCONTRAST,
                        contrast.cbSize,
                        (&mut contrast as *mut HIGHCONTRASTW).cast(),
                        0,
                    ) != 0
                        && contrast.dwFlags & HCF_HIGHCONTRASTON != 0;
                    let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
                    // Hide caption branding while retaining the native caption
                    // controls, taskbar identity, dragging and Snap behavior.
                    #[repr(C)]
                    struct ThemeOptions {
                        flags: u32,
                        mask: u32,
                    }
                    #[link(name = "uxtheme")]
                    unsafe extern "system" {
                        fn SetWindowThemeAttribute(
                            hwnd: windows_sys::Win32::Foundation::HWND,
                            attribute: u32,
                            options: *const ThemeOptions,
                            size: u32,
                        ) -> i32;
                    }
                    let options = ThemeOptions { flags: 3, mask: 3 };
                    let _ = SetWindowThemeAttribute(
                        hwnd,
                        1,
                        &options,
                        std::mem::size_of_val(&options) as u32,
                    );
                    for (attribute, value) in [
                        (20, u32::from(dark && !high)),
                        (
                            35,
                            if high {
                                u32::MAX
                            } else {
                                t.navigation_background
                            },
                        ),
                        (36, if high { u32::MAX } else { t.text }),
                        (33, if high { 0 } else { 2 }),
                    ] {
                        let _ = set(hwnd, attribute, (&value as *const u32).cast(), 4);
                    }
                }
                FreeLibrary(library);
            }
        }
    }
    let _ = (window, t, dark);
}
