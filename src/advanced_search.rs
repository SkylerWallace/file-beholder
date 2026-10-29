//! Validated filters and cancellable, batched recursive searches.
mod view;
use crate::typography::checkbox;
use crate::*;
pub use view::layer;
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(default)]
pub struct Options {
    pub name: String,
    pub extensions: String,
    pub minimum: String,
    pub maximum: String,
    pub after: String,
    pub before: String,
    pub kind: u8,
    pub case_sensitive: bool,
    pub recursive: bool,
}
pub struct Editor {
    pub tab: u64,
    pub draft: Options,
    pub error: Option<String>,
}
#[derive(Debug, Clone)]
pub enum Change {
    Name(String),
    Extensions(String),
    Minimum(String),
    Maximum(String),
    After(String),
    Before(String),
    Kind(u8),
    Case(bool),
    Recursive(bool),
}
impl Options {
    pub fn edit(&mut self, change: Change) {
        match change {
            Change::Name(s) => self.name = s,
            Change::Extensions(s) => self.extensions = s,
            Change::Minimum(s) => self.minimum = s,
            Change::Maximum(s) => self.maximum = s,
            Change::After(s) => self.after = s,
            Change::Before(s) => self.before = s,
            Change::Kind(k) => self.kind = k,
            Change::Case(v) => self.case_sensitive = v,
            Change::Recursive(v) => self.recursive = v,
        }
    }
    pub fn compile(&self) -> Result<Filter, String> {
        let minimum = parse_size(&self.minimum)?;
        let maximum = parse_size(&self.maximum)?;
        if minimum.zip(maximum).is_some_and(|(a, b)| a > b) {
            return Err("Minimum size must not exceed maximum size.".into());
        }
        for date in [&self.after, &self.before] {
            if !date.is_empty() && !valid_date(date) {
                return Err("Dates must be valid YYYY-MM-DD dates.".into());
            }
        }
        if !self.after.is_empty() && !self.before.is_empty() && self.after > self.before {
            return Err("Start date must not follow end date.".into());
        }
        if self.kind > 2 {
            return Err("Invalid item type.".into());
        }
        let unquoted_name = self.name.replace('"', "");
        let patterns = unquoted_name
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                if self.case_sensitive {
                    s.trim().to_string()
                } else {
                    s.trim().to_lowercase()
                }
            })
            .collect();
        let extensions = self
            .extensions
            .split([',', ';', ' '])
            .map(|s| {
                s.trim()
                    .trim_start_matches("*.")
                    .trim_start_matches('.')
                    .to_lowercase()
            })
            .filter(|s| !s.is_empty())
            .collect();
        Ok(Filter {
            options: self.clone(),
            minimum,
            maximum,
            patterns,
            extensions,
        })
    }
    pub fn has_size(&self) -> bool {
        !self.minimum.is_empty() || !self.maximum.is_empty()
    }
    pub fn active(&self) -> bool {
        self != &Self::default()
    }
}
pub struct Filter {
    options: Options,
    minimum: Option<u64>,
    maximum: Option<u64>,
    patterns: Vec<String>,
    extensions: Vec<String>,
}
impl Filter {
    pub fn matches(&self, e: &Entry) -> bool {
        if (self.options.kind == 1 && e.directory) || (self.options.kind == 2 && !e.directory) {
            return false;
        }
        let name = if self.options.case_sensitive {
            e.name.clone()
        } else {
            e.name.to_lowercase()
        };
        if !self.patterns.is_empty()
            && !self.patterns.iter().any(|p| {
                if p.contains(['*', '?']) {
                    glob(p, &name)
                } else {
                    name.contains(p)
                }
            })
        {
            return false;
        }
        if !self.extensions.is_empty()
            && (e.directory
                || !e.path.extension().is_some_and(|s| {
                    self.extensions
                        .iter()
                        .any(|ext| *ext == s.to_string_lossy().to_lowercase())
                }))
        {
            return false;
        }
        if self
            .minimum
            .is_some_and(|n| e.bytes.is_none_or(|bytes| bytes < n))
            || self
                .maximum
                .is_some_and(|n| e.bytes.is_none_or(|bytes| bytes > n))
        {
            return false;
        }
        if !self.options.after.is_empty() || !self.options.before.is_empty() {
            let Some(date) = e.modified_label.get(..10) else {
                return false;
            };
            if (!self.options.after.is_empty() && date < self.options.after.as_str())
                || (!self.options.before.is_empty() && date > self.options.before.as_str())
            {
                return false;
            }
        }
        true
    }
}
fn glob(pattern: &str, name: &str) -> bool {
    let p: Vec<_> = pattern.chars().collect();
    let n: Vec<_> = name.chars().collect();
    let (mut i, mut j, mut star, mut retry) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            i += 1;
            retry = j;
        } else if let Some(s) = star {
            retry += 1;
            j = retry;
            i = s + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}
