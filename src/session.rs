#![cfg_attr(test, allow(dead_code))]
//! Small, versioned workspace snapshots. No directory entries or scan tasks are serialized.
use crate::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    #[serde(default)]
    shadow: Option<shadow_copies::ShadowCopy>,
    path: PathBuf,
    search: String,
    sort: Sort,
    descending: bool,
    #[serde(default)]
    filters: advanced_search::Options,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedPanel {
    tabs: Vec<SavedTab>,
    selected: usize,
    columns: [f32; 4],
    #[serde(default = "default_extension_width")]
    extension_width: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    version: u32,
    root: container_layout::Container,
    panels: Vec<SavedPanel>,
    active: usize,
}
fn default_extension_width() -> f32 {
    84.0
}
impl Session {
    pub fn capture(app: &App) -> Self {
        Self {
            version: 1,
            root: app.files_layout.saved_root(),
            active: app.active,
            panels: app.panes[..app.pane_count]
                .iter()
                .map(|p| SavedPanel {
                    selected: p.selected_tab,
                    columns: p.columns,
                    extension_width: p.extension_width,
                    tabs: p
                        .tabs
                        .iter()
                        .map(|t| SavedTab {
                            shadow: t.shadow.clone(),
                            path: t.path.clone(),
                            search: t.search.clone(),
                            sort: t.sort,
                            descending: t.descending,
                            filters: t.filters.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
    pub fn restore(self, app: &mut App) -> Result<(), String> {
        let count = self.panels.len();
        if self.version != 1
            || !(1..=4).contains(&count)
            || self.active >= count
            || self.panels.iter().any(|p| {
                p.tabs.is_empty()
                    || p.tabs.len() > 128
                    || p.selected >= p.tabs.len()
                    || p.tabs
                        .iter()
                        .any(|t| t.path.as_os_str().is_empty() || t.filters.compile().is_err())
                    || !p.extension_width.is_finite()
                    || !(30.0..=400.0).contains(&p.extension_width)
                    || p.columns
                        .iter()
                        .any(|w| !w.is_finite() || *w < 30. || *w > 400.)
            })
        {
            return Err("Invalid saved workspace".into());
        }
        let layout = container_layout::Layout::restore(self.root, count)
            .ok_or("Invalid saved panel arrangement")?;
        for (id, p) in self.panels.into_iter().enumerate() {
            let mut panel = Panel::new(p.tabs[0].path.clone());
            panel.tabs.clear();
            for saved in p.tabs {
                let mut tab = Tab::new(saved.path);
                tab.shadow = saved.shadow.filter(|copy| copy.valid());
                tab.search = saved.search;
                tab.sort = saved.sort;
                tab.descending = saved.descending;
                tab.filters = saved.filters;
                panel.tabs.push(tab);
            }
            panel.selected_tab = p.selected;
            panel.columns = p.columns;
            panel.extension_width = p.extension_width;
            app.panes[id] = panel;
        }
        app.files_layout = layout;
        app.pane_count = count;
        app.active = self.active;
        Ok(())
    }
}
fn path() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .with_file_name("file-beholder-session.json")
}
pub fn load() -> Result<Option<Session>, String> {
    let path = path();
    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if meta.len() > 1024 * 1024 {
        return Err("Saved workspace exceeds size limit".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| e.to_string())
}
static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static LAST: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
fn write(session: Session, sequence: u64) -> Result<(), String> {
    let mut last = LAST.lock().map_err(|e| e.to_string())?;
    if sequence <= *last {
        return Ok(());
    }
    write_file(&path(), &session)?;
    *last = sequence;
    Ok(())
}
fn write_file(path: &std::path::Path, session: &Session) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(session).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Saved workspace exceeds size limit".into());
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .and_then(|_| std::fs::rename(temporary, path))
        .map_err(|e| e.to_string())
}
struct Request {
    session: Session,
    sequence: u64,
    done: iced::futures::channel::oneshot::Sender<Result<(), String>>,
}
pub fn save(session: Session) -> Task<Message> {
    static WRITER: std::sync::OnceLock<std::sync::mpsc::Sender<Request>> =
        std::sync::OnceLock::new();
    let writer = WRITER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Request>();
        std::thread::spawn(move || {
            while let Ok(mut request) = rx.recv() {
                while let Ok(newer) = rx.recv_timeout(std::time::Duration::from_millis(200)) {
                    let _ = request.done.send(Ok(()));
                    request = newer;
                }
                let result = write(request.session, request.sequence);
                let _ = request.done.send(result);
            }
        });
        tx
    });
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    let request = Request {
        session,
        sequence: SEQUENCE.fetch_add(1, Ordering::Relaxed),
        done: tx,
    };
    if writer.send(request).is_err() {
        return Task::done(Message::SessionSaved(
            Err("Workspace writer stopped".into()),
        ));
    }
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("Workspace save was interrupted".into()))
        },
        Message::SessionSaved,
    )
}
pub fn save_now(session: Session) -> Result<(), String> {
    write(session, SEQUENCE.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_file_is_atomically_replaced_and_can_be_read_back() {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("session-store-test-{}", next_identity()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("session.json");
        let mut app = App::new("home".into(), "other".into(), vec![]);
        write_file(&file, &Session::capture(&app)).unwrap();
        app.panes[1].tabs.push(Tab::new("second".into()));
        app.panes[1].selected_tab = 1;
        app.active = 1;
        write_file(&file, &Session::capture(&app)).unwrap();
        let saved: Session = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(saved).unwrap(),
            serde_json::to_value(Session::capture(&app)).unwrap()
        );
        assert!(!file.with_extension("json.tmp").exists());
        assert!(root.starts_with(std::env::current_dir().unwrap().join("target")));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn json_roundtrip_restores_arrangement_shares_columns_tabs_and_filters() {
        let mut source = App::new("home".into(), "other".into(), vec![]);
        source.preferences.default_layout = preferences::DefaultLayout::Grid;
        source.initialize_default_layout();
        source.files_layout.set_size(iced::Size::new(1500., 900.));
        let split = *source.files_layout.layout().splits().next().unwrap();
        source.files_layout.resize(split, 0.63);
        source.panes[2]
            .tabs
            .push(Tab::new(PathBuf::from(r"\server\share\資料")));
        source.panes[2].selected_tab = 1;
        source.panes[2].search = "*.rs;*.toml".into();
        source.panes[2].filters.recursive = true;
        source.panes[2].filters.minimum = "2 KiB".into();
        source.panes[2].sort = Sort::Modified;
        source.panes[2].descending = true;
        source.panes[2].columns = [50., 150., 90., 200.];
        source.panes[2].extension_width = 113.;
        source.active = 2;
        let bytes = serde_json::to_vec(&Session::capture(&source)).unwrap();
        let saved: Session = serde_json::from_slice(&bytes).unwrap();
        let mut target = App::new("fallback".into(), "fallback".into(), vec![]);
        saved.restore(&mut target).unwrap();
        assert_eq!(
            serde_json::to_value(Session::capture(&source)).unwrap(),
            serde_json::to_value(Session::capture(&target)).unwrap()
        );
        target.files_layout.set_size(iced::Size::new(1500., 900.));
        let rects = |app: &App| {
            let r = app
                .files_layout
                .layout()
                .pane_regions(6., 180., iced::Size::new(1500., 900.));
            app.files_layout
                .iter()
                .map(|(p, id)| (*id, r[p]))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(rects(&source), rects(&target));
        assert_eq!(target.pane_count, 4);
        assert_eq!(target.active, 2);
        assert!(
            target.panes[..4]
                .iter()
                .flat_map(|p| &p.tabs)
                .all(|t| t.generation == 0 && !t.busy)
        );
    }
    #[test]
    fn malformed_sessions_leave_existing_workspace_untouched() {
        for mutation in 0..5 {
            let mut app = App::new("home".into(), "other".into(), vec![]);
            let before = serde_json::to_value(Session::capture(&app)).unwrap();
            let mut saved = Session::capture(&app);
            match mutation {
                0 => saved.version = 99,
                1 => saved.panels[0].tabs.clear(),
                2 => saved.root = container_layout::Container::Panel(3),
                3 => saved.panels[0].columns[0] = f32::NAN,
                _ => saved.panels[0].tabs[0].filters.after = "2026-02-30".into(),
            }
            assert!(saved.restore(&mut app).is_err());
            assert_eq!(
                before,
                serde_json::to_value(Session::capture(&app)).unwrap()
            );
        }
    }
    #[test]
    fn session_save_errors_are_reported_without_changing_layout() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let root = app.files_layout.saved_root();
        let _ = app.update(Message::SessionSaved(Err("read only".into())));
        assert!(app.notice.contains("read only"));
        assert_eq!(app.files_layout.saved_root(), root);
    }
}
