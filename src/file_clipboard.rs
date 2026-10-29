//! Native Windows file clipboard interoperability (CF_HDROP + drop effect).
use crate::transfers::Kind;
use std::path::PathBuf;
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
    };
    use windows_sys::Win32::{
        Foundation::{GlobalFree, HANDLE},
        System::{DataExchange::*, Memory::*},
        UI::{Shell::DragQueryFileW, WindowsAndMessaging::*},
    };
    struct Clipboard;
    impl Drop for Clipboard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    fn open() -> Option<Clipboard> {
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != std::process::id() || OpenClipboard(hwnd) == 0 {
                None
            } else {
                Some(Clipboard)
            }
        }
    }
    struct Memory(HANDLE);
    impl Memory {
        fn new(bytes: &[u8]) -> Option<Self> {
            unsafe {
                let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
                if handle.is_null() {
                    return None;
                }
                let memory = Self(handle);
                let pointer = GlobalLock(handle);
                if pointer.is_null() {
                    return None;
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast(), bytes.len());
                GlobalUnlock(handle);
                Some(memory)
            }
        }
        fn publish(self, format: u32) -> bool {
            unsafe {
                if SetClipboardData(format, self.0).is_null() {
                    false
                } else {
                    std::mem::forget(self);
                    true
                }
            }
        }
    }
    impl Drop for Memory {
        fn drop(&mut self) {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
    fn effect_format() -> u32 {
        let name: Vec<u16> = "Preferred DropEffect"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe { RegisterClipboardFormatW(name.as_ptr()) }
    }
    pub fn write(paths: &[PathBuf], kind: Kind) -> bool {
        let mut bytes = vec![0; 20];
        bytes[..4].copy_from_slice(&20u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        for path in paths {
            for c in path.as_os_str().encode_wide().chain(Some(0)) {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0, 0]);
        let Some(files) = Memory::new(&bytes) else {
            return false;
        };
        let Some(effect) =
            Memory::new(&(if kind == Kind::Move { 2u32 } else { 1u32 }).to_le_bytes())
        else {
            return false;
        };
        let Some(_clipboard) = open() else {
            return false;
        };
        if unsafe { EmptyClipboard() } == 0 {
            return false;
        }
        if !files.publish(15) {
            return false;
        }
        let _ = effect.publish(effect_format());
        true
    }
    pub fn read() -> Result<Option<(Kind, Vec<PathBuf>)>, ()> {
        unsafe {
            let _clipboard = open().ok_or(())?;
            let handle = GetClipboardData(15);
            if handle.is_null() {
                return Ok(None);
            }
            let count = DragQueryFileW(handle, 0xffff_ffff, std::ptr::null_mut(), 0);
            if count > 100_000 {
                return Err(());
            }
            let mut paths = Vec::new();
            for i in 0..count {
                let len = DragQueryFileW(handle, i, std::ptr::null_mut(), 0);
                if len == 0 || len > 32767 {
                    return Err(());
                }
                let mut name = vec![0u16; len as usize + 1];
                if DragQueryFileW(handle, i, name.as_mut_ptr(), len + 1) != len {
                    return Err(());
                }
                name.pop();
                paths.push(PathBuf::from(OsString::from_wide(&name)));
            }
            let effect = GetClipboardData(effect_format());
            let mut moving = false;
            if !effect.is_null() && GlobalSize(effect) >= 4 {
                let pointer = GlobalLock(effect);
                if !pointer.is_null() {
                    moving = std::ptr::read_unaligned(pointer as *const u32) & 2 != 0;
                    GlobalUnlock(effect);
                }
            }
            Ok(
                (!paths.is_empty())
                    .then_some((if moving { Kind::Move } else { Kind::Copy }, paths)),
            )
        }
    }
    pub fn clear() {
        if let Some(_clipboard) = open() {
            unsafe {
                EmptyClipboard();
            }
        }
    }
}
#[cfg(windows)]
pub use windows::{clear, read, write};
#[cfg(not(windows))]
pub fn write(_: &[PathBuf], _: Kind) -> bool {
    false
}
#[cfg(not(windows))]
pub fn read() -> Result<Option<(Kind, Vec<PathBuf>)>, ()> {
    Err(())
}
#[cfg(not(windows))]
pub fn clear() {}