fn parse_size(s: &str) -> Result<Option<u64>, String> {
    let text = s.trim().to_lowercase();
    if text.is_empty() {
        return Ok(None);
    }
    let split = text
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(text.len());
    let number = text[..split].parse::<f64>().map_err(
        |_| "Sizes must be numbers, optionally followed by B, KB, KiB, MB, MiB, GB or GiB.",
    )?;
    let unit = match text[split..].trim() {
        "" | "b" => 1.,
        "kb" => 1000.,
        "kib" => 1024.,
        "mb" => 1e6,
        "mib" => 1048576.,
        "gb" => 1e9,
        "gib" => 1073741824.,
        "tb" => 1e12,
        "tib" => 1099511627776.,
        _ => return Err("Unknown size unit.".into()),
    };
    let value = number * unit;
    if !value.is_finite() || value < 0. || value >= u64::MAX as f64 {
        return Err("Size is out of range.".into());
    }
    Ok(Some(value.ceil() as u64))
}
fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let y = s[..4].parse::<u32>().unwrap_or(0);
    let m = s[5..7].parse::<usize>().unwrap_or(0);
    let d = s[8..].parse::<u32>().unwrap_or(0);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    y > 0 && (1..=12).contains(&m) && d > 0 && d <= days[m - 1]
}
#[derive(Debug, Clone)]
pub enum SearchEvent {
    Batch(Vec<Entry>),
    Progress {
        visited: usize,
        skipped: usize,
    },
    Finished {
        visited: usize,
        skipped: usize,
        limited: bool,
    },
    Failed(String),
}
pub fn scan(
    root: PathBuf,
    options: Options,
    cancel: Arc<AtomicBool>,
    mode: crate::preferences::ScanMode,
) -> Task<SearchEvent> {
    Task::run(stream_with_mode(root, options, cancel, mode), |event| event)
}
#[cfg(test)]
fn stream(
    root: PathBuf,
    options: Options,
    cancel: Arc<AtomicBool>,
) -> iced::futures::channel::mpsc::UnboundedReceiver<SearchEvent> {
    stream_with_mode(root, options, cancel, crate::preferences::ScanMode::Auto)
}
fn stream_with_mode(
    root: PathBuf,
    options: Options,
    cancel: Arc<AtomicBool>,
    mode: crate::preferences::ScanMode,
) -> iced::futures::channel::mpsc::UnboundedReceiver<SearchEvent> {
    let (tx, rx) = iced::futures::channel::mpsc::unbounded();
    std::thread::spawn(move || {
        browser::background_priority();
        let filter = match options.compile() {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.unbounded_send(SearchEvent::Failed(e));
                return;
            }
        };
        let index = if options.has_size()
            && options.kind != 1
            && matches!(
                mode,
                crate::preferences::ScanMode::Auto | crate::preferences::ScanMode::Mft
            ) {
            crate::ntfs::get(&root, &cancel, false).ok()
        } else {
            None
        };
        let mut directory_sizes = std::collections::HashMap::new();
        let mut dirs = vec![root.clone()];
        let mut batch = Vec::new();
        let (mut visited, mut skipped, mut matches) = (0, 0, 0);
        let mut last = std::time::Instant::now();
        let mut last_progress = std::time::Instant::now();
        while let Some(dir) = dirs.pop() {
            if cancel.load(Ordering::Relaxed) || tx.is_closed() {
                return;
            }
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) => {
                    if dir == root {
                        let _ = tx.unbounded_send(SearchEvent::Failed(e.to_string()));
                        return;
                    }
                    skipped += 1;
                    continue;
                }
            };
            for item in entries {
                if cancel.load(Ordering::Relaxed) || tx.is_closed() {
                    return;
                }
                let item = match item {
                    Ok(item) => item,
                    Err(_) => {
                        skipped += 1;
                        continue;
                    }
                };
                let mut entry = browser::describe(item);
                visited += 1;
                if last_progress.elapsed().as_millis() >= 250 {
                    let _ = tx.unbounded_send(SearchEvent::Progress { visited, skipped });
                    last_progress = std::time::Instant::now();
                }
                if entry.directory {
                    dirs.push(entry.path.clone());
                    if options.has_size() {
                        let indexed = index
                            .as_ref()
                            .and_then(|i| i.folder(&entry.path))
                            .map(|s| (s.bytes, s.partial));
                        use crate::preferences::ScanMode;
                        let size = match mode {
                            ScanMode::Off => None,
                            ScanMode::Mft => indexed,
                            ScanMode::Auto if indexed.is_some() => indexed,
                            _ => {
                                if !directory_sizes.contains_key(&entry.path) {
                                    directory_sizes
                                        .extend(browser::size_tree(&entry.path, &cancel));
                                }
                                directory_sizes.get(&entry.path).copied()
                            }
                        };
                        if let Some((bytes, partial)) = size {
                            entry.bytes = Some(bytes);
                            entry.partial = partial;
                        }
                    }
                }
                entry.name = entry
                    .path
                    .strip_prefix(&root)
                    .unwrap_or(&entry.path)
                    .to_string_lossy()
                    .into_owned();
                if filter.matches(&entry) {
                    batch.push(entry);
                    matches += 1;
                }
                let batch_size = if matches < 1024 {
                    256
                } else if matches < 10_000 {
                    2048
                } else if matches < 50_000 {
                    8192
                } else {
                    32768
                };
                if batch.len() >= batch_size
                    || (!batch.is_empty() && last.elapsed().as_millis() > 300)
                {
                    if tx
                        .unbounded_send(SearchEvent::Batch(std::mem::take(&mut batch)))
                        .is_err()
                    {
                        return;
                    }
                    last = std::time::Instant::now();
                }
                if matches >= 200_000 {
                    if !batch.is_empty() {
                        let _ = tx.unbounded_send(SearchEvent::Batch(batch));
                    }
                    let _ = tx.unbounded_send(SearchEvent::Finished {
                        visited,
                        skipped,
                        limited: true,
                    });
                    return;
                }
            }
        }
        if !batch.is_empty() {
            let _ = tx.unbounded_send(SearchEvent::Batch(batch));
        }
        let _ = tx.unbounded_send(SearchEvent::Finished {
            visited,
            skipped,
            limited: false,
        });
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::StreamExt;
    fn entry(name: &str, directory: bool, bytes: Option<u64>, date: &str) -> Entry {
        Entry {
            hidden: false,
            path: PathBuf::from(name),
            name: name.into(),
            directory,
            bytes,
            partial: false,
            modified: None,
            modified_label: date.into(),
            icon: None,
        }
    }
    #[test]
    fn patterns_extensions_dates_size_type_and_case_are_combined() {
        let mut options = Options {
            name: "Report*;Résumé?.txt".into(),
            extensions: "txt, md".into(),
            minimum: "1 KiB".into(),
            maximum: "2 MB".into(),
            after: "2026-01-01".into(),
            before: "2026-10-23".into(),
            kind: 1,
            ..Default::default()
        };
        let f = options.compile().unwrap();
        assert!(f.matches(&entry("REPORT.TXT", false, Some(1500), "2026-10-23 23:59")));
        assert!(f.matches(&entry("Résuméé.txt", false, Some(1024), "2026-01-01 00:00")));
        for e in [
            entry("report.png", false, Some(1500), "2026-10-23"),
            entry("report.txt", true, Some(1500), "2026-10-23"),
            entry("report.txt", false, None, "2026-10-23"),
            entry("report.txt", false, Some(1023), "2026-10-23"),
            entry("report.txt", false, Some(1500), "2026-10-24"),
        ] {
            assert!(!f.matches(&e));
        }
        options.case_sensitive = true;
        assert!(!options.compile().unwrap().matches(&entry(
            "REPORT.TXT",
            false,
            Some(1500),
            "2026-10-23"
        )));
    }
    #[test]
    fn invalid_ranges_and_dates_are_rejected_without_applying() {
        for options in [
            Options {
                minimum: "-1".into(),
                ..Default::default()
            },
            Options {
                minimum: "2 GB".into(),
                maximum: "1 MB".into(),
                ..Default::default()
            },
            Options {
                after: "2026-02-29".into(),
                ..Default::default()
            },
            Options {
                before: "2026-13-01".into(),
                ..Default::default()
            },
        ] {
            assert!(options.compile().is_err());
        }
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2100-02-29"));
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let _ = app.update(Message::AdvancedToggle);
        let _ = app.update(Message::AdvancedChange(Change::Minimum("oops".into())));
        let _ = app.update(Message::AdvancedApply);
        assert!(app.advanced.as_ref().unwrap().error.is_some());
        assert_eq!(app.panes[0].filters, Options::default());
        let _ = app.update(Message::Confirm(false));
        assert!(app.advanced.is_none());
    }
    #[test]
    fn recursive_search_returns_nested_matches_and_obeys_pre_cancel() {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("advanced-search-test-{}", crate::next_identity()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("one.rs"), "abc").unwrap();
        std::fs::write(root.join("nested/two.RS"), "abcde").unwrap();
        std::fs::write(root.join("nested/no.txt"), "x").unwrap();
        let options = Options {
            name: "*.rs".into(),
            recursive: true,
            ..Default::default()
        };
        let events = iced::futures::executor::block_on(
            stream(
                root.clone(),
                options.clone(),
                Arc::new(AtomicBool::new(false)),
            )
            .collect::<Vec<_>>(),
        );
        let matches = events
            .iter()
            .filter_map(|e| {
                if let SearchEvent::Batch(b) = e {
                    Some(b.as_slice())
                } else {
                    None
                }
            })
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|e| e.path == root.join("nested/two.RS")));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SearchEvent::Finished { limited: false, .. }))
        );
        let canceled = iced::futures::executor::block_on(
            stream(root.clone(), options, Arc::new(AtomicBool::new(true))).collect::<Vec<_>>(),
        );
        assert!(canceled.is_empty());
        assert!(root.starts_with(std::env::current_dir().unwrap().join("target")));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn results_follow_moved_tabs_and_old_generations_cannot_reappear() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let identity = app.panes[0].identity;
        app.panes[0].generation = 900;
        app.panes[0].filters.recursive = true;
        let _ = app.update(Message::MoveTab(0, identity, 1, 1));
        let _ = app.update(Message::AdvancedResult(
            identity,
            900,
            SearchEvent::Batch(vec![entry("nested/one.rs", false, Some(4), "2026-10-23")]),
        ));
        assert_eq!(app.panes[0].entries.len(), 1);
        assert_eq!(app.panes[0].identity, identity);
        let _ = app.update(Message::CancelSearch);
        let _ = app.update(Message::AdvancedResult(
            identity,
            900,
            SearchEvent::Batch(vec![entry("stale", false, Some(4), "2026-10-23")]),
        ));
        assert_eq!(app.panes[0].entries.len(), 1);
        assert!(!app.panes[0].busy);
    }
}
