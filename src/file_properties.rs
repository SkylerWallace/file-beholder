//! Cheap listing metadata and optional Shell properties loaded after the listing.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
type Values = [String; 6];
fn cache() -> &'static Mutex<HashMap<PathBuf, Values>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Values>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
#[cfg(test)]
mod tests {
    #[test]
    fn shell_properties_are_only_requested_for_visible_shell_columns() {
        use crate::columns::{Column, Configuration};
        let mut columns = Configuration::default();
        assert!(!super::needed(&columns));
        columns.toggle(Column::Created);
        columns.toggle(Column::Attributes);
        assert!(!super::needed(&columns));
        columns.toggle(Column::Company);
        assert!(super::needed(&columns));
        columns.toggle(Column::Company);
        assert!(!super::needed(&columns));
    }
}
pub fn get(path: &Path) -> Values {
    cache()
        .lock()
        .unwrap()
        .get(path)
        .cloned()
        .unwrap_or_default()
}
pub fn read(path: &Path, meta: Option<&std::fs::Metadata>) {
    let mut values = Values::default();
    values[0] = crate::browser::format_modified(meta.and_then(|m| m.created().ok()));
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let flags = meta.map_or(0, |m| m.file_attributes());
        values[2] = [
            (1, "R"),
            (2, "H"),
            (4, "S"),
            (16, "D"),
            (32, "A"),
            (2048, "C"),
            (16384, "E"),
        ]
        .into_iter()
        .filter(|(flag, _)| flags & flag != 0)
        .map(|(_, label)| label)
        .collect::<Vec<_>>()
        .join(" ");
    }
    let mut cache = cache().lock().unwrap();
    if cache.len() > 50_000 {
        // Evict a small batch instead of dropping thousands of values under
        // the mutex shared with rendering.
        let stale: Vec<_> = cache.keys().take(64).cloned().collect();
        for path in stale {
            cache.remove(&path);
        }
    }
    cache.insert(path.to_owned(), values);
}
pub fn needed(columns: &crate::columns::Configuration) -> bool {
    columns
        .visible()
        .iter()
        .any(|column| matches!(column.property_index(), Some(1 | 3 | 4 | 5)))
}
pub fn load(
    paths: Vec<PathBuf>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> iced::Task<()> {
    let (sender, receiver) = iced::futures::channel::mpsc::unbounded();
    std::thread::spawn(move || {
        crate::browser::background_priority();
        let mut last = std::time::Instant::now();
        for path in paths {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) || sender.is_closed() {
                return;
            }
            let mut values = get(&path);
            #[cfg(windows)]
            shell(&path, &mut values);
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            cache().lock().unwrap().insert(path, values);
            if last.elapsed() >= std::time::Duration::from_millis(250) {
                let _ = sender.unbounded_send(());
                last = std::time::Instant::now();
            }
        }
        let _ = sender.unbounded_send(());
    });
    iced::Task::run(receiver, |()| ())
}
#[cfg(windows)]
fn shell(path: &Path, values: &mut Values) {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};
    use windows_sys::{
        Win32::{
            Foundation::PROPERTYKEY,
            System::Com::{StructuredStorage::*, *},
            UI::Shell::PropertiesSystem::*,
        },
        core::GUID,
    };
    #[repr(C)]
    struct VTable {
        query: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        count: usize,
        at: usize,
        get: unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> i32,
    }
    unsafe {
        if CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) < 0 {
            return;
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let iid = GUID::from_u128(0x886d8eeb_8cf2_4446_8d02_cdba1dbdcf99);
        let mut store = ptr::null_mut();
        if SHGetPropertyStoreFromParsingName(
            wide.as_ptr(),
            ptr::null_mut(),
            GPS_BESTEFFORT,
            &iid,
            &mut store,
        ) >= 0
            && !store.is_null()
        {
            let table = &**(store as *const *const VTable);
            for (index, name) in [
                (1, "System.FileDescription"),
                (3, "System.Comment"),
                (4, "System.Author"),
                (5, "System.Company"),
            ] {
                let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                let mut key = std::mem::zeroed();
                let mut value: PROPVARIANT = std::mem::zeroed();
                if PSGetPropertyKeyFromName(name.as_ptr(), &mut key) >= 0
                    && (table.get)(store, &key, &mut value) >= 0
                {
                    let mut string = ptr::null_mut();
                    if PropVariantToStringAlloc(&value, &mut string) >= 0 && !string.is_null() {
                        let mut len = 0;
                        while *string.add(len) != 0 {
                            len += 1;
                        }
                        values[index] =
                            String::from_utf16_lossy(std::slice::from_raw_parts(string, len));
                        CoTaskMemFree(string.cast());
                    }
                }
                PropVariantClear(&mut value);
            }
            (table.release)(store);
        }
        CoUninitialize();
    }
}
