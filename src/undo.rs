//! Durable completed-action journal. Undo never overwrites an occupied original
//! path and refuses to remove files whose content changed after the operation.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stamp {
    pub bytes: u64,
    pub hash: String,
}
pub fn stamp(path: &Path) -> Result<Stamp, String> {
    let mut file = crate::transfers::source_file(path, false)?;
    stamp_file(&mut file)
}
pub(crate) fn stamp_file(file: &mut File) -> Result<Stamp, String> {
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || reparse(&meta) {
        return Err("Expected an ordinary file".into());
    }
    let mut hash = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    if bytes != meta.len() {
        return Err("File changed while checking content".into());
    }
    Ok(Stamp {
        bytes,
        hash: hash.finalize().to_hex().to_string(),
    })
}

fn reparse(m: &fs::Metadata) -> bool {
    if m.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    RenameBack {
        from: PathBuf,
        to: PathBuf,
        identity: String,
    },
    RemoveCopy {
        path: PathBuf,
        stamp: Stamp,
    },
    MoveBack {
        from: PathBuf,
        to: PathBuf,
        stamp: Stamp,
    },
    RestoreBackup {
        backup: PathBuf,
        target: PathBuf,
        stamp: Stamp,
    },
    RemoveEmpty {
        path: PathBuf,
        created: u64,
    },
    EnsureDirectory {
        path: PathBuf,
    },
    RestoreRecycle {
        original: PathBuf,
        data: PathBuf,
        info: PathBuf,
    },
}
impl Action {
    pub fn paths(&self) -> Vec<PathBuf> {
        match self {
            Self::RemoveCopy { path, .. }
            | Self::RemoveEmpty { path, .. }
            | Self::EnsureDirectory { path } => vec![path.clone()],
            Self::RenameBack { from, to, .. } | Self::MoveBack { from, to, .. } => {
                vec![from.clone(), to.clone()]
            }
            Self::RestoreBackup { target, .. } => vec![target.clone()],
            Self::RestoreRecycle { original, .. } => vec![original.clone()],
        }
    }
}
#[derive(Serialize, Deserialize)]
enum Event {
    Action(Action),
    Undone(usize),
    End { barrier: bool },
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub label: String,
    pub actions: Vec<(usize, Action)>,
}
fn root() -> PathBuf {
    #[cfg(test)]
    {
        std::env::current_dir()
            .unwrap()
            .join("target/undo-test-history")
    }
    #[cfg(not(test))]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("FileBeholder/history")
    }
}
pub struct Batch {
    pub directory: PathBuf,
    file: File,
    pub warning: Option<String>,
    count: usize,
}
impl Batch {
    pub fn new(label: &str) -> Result<Self, String> {
        Self::new_at(&root(), label)
    }
    fn new_at(root: &Path, label: &str) -> Result<Self, String> {
        static ID: AtomicU64 = AtomicU64::new(1);
        let name = format!(
            "{:032}-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        );
        let directory = root.join(name);
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        fs::write(directory.join("label"), label).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(directory.join("actions.jsonl"))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            directory,
            file,
            warning: None,
            count: 0,
        })
    }
    pub fn record(&mut self, action: Action) {
        self.count += 1;
        if let Err(e) = append(&mut self.file, &Event::Action(action)) {
            self.warning = Some(format!(
                "Operation completed, but Undo history could not be saved: {e}"
            ));
        }
    }
    pub fn finish(mut self, barrier: bool) -> Option<String> {
        let barrier = barrier || self.warning.is_some();
        if let Err(e) =
            append(&mut self.file, &Event::End { barrier }).and_then(|_| self.file.sync_all())
        {
            self.warning = Some(format!("Undo history could not be finalized: {e}"));
        }
        self.warning
    }
    pub fn backup(
        &mut self,
        path: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(PathBuf, Stamp), String> {
        let backup = self.directory.join(format!(
            "backup-{}-{}",
            self.count,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let stamp =
            crate::transfers::copy_file_checked(path, &backup, false, false, cancel, |_| {}, None)?;
        Ok((backup, stamp))
    }
}
fn append(file: &mut File, event: &Event) -> std::io::Result<()> {
    serde_json::to_writer(&mut *file, event)?;
    file.write_all(b"\n")?;
    file.flush()
}
pub fn latest() -> Option<Entry> {
    latest_from(&root())
}
fn latest_from(root: &Path) -> Option<Entry> {
    let mut directories = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect::<Vec<_>>();
    directories.sort();
    for directory in directories.into_iter().rev() {
        match load(&directory) {
            Ok((true, _)) => return None,
            Ok((false, Some(entry))) => return Some(entry),
            Ok((false, None)) => {}
            Err(_) => return None,
        }
    }
    None
}
pub fn load(directory: &Path) -> Result<(bool, Option<Entry>), String> {
    let file = File::open(directory.join("actions.jsonl")).map_err(|e| e.to_string())?;
    let mut actions = Vec::new();
    let mut undone = std::collections::BTreeSet::new();
    let (mut ended, mut barrier) = (false, false);
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        match serde_json::from_str::<Event>(&line).map_err(|e| e.to_string())? {
            Event::Action(action) => actions.push(action),
            Event::Undone(index) => {
                undone.insert(index);
            }
            Event::End { barrier: b } => {
                ended = true;
                barrier = b;
            }
        }
    }
    if !ended {
        return Ok((false, None));
    }
    if barrier {
        return Ok((true, None));
    }
    let actions = actions
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !undone.contains(i))
        .collect::<Vec<_>>();
    if actions.is_empty() {
        return Ok((false, None));
    }
    Ok((
        false,
        Some(Entry {
            path: directory.to_path_buf(),
            label: fs::read_to_string(directory.join("label"))
                .unwrap_or_else(|_| "file operation".into()),
            actions,
        }),
    ))
}

