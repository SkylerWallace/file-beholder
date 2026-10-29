//! Native Shell property sheets for one item or a same-folder selection.
//! All PIDLs, COM objects and the apartment stay on the properties worker.
use std::path::{Path, PathBuf};
pub fn show(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No selected items".into());
    }
    #[cfg(windows)]
    {
        windows::request(paths)
    }
    #[cfg(not(windows))]
    {
        Err("Native Windows properties are unavailable on this platform".into())
    }
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};
    use windows_sys::{
        Win32::{
            System::Com::{
                COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree,
                CoUninitialize,
            },
            UI::Shell::{
                Common::ITEMIDLIST, ILFindLastID, SHCreateDataObject, SHMultiFileProperties,
                SHParseDisplayName,
            },
        },
        core::GUID,
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    struct Pidl(*mut ITEMIDLIST);
    impl Drop for Pidl {
        fn drop(&mut self) {
            unsafe {
                CoTaskMemFree(self.0.cast());
            }
        }
    }
    #[repr(C)]
    struct UnknownVTable {
        query: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }
    struct DataObject(*mut c_void);
    impl Drop for DataObject {
        fn drop(&mut self) {
            unsafe {
                let table = *(self.0 as *const *const UnknownVTable);
                ((*table).release)(self.0);
            }
        }
    }
    fn check(result: i32, operation: &str) -> Result<(), String> {
        if result < 0 {
            Err(format!("{operation} failed (0x{:08X})", result as u32))
        } else {
            Ok(())
        }
    }
    fn parse(path: &Path) -> Result<Pidl, String> {
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        if wide[..wide.len() - 1].contains(&0) {
            return Err("Invalid path".into());
        }
        let mut raw = ptr::null_mut();
        let result = unsafe {
            SHParseDisplayName(wide.as_ptr(), ptr::null_mut(), &mut raw, 0, ptr::null_mut())
        };
        if result < 0 {
            if !raw.is_null() {
                unsafe {
                    CoTaskMemFree(raw.cast());
                }
            }
            check(result, "Resolve Shell item")?;
        }
        if raw.is_null() {
            return Err("Shell returned no item".into());
        }
        Ok(Pidl(raw))
    }
    struct Request {
        paths: Vec<PathBuf>,
        reply: std::sync::mpsc::SyncSender<Result<(), String>>,
    }
    struct Broker {
        sender: std::sync::mpsc::Sender<Request>,
        thread: u32,
    }
    fn broker() -> Result<&'static Broker, String> {
        static BROKER: std::sync::OnceLock<Result<Broker, String>> = std::sync::OnceLock::new();
        BROKER
            .get_or_init(|| {
                let (sender, receiver) = std::sync::mpsc::channel::<Request>();
                let (ready, started) = std::sync::mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    use windows_sys::Win32::{
                        System::Threading::GetCurrentThreadId, UI::WindowsAndMessaging::*,
                    };
                    let initialized = check(
                        unsafe {
                            CoInitializeEx(
                                ptr::null(),
                                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
                            )
                        },
                        "Initialize Shell apartment",
                    );
                    if let Err(error) = initialized {
                        let _ = ready.send(Err(error));
                        return;
                    }
                    let _apartment = Apartment;
                    let mut message: MSG = unsafe { std::mem::zeroed() };
                    unsafe {
                        PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
                    }
                    if ready.send(Ok(unsafe { GetCurrentThreadId() })).is_err() {
                        return;
                    }
                    loop {
                        let status = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
                        if status < 0 {
                            break;
                        }
                        if status == 0 {
                            continue;
                        }
                        if message.message == WM_APP + 37 {
                            while let Ok(request) = receiver.try_recv() {
                                let result = open(&request.paths);
                                let _ = request.reply.send(result);
                            }
                        } else {
                            unsafe {
                                TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                        }
                    }
                });
                let thread = started
                    .recv()
                    .map_err(|_| "Shell worker stopped during initialization".to_string())??;
                Ok(Broker { sender, thread })
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    pub(super) fn request(paths: &[PathBuf]) -> Result<(), String> {
        let broker = broker()?;
        let (reply, receive) = std::sync::mpsc::sync_channel(1);
        broker
            .sender
            .send(Request {
                paths: paths.to_vec(),
                reply,
            })
            .map_err(|_| "Shell worker is unavailable".to_string())?;
        if unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                broker.thread,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 37,
                0,
                0,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        receive
            .recv()
            .map_err(|_| "Shell worker stopped before opening Properties".to_string())?
    }
    fn open(paths: &[PathBuf]) -> Result<(), String> {
        check(
            unsafe {
                CoInitializeEx(
                    ptr::null(),
                    (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
                )
            },
            "Initialize Shell apartment",
        )?;
        let _apartment = Apartment;
        let absolute = paths
            .iter()
            .map(std::path::absolute)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let parent = absolute[0]
            .parent()
            .ok_or("Selected item has no parent folder")?;
        if absolute.iter().any(|p| {
            p.parent().is_none_or(|other| {
                crate::undo::normalized(other) != crate::undo::normalized(parent)
            })
        }) {
            return Err("Selected items do not share a parent folder".into());
        }
        let folder = parse(parent)?;
        let items = absolute
            .iter()
            .map(|p| parse(p))
            .collect::<Result<Vec<_>, _>>()?;
        let children = items
            .iter()
            .map(|item| unsafe { ILFindLastID(item.0) as *const ITEMIDLIST })
            .collect::<Vec<_>>();
        if children.iter().any(|p| p.is_null()) {
            return Err("Shell returned an invalid child item".into());
        }
        let iid = GUID {
            data1: 0x0000010e,
            data2: 0,
            data3: 0,
            data4: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
        };
        let mut raw = ptr::null_mut();
        let result = unsafe {
            SHCreateDataObject(
                folder.0,
                children
                    .len()
                    .try_into()
                    .map_err(|_| "Too many selected items")?,
                children.as_ptr(),
                ptr::null_mut(),
                &iid,
                &mut raw,
            )
        };
        if result < 0 {
            if !raw.is_null() {
                drop(DataObject(raw));
            }
            check(result, "Create Shell selection")?;
        }
        if raw.is_null() {
            return Err("Shell returned no selection".into());
        }
        let object = DataObject(raw);
        check(
            unsafe { SHMultiFileProperties(object.0, 0) },
            "Open Windows properties",
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_selection_is_rejected_without_a_dialog() {
        assert!(show(&[]).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn missing_item_returns_error_for_the_metadata_fallback() {
        let missing = std::env::current_dir()
            .unwrap()
            .join("target/does-not-exist-native-properties-check");
        assert!(show(&[missing]).is_err());
    }
}
