//! Sequential background jobs with bounded progress notifications and retries.
//! Destination replacement requires an explicit conflict decision. A move removes only a successfully copied
//! source, after the destination has been flushed and committed.
use iced::{Task, futures::channel::mpsc};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Copy,
    Move,
    Delete,
    Recycle,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Move => "Move",
            Self::Delete => "Delete permanently",
            Self::Recycle => "Recycle",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Request {
    pub kind: Kind,
    pub sources: Vec<PathBuf>,
    pub destination: Option<PathBuf>,
}
impl Request {
    fn target_for(&self, source: &Path) -> Option<PathBuf> {
        let root = self.sources.iter().find(|root| source.starts_with(root))?;
        Some(
            self.destination
                .as_ref()?
                .join(root.file_name()?)
                .join(source.strip_prefix(root).ok()?),
        )
    }
    fn source_for(&self, target: &Path) -> Option<PathBuf> {
        let destination = self
            .destination
            .as_ref()
            .and_then(|d| fs::canonicalize(d).ok())?;
        self.sources.iter().find_map(|root| {
            let base = destination.join(root.file_name()?);
            Some(root.join(target.strip_prefix(base).ok()?))
        })
    }
}
#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub total_items: usize,
    pub done_items: usize,
    pub transferred: u64,
    pub bytes_per_second: f64,
    pub failures: usize,
    pub status: String,
    pub eta: Option<u64>,
}
impl Progress {
    pub fn percent(&self) -> f32 {
        if self.total_bytes > 0 {
            (self.done_bytes as f64 / self.total_bytes as f64 * 100.0).clamp(0.0, 100.0) as f32
        } else if self.total_items > 0 {
            self.done_items as f32 / self.total_items as f32 * 100.0
        } else {
            0.0
        }
    }
}
#[derive(Debug, Clone)]
pub struct Issue {
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub reason: String,
    pub skipped: bool,
}
impl Issue {
    pub fn retry_request(&self, kind: Kind) -> Option<Request> {
        if !matches!(kind, Kind::Copy | Kind::Move) {
            return None;
        }
        Some(Request {
            kind,
            sources: vec![self.source.clone()],
            destination: Some(self.target.as_ref()?.parent()?.to_path_buf()),
        })
    }
}
#[derive(Debug, Clone)]
pub struct Report {
    pub issues: Vec<Issue>,
    pub cancelled: bool,
    pub failures: Vec<String>,
    pub failure_count: usize,
    pub succeeded: usize,
    /// Successfully completed selected roots, never their recursive children.
    pub completed_sources: Vec<(PathBuf, Option<PathBuf>)>,
    pub undo_warning: Option<String>,
    #[cfg(test)]
    pub history: Option<PathBuf>,
}
#[derive(Debug, Clone)]
pub enum Event {
    Progress(Progress),
    Conflict(
        PathBuf,
        PathBuf,
        std::sync::mpsc::SyncSender<ConflictChoice>,
    ),
    Finished(Report),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    Replace,
    Skip,
    KeepBoth,
    RenameExisting,
}
pub struct Job {
    pub id: u64,
    pub request: Request,
    pub progress: Progress,
    pub state: &'static str,
    pub report: Option<Report>,
    pub cancel: Arc<AtomicBool>,
}
impl Job {
    pub fn has_retryable_issues(&self) -> bool {
        self.report.as_ref().is_some_and(|report| {
            report
                .issues
                .iter()
                .any(|issue| issue.retry_request(self.request.kind).is_some())
        })
    }
    pub fn failed_retry_requests(&self) -> Vec<Request> {
        let mut requests: Vec<Request> = Vec::new();
        if let Some(report) = &self.report {
            for issue in &report.issues {
                let Some(request) = issue.retry_request(self.request.kind) else {
                    continue;
                };
                if let Some(group) = requests
                    .iter_mut()
                    .find(|group| group.destination == request.destination)
                {
                    if !group.sources.contains(&issue.source) {
                        group.sources.push(issue.source.clone());
                    }
                } else {
                    requests.push(request);
                }
            }
        }
        requests
    }
    pub fn new(id: u64, request: Request) -> Self {
        Self {
            id,
            request,
            progress: Progress {
                status: "Waiting".into(),
                ..Default::default()
            },
            state: "Queued",
            report: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}
pub fn run(request: Request, cancel: Arc<AtomicBool>) -> Task<Event> {
    let (sender, receiver) = mpsc::unbounded();
    std::thread::spawn(move || {
        execute_with_conflicts(
            request,
            &cancel,
            |event| {
                let _ = sender.unbounded_send(event);
            },
            |source, target| {
                let (reply, receive) = std::sync::mpsc::sync_channel(1);
                if sender
                    .unbounded_send(Event::Conflict(source.into(), target.into(), reply))
                    .is_err()
                {
                    return ConflictChoice::Skip;
                }
                loop {
                    match receive.recv_timeout(Duration::from_millis(100)) {
                        Ok(choice) => return choice,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            return ConflictChoice::Skip;
                        }
                        _ if cancel.load(Ordering::Relaxed) => return ConflictChoice::Skip,
                        _ => {}
                    }
                }
            },
        );
    });
    Task::run(receiver, |event| event)
}
#[derive(Debug)]
enum Operation {
    Directory(PathBuf),
    File {
        source: PathBuf,
        target: PathBuf,
        bytes: u64,
    },
    Delete(PathBuf, bool),
    Recycle(PathBuf),
    RemoveDirectory(PathBuf),
    Failure(PathBuf, String),
}
fn link(meta: &fs::Metadata) -> bool {
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
fn roots(sources: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut sorted = sources;
    sorted.sort();
    sorted.dedup();
    let mut out: Vec<PathBuf> = vec![];
    for path in sorted {
        if !out.iter().any(|root| path.starts_with(root)) {
            out.push(path);
        }
    }
    out
}
fn plan(request: &Request, cancel: &AtomicBool) -> Vec<Operation> {
    let destination = request.destination.as_ref().map(|p| fs::canonicalize(p));
    let mut out = vec![];
    for root in roots(request.sources.clone()) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if root.file_name().is_none() {
            out.push(Operation::Failure(
                root.clone(),
                "Drive roots cannot be operated on".into(),
            ));
            continue;
        }
        let canonical = match fs::canonicalize(&root) {
            Ok(p) => p,
            Err(e) => {
                out.push(Operation::Failure(
                    root.clone(),
                    format!("{}: {e}", root.display()),
                ));
                continue;
            }
        };
        if request.kind == Kind::Recycle {
            out.push(Operation::Recycle(root));
            continue;
        }
        let target = if request.kind == Kind::Delete {
            PathBuf::new()
        } else {
            let dest = match destination.as_ref() {
                Some(Ok(p)) if p.is_dir() => p,
                _ => {
                    out.push(Operation::Failure(
                        root.clone(),
                        "Destination directory is unavailable".into(),
                    ));
                    continue;
                }
            };
            if dest.starts_with(&canonical) {
                out.push(Operation::Failure(
                    root.clone(),
                    format!(
                        "Cannot paste {} into itself or a descendant",
                        root.display()
                    ),
                ));
                continue;
            }
            let target = dest.join(root.file_name().unwrap());
            if fs::canonicalize(&target).ok().as_ref() == Some(&canonical) {
                out.push(Operation::Failure(
                    root.clone(),
                    "Source and destination are the same".into(),
                ));
                continue;
            }
            target
        };
        let mut pending = vec![(root, target, false)];
        while let Some((source, target, visited)) = pending.pop() {
            if cancel.load(Ordering::Relaxed) {
                return out;
            }
            if visited {
                if request.kind != Kind::Copy {
                    out.push(Operation::RemoveDirectory(source));
                }
                continue;
            }
            let meta = match fs::symlink_metadata(&source) {
                Ok(m) => m,
                Err(e) => {
                    out.push(Operation::Failure(
                        source.clone(),
                        format!("{}: {e}", source.display()),
                    ));
                    continue;
                }
            };
            if link(&meta) {
                // A delete removes the link itself; never enumerate its target.
                if request.kind == Kind::Delete {
                    out.push(Operation::Delete(source, meta.is_dir()));
                } else {
                    out.push(Operation::Failure(
                        source.clone(),
                        format!("Skipped link/reparse point: {}", source.display()),
                    ));
                }
            } else if meta.is_dir() {
                if request.kind != Kind::Delete {
                    out.push(Operation::Directory(target.clone()));
                }
                pending.push((source.clone(), target.clone(), true));
                match fs::read_dir(&source) {
                    Ok(reader) => {
                        for item in reader {
                            match item {
                                Ok(e) => {
                                    pending.push((e.path(), target.join(e.file_name()), false))
                                }
                                Err(e) => out.push(Operation::Failure(
                                    source.clone(),
                                    format!("{}: {e}", source.display()),
                                )),
                            }
                        }
                    }
                    Err(e) => out.push(Operation::Failure(
                        source.clone(),
                        format!("{}: {e}", source.display()),
                    )),
                }
            } else if meta.is_file() {
                if request.kind == Kind::Delete {
                    out.push(Operation::Delete(source, false));
                } else {
                    out.push(Operation::File {
                        source,
                        target,
                        bytes: meta.len(),
                    });
                }
            } else {
                out.push(Operation::Failure(
                    source.clone(),
                    format!("Unsupported file type: {}", source.display()),
                ));
            }
        }
    }
    out
}
const ATTEMPTS: usize = 4;
fn retry<T>(
    cancel: &AtomicBool,
    mut action: impl FnMut(usize) -> Result<T, String>,
    mut failed: impl FnMut(usize, &str),
) -> Result<T, String> {
    let mut error = "Cancelled".to_string();
    for attempt in 1..=ATTEMPTS {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        match action(attempt) {
            Ok(value) => return Ok(value),
            Err(e) => {
                if e.starts_with("Copied; source retained:") {
                    return Err(e);
                }
                error = e;
                failed(attempt, &error);
            }
        }
        if attempt < ATTEMPTS {
            for _ in 0..attempt * 5 {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        }
    }
    Err(error)
}
struct Stage {
    path: PathBuf,
    owned: bool,
}
impl Drop for Stage {
    fn drop(&mut self) {
        if self.owned {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub(crate) fn source_file(path: &Path, moving: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .access_mode(0x8000_0000 | if moving { 0x0001_0000 } else { 0 })
            .share_mode(1)
            .custom_flags(0x0020_0000);
    }
    let _ = moving;
    options.open(path).map_err(|e| e.to_string())
}
fn commit(source: &Path, target: &Path, replace: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_RENAME_INFO, FileRenameInfo, SetFileInformationByHandle,
        };
        let file = OpenOptions::new()
            .access_mode(0x0001_0000)
            .share_mode(7)
            .open(source)
            .map_err(|e| format!("Open staged file for commit: {e}"))?;
        let name: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
        let bytes = (offset + name.len() * 2).max(std::mem::size_of::<FILE_RENAME_INFO>());
        let mut buffer = vec![0u64; bytes.div_ceil(8)];
        unsafe {
            let info = buffer.as_mut_ptr() as *mut FILE_RENAME_INFO;
            (*info).Anonymous.ReplaceIfExists = replace;
            (*info).RootDirectory = std::ptr::null_mut();
            (*info).FileNameLength = ((name.len() - 1) * 2) as u32;
            // FileNameLength excludes the terminator, but Win32 requires the buffer to include it.
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                (info as *mut u8).add(offset).cast(),
                name.len(),
            );
            if SetFileInformationByHandle(
                file.as_raw_handle(),
                FileRenameInfo,
                info.cast(),
                bytes as u32,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if replace {
            return fs::rename(source, target).map_err(|e| e.to_string());
        }
        fs::hard_link(source, target).map_err(|e| e.to_string())?;
        fs::remove_file(source).map_err(|e| e.to_string())
    }
}
fn remove_copied_source(file: &File, path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
        };
        let info = FILE_DISPOSITION_INFO { DeleteFile: true };
        if unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle(),
                FileDispositionInfo,
                (&info as *const FILE_DISPOSITION_INFO).cast(),
                std::mem::size_of_val(&info) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let _ = path;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}
#[cfg(test)]
fn copy_file_with_replace(
    source: &Path,
    target: &Path,
    moving: bool,
    replace: bool,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), String> {
    copy_file_checked(source, target, moving, replace, cancel, progress, None).map(|_| ())
}
pub(crate) fn copy_file_checked(
    source: &Path,
    target: &Path,
    moving: bool,
    replace: bool,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
    expected: Option<&crate::undo::Stamp>,
) -> Result<crate::undo::Stamp, String> {
    let mut input = source_file(source, moving).map_err(|e| format!("Open source: {e}"))?;
    let before = input
        .metadata()
        .map_err(|e| format!("Source metadata: {e}"))?;
    if !before.is_file() || link(&before) {
        return Err("Source is no longer an ordinary file".into());
    }
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    let temporary = target.with_file_name(format!(
        ".fileman-{}-{}.part",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let mut stage = Stage {
        path: temporary,
        owned: true,
    };
    let mut buffer = vec![0; 1024 * 1024];
    let mut bytes = 0u64;
    let mut hash = blake3::Hasher::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        hash.update(&buffer[..n]);
        bytes = bytes.saturating_add(n as u64);
        progress(bytes);
    }
    let after = input.metadata().map_err(|e| e.to_string())?;
    if bytes != before.len()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err("Source changed while copying".into());
    }
    let fingerprint = crate::undo::Stamp {
        bytes,
        hash: hash.finalize().to_hex().to_string(),
    };
    if expected.is_some_and(|e| *e != fingerprint) {
        return Err("File changed since the operation; Undo left it untouched".into());
    }
    if let Ok(modified) = before.modified() {
        output
            .set_times(fs::FileTimes::new().set_modified(modified))
            .map_err(|e| format!("Preserve timestamp: {e}"))?;
    }
    output
        .sync_all()
        .map_err(|e| format!("Flush output: {e}"))?;
    drop(output);
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    commit(&stage.path, target, replace).map_err(|e| format!("Commit destination: {e}"))?;
    stage.owned = false;
    if moving {
        // If deletion fails, retain both copies and report it. Never retry a
        // fresh copy over the committed destination or silently remove it.
        retry(cancel, |_| remove_copied_source(&input, source), |_, _| {})
            .map_err(|e| format!("Copied; source retained: {e}"))?;
    }
    Ok(fingerprint)
}
#[cfg(test)]
pub fn execute(request: Request, cancel: &AtomicBool, emit: impl FnMut(Event)) {
    execute_with_conflicts(request, cancel, emit, |_, _| ConflictChoice::Skip);
}
#[cfg(test)]
fn copy_file(
    source: &Path,
    target: &Path,
    moving: bool,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), String> {
    copy_file_with_replace(source, target, moving, false, cancel, progress)
}
fn execute_with_conflicts(
    request: Request,
    cancel: &AtomicBool,
    mut emit: impl FnMut(Event),
    mut resolve: impl FnMut(&Path, &Path) -> ConflictChoice,
) {
    let mut progress = Progress {
        status: "Preparing file list…".into(),
        ..Default::default()
    };
    emit(Event::Progress(progress.clone()));
    let mut request = request;
    for source in &mut request.sources {
        if let Ok(absolute) = std::path::absolute(&*source) {
            *source = absolute;
        }
    }
    if let Some(destination) = &mut request.destination {
        if let Ok(absolute) = std::path::absolute(&*destination) {
            *destination = absolute;
        }
    }
    let mut history = match crate::undo::Batch::new(request.kind.label()) {
        Ok(h) => h,
        Err(e) => {
            emit(Event::Finished(Report {
                cancelled: false,
                issues: request
                    .sources
                    .iter()
                    .map(|source| Issue {
                        source: source.clone(),
                        target: request
                            .destination
                            .as_ref()
                            .and_then(|d| source.file_name().map(|n| d.join(n))),
                        reason: format!("Cannot prepare Undo history: {e}"),
                        skipped: false,
                    })
                    .collect(),
                failures: vec![format!("Cannot prepare Undo history: {e}")],
                failure_count: 1,
                succeeded: 0,
                completed_sources: vec![],
                undo_warning: None,
                #[cfg(test)]
                history: None,
            }));
            return;
        }
    };
    let operations = plan(&request, cancel);
    // Resolve known collisions before any destination writes. Recheck during
    // execution too: another process may create a destination after this scan.
    let mut decisions = std::collections::HashMap::new();
    for operation in &operations {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if let Operation::File { source, target, .. } = operation {
            if fs::symlink_metadata(target).is_ok() {
                decisions.insert(target.clone(), resolve(source, target));
            }
        }
    }
    progress.total_items = operations.len();
    progress.total_bytes = operations
        .iter()
        .filter_map(|op| {
            if let Operation::File { bytes, .. } = op {
                Some(*bytes)
            } else {
                None
            }
        })
        .fold(0, u64::saturating_add);
    let mut root_counts = request
        .sources
        .iter()
        .map(|root| (root.clone(), 0usize, 0usize, request.target_for(root)))
        .collect::<Vec<_>>();
    let root_indices = root_counts
        .iter()
        .enumerate()
        .map(|(index, (root, _, _, _))| (root.clone(), index))
        .collect::<std::collections::BTreeMap<_, _>>();
    for op in &operations {
        let source = match op {
            Operation::File { source, .. }
            | Operation::Failure(source, _)
            | Operation::Recycle(source)
            | Operation::Delete(source, _)
            | Operation::RemoveDirectory(source) => source.clone(),
            Operation::Directory(target) => {
                request.source_for(target).unwrap_or_else(|| target.clone())
            }
        };
        for ancestor in source.ancestors() {
            if let Some(index) = root_indices.get(ancestor) {
                root_counts[*index].1 += 1;
            }
        }
    }
    let started = Instant::now();
    let mut notified = Instant::now();
    let mut errors = Vec::new();
    let mut issues = Vec::new();
    let mut succeeded = 0;
    let mut irreversible = false;
    let mut blocked = Vec::<PathBuf>::new();
    emit(Event::Progress(progress.clone()));
    for operation in operations {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let planned_bytes = match &operation {
            Operation::File { bytes, .. } => *bytes,
            _ => 0,
        };
        let (source, target) = match &operation {
            Operation::File { source, target, .. } => (source.clone(), Some(target.clone())),
            Operation::Failure(source, _)
            | Operation::Recycle(source)
            | Operation::Delete(source, _)
            | Operation::RemoveDirectory(source) => (source.clone(), None),
            Operation::Directory(target) => (
                request.source_for(target).unwrap_or_else(|| target.clone()),
                None,
            ),
        };
        let mut issue = Issue {
            source,
            target,
            reason: String::new(),
            skipped: false,
        };
        if matches!(&operation, Operation::Failure(_, _)) {
            issue.target = request.target_for(&issue.source);
        }
        let completed = progress.done_bytes;
        let path = match &operation {
            Operation::File { source, .. }
            | Operation::Recycle(source)
            | Operation::Delete(source, _)
            | Operation::RemoveDirectory(source)
            | Operation::Directory(source) => source.display().to_string(),
            Operation::Failure(_, message) => message.clone(),
        };
        let result = match &operation {
            Operation::Failure(_, error) => Err(error.clone()),
            Operation::File { source, target, .. } => {
                if blocked.iter().any(|root| target.starts_with(root)) {
                    Err("Destination parent failed; skipped".into())
                } else {
                    let mut target = target.clone();
                    let mut replace = false;
                    if fs::symlink_metadata(&target).is_ok() {
                        let choice = decisions
                            .remove(&target)
                            .unwrap_or_else(|| resolve(source, &target));
                        // Refuse replacing the source itself, including case aliases.
                        if fs::canonicalize(source).ok() == fs::canonicalize(&target).ok()
                            && choice != ConflictChoice::KeepBoth
                        {
                            progress.status = "Source and destination are the same; skipped".into();
                            issue.reason = progress.status.clone();
                            issue.skipped = true;
                            issues.push(issue);
                            progress.done_items += 1;
                            progress.done_bytes += planned_bytes;
                            continue;
                        }
                        progress.status =
                            format!("Waiting for conflict decision: {}", target.display());
                        emit(Event::Progress(progress.clone()));
                        match choice {
                            ConflictChoice::Skip => {
                                issue.reason = "Skipped existing destination".into();
                                issue.skipped = true;
                                issues.push(issue);
                                progress.done_items += 1;
                                progress.done_bytes += planned_bytes;
                                continue;
                            }
                            ConflictChoice::KeepBoth => target = unique_target(&target),
                            ConflictChoice::Replace => replace = true,
                            ConflictChoice::RenameExisting => {
                                let old = old_target(&target);
                                match crate::undo::stamp(&target).and_then(|stamp| {
                                    crate::rename::rename_item(&target, &old)?;
                                    history.record(crate::undo::Action::MoveBack {
                                        from: old,
                                        to: target.clone(),
                                        stamp,
                                    });
                                    Ok(())
                                }) {
                                    Ok(()) => {}
                                    Err(error) => {
                                        issue.reason =
                                            format!("Cannot rename existing file: {error}");
                                        issues.push(issue);
                                        progress.failures += 1;
                                        progress.done_items += 1;
                                        errors.push(error);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    issue.target = Some(target.clone());
                    let backup = if replace {
                        progress.status = format!("Preserving {} for Undo…", target.display());
                        emit(Event::Progress(progress.clone()));
                        match history.backup(&target, cancel) {
                            Ok(b) => Some(b),
                            Err(e) => {
                                progress.failures += 1;
                                errors.push(format!("Cannot preserve destination for Undo: {e}"));
                                issue.reason = format!("Cannot preserve destination for Undo: {e}");
                                issues.push(issue);
                                progress.done_items += 1;
                                progress.done_bytes += planned_bytes;
                                continue;
                            }
                        }
                    } else {
                        None
                    };
                    let result = retry(
                        cancel,
                        |attempt| {
                            progress.status = format!(
                                "{} {} · attempt {attempt}/{ATTEMPTS}",
                                request.kind.label(),
                                source.display()
                            );
                            copy_file_checked(
                                source,
                                &target,
                                request.kind == Kind::Move,
                                replace,
                                cancel,
                                |bytes| {
                                    if notified.elapsed() >= Duration::from_millis(100) {
                                        let mut current = progress.clone();
                                        current.done_bytes = completed
                                            .saturating_add(bytes)
                                            .min(current.total_bytes);
                                        let rate = (progress.transferred + bytes) as f64
                                            / started.elapsed().as_secs_f64().max(0.001);
                                        current.bytes_per_second = rate;
                                        current.eta = (rate > 0.0).then(|| {
                                            ((current
                                                .total_bytes
                                                .saturating_sub(current.done_bytes))
                                                as f64
                                                / rate)
                                                .ceil()
                                                as u64
                                        });
                                        emit(Event::Progress(current));
                                        notified = Instant::now();
                                    }
                                },
                                None,
                            )
                        },
                        |_, _| {},
                    );
                    if result.is_ok()
                        || result
                            .as_ref()
                            .is_err_and(|e| e.starts_with("Copied; source retained:"))
                    {
                        if let Some((backup, stamp)) = backup {
                            history.record(crate::undo::Action::RestoreBackup {
                                backup,
                                target: target.clone(),
                                stamp,
                            });
                        }
                        match result.clone().or_else(|_| crate::undo::stamp(&target)) {
                            Ok(stamp) => {
                                history.record(if request.kind == Kind::Move && result.is_ok() {
                                    crate::undo::Action::MoveBack {
                                        from: target.clone(),
                                        to: source.clone(),
                                        stamp,
                                    }
                                } else {
                                    crate::undo::Action::RemoveCopy {
                                        path: target.clone(),
                                        stamp,
                                    }
                                })
                            }
                            Err(e) => {
                                history.warning =
                                    Some(format!("Undo unavailable for {}: {e}", target.display()))
                            }
                        }
                    } else if let Some((backup, _)) = backup {
                        let _ = fs::remove_file(backup);
                    }
                    if result.is_ok() {
                        progress.transferred = progress.transferred.saturating_add(planned_bytes);
                    }
                    result.map(|_| ())
                }
            }
            Operation::Directory(target) => {
                let existed = fs::symlink_metadata(target).is_ok();
                let result = if blocked.iter().any(|root| target.starts_with(root)) {
                    Err("Destination parent failed; skipped".into())
                } else {
                    retry(
                        cancel,
                        |_| {
                            if target.is_dir()
                                && fs::symlink_metadata(target).is_ok_and(|m| !link(&m))
                            {
                                Ok(())
                            } else {
                                fs::create_dir(target).map_err(|e| e.to_string())
                            }
                        },
                        |_, _| {},
                    )
                };
                if result.is_ok() && !existed {
                    history.record(crate::undo::Action::RemoveEmpty {
                        path: target.clone(),
                        created: crate::undo::created(target),
                    });
                }
                if result.is_err() {
                    blocked.push(target.clone());
                }
                result
            }
            Operation::Recycle(path) => {
                let previous = crate::undo::recycle_records(path);
                let result = retry(cancel, |_| recycle(path), |_, _| {});
                if result.is_ok() {
                    if let Some((info, data)) = crate::undo::recycle_records(path)
                        .into_iter()
                        .find(|r| !previous.contains(r))
                    {
                        history.record(crate::undo::Action::RestoreRecycle {
                            original: path.clone(),
                            data,
                            info,
                        });
                    } else {
                        history.warning=Some("Undo could not locate this Recycle Bin item; restore it from Windows Recycle Bin.".into());
                    }
                }
                result
            }
            Operation::Delete(path, directory) => retry(
                cancel,
                |_| {
                    if *directory {
                        fs::remove_dir(path)
                    } else {
                        fs::remove_file(path)
                    }
                    .map_err(|e| e.to_string())
                },
                |_, _| {},
            ),
            // remove_dir never recursively removes an untransferred child.
            Operation::RemoveDirectory(path) => {
                let result = retry(
                    cancel,
                    |_| fs::remove_dir(path).map_err(|e| e.to_string()),
                    |_, _| {},
                );
                if result.is_ok() && request.kind == Kind::Move {
                    history.record(crate::undo::Action::EnsureDirectory { path: path.clone() });
                }
                result
            }
        };
        if result.is_ok() {
            for ancestor in issue.source.ancestors() {
                if let Some(index) = root_indices.get(ancestor) {
                    let (root, _, completed, target) = &mut root_counts[*index];
                    *completed += 1;
                    if issue.source == *root && issue.target.is_some() {
                        *target = issue.target.clone();
                    }
                }
            }
        }
        if request.kind == Kind::Delete && result.is_ok() {
            irreversible = true;
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        progress.done_items += 1;
        progress.done_bytes = completed.saturating_add(planned_bytes);
        let failed = result.is_err();
        match result {
            Ok(()) => {
                succeeded += 1;
                progress.status = format!("{}: {}", request.kind.label(), path);
            }
            Err(error) => {
                progress.failures += 1;
                progress.status = format!("Failed: {path}: {error}");
                issue.skipped = error.contains("skipped") || error.starts_with("Skipped");
                issue.reason = error;
                issues.push(issue);
                errors.push(progress.status.clone());
            }
        }
        progress.bytes_per_second =
            progress.transferred as f64 / started.elapsed().as_secs_f64().max(0.001);
        if notified.elapsed() >= Duration::from_millis(100) || failed {
            emit(Event::Progress(progress.clone()));
            notified = Instant::now();
        }
    }
    let cancelled = cancel.load(Ordering::Relaxed);
    if !cancelled {
        progress.done_items = progress.total_items;
        progress.done_bytes = progress.total_bytes;
    }
    progress.eta = None;
    progress.bytes_per_second = 0.0;
    progress.status = if cancelled {
        "Cancelled; completed files retained".into()
    } else if !issues.is_empty() {
        format!(
            "Completed with {} failed, {} skipped items",
            progress.failures,
            issues.iter().filter(|issue| issue.skipped).count()
        )
    } else {
        "Completed".into()
    };
    emit(Event::Progress(progress.clone()));
    emit(Event::Finished(Report {
        cancelled,
        issues,
        failures: errors,
        failure_count: progress.failures,
        succeeded,
        completed_sources: root_counts
            .into_iter()
            .filter(|(_, total, done, _)| *total > 0 && total == done)
            .map(|(source, _, _, target)| (source, target))
            .collect(),
        #[cfg(test)]
        history: Some(history.directory.clone()),
        undo_warning: history.finish(irreversible),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static ID: AtomicU64 = AtomicU64::new(1);
            let base = std::env::current_dir()
                .unwrap()
                .join("target/test-transfer-fixtures");
            fs::create_dir_all(&base).unwrap();
            let p = base.join(format!(
                "fileman-transfer-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn run_job(request: Request) -> Report {
        let mut report = None;
        execute(request, &AtomicBool::new(false), |event| {
            if let Event::Finished(r) = event {
                report = Some(r)
            }
        });
        report.unwrap()
    }
    #[test]
    fn copies_nested_and_empty_files_and_preserves_sources() {
        let f = Fixture::new();
        let src = f.0.join("source");
        let dst = f.0.join("dest");
        fs::create_dir_all(src.join("nested/empty")).unwrap();
        fs::create_dir(&dst).unwrap();
        fs::write(src.join("nested/data"), vec![7; 2_100_001]).unwrap();
        fs::write(src.join("zero"), []).unwrap();
        let r = run_job(Request {
            kind: Kind::Copy,
            sources: vec![src.clone()],
            destination: Some(dst.clone()),
        });
        assert_eq!(r.failure_count, 0, "{:?}", r.failures);
        assert_eq!(
            fs::read(dst.join("source/nested/data")).unwrap(),
            vec![7; 2_100_001]
        );
        assert!(dst.join("source/nested/empty").is_dir());
        assert!(src.exists());
    }
    #[test]
    fn completed_items_include_only_selected_roots_and_all_individual_files() {
        let f = Fixture::new();
        let src = f.0.join("source");
        let dst = f.0.join("dest");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::create_dir(&dst).unwrap();
        fs::write(src.join("nested/child"), b"child").unwrap();
        let r = run_job(Request {
            kind: Kind::Copy,
            sources: vec![src.clone()],
            destination: Some(dst.clone()),
        });
        assert_eq!(r.completed_sources, vec![(src, Some(dst.join("source")))]);
        let files = (0..10)
            .map(|i| {
                let p = f.0.join(format!("file{i}"));
                fs::write(&p, b"data").unwrap();
                p
            })
            .collect::<Vec<_>>();
        let r = run_job(Request {
            kind: Kind::Copy,
            sources: files.clone(),
            destination: Some(dst.clone()),
        });
        assert_eq!(r.completed_sources.len(), 10);
        for path in files {
            assert!(r.completed_sources.iter().any(|(p, _)| p == &path));
        }
    }
    #[test]
    fn successful_roots_survive_cancellation_and_keep_both_records_actual_target() {
        let f = Fixture::new();
        let dst = f.0.join("dest");
        fs::create_dir(&dst).unwrap();
        let a = f.0.join("a");
        let b = f.0.join("b");
        fs::write(&a, b"first").unwrap();
        fs::write(&b, b"second").unwrap();
        fs::write(dst.join("b"), b"existing").unwrap();
        let cancel = AtomicBool::new(false);
        let mut report = None;
        execute_with_conflicts(
            Request {
                kind: Kind::Copy,
                sources: vec![a.clone(), b.clone()],
                destination: Some(dst.clone()),
            },
            &cancel,
            |e| {
                if let Event::Finished(r) = e {
                    report = Some(r)
                }
            },
            |_, _| {
                cancel.store(true, Ordering::Relaxed);
                ConflictChoice::Skip
            },
        );
        let report = report.unwrap();
        assert!(report.cancelled);
        assert!(
            report.completed_sources.is_empty(),
            "preflight cancellation must occur before any files are copied"
        );
        let mut report = None;
        execute_with_conflicts(
            Request {
                kind: Kind::Copy,
                sources: vec![b.clone()],
                destination: Some(dst.clone()),
            },
            &AtomicBool::new(false),
            |e| {
                if let Event::Finished(r) = e {
                    report = Some(r)
                }
            },
            |_, _| ConflictChoice::KeepBoth,
        );
        let report = report.unwrap();
        assert_eq!(report.completed_sources.len(), 1);
        let actual = report.completed_sources[0].1.as_ref().unwrap();
        assert!(!crate::path_utils::same(actual, &dst.join("b")));
        assert_eq!(fs::read(actual).unwrap(), b"second");
    }
    #[test]
    fn incomplete_folder_is_not_reported_as_successful() {
        let f = Fixture::new();
        let src = f.0.join("source");
        let dst = f.0.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(dst.join("source")).unwrap();
        fs::write(src.join("skipped"), b"new").unwrap();
        fs::write(dst.join("source/skipped"), b"existing").unwrap();
        fs::write(src.join("copied"), b"ok").unwrap();
        let r = run_job(Request {
            kind: Kind::Copy,
            sources: vec![src],
            destination: Some(dst),
        });
        assert!(r.succeeded > 0);
        assert!(!r.issues.is_empty());
        assert!(r.completed_sources.is_empty());
    }
    #[test]
    fn move_removes_only_successful_sources_and_continues_after_conflict() {
        let f = Fixture::new();
        let dst = f.0.join("dest");
        fs::create_dir(&dst).unwrap();
        let a = f.0.join("a");
        let b = f.0.join("b");
        fs::write(&a, b"original").unwrap();
        fs::write(&b, b"move").unwrap();
        fs::write(dst.join("a"), b"existing").unwrap();
        let r = run_job(Request {
            kind: Kind::Move,
            sources: vec![a.clone(), b.clone()],
            destination: Some(dst.clone()),
        });
        assert_eq!(r.failure_count, 0, "{:?}", r.failures);
        assert!(a.exists());
        assert!(!b.exists());
        assert_eq!(r.completed_sources.len(), 1);
        assert!(crate::path_utils::same(&r.completed_sources[0].0, &b));
        assert!(crate::path_utils::same(
            r.completed_sources[0].1.as_ref().unwrap(),
            &dst.join("b")
        ));
        assert_eq!(fs::read(dst.join("a")).unwrap(), b"existing");
        assert_eq!(fs::read(dst.join("b")).unwrap(), b"move");
    }
    #[test]
    fn refuses_descendants_and_same_location_without_mutation() {
        let f = Fixture::new();
        let src = f.0.join("source");
        fs::create_dir_all(src.join("child")).unwrap();
        fs::write(src.join("file"), b"keep").unwrap();
        for dest in [f.0.clone(), src.join("child")] {
            let r = run_job(Request {
                kind: Kind::Move,
                sources: vec![src.clone()],
                destination: Some(dest),
            });
            assert!(r.failure_count > 0);
            assert_eq!(fs::read(src.join("file")).unwrap(), b"keep");
        }
    }
    #[test]
    fn cancellation_does_not_commit_or_remove_source() {
        let f = Fixture::new();
        let source = f.0.join("source");
        let target = f.0.join("target");
        fs::write(&source, vec![1; 3_000_000]).unwrap();
        let cancel = AtomicBool::new(false);
        let result = copy_file(&source, &target, true, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed)
        });
        assert!(result.is_err());
        assert!(source.exists());
        assert!(!target.exists());
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    }
    #[test]
    fn delete_removes_only_requested_tree() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("remove/nested")).unwrap();
        fs::write(f.0.join("remove/nested/file"), b"delete").unwrap();
        fs::write(f.0.join("keep"), b"keep").unwrap();
        let r = run_job(Request {
            kind: Kind::Delete,
            sources: vec![f.0.join("remove")],
            destination: None,
        });
        assert_eq!(r.failure_count, 0, "{:?}", r.failures);
        assert!(!f.0.join("remove").exists());
        assert!(f.0.join("keep").exists());
    }
    #[test]
    fn retries_exactly_four_times_before_giving_up() {
        let mut attempts = 0;
        let error = retry(
            &AtomicBool::new(false),
            |_| {
                attempts += 1;
                Err::<(), _>("fail".into())
            },
            |_, _| {},
        );
        assert!(error.is_err());
        assert_eq!(attempts, 4);
    }
    #[test]
    fn all_failed_files_are_retained_with_retry_destinations() {
        let f = Fixture::new();
        let destination = f.0.join("destination");
        fs::create_dir(&destination).unwrap();
        let sources: Vec<_> = (0..110).map(|i| f.0.join(format!("missing-{i}"))).collect();
        let report = run_job(Request {
            kind: Kind::Copy,
            sources: sources.clone(),
            destination: Some(destination.clone()),
        });
        assert_eq!(report.issues.len(), 110);
        assert_eq!(report.failures.len(), 110);
        for source in sources {
            let issue = report
                .issues
                .iter()
                .find(|issue| issue.source == source)
                .unwrap();
            let retry = issue.retry_request(Kind::Copy).unwrap();
            assert_eq!(retry.sources, vec![source]);
            assert_eq!(retry.destination, Some(destination.clone()));
            assert!(!issue.skipped);
            assert!(issue.retry_request(Kind::Delete).is_none());
        }
    }
    #[test]
    fn retry_all_groups_destinations_and_includes_skipped_files() {
        let mut job = Job::new(
            1,
            Request {
                kind: Kind::Copy,
                sources: vec!["source".into()],
                destination: Some("dest".into()),
            },
        );
        job.report = Some(Report {
            cancelled: false,
            failures: vec![],
            failure_count: 3,
            succeeded: 1,
            completed_sources: vec![],
            undo_warning: None,
            history: None,
            issues: [
                ("source/a", "dest/source/a", false),
                ("source/b", "dest/source/b", false),
                ("source/nested/c", "dest/source/nested/c", false),
                ("source/skip", "dest/source/skip", true),
            ]
            .into_iter()
            .map(|(source, target, skipped)| Issue {
                source: source.into(),
                target: Some(target.into()),
                reason: "test".into(),
                skipped,
            })
            .collect(),
        });
        let requests = job.failed_retry_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].sources,
            vec![
                PathBuf::from("source/a"),
                PathBuf::from("source/b"),
                PathBuf::from("source/skip")
            ]
        );
        assert_eq!(requests[0].destination, Some(PathBuf::from("dest/source")));
        assert_eq!(requests[1].sources, vec![PathBuf::from("source/nested/c")]);
        assert_eq!(
            requests[1].destination,
            Some(PathBuf::from("dest/source/nested"))
        );
        job.request.kind = Kind::Delete;
        assert!(job.failed_retry_requests().is_empty());
    }
    #[test]
    fn skipped_nested_file_retries_to_its_original_folder() {
        let f = Fixture::new();
        let source = f.0.join("source");
        let destination = f.0.join("destination");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir_all(destination.join("source/nested")).unwrap();
        fs::write(source.join("nested/file"), b"new").unwrap();
        fs::write(destination.join("source/nested/file"), b"old").unwrap();
        let report = run_job(Request {
            kind: Kind::Copy,
            sources: vec![source.clone()],
            destination: Some(destination.clone()),
        });
        assert_eq!(report.failure_count, 0);
        assert_eq!(report.issues.len(), 1);
        let issue = &report.issues[0];
        assert!(issue.skipped);
        assert_eq!(issue.source, source.join("nested/file"));
        let retry = issue.retry_request(Kind::Copy).unwrap();
        assert_eq!(
            retry
                .destination
                .as_ref()
                .map(|path| fs::canonicalize(path).unwrap()),
            Some(fs::canonicalize(destination.join("source/nested")).unwrap())
        );
        fs::remove_file(destination.join("source/nested/file")).unwrap();
        let retried = run_job(retry);
        assert!(retried.issues.is_empty());
        assert_eq!(
            fs::read(destination.join("source/nested/file")).unwrap(),
            b"new"
        );
    }
}

fn unique_target(target: &Path) -> PathBuf {
    let stem = target.file_stem().unwrap_or_default().to_string_lossy();
    let ext = target
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 1u64.. {
        let suffix = if n == 1 {
            String::new()
        } else {
            format!(" ({n})")
        };
        let candidate = target.with_file_name(format!("{stem} - Copy{suffix}{ext}"));
        if fs::symlink_metadata(&candidate).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            return candidate;
        }
    }
    unreachable!()
}
fn old_target(target: &Path) -> PathBuf {
    for n in 0u64.. {
        let suffix = if n == 0 {
            ".old".into()
        } else {
            format!(".old{n}")
        };
        let mut name = target.as_os_str().to_os_string();
        name.push(suffix);
        let candidate = PathBuf::from(name);
        if fs::symlink_metadata(&candidate).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            return candidate;
        }
    }
    unreachable!()
}
fn recycle(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{Storage::FileSystem::GetDriveTypeW, UI::Shell::*};
        let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
        let display = absolute.to_string_lossy().replace('/', "\\");
        let plain = display.strip_prefix("\\\\?\\").unwrap_or(&display);
        if plain.starts_with("\\\\") || plain.len() < 3 {
            return Err("Recycle Bin unavailable for this network path; use Shift+Delete for permanent deletion".into());
        }
        let root: Vec<u16> = plain[..3].encode_utf16().chain(Some(0)).collect();
        if unsafe { GetDriveTypeW(root.as_ptr()) } != 3 {
            return Err(
                "Recycle Bin unavailable on this drive; use Shift+Delete for permanent deletion"
                    .into(),
            );
        }
        let names: Vec<u16> = std::ffi::OsStr::new(plain)
            .encode_wide()
            .chain([0, 0])
            .collect();
        let mut operation: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
        operation.wFunc = FO_DELETE;
        operation.pFrom = names.as_ptr();
        operation.fFlags =
            (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING)
                as u16;
        let result = unsafe { SHFileOperationW(&mut operation) };
        if result != 0 || operation.fAnyOperationsAborted != 0 {
            Err(format!("Recycle operation cancelled or failed ({result})"))
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Recycle Bin is available only on Windows".into())
    }
}

#[cfg(test)]
mod conflict_tests {
    use super::*;
    #[test]
    fn preflight_backup_numbering_and_undo_preserve_existing_files() {
        let base = std::env::current_dir()
            .unwrap()
            .join(format!("target/old-backup-{}", crate::next_identity()));
        fs::create_dir_all(base.join("dest")).unwrap();
        let source = base.join("report.txt");
        let target = base.join("dest/report.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"existing").unwrap();
        fs::write(target.with_file_name("report.txt.old"), b"older").unwrap();
        fs::write(target.with_file_name("report - Copy.txt"), b"copy").unwrap();
        assert_eq!(
            unique_target(&target),
            target.with_file_name("report - Copy (2).txt")
        );
        let mut report = None;
        execute_with_conflicts(
            Request {
                kind: Kind::Copy,
                sources: vec![source.clone()],
                destination: Some(base.join("dest")),
            },
            &AtomicBool::new(false),
            |event| {
                if let Event::Finished(result) = event {
                    report = Some(result);
                }
            },
            |_, _| {
                assert_eq!(
                    fs::read(&target).unwrap(),
                    b"existing",
                    "preflight must not modify the target"
                );
                ConflictChoice::RenameExisting
            },
        );
        let report = report.unwrap();
        assert_eq!(report.failure_count, 0, "{:?}", report.failures);
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(
            fs::read(target.with_file_name("report.txt.old1")).unwrap(),
            b"existing"
        );
        assert_eq!(
            fs::read(target.with_file_name("report.txt.old")).unwrap(),
            b"older"
        );
        let (_, entry) = crate::undo::load(&report.history.unwrap()).unwrap();
        let (_, result) = crate::undo::run(entry.unwrap());
        result.unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        assert!(!target.with_file_name("report.txt.old1").exists());
    }
    #[test]
    fn conflict_choices_preserve_or_replace_and_keep_extension() {
        let base = std::env::current_dir()
            .unwrap()
            .join(format!("target/conflict-tests-{}", std::process::id()));
        fs::create_dir_all(base.join("dest")).unwrap();
        let source = base.join("report.txt");
        let target = base.join("dest/report.txt");
        for choice in [
            ConflictChoice::Skip,
            ConflictChoice::KeepBoth,
            ConflictChoice::Replace,
        ] {
            fs::write(&source, b"new").unwrap();
            fs::write(&target, b"old").unwrap();
            let mut called = 0;
            execute_with_conflicts(
                Request {
                    kind: Kind::Copy,
                    sources: vec![source.clone()],
                    destination: Some(base.join("dest")),
                },
                &AtomicBool::new(false),
                |_| {},
                |s, t| {
                    assert_eq!(s, source);
                    assert_eq!(
                        fs::canonicalize(t).unwrap(),
                        fs::canonicalize(&target).unwrap()
                    );
                    called += 1;
                    choice
                },
            );
            assert_eq!(called, 1);
            assert_eq!(fs::read(&source).unwrap(), b"new");
            assert_eq!(
                fs::read(&target).unwrap(),
                if choice == ConflictChoice::Replace {
                    b"new"
                } else {
                    b"old"
                }
            );
            if choice == ConflictChoice::KeepBoth {
                assert_eq!(
                    fs::read(base.join("dest/report - Copy.txt")).unwrap(),
                    b"new"
                );
            }
        }
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn empty_directory_same_location_move_never_removes_source() {
        let base = std::env::current_dir()
            .unwrap()
            .join(format!("target/same-directory-{}", std::process::id()));
        fs::create_dir_all(base.join("empty")).unwrap();
        execute(
            Request {
                kind: Kind::Move,
                sources: vec![base.join("empty")],
                destination: Some(base.clone()),
            },
            &AtomicBool::new(false),
            |_| {},
        );
        assert!(base.join("empty").is_dir());
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod rename_buffer_tests {
    use super::*;
    #[test]
    fn commits_exact_unicode_names_across_buffer_alignments() {
        let base = std::env::current_dir()
            .unwrap()
            .join(format!("target/rename-buffer-tests-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let source = base.join("source");
        fs::write(&source, b"exact name").unwrap();
        for length in 1..=16 {
            let target = base.join(format!("{}é.txt", "x".repeat(length)));
            copy_file(&source, &target, false, &AtomicBool::new(false), |_| {}).unwrap();
            assert_eq!(fs::read(&target).unwrap(), b"exact name");
        }
        assert_eq!(fs::read_dir(&base).unwrap().count(), 17);
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod undo_integration_tests {
    use super::*;
    #[test]
    fn completed_copy_and_move_with_replacement_reverse_entire_directory_tree() {
        for kind in [Kind::Copy, Kind::Move] {
            let base = std::env::current_dir().unwrap().join(format!(
                "target/history-integration-{}-{:?}",
                std::process::id(),
                kind
            ));
            fs::create_dir_all(base.join("source/nested/empty")).unwrap();
            fs::create_dir_all(base.join("destination/source/nested")).unwrap();
            let source = base.join("source");
            let dest = base.join("destination");
            fs::write(source.join("nested/file"), b"new").unwrap();
            fs::write(dest.join("source/nested/file"), b"old").unwrap();
            let mut report = None;
            execute_with_conflicts(
                Request {
                    kind,
                    sources: vec![source.clone()],
                    destination: Some(dest.clone()),
                },
                &AtomicBool::new(false),
                |event| {
                    if let Event::Finished(r) = event {
                        report = Some(r)
                    }
                },
                |_, _| ConflictChoice::Replace,
            );
            let report = report.unwrap();
            assert_eq!(report.failure_count, 0, "{:?}", report.failures);
            assert!(report.undo_warning.is_none(), "{:?}", report.undo_warning);
            let history = report.history.unwrap();
            let entry = crate::undo::load(&history).unwrap().1.unwrap();
            let result = crate::undo::run(entry).1;
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(fs::read(source.join("nested/file")).unwrap(), b"new");
            assert!(source.join("nested/empty").is_dir());
            assert_eq!(fs::read(dest.join("source/nested/file")).unwrap(), b"old");
            assert!(!dest.join("source/nested/empty").exists());
            assert!(crate::undo::load(&history).unwrap().1.is_none());
            fs::remove_dir_all(base).unwrap();
            fs::remove_dir_all(history).unwrap();
        }
    }
    #[test]
    #[ignore = "Opt-in native Recycle Bin roundtrip on an owned fixture"]
    fn native_recycle_roundtrip() {
        let base = std::env::current_dir().unwrap().join(format!(
            "target/native-recycle-roundtrip-{}",
            std::process::id()
        ));
        fs::create_dir_all(&base).unwrap();
        let source = base.join("owned-fixture.txt");
        fs::write(&source, b"restore this fixture").unwrap();
        let mut report = None;
        execute(
            Request {
                kind: Kind::Recycle,
                sources: vec![source.clone()],
                destination: None,
            },
            &AtomicBool::new(false),
            |e| {
                if let Event::Finished(r) = e {
                    report = Some(r)
                }
            },
        );
        let report = report.unwrap();
        assert_eq!(report.failure_count, 0, "{:?}", report.failures);
        assert!(report.undo_warning.is_none(), "{:?}", report.undo_warning);
        assert!(!source.exists());
        let history = report.history.unwrap();
        let result = crate::undo::run(crate::undo::load(&history).unwrap().1.unwrap()).1;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(fs::read(source).unwrap(), b"restore this fixture");
        fs::remove_dir_all(base).unwrap();
        fs::remove_dir_all(history).unwrap();
    }
}

pub(crate) fn remove_verified(path: &Path, expected: &crate::undo::Stamp) -> Result<(), String> {
    let mut file = source_file(path, true)?;
    if crate::undo::stamp_file(&mut file)? != *expected {
        return Err(format!(
            "{} changed since the operation; Undo left it untouched",
            path.display()
        ));
    }
    remove_copied_source(&file, path)
}