fn vacant(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(format!(
            "{} already exists; Undo will not overwrite it",
            path.display()
        )),
    }
}
pub fn created(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.created())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}
fn ordinary_parents(path: &Path) -> Result<(), String> {
    for parent in path.parent().into_iter().flat_map(|p| p.ancestors()) {
        if parent.as_os_str().is_empty() {
            continue;
        }
        let meta = fs::symlink_metadata(parent)
            .map_err(|e| format!("Parent directory unavailable: {}: {e}", parent.display()))?;
        if !meta.is_dir() || reparse(&meta) {
            return Err(format!(
                "Parent directory is a link or reparse point: {}; Undo left it untouched",
                parent.display()
            ));
        }
    }
    Ok(())
}
fn reverse(action: &Action) -> Result<(), String> {
    for path in action.paths() {
        ordinary_parents(&path)?;
    }
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match action {
        Action::RenameBack { from, to, identity } => {
            if rename_identity(from)? != *identity {
                return Err("Renamed item was replaced; Undo left it untouched.".into());
            }
            if normalized(from) != normalized(to) {
                vacant(to)?;
            }
            crate::rename::rename_item(from, to)
        }
        Action::RemoveCopy {
            path,
            stamp: expected,
        } => crate::transfers::remove_verified(path, expected),
        Action::MoveBack {
            from,
            to,
            stamp: expected,
        } => {
            vacant(to)?;
            crate::transfers::copy_file_checked(
                from,
                to,
                true,
                false,
                &cancel,
                |_| {},
                Some(expected),
            )
            .map(|_| ())
        }
        Action::RestoreBackup {
            backup,
            target,
            stamp: expected,
        } => {
            vacant(target)?;
            crate::transfers::copy_file_checked(
                backup,
                target,
                true,
                false,
                &cancel,
                |_| {},
                Some(expected),
            )
            .map(|_| ())
        }
        Action::RemoveEmpty {
            path,
            created: expected,
        } => {
            if *expected == 0
                || !fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !reparse(&m))
                || created(path) != *expected
            {
                return Err("Created directory was replaced; left untouched".into());
            }
            fs::remove_dir(path).map_err(|e| e.to_string())
        }
        Action::EnsureDirectory { path } => {
            if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !reparse(&m)) {
                Ok(())
            } else {
                fs::create_dir(path).map_err(|e| e.to_string())
            }
        }
        Action::RestoreRecycle {
            original,
            data,
            info,
        } => {
            vacant(original)?;
            let recorded = recycle_original(info).ok_or("Recycle Bin record is unavailable")?;
            if normalized(&recorded) != normalized(original) {
                return Err("Recycle Bin record no longer matches".into());
            }
            if info.file_name().and_then(|n| n.to_str()).is_none_or(|n| {
                !n.starts_with("$I")
                    || data.file_name().and_then(|r| r.to_str()) != Some(&format!("$R{}", &n[2..]))
            }) || data.parent() != info.parent()
                || data
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_none_or(|n| !n.starts_with("$R"))
            {
                return Err("Invalid Recycle Bin item".into());
            }
            fs::rename(data, original).map_err(|e| e.to_string())?;
            let _ = fs::remove_file(info);
            Ok(())
        }
    }
}
pub fn run(entry: Entry) -> (Vec<PathBuf>, Result<(), String>) {
    let affected = entry.actions.iter().flat_map(|(_, a)| a.paths()).collect();
    let result = (|| {
        let _lease = Lease::acquire(&entry.path)?;
        let mut file = OpenOptions::new()
            .append(true)
            .open(entry.path.join("actions.jsonl"))
            .map_err(|e| e.to_string())?;
        for (index, action) in entry.actions.iter().rev() {
            reverse(action)?;
            append(&mut file, &Event::Undone(*index))
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    (affected, result)
}
pub fn normalized(path: &Path) -> String {
    let s = path.to_string_lossy().replace('/', "\\");
    s.strip_prefix("\\\\?\\")
        .unwrap_or(&s)
        .trim_end_matches('\\')
        .to_lowercase()
}
pub fn recycle_records(original: &Path) -> Vec<(PathBuf, PathBuf)> {
    let text = original.to_string_lossy().replace('/', "\\");
    let text = text.strip_prefix("\\\\?\\").unwrap_or(&text);
    if text.len() < 3
        || !text.as_bytes()[0].is_ascii_alphabetic()
        || text.as_bytes()[1] != b':'
        || text.as_bytes()[2] != b'\\'
    {
        return vec![];
    }
    let root = PathBuf::from(&text[..3]).join("$Recycle.Bin");
    let mut records = vec![];
    if let Ok(owners) = fs::read_dir(root) {
        for owner in owners.flatten() {
            if let Ok(items) = fs::read_dir(owner.path()) {
                for item in items.flatten() {
                    let name = item.file_name().to_string_lossy().into_owned();
                    if name.starts_with("$I") {
                        if let Some(recorded) = recycle_original(&item.path()) {
                            if normalized(&recorded) == normalized(original) {
                                records.push((
                                    item.path(),
                                    item.path().with_file_name(format!("$R{}", &name[2..])),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    records
}
fn recycle_original(info: &Path) -> Option<PathBuf> {
    let bytes = fs::read(info).ok()?;
    if bytes.len() < 28 {
        return None;
    }
    let version = u64::from_le_bytes(bytes[..8].try_into().ok()?);
    let slice = match version {
        1 => bytes.get(24..544)?,
        2 => {
            let length = u32::from_le_bytes(bytes[24..28].try_into().ok()?) as usize;
            if !(1..=32768).contains(&length) {
                return None;
            }
            bytes.get(28..28 + length * 2)?
        }
        _ => return None,
    };
    let units = slice
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    let end = units.iter().position(|&x| x == 0)?;
    let units = &units[..end];

    Some(PathBuf::from(String::from_utf16(units).ok()?))
}

struct Lease {
    path: PathBuf,
    _file: File,
}
impl Lease {
    fn acquire(directory: &Path) -> Result<Self, String> {
        let path = directory.join("undo.lock");
        let file=OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_|"This action is already being undone. If a previous session crashed, inspect the history before removing undo.lock.".to_string())?;
        Ok(Self { path, _file: file })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static ID: AtomicU64 = AtomicU64::new(1);
            let path = std::env::current_dir().unwrap().join(format!(
                "target/undo-fixture-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn serialized_history_restores_replacement_and_checkpoints_completion() {
        let f = Fixture::new();
        let target = f.0.join("target");
        fs::write(&target, b"old").unwrap();
        let root = f.0.join("history");
        let mut batch = Batch::new_at(&root, "Copy").unwrap();
        let (backup, old) = batch
            .backup(&target, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();
        fs::write(&target, b"new").unwrap();
        batch.record(Action::RestoreBackup {
            backup,
            target: target.clone(),
            stamp: old,
        });
        batch.record(Action::RemoveCopy {
            path: target.clone(),
            stamp: stamp(&target).unwrap(),
        });
        assert!(batch.finish(false).is_none());
        let entry = latest_from(&root).unwrap();
        assert_eq!(entry.actions.len(), 2);
        assert!(run(entry).1.is_ok());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert!(latest_from(&root).is_none());
    }
    #[test]
    fn changed_copy_is_not_removed_and_original_collision_is_not_overwritten() {
        let f = Fixture::new();
        let path = f.0.join("copy");
        fs::write(&path, b"initial").unwrap();
        let expected = stamp(&path).unwrap();
        fs::write(&path, b"changed").unwrap();
        assert!(
            reverse(&Action::RemoveCopy {
                path: path.clone(),
                stamp: expected
            })
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"changed");
        let original = f.0.join("original");
        fs::write(&original, b"unrelated").unwrap();
        assert!(
            reverse(&Action::MoveBack {
                from: path.clone(),
                to: original.clone(),
                stamp: stamp(&path).unwrap()
            })
            .is_err()
        );
        assert_eq!(fs::read(original).unwrap(), b"unrelated");
        assert!(path.exists());
    }
    #[test]
    fn successful_inverse_steps_are_not_repeated_after_partial_failure() {
        let f = Fixture::new();
        let root = f.0.join("history");
        let a = f.0.join("a");
        let b = f.0.join("b");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let mut batch = Batch::new_at(&root, "Copy").unwrap();
        batch.record(Action::RemoveCopy {
            path: a.clone(),
            stamp: stamp(&a).unwrap(),
        });
        batch.record(Action::RemoveCopy {
            path: b.clone(),
            stamp: stamp(&b).unwrap(),
        });
        batch.finish(false);
        fs::write(&a, b"edited").unwrap();
        assert!(run(latest_from(&root).unwrap()).1.is_err());
        assert!(!b.exists());
        assert_eq!(latest_from(&root).unwrap().actions.len(), 1);
        fs::write(&a, b"a").unwrap();
        assert!(run(latest_from(&root).unwrap()).1.is_ok());
        assert!(!a.exists());
    }
    #[test]
    fn permanent_delete_barrier_does_not_offer_unrelated_older_undo() {
        let f = Fixture::new();
        let root = f.0.join("history");
        let mut older = Batch::new_at(&root, "old").unwrap();
        older.record(Action::EnsureDirectory {
            path: f.0.join("x"),
        });
        older.finish(false);
        Batch::new_at(&root, "Delete permanently")
            .unwrap()
            .finish(true);
        assert!(latest_from(&root).is_none());
    }
    #[test]
    fn recycle_metadata_and_restore_preserve_existing_original() {
        let f = Fixture::new();
        let original = f.0.join("original.txt");
        let info = f.0.join("$I123.txt");
        let data = f.0.join("$R123.txt");
        let mut bytes = vec![0; 28];
        bytes[..8].copy_from_slice(&2u64.to_le_bytes());
        let name = original
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        bytes[24..28].copy_from_slice(&(name.len() as u32).to_le_bytes());
        for unit in name {
            bytes.extend(unit.to_le_bytes());
        }
        fs::write(&info, bytes).unwrap();
        fs::write(&data, b"restore").unwrap();
        assert_eq!(recycle_original(&info), Some(original.clone()));
        let action = Action::RestoreRecycle {
            original: original.clone(),
            info: info.clone(),
            data: data.clone(),
        };
        fs::write(&original, b"occupied").unwrap();
        assert!(reverse(&action).is_err());
        assert!(data.exists());
        fs::remove_file(&original).unwrap();
        assert!(reverse(&action).is_ok());
        assert_eq!(fs::read(&original).unwrap(), b"restore");
        assert!(!info.exists());
    }
}

pub(crate) fn rename_identity(path: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        let file = OpenOptions::new()
            .access_mode(0)
            .share_mode(7)
            .custom_flags(0x02000000)
            .open(path)
            .map_err(|e| e.to_string())?;
        let mut info: windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION =
            unsafe { std::mem::zeroed() };
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(
                file.as_raw_handle(),
                &mut info,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(format!(
            "{}:{}:{}",
            info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
        ))
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;
    #[test]
    fn durable_rename_undo_restores_a_folder_and_refuses_occupied_or_replaced_items() {
        let dir = std::env::current_dir().unwrap().join(format!(
            "target/rename-undo-{}-{}",
            std::process::id(),
            crate::next_identity()
        ));
        fs::create_dir_all(&dir).unwrap();
        let from = dir.join("original");
        let to = dir.join("renamed");
        fs::create_dir(&from).unwrap();
        fs::write(from.join("child"), b"content").unwrap();
        let identity = rename_identity(&from).unwrap();
        let mut batch = Batch::new("Rename").unwrap();
        let history = batch.directory.clone();
        crate::rename::rename_item(&from, &to).unwrap();
        batch.record(Action::RenameBack {
            from: to.clone(),
            to: from.clone(),
            identity: identity.clone(),
        });
        batch.finish(false);
        fs::create_dir(&from).unwrap();
        assert!(run(load(&history).unwrap().1.unwrap()).1.is_err());
        fs::remove_dir(&from).unwrap();
        assert!(run(load(&history).unwrap().1.unwrap()).1.is_ok());
        assert_eq!(fs::read(from.join("child")).unwrap(), b"content");
        assert!(!to.exists());
        assert!(load(&history).unwrap().1.is_none());
        let other = dir.join("other");
        fs::create_dir(&other).unwrap();
        assert!(
            reverse(&Action::RenameBack {
                from: other.clone(),
                to: to.clone(),
                identity
            })
            .is_err()
        );
        assert!(other.exists());
        fs::remove_dir_all(dir).unwrap();
        fs::remove_dir_all(history).unwrap();
    }
}
