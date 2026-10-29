use iced::{Task, futures::channel::mpsc};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub hidden: bool,
    pub bytes: Option<u64>,
    pub partial: bool,
    pub modified: Option<std::time::SystemTime>,
    pub modified_label: String,
    pub icon: Option<iced::widget::image::Handle>,
}
#[derive(Debug, Clone)]
pub enum Event {
    Backend(String),
    Listed(Result<Vec<Entry>, String>),
    Sizes(Vec<(usize, u64, bool)>),
    Finished,
}
/// Keep filesystem enumeration and recursive scans below interactive UI work.
pub(crate) fn background_priority() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::*;
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}
pub fn scan(
    path: PathBuf,
    cancel: Arc<AtomicBool>,
    refresh: bool,
    mode: crate::preferences::ScanMode,
) -> Task<Event> {
    let (sender, receiver) = mpsc::unbounded();
    std::thread::spawn(move || {
        background_priority();
        let entries = match list(&path) {
            Ok(e) => e,
            Err(e) => {
                let _ = sender.unbounded_send(Event::Listed(Err(e)));
                return;
            }
        };
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if sender
            .unbounded_send(Event::Listed(Ok(entries.clone())))
            .is_err()
        {
            return;
        }
        use crate::preferences::ScanMode;
        if mode == ScanMode::Off {
            let _ = sender.unbounded_send(Event::Backend("Directory size scanning off".into()));
            let _ = sender.unbounded_send(Event::Finished);
            return;
        }
        let index = if mode == ScanMode::Recursive {
            Err("Recursive mode".into())
        } else {
            let _ = sender.unbounded_send(Event::Backend("Checking NTFS index…".into()));
            crate::ntfs::get(&path, &cancel, refresh)
        };
        let backend = match &index {
            Ok(index) => format!("NTFS MFT · {} records · snapshot", index.record_count()),
            Err(reason) if mode == ScanMode::Mft => format!("MFT unavailable · {reason}"),
            Err(reason) => format!("Recursive · {reason}"),
        };
        let _ = sender.unbounded_send(Event::Backend(backend));
        let mut sizes = Vec::new();
        let mut last_flush = std::time::Instant::now();
        for (i, entry) in entries.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            if entry.directory {
                let indexed = index
                    .as_ref()
                    .ok()
                    .and_then(|index| index.folder(&entry.path));
                let indexed = indexed.map(|s| (s.bytes, s.partial));
                let result = match mode {
                    ScanMode::Off => None,
                    ScanMode::Mft => indexed,
                    ScanMode::Auto if indexed.is_some() => indexed,
                    _ => Some(size_progress(&entry.path, &cancel, |bytes| {
                        if last_flush.elapsed() >= std::time::Duration::from_millis(250) {
                            sizes.push((i, bytes, true));
                            if sender
                                .unbounded_send(Event::Sizes(std::mem::take(&mut sizes)))
                                .is_err()
                            {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            last_flush = std::time::Instant::now();
                        }
                    })),
                };
                let Some((bytes, partial)) = result else {
                    continue;
                };
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                sizes.push((i, bytes, partial));
                if last_flush.elapsed() >= std::time::Duration::from_millis(250) {
                    if sender
                        .unbounded_send(Event::Sizes(std::mem::take(&mut sizes)))
                        .is_err()
                    {
                        return;
                    }
                    last_flush = std::time::Instant::now();
                }
            }
        }
        if !sizes.is_empty() {
            let _ = sender.unbounded_send(Event::Sizes(sizes));
        }
        let _ = sender.unbounded_send(Event::Finished);
    });
    Task::run(receiver, |e| e)
}
fn link_like(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
pub(crate) fn list(path: &Path) -> Result<Vec<Entry>, String> {
    let reader = fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for item in reader {
        let item = item.map_err(|e| e.to_string())?;
        entries.push(describe(item));
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.path.cmp(&b.path))
    });
    Ok(entries)
}
/// DirEntry metadata reuses Windows find data instead of reopening every path.
pub(crate) fn describe(item: fs::DirEntry) -> Entry {
    let meta = item.metadata();
    #[cfg(windows)]
    let hidden = {
        use std::os::windows::fs::MetadataExt;
        meta.as_ref().is_ok_and(|m| m.file_attributes() & 2 != 0)
    };
    #[cfg(not(windows))]
    let hidden = item.file_name().to_string_lossy().starts_with('.');
    let modified = meta.as_ref().ok().and_then(|m| m.modified().ok());
    crate::file_properties::read(&item.path(), meta.as_ref().ok());
    let (directory, bytes, partial) = match meta {
        Ok(m) => (
            m.is_dir() && !link_like(&m),
            if m.is_file() && !link_like(&m) {
                Some(m.len())
            } else if link_like(&m) {
                Some(0)
            } else {
                None
            },
            link_like(&m),
        ),
        Err(_) => (false, None, true),
    };
    let name = item.file_name().to_string_lossy().into_owned();
    Entry {
        path: item.path(),
        hidden,
        icon: crate::file_icons::get(&name, directory),
        name,
        directory,
        bytes,
        partial,
        modified,
        modified_label: format_modified(modified),
    }
}
pub fn format_modified(time: Option<std::time::SystemTime>) -> String {
    let Some(time) = time else {
        return "—".into();
    };
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{FILETIME, SYSTEMTIME},
            Storage::FileSystem::FileTimeToLocalFileTime,
            System::Time::FileTimeToSystemTime,
        };
        let ticks = match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => (d.as_nanos() / 100) as i128 + 116444736000000000,
            Err(e) => 116444736000000000 - (e.duration().as_nanos() / 100) as i128,
        };
        if !(0..=u64::MAX as i128).contains(&ticks) {
            return "—".into();
        }
        let ticks = ticks as u64;
        let ft = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut local = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
        if unsafe { FileTimeToLocalFileTime(&ft, &mut local) } != 0
            && unsafe { FileTimeToSystemTime(&local, &mut st) } != 0
        {
            return format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute
            );
        }
    }
    format!(
        "{} s UTC",
        time.duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    )
}
/// Collect every descendant directory total in one traversal for recursive search.
/// Parent accumulation runs backwards so nested totals never require another walk.
pub(crate) fn size_tree(
    root: &Path,
    cancel: &AtomicBool,
) -> std::collections::HashMap<PathBuf, (u64, bool)> {
    let mut totals = std::collections::HashMap::new();
    let mut pending = vec![root.to_path_buf()];
    let mut parents = Vec::new();
    totals.insert(root.to_path_buf(), (0u64, false));
    while let Some(path) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return totals;
        }
        let mut bytes = 0u64;
        let mut partial = false;
        match fs::read_dir(&path) {
            Err(_) => partial = true,
            Ok(reader) => {
                for item in reader {
                    if cancel.load(Ordering::Relaxed) {
                        return totals;
                    }
                    let item = match item {
                        Ok(item) => item,
                        Err(_) => {
                            partial = true;
                            continue;
                        }
                    };
                    match item.metadata() {
                        Ok(m) if link_like(&m) => partial = true,
                        Ok(m) if m.is_dir() => {
                            let child = item.path();
                            totals.insert(child.clone(), (0, false));
                            parents.push((child.clone(), path.clone()));
                            pending.push(child);
                        }
                        Ok(m) if m.is_file() => bytes = bytes.saturating_add(m.len()),
                        Ok(_) => {}
                        Err(_) => partial = true,
                    }
                }
            }
        }
        totals.insert(path, (bytes, partial));
    }
    for (child, parent) in parents.into_iter().rev() {
        let (bytes, partial) = totals[&child];
        let total = totals.get_mut(&parent).expect("enumerated parent");
        total.0 = total.0.saturating_add(bytes);
        total.1 |= partial;
    }
    totals
}
fn size_progress(path: &Path, cancel: &AtomicBool, mut progress: impl FnMut(u64)) -> (u64, bool) {
    let mut visited = 0usize;
    let mut pending = vec![path.to_path_buf()];
    let mut total = 0u64;
    let mut partial = false;
    while let Some(path) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return (total, true);
        }
        let reader = match fs::read_dir(path) {
            Ok(r) => r,
            Err(_) => {
                partial = true;
                continue;
            }
        };
        for item in reader {
            if cancel.load(Ordering::Relaxed) {
                return (total, true);
            }
            let item = match item {
                Ok(i) => i,
                Err(_) => {
                    partial = true;
                    continue;
                }
            };
            visited += 1;
            if visited % 128 == 0 {
                progress(total);
            }
            match item.metadata() {
                Ok(m) if link_like(&m) => partial = true,
                Ok(m) if m.is_dir() => pending.push(item.path()),
                Ok(m) if m.is_file() => total = total.saturating_add(m.len()),
                Ok(_) => {}
                Err(_) => partial = true,
            }
        }
    }
    progress(total);
    (total, partial)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursive_sizes_and_real_directory_types() {
        let path = std::env::temp_dir().join(format!("fileman-test-{}", std::process::id()));
        fs::create_dir_all(path.join("folder.with.dot/nested")).unwrap();
        fs::write(path.join("folder.with.dot/nested/data"), [0u8; 37]).unwrap();
        fs::write(path.join("extensionless"), [0u8; 11]).unwrap();
        let entries = list(&path).unwrap();
        assert!(entries[0].directory);
        assert_eq!(entries[1].bytes, Some(11));
        assert_eq!(
            size_progress(&path, &AtomicBool::new(false), |_| {}),
            (48, false)
        );
        assert_eq!(
            size_progress(&path, &AtomicBool::new(true), |_| {}),
            (0, true)
        );
        let cancel = AtomicBool::new(false);
        let totals = size_tree(&path, &cancel);
        assert_eq!(totals[&path], (48, false));
        assert_eq!(totals[&path.join("folder.with.dot")], (37, false));
        assert_eq!(totals[&path.join("folder.with.dot/nested")], (37, false));
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn missing_path_is_an_error() {
        assert!(list(Path::new("missing-fileman-test-directory/absent")).is_err());
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;
    #[test]
    fn large_directory_emits_partial_totals_before_completion_and_cancels_promptly() {
        let root = std::env::temp_dir().join(format!(
            "file-beholder-streaming-{}-{}",
            std::process::id(),
            crate::next_identity()
        ));
        fs::create_dir(&root).unwrap();
        for i in 0..512 {
            fs::write(root.join(i.to_string()), [0u8; 32]).unwrap();
        }
        let cancel = AtomicBool::new(false);
        let mut samples = Vec::new();
        let total = size_progress(&root, &cancel, |n| samples.push(n));
        assert_eq!(total, (512 * 32, false));
        assert!(samples.iter().any(|n| *n > 0 && *n < total.0));
        assert!(samples.windows(2).all(|p| p[0] <= p[1]));
        assert!(
            samples.len() <= 6,
            "callbacks are sampled, not emitted per entry"
        );
        let total = size_progress(&root, &cancel, |_| cancel.store(true, Ordering::Relaxed));
        assert!(total.1);
        assert!(total.0 < 512 * 32);
        fs::remove_dir_all(root).unwrap();
    }
}
