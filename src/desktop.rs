// SPDX-License-Identifier: MIT
//! Windows completion notifications and desktop actions.
use crate::{Message, Task};
use std::path::PathBuf;
const DESKTOP_ERROR: &str = "Windows completion notification was unavailable.";
pub fn default_paths(mut drives: Vec<PathBuf>, home: PathBuf) -> [PathBuf; 4] {
    drives.sort_by_key(|p| {
        let key = p.to_string_lossy().to_ascii_uppercase();
        (key.as_bytes().first().is_some_and(|c| *c < b'C'), key)
    });
    drives.dedup();
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32");
    drives.push(home);
    drives.push(system.clone());
    std::array::from_fn(|i| drives.get(i).cloned().unwrap_or_else(|| system.clone()))
}
fn background(work: impl FnOnce() -> Result<(), String> + Send + 'static) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("Desktop action interrupted".into()))
        },
        Message::DesktopResult,
    )
}
pub fn open_task(path: PathBuf) -> Task<Message> {
    background(move || open(&path, false))
}
pub fn open_with_task(path: PathBuf) -> Task<Message> {
    background(move || open(&path, true))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenResult {
    Opened,
    NoAssociation,
    Cancelled,
    Failed(u32),
}
fn finish_open(
    result: OpenResult,
    choose: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match result {
        OpenResult::Opened | OpenResult::Cancelled => Ok(()),
        OpenResult::NoAssociation => choose(),
        OpenResult::Failed(error) => Err(format!("Could not open file (Windows error {error})")),
    }
}
#[cfg(windows)]
fn choose_app(file: &[u16]) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{OAIF_EXEC, OPENASINFO, SHOpenWithDialog};
    let info = OPENASINFO {
        pcszFile: file.as_ptr(),
        pcszClass: std::ptr::null(),
        oaifInFlags: OAIF_EXEC,
    };
    let result = unsafe { SHOpenWithDialog(std::ptr::null_mut(), &info) };
    // Cancelling the native chooser is a normal outcome, not a failed transfer/action.
    if result >= 0 || result as u32 == 0x800704c7 || result as u32 == 0x80004004 {
        Ok(())
    } else {
        Err(format!(
            "Could not show Open file with (Windows error 0x{:08X})",
            result as u32
        ))
    }
}
fn open(path: &std::path::Path, choose: bool) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use std::os::windows::ffi::OsStrExt;
        let file: Vec<u16> = std::path::absolute(path)
            .map_err(|e| e.to_string())?
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        use windows_sys::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
        };
        let initialized = CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
        if initialized < 0 {
            return Err(format!(
                "Could not initialize Windows file opening (0x{:08X})",
                initialized as u32
            ));
        }
        let result = if choose {
            choose_app(&file)
        } else {
            use windows_sys::Win32::{
                Foundation::{ERROR_CANCELLED, ERROR_NO_ASSOCIATION, GetLastError},
                UI::Shell::*,
            };
            let verb = wide("open");
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of_val(&info) as u32;
            info.fMask = SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC;
            info.lpVerb = verb.as_ptr();
            info.lpFile = file.as_ptr();
            info.nShow = 1;
            let result = if ShellExecuteExW(&mut info) != 0 {
                OpenResult::Opened
            } else {
                let error = GetLastError();
                if error == ERROR_NO_ASSOCIATION
                    || info.hInstApp as usize == SE_ERR_NOASSOC as usize
                {
                    OpenResult::NoAssociation
                } else if error == ERROR_CANCELLED {
                    OpenResult::Cancelled
                } else {
                    OpenResult::Failed(error)
                }
            };
            finish_open(result, || choose_app(&file))
        };
        CoUninitialize();
        result
    }
    #[cfg(not(windows))]
    {
        if choose {
            return Err("The Open file with chooser is available on Windows only.".into());
        }
        std::process::Command::new(if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        })
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
    }
}
pub fn completion_task(
    notification: bool,
    sound: crate::preferences::SoundChoice,
    custom: String,
    text: String,
) -> Task<Message> {
    background(move || {
        #[cfg(windows)]
        {
            let sound_result = crate::sounds::play(sound, &custom);
            let notification_result = if notification { notify(&text) } else { Ok(()) };
            sound_result.and(notification_result)
        }
        #[cfg(not(windows))]
        {
            let _ = (notification, sound, custom, text);
            Ok(())
        }
    })
}
#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
#[cfg(windows)]
fn notify(text: &str) -> Result<(), String> {
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    unsafe {
        let owner = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("File Beholder completion").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of_val(&data) as u32;
        data.hWnd = owner;
        data.uID = 765;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_INFO;
        data.hIcon = LoadIconW(std::ptr::null_mut(), IDI_INFORMATION);
        fn copy(out: &mut [u16], text: &str) {
            let count = out.len().saturating_sub(1);
            for (a, b) in out.iter_mut().take(count).zip(text.encode_utf16()) {
                *a = b
            }
        }
        copy(&mut data.szTip, "File Beholder");
        copy(&mut data.szInfoTitle, "Transfers completed");
        copy(
            &mut data.szInfo,
            if text.is_empty() {
                "Queue completed"
            } else {
                text
            },
        );
        data.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
        let ok = !owner.is_null() && Shell_NotifyIconW(NIM_ADD, &data) != 0;
        if ok {
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_secs(10) {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 100, QS_ALLINPUT);
            }
            Shell_NotifyIconW(NIM_DELETE, &data);
        }
        if !owner.is_null() {
            DestroyWindow(owner);
        }
        if ok {
            Ok(())
        } else {
            Err(DESKTOP_ERROR.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unassociated_files_choose_an_app_without_reprompting_cancelled_or_failed_opens() {
        let mut chosen = 0;
        assert!(
            finish_open(OpenResult::NoAssociation, || {
                chosen += 1;
                Ok(())
            })
            .is_ok()
        );
        assert_eq!(chosen, 1);
        for result in [
            OpenResult::Opened,
            OpenResult::Cancelled,
            OpenResult::Failed(5),
        ] {
            let result = finish_open(result, || {
                panic!(
                    "The app chooser must not handle successful, cancelled or access-denied opens"
                )
            });
            if let Err(message) = result {
                assert!(message.contains("5"));
            }
        }
        assert!(
            finish_open(OpenResult::NoAssociation, || Err(
                "Chooser unavailable".into()
            ))
            .unwrap_err()
            .contains("Chooser unavailable")
        );
    }
    #[test]
    fn default_paths_start_at_c_then_use_home_and_system32() {
        let paths = default_paths(
            vec![r"E:\".into(), r"C:\".into(), r"D:\".into()],
            "home".into(),
        );
        assert_eq!(
            paths,
            [
                PathBuf::from(r"C:\"),
                PathBuf::from(r"D:\"),
                PathBuf::from(r"E:\"),
                PathBuf::from("home")
            ]
        );
        let paths = default_paths(vec![r"C:\".into()], "home".into());
        assert_eq!(paths[0], PathBuf::from(r"C:\"));
        assert_eq!(paths[1], PathBuf::from("home"));
        assert!(paths[2].ends_with("System32"));
        assert_eq!(paths[2], paths[3]);
        let paths = default_paths(vec![], "home".into());
        assert_eq!(paths[0], PathBuf::from("home"));
        assert!(paths[1].ends_with("System32"));
    }
}

pub fn terminal_task(path: PathBuf) -> Task<Message> {
    background(move || {
        if !path.is_dir() {
            return Err("Terminal location is not an available folder.".into());
        }
        // Use the system PowerShell executable directly; the folder is a working
        // directory, never interpolated into a shell command.
        let executable = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = std::process::Command::new(executable);
        command.current_dir(path).arg("-NoLogo");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x00000010);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open terminal: {e}"))
    })
}
