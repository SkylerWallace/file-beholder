#![windows_subsystem = "windows"]
mod activity;
mod adaptive_sidebar;
mod advanced_search;
mod browser;
mod button_feedback;
mod chrome;
mod columns;
mod context_popup;
mod desktop;
mod file_clipboard;
mod file_properties;
mod header_drag;
mod highlight;
mod horizontal_rail;
mod interactions;
mod jump_navigation;
mod motion;
mod motion_strip;
mod native_properties;
mod operations_ui;
mod pane_view;
mod path_input;
mod path_utils;
mod preferences;
mod rename;
mod responsive_cache;
mod ribbon;
mod session;
mod settings_popup;
mod settings_ui;
mod shadow_copies;
mod sounds;
mod tab_drag;
mod transfer_resize;
mod transfers;
mod undo;
mod utility_dialogs;
#[cfg(test)]
mod utility_tests;
mod workspace;
mod workspace_view;
use operations_ui::{Action as FileAction, Clipboard as FileClipboard, Confirmation};
mod column_resize;
mod components;
mod container_layout;
mod docking;
mod file_icons;
mod licenses;
mod locations;
mod ntfs;
mod palette;
mod share_bar;
mod theme;
mod typography;
mod ui_icons;
mod virtual_list;
use browser::{Entry, Event};
use components::*;
use iced::widget::{Space, button, container, pane_grid, responsive, row};
use iced::{Element, Fill, Task};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use typography::{text, text_input};
use workspace::ribbon_divider;
fn main() -> iced::Result {
    preferences::request_admin_on_launch();
    iced::application(App::boot, App::update, App::view)
        .title("File Beholder")
        .theme(|app: &App| app.app_theme.clone())
        .window(iced::window::Settings {
            size: iced::Size::new(1400.0, 900.0),
            min_size: Some(iced::Size::new(480.0, 480.0)),
            icon: iced::window::icon::from_rgba(
                include_bytes!("../assets/app-icon.rgba").to_vec(),
                64,
                64,
            )
            .ok(),
            ..Default::default()
        })
        .subscription(|app| {
            iced::Subscription::batch([
                iced::event::listen_with(keyboard_message),
                iced::Subscription::run(preferences::theme_watch).map(Message::SystemTheme),
                iced::Subscription::run(chrome::events),
                if app.undo_busy {
                    iced::window::frames().map(|_| Message::UndoTick)
                } else {
                    iced::Subscription::none()
                },
            ])
        })
        .run()
}
fn keyboard_message(
    event: iced::Event,
    status: iced::event::Status,
    _: iced::window::Id,
) -> Option<Message> {
    use iced::keyboard::{Event, Key, key::Named};
    match event {
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Back)) => {
            Some(Message::MouseHistory(false))
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Forward)) => {
            Some(Message::MouseHistory(true))
        }
        iced::Event::Keyboard(Event::ModifiersChanged(modifiers)) => {
            Some(Message::Modifiers(modifiers))
        }
        iced::Event::Window(iced::window::Event::Resized(size)) => {
            Some(Message::SidebarWindowResized(size))
        }
        iced::Event::Window(iced::window::Event::Unfocused) => Some(Message::WindowUnfocused),
        iced::Event::Window(iced::window::Event::Focused) => Some(Message::MotionSettingsChanged),
        iced::Event::Keyboard(Event::KeyPressed { key, modifiers, .. }) => {
            match key.as_ref() {
                Key::Named(Named::Escape | Named::Enter)
                    if status == iced::event::Status::Ignored =>
                {
                    if modifiers.alt() && matches!(key, Key::Named(Named::Enter)) {
                        return Some(Message::PropertiesShortcut);
                    }
                    return sk_iced::components::DialogCommands::new()
                        .default_action(Message::Confirm(true))
                        .cancel_action(Message::Confirm(false))
                        .action(&key);
                }
                Key::Named(
                    Named::ArrowUp | Named::ArrowDown | Named::ArrowLeft | Named::ArrowRight,
                ) if status == iced::event::Status::Ignored
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo() =>
                {
                    return Some(Message::ArrowSelection(match key.as_ref() {
                        Key::Named(Named::ArrowUp) => -1,
                        Key::Named(Named::ArrowLeft) => -2,
                        Key::Named(Named::ArrowRight) => 2,
                        _ => 1,
                    }));
                }
                Key::Named(Named::Tab)
                    if status == iced::event::Status::Ignored
                        && !modifiers.control()
                        && !modifiers.alt()
                        && !modifiers.logo() =>
                {
                    return Some(Message::CyclePanel(modifiers.shift()));
                }
                Key::Named(Named::F2) if status == iced::event::Status::Ignored => {
                    return Some(Message::FileAction(FileAction::Rename));
                }
                Key::Named(Named::Delete) if status == iced::event::Status::Ignored => {
                    return Some(Message::FileAction(if modifiers.shift() {
                        FileAction::DeletePermanent
                    } else {
                        FileAction::Delete
                    }));
                }
                _ => {}
            }
            if modifiers.control() {
                match key.as_ref() {
                    Key::Character("n" | "N")
                        if modifiers.shift() && status == iced::event::Status::Ignored =>
                    {
                        return Some(Message::FileAction(FileAction::NewFolder));
                    }
                    Key::Character("n" | "N") if status == iced::event::Status::Ignored => {
                        return Some(Message::PanelShortcut);
                    }
                    Key::Character("c" | "C")
                        if modifiers.shift() && status == iced::event::Status::Ignored =>
                    {
                        return Some(Message::CopyPath);
                    }
                    Key::Character("f" | "F") if !modifiers.alt() && !modifiers.logo() => {
                        return Some(Message::FocusSearch);
                    }
                    Key::Character("w" | "W") if modifiers.shift() => {
                        return Some(Message::ClosePanelShortcut);
                    }
                    Key::Character("t") => return Some(Message::TabShortcut(0)),
                    Key::Character("w") => return Some(Message::TabShortcut(1)),
                    Key::Named(Named::Tab) => {
                        return Some(Message::TabShortcut(if modifiers.shift() { 3 } else { 2 }));
                    }
                    _ => {}
                }
            }
            if modifiers.control() && status == iced::event::Status::Ignored {
                match key.as_ref() {
                    Key::Character("a") => Some(Message::FileAction(FileAction::SelectAll)),
                    Key::Character("c") => Some(Message::FileAction(FileAction::Copy)),
                    Key::Character("x") => Some(Message::FileAction(FileAction::Cut)),
                    Key::Character("v") => Some(Message::FileAction(FileAction::Paste)),
                    Key::Character("z") => Some(Message::Undo),
                    _ => None,
                }
            } else if status == iced::event::Status::Ignored
                && !modifiers.alt()
                && !modifiers.logo()
            {
                if let Key::Character(value) = key.as_ref() {
                    let mut chars = value.chars();
                    if let Some(c) = chars.next().filter(|c| c.is_alphanumeric()) {
                        if chars.next().is_none() {
                            return Some(Message::JumpInitial(c));
                        }
                    }
                }
                None
            } else {
                None
            }
        }
        _ => None,
    }
}
struct Tab {
    shadow: Option<shadow_copies::ShadowCopy>,
    identity: u64,
    path: PathBuf,
    address: String,
    address_request: Option<u64>,
    search: String,
    filters: advanced_search::Options,
    entries: Vec<Entry>,
    selected: Option<usize>,
    pending_selection: Option<PathBuf>,
    reveal_selection: Option<(u64, usize)>,
    selections: std::collections::BTreeSet<usize>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    busy: bool,
    error: Option<String>,
    backend: String,
    total_bytes: u64,
    incomplete_entries: usize,
    order: Vec<usize>,
    sort: Sort,
    descending: bool,
    view_identity: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Sort {
    Property(usize),
    Extension,
    Name,
    Type,
    Size,
    Share,
    Modified,
}
impl Tab {
    fn parent(&self) -> Option<PathBuf> {
        let parent = self.path.parent()?;
        if self
            .shadow
            .as_ref()
            .is_some_and(|copy| copy.contains(&self.path) && !copy.contains(parent))
        {
            return None;
        }
        Some(parent.to_path_buf())
    }
    fn new(path: PathBuf) -> Self {
        Self {
            shadow: None,
            identity: next_identity(),
            address: path.display().to_string(),
            address_request: None,
            path,
            search: String::new(),
            filters: Default::default(),
            entries: vec![],
            selected: None,
            pending_selection: None,
            reveal_selection: None,
            selections: Default::default(),
            back: vec![],
            forward: vec![],
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            busy: false,
            error: None,
            backend: String::new(),
            total_bytes: 0,
            incomplete_entries: 0,
            order: vec![],
            sort: Sort::Type,
            descending: false,
            view_identity: 0,
        }
    }
    fn total(&self) -> u64 {
        self.total_bytes
    }
    fn start(&mut self, id: usize, mode: preferences::ScanMode) -> Task<Message> {
        self.start_refresh(id, false, mode)
    }
    fn start_refresh(
        &mut self,
        id: usize,
        refresh: bool,
        mode: preferences::ScanMode,
    ) -> Task<Message> {
        let mode = if self
            .shadow
            .as_ref()
            .is_some_and(|copy| copy.contains(&self.path))
            && mode != preferences::ScanMode::Off
        {
            preferences::ScanMode::Recursive
        } else {
            mode
        };
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation = next_identity();
        self.pending_selection = None;
        self.reveal_selection = None;
        self.entries.clear();
        self.total_bytes = 0;
        self.incomplete_entries = 0;
        self.order.clear();
        self.view_identity = next_identity();
        self.selected = None;
        self.selections.clear();
        self.error = None;
        self.busy = true;
        self.address = self.path.display().to_string();
        self.address_request = None;
        let generation = self.generation;
        if self.filters.recursive {
            let identity = self.identity;
            let mut options = self.filters.clone();
            options.name = self.search.clone();
            self.backend = "Searching subfolders…".into();
            return advanced_search::scan(self.path.clone(), options, self.cancel.clone(), mode)
                .map(move |event| Message::AdvancedResult(identity, generation, event));
        }
        browser::scan(self.path.clone(), self.cancel.clone(), refresh, mode)
            .map(move |event| Message::Scan(id, generation, event))
    }
    fn reorder(&mut self) {
        let mut options = self.filters.clone();
        options.name = self.search.clone();
        let filter = options.compile().ok();
        self.order = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| filter.as_ref().is_none_or(|f| f.matches(e)))
            .map(|(i, _)| i)
            .collect();
        // Cache case folding once per sort, rather than once per comparison.
        let names: Vec<_> = self.entries.iter().map(|e| e.name.to_lowercase()).collect();
        let extensions: Vec<_> = if self.sort == Sort::Extension {
            self.entries
                .iter()
                .map(|e| columns::extension(&e.path, e.directory).to_lowercase())
                .collect()
        } else {
            Vec::new()
        };
        let properties: Vec<_> = if let Sort::Property(index) = self.sort {
            self.entries
                .iter()
                .map(|e| {
                    file_properties::get(&e.path)
                        .get(index)
                        .cloned()
                        .unwrap_or_default()
                })
                .collect()
        } else {
            Vec::new()
        };
        self.order.sort_by(|&a, &b| {
            let left = &self.entries[a];
            let right = &self.entries[b];
            let comparison = match self.sort {
                Sort::Name => names[a].cmp(&names[b]),
                Sort::Extension => extensions[a].cmp(&extensions[b]),
                Sort::Modified => left.modified.cmp(&right.modified),
                Sort::Property(_) => properties[a].cmp(&properties[b]),
                Sort::Type => right.directory.cmp(&left.directory),
                Sort::Size | Sort::Share => left.bytes.cmp(&right.bytes),
            };
            let comparison = if self.descending {
                comparison.reverse()
            } else {
                comparison
            };
            // Unavailable/pending sizes always sort last, even descending.
            let pending = if self.sort == Sort::Modified {
                left.modified.is_none().cmp(&right.modified.is_none())
            } else if matches!(self.sort, Sort::Size | Sort::Share) {
                left.bytes.is_none().cmp(&right.bytes.is_none())
            } else {
                std::cmp::Ordering::Equal
            };
            pending
                .then(comparison)
                .then_with(|| names[a].cmp(&names[b]))
                .then(a.cmp(&b))
        });
        let visible: std::collections::BTreeSet<_> = self.order.iter().copied().collect();
        self.selections.retain(|i| visible.contains(i));
        if self.selected.is_some_and(|i| !visible.contains(&i)) {
            self.selected = None;
        }
    }
}
fn next_identity() -> u64 {
    static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    ID.fetch_add(1, Ordering::Relaxed)
}
struct Panel {
    widget_key: u64,
    drop_highlight: Option<highlight::Pulse>,
    tabs: Vec<Tab>,
    selected_tab: usize,
    columns: [f32; 4],
    extension_width: f32,
}
impl Panel {
    fn new(path: PathBuf) -> Self {
        Self {
            widget_key: next_identity(),
            drop_highlight: None,
            tabs: vec![Tab::new(path)],
            selected_tab: 0,
            columns: [44.0, 132.0, 82.0, 144.0],
            extension_width: 84.0,
        }
    }
}
impl std::ops::Deref for Panel {
    type Target = Tab;
    fn deref(&self) -> &Tab {
        &self.tabs[self.selected_tab]
    }
}
impl std::ops::DerefMut for Panel {
    fn deref_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.selected_tab]
    }
}
impl Drop for Tab {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
struct App {
    highlights: highlight::Store,
    failed_view: Option<utility_dialogs::FailedView>,
    activity_view: Option<activity::View>,
    activities: Vec<activity::Record>,
    shadow_picker: Option<shadow_copies::Picker>,
    context: Option<iced::Point>,
    header_context: Option<iced::Point>,
    properties: Option<String>,
    properties_request: Option<u64>,
    drag_files: Option<interactions::DraggedFiles>,
    drop_confirmation: Option<transfers::Request>,
    undo_entry: Option<undo::Entry>,
    undo_busy: bool,
    undo_frame: usize,
    window_width: f32,
    sidebar_width: f32,
    status_hovered: Option<usize>,
    history_generation: u64,
    dark: bool,
    window: Option<iced::window::Id>,
    modifiers: iced::keyboard::Modifiers,
    clipboard: Option<FileClipboard>,
    confirmation: Option<Confirmation>,
    dont_ask_again: bool,
    pending_skip_confirmation: Option<String>,
    rename: Option<rename::Editor>,
    undo_rename: Option<(PathBuf, PathBuf)>,
    jobs: std::collections::VecDeque<transfers::Job>,
    running_job: Option<u64>,
    notice: String,
    queue_expanded: bool,
    queue_height: f32,
    queue_batch: u64,
    conflict: Option<(
        PathBuf,
        PathBuf,
        std::sync::mpsc::SyncSender<transfers::ConflictChoice>,
    )>,
    panes: [Panel; 4],
    pane_count: usize,
    preferences: preferences::Preferences,
    look_override: Option<preferences::Look>,
    system_dark: Option<bool>,
    settings_open: bool,
    licenses: Option<licenses::Viewer>,
    sound_busy: bool,
    tree_keyboard_focus: bool,
    sidebar_drawer: bool,
    advanced: Option<advanced_search::Editor>,
    stacked: bool,
    grid: bool,
    elevated: bool,
    elevating: bool,
    app_theme: iced::Theme,
    motion: motion::Policy,
    active: usize,
    locations: locations::Locations,
    panel_locations: Vec<locations::Locations>,
    files_layout: container_layout::Layout,
}
#[derive(Debug, Clone)]
enum Message {
    Noop,
    LicensesOpen,
    LicensesClose,
    LicenseQuery(String),
    LicenseSelect(usize),
    RenameName(String),
    RenameConfirm(bool),
    RenameFinished(u64, Result<Option<String>, String>),
    CopyPath,
    OpenTerminal,
    AdvancedToggle,
    AdvancedChange(advanced_search::Change),
    AdvancedApply,
    AdvancedClose,
    AdvancedClear,
    CancelSearch,
    AdvancedResult(u64, u64, advanced_search::SearchEvent),
    SessionSaved(Result<(), String>),
    LayoutSize(iced::Size),
    Dock(usize, Option<u64>, docking::Zone, iced::Size),
    ResizeColumn(usize, usize, f32),
    Context(usize, Option<usize>, iced::Point),
    CloseContext,
    ContextAction(Option<FileAction>),
    RevealLocation,
    OpenContextEntry,
    OpenContextWith,
    DesktopResult(Result<(), String>),
    CompletionNotifications(bool),
    CompletionSound(preferences::SoundChoice),
    ChooseSound,
    TestSound,
    SoundChosen(Result<Option<String>, String>),
    SoundPreviewed(Result<(), String>),
    JumpInitial(char),
    ArrowSelection(i8),
    NewPanel(usize),
    PanelShortcut,
    PropertiesShortcut,
    ClosePanelShortcut,
    CyclePanel(bool),
    FocusSearch,
    OpenFiles(bool),
    AlternatingRows(bool),
    CloseUtility,
    ShowActivity(Option<u64>),
    ActivityFilter(String),
    TransferIssueFilter(String),
    TransferIssueLocation(u64, usize),
    ShowShadows,
    RefreshShadows,
    ShadowsLoaded(u64, Result<Vec<shadow_copies::ShadowCopy>, String>),
    OpenShadow(usize),
    SizeUnits(preferences::SizeUnits),
    HeaderContext(usize, iced::Point),
    ColumnToggle(columns::Column),
    ColumnMove(columns::Column, columns::Column),
    ScanModeChoice(preferences::ScanMode),
    PropertiesLoaded(u64, Option<String>),
    CloseProperties,
    FileDragStarted(usize, usize),
    FileDrop(usize, usize, usize),
    FileDropInto(usize, usize, usize, PathBuf),
    MouseHistory(bool),
    FolderTabDrop(usize, usize, PathBuf, Option<usize>),
    CancelDrag,
    Undo,
    HistoryLoaded(u64, Option<undo::Entry>),
    UndoFinished(Vec<PathBuf>, Result<(), String>, Option<undo::Entry>),
    ToggleQueue,
    ResizeTransfers(f32),
    ResolveConflict(transfers::ConflictChoice),
    WindowReady(Option<iced::window::Id>),
    FileAction(FileAction),
    Confirm(bool),
    Transfer(u64, transfers::Event),
    CancelTransfer(u64),
    ToggleTransferIssues(u64),
    RetryTransferIssue(u64, usize),
    RetryAllTransferIssues(u64),
    Modifiers(iced::keyboard::Modifiers),
    OpenEntry(usize, usize),
    SettingsToggle,
    ToggleSidebar,
    SidebarWidth(f32),
    CloseSidebar,
    SidebarWindowResized(iced::Size),
    CloseSettings,
    UndoTick,
    StatusHover(Option<usize>),
    DontAskAgain(bool),
    ColumnPropertiesLoaded,
    SkipConfirmation(String, bool),
    ApproveSkipConfirmation(bool),
    WindowUnfocused,
    ThemeChoice(preferences::ThemeMode),
    LookChoice(preferences::Look),
    AccentChoice(palette::Accent),
    TintChoice(preferences::TintMode),
    SystemTheme(Option<bool>),
    MotionSettingsChanged,
    LaunchAdmin(bool),
    DefaultLayoutChoice(preferences::DefaultLayout),
    RestoreDefaults,
    MoveTab(usize, u64, usize, usize),
    TabShortcut(u8),
    RelaunchAdmin,
    AdminRelaunched(Result<(), String>),
    NewTab(usize),
    SelectTab(usize, usize),
    CloseTab(usize, usize),
    ClosePanel(usize),
    ClickPanel(pane_grid::Pane),
    Locations(locations::Message),
    PanelLocations(usize, locations::Message),
    PanelTrees(bool),
    SearchSubfolders(bool),
    SearchSubmit(usize),
    SearchPathValidated(u64, u64, Result<PathBuf, String>),
    ResizeFiles(pane_grid::ResizeEvent, iced::Size),
    Sort(usize, Sort),
    Scan(usize, u64, Event),
    Address(usize, String),
    AddressBlur(usize),
    AddressValidated(u64, u64, Result<PathBuf, String>),
    Density(u16),
    Go(usize),
    Navigate(usize, PathBuf),
    Up(usize),
    Back(usize),
    Forward(usize),
    Refresh(usize),
    Search(usize, String),
    Select(usize, usize),
}
impl App {
    fn look(&self) -> preferences::Look {
        self.look_override.unwrap_or(self.preferences.look)
    }
    fn undo_label(&self) -> &'static str {
        if !self.undo_busy {
            "Undo"
        } else {
            ["◐ Undoing…", "◓ Undoing…", "◑ Undoing…", "◒ Undoing…"][(self.undo_frame / 8) % 4]
        }
    }
    fn new(home: PathBuf, right: PathBuf, drives: Vec<PathBuf>) -> Self {
        Self {
            context: None,
            header_context: None,
            properties: None,
            properties_request: None,
            drag_files: None,
            drop_confirmation: None,
            undo_entry: None,
            undo_busy: false,
            undo_frame: 0,
            window_width: 1400.0,
            sidebar_width: 210.0,
            status_hovered: None,
            history_generation: 0,
            dark: true,
            window: None,
            modifiers: Default::default(),
            clipboard: None,
            confirmation: None,
            dont_ask_again: false,
            pending_skip_confirmation: None,
            rename: None,
            undo_rename: None,
            jobs: Default::default(),
            running_job: None,
            notice: String::new(),
            queue_expanded: false,
            queue_height: 150.0,
            highlights: highlight::Store::default(),
            failed_view: None,
            activity_view: None,
            activities: vec![],
            shadow_picker: None,
            queue_batch: 0,
            conflict: None,
            panes: [
                Panel::new(home.clone()),
                Panel::new(right.clone()),
                Panel::new(home.clone()),
                Panel::new(right),
            ],
            pane_count: 2,
            preferences: preferences::Preferences::default(),
            look_override: None,
            system_dark: None,
            settings_open: false,
            licenses: None,
            sound_busy: false,
            tree_keyboard_focus: false,
            sidebar_drawer: false,
            advanced: None,
            stacked: true,
            grid: false,
            elevated: preferences::is_elevated(),
            elevating: false,
            app_theme: palette::with_tint(true, Default::default(), Default::default()).theme(),
            motion: motion::detect_policy(),
            active: 0,
            panel_locations: (0..4)
                .map(|_| locations::Locations::new(home.clone(), drives.clone()))
                .collect(),
            locations: locations::Locations::new(home, drives),
            files_layout: container_layout::Layout::preset(2, true, false, 0),
        }
    }
    fn boot() -> (Self, Task<Message>) {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let drives = locations::drive_roots();
        let defaults = desktop::default_paths(drives.clone(), home.clone());
        let mut app = Self::new(home, defaults[1].clone(), drives);
        for (panel, path) in app.panes.iter_mut().zip(defaults) {
            *panel = Panel::new(path);
        }
        app.preferences = preferences::Preferences::load();
        if let Some(look) =
            std::env::args().find_map(|arg| arg.strip_prefix("--look=").map(str::to_owned))
        {
            app.look_override = Some(preferences::Look::from_key(&look));
        }
        app.sidebar_width = app.preferences.sidebar * app.window_width;
        app.system_dark = preferences::system_dark();
        app.dark = app.preferences.theme.dark(app.system_dark);
        app.app_theme = palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        )
        .theme();
        typography::set_theme(palette::from_theme(&app.app_theme));
        app.initialize_default_layout();
        if app.preferences.default_layout == preferences::DefaultLayout::Remember {
            if let Err(error) = session::load()
                .and_then(|saved| saved.map_or(Ok(()), |saved| saved.restore(&mut app)))
            {
                app.notice = format!("Could not restore workspace: {error}");
            }
        }
        if let Some(note) = preferences::launch_error() {
            app.notice = note.into();
        }
        let mut tasks = vec![
            iced::window::oldest().map(Message::WindowReady),
            app.locations.load_labels().map(Message::Locations),
            app.locations
                .reveal(app.panes[app.active].path.clone())
                .map(Message::Locations),
            interactions::history_task(app.history_generation),
        ];
        for id in 0..app.pane_count {
            if app.preferences.panel_trees {
                tasks.push(
                    app.panel_locations[id]
                        .load_labels()
                        .map(move |m| Message::PanelLocations(id, m)),
                );
                tasks.push(
                    app.panel_locations[id]
                        .reveal(app.panes[id].path.clone())
                        .map(move |m| Message::PanelLocations(id, m)),
                );
            }
            tasks.push(app.panes[id].start(id, app.preferences.scan_mode));
        }
        let task = Task::batch(tasks);
        (app, task)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        #[cfg(not(test))]
        let save = matches!(
            &message,
            Message::Dock(..)
                | Message::MoveTab(..)
                | Message::NewTab(..)
                | Message::NewPanel(..)
                | Message::CyclePanel(..)
                | Message::FolderTabDrop(..)
                | Message::CloseTab(..)
                | Message::ClosePanel(..)
                | Message::SelectTab(..)
                | Message::ClickPanel(..)
                | Message::Navigate(..)
                | Message::Go(..)
                | Message::AddressValidated(..)
                | Message::Back(..)
                | Message::Forward(..)
                | Message::Up(..)
                | Message::ResizeFiles(..)
                | Message::ResizeColumn(..)
                | Message::Sort(..)
                | Message::Search(..)
                | Message::DefaultLayoutChoice(..)
                | Message::AdvancedApply
                | Message::AdvancedClear
                | Message::OpenShadow(..)
        );
        let mut task = self.update_inner(message);
        if let Some(mut request) = self.drop_confirmation.take() {
            // Dragging always copies; explicit Cut/Paste remains a separate command.
            request.kind = transfers::Kind::Copy;
            task = Task::batch([task, self.enqueue(request)]);
        }
        #[cfg(not(test))]
        if save && self.preferences.default_layout == preferences::DefaultLayout::Remember {
            return Task::batch([task, session::save(session::Session::capture(self))]);
        }
        task
    }
    fn update_inner(&mut self, message: Message) -> Task<Message> {
        if (self.licenses.is_some()
            || (self.failed_view.is_some() || self.activity_view.is_some())
            || self.shadow_picker.is_some())
            && matches!(
                &message,
                Message::FileAction(_)
                    | Message::TabShortcut(_)
                    | Message::NewTab(_)
                    | Message::NewPanel(_)
                    | Message::FolderTabDrop(..)
                    | Message::CloseTab(..)
                    | Message::ClosePanel(..)
                    | Message::MoveTab(..)
                    | Message::Dock(..)
                    | Message::OpenEntry(..)
                    | Message::Navigate(..)
                    | Message::Back(..)
                    | Message::Forward(..)
                    | Message::Up(..)
                    | Message::Go(..)
                    | Message::Search(..)
                    | Message::Sort(..)
                    | Message::Select(..)
                    | Message::Undo
                    | Message::SettingsToggle
                    | Message::AdvancedToggle
                    | Message::RestoreDefaults
                    | Message::SelectTab(..)
                    | Message::Context(..)
                    | Message::HeaderContext(..)
                    | Message::FileDragStarted(..)
            )
        {
            return Task::none();
        }
        if self.advanced.is_some()
            && matches!(
                &message,
                Message::Search(..)
                    | Message::Address(..)
                    | Message::Sort(..)
                    | Message::ClickPanel(..)
                    | Message::Navigate(..)
                    | Message::Back(..)
                    | Message::Forward(..)
                    | Message::Up(..)
                    | Message::Go(..)
            )
        {
            return Task::none();
        }
        if (self.licenses.is_some()
            || self.rename.is_some()
            || self.advanced.is_some()
            || self.confirmation.is_some()
            || self.pending_skip_confirmation.is_some()
            || self.conflict.is_some()
            || self.drop_confirmation.is_some()
            || self.properties.is_some()
            || self.undo_busy)
            && matches!(
                &message,
                Message::FileAction(_)
                    | Message::TabShortcut(_)
                    | Message::NewTab(_)
                    | Message::FolderTabDrop(..)
                    | Message::SelectTab(..)
                    | Message::CloseTab(..)
                    | Message::ClosePanel(..)
                    | Message::OpenEntry(..)
                    | Message::MoveTab(..)
                    | Message::Dock(..)
                    | Message::RestoreDefaults
                    | Message::Context(..)
                    | Message::ContextAction(_)
                    | Message::HeaderContext(..)
                    | Message::ColumnMove(..)
                    | Message::ColumnToggle(..)
                    | Message::OpenContextEntry
                    | Message::OpenContextWith
                    | Message::RevealLocation
                    | Message::Undo
                    | Message::FileDragStarted(..)
                    | Message::FileDrop(..)
            )
        {
            return Task::none();
        }
        if self.settings_open
            && matches!(
                &message,
                Message::FileAction(_)
                    | Message::TabShortcut(_)
                    | Message::NewTab(_)
                    | Message::FolderTabDrop(..)
                    | Message::CloseTab(..)
                    | Message::ClosePanel(..)
                    | Message::MoveTab(..)
                    | Message::Dock(..)
                    | Message::OpenEntry(..)
            )
        {
            return Task::none();
        }
        match message {
            Message::CopyPath => {
                if self.browsing_keys_blocked() && self.context.is_none() {
                    return Task::none();
                }
                self.context = None;
                let value = self.copy_path_text();
                self.notice = "Path copied to clipboard".into();
                return iced::clipboard::write(value);
            }
            Message::OpenTerminal => {
                self.context = None;
                return desktop::terminal_task(self.terminal_path());
            }
            Message::RenameName(name) => {
                if let Some(editor) = &mut self.rename {
                    if !editor.busy {
                        editor.name = name;
                        editor.error = None;
                    }
                }
            }
            Message::RenameConfirm(accept) => return self.confirm_rename(accept),
            Message::RenameFinished(id, result) => return self.finish_rename(id, result),
            Message::AlternatingRows(value) => {
                self.preferences.alternating_rows = value;
                self.persist_preferences();
            }
            Message::ShowActivity(job) => {
                self.failed_view = None;
                self.shadow_picker = None;
                self.settings_open = false;
                self.activity_view = Some(activity::View {
                    job,
                    filter: String::new(),
                    identity: next_identity(),
                });
            }
            Message::ActivityFilter(filter) => {
                if let Some(view) = &mut self.activity_view {
                    view.filter = filter;
                    view.identity = next_identity();
                }
            }
            Message::CloseUtility => {
                self.activity_view = None;
                self.failed_view = None;
                self.shadow_picker = None;
            }
            Message::TransferIssueFilter(filter) => {
                if let Some(view) = &mut self.failed_view {
                    view.filter = filter;
                    view.identity = next_identity();
                }
            }
            Message::TransferIssueLocation(id, index) => {
                let path = self.jobs.iter().find(|job| job.id == id).and_then(|job| {
                    job.report
                        .as_ref()?
                        .issues
                        .get(index)?
                        .source
                        .parent()
                        .map(|p| p.to_path_buf())
                });
                self.failed_view = None;
                if let Some(path) = path {
                    return self.update(Message::Navigate(self.active, path));
                }
            }
            Message::ShowShadows | Message::RefreshShadows => {
                let panel = self
                    .shadow_picker
                    .as_ref()
                    .map_or(self.active, |picker| picker.panel);
                let request = next_identity();
                self.failed_view = None;
                self.activity_view = None;
                self.settings_open = false;
                self.shadow_picker = Some(shadow_copies::Picker {
                    panel,
                    request,
                    result: None,
                });
                return shadow_copies::load(request);
            }
            Message::ShadowsLoaded(request, result) => {
                if let Some(picker) = self
                    .shadow_picker
                    .as_mut()
                    .filter(|picker| picker.request == request)
                {
                    picker.result = Some(result);
                }
            }
            Message::OpenShadow(index) => {
                let choice = self.shadow_picker.as_ref().and_then(|picker| {
                    Some((
                        picker.panel,
                        picker.result.as_ref()?.as_ref().ok()?.get(index)?.clone(),
                    ))
                });
                if let Some((panel, copy)) = choice {
                    if !copy.valid() || panel >= self.pane_count {
                        return Task::none();
                    }
                    let mut tab = Tab::new(copy.root());
                    tab.shadow = Some(copy);
                    self.panes[panel].tabs.push(tab);
                    self.panes[panel].selected_tab = self.panes[panel].tabs.len() - 1;
                    self.active = panel;
                    self.shadow_picker = None;
                    return self.panes[panel].start(panel, self.preferences.scan_mode);
                }
            }
            Message::Noop => {}
            Message::LicensesOpen => {
                self.settings_open = false;
                self.focus_files();
                self.licenses = Some(licenses::Viewer::default());
                return iced::widget::operation::focus("license-filter");
            }
            Message::LicensesClose => self.licenses = None,
            Message::LicenseQuery(query) => {
                if let Some(viewer) = &mut self.licenses {
                    viewer.filter(query);
                }
                return licenses::reset_scroll();
            }
            Message::LicenseSelect(index) => {
                if let Some(viewer) = &mut self.licenses {
                    if viewer.choices().iter().any(|c| c.index == index) {
                        viewer.selected = index;
                    }
                }
                return licenses::reset_scroll();
            }
            Message::AdvancedToggle => {
                if self.advanced.is_some() {
                    self.advanced = None;
                } else {
                    let mut draft = self.panes[self.active].filters.clone();
                    if !draft.active() && self.panes[self.active].search.is_empty() {
                        draft.recursive = self.preferences.search_subfolders;
                    }
                    draft.name = self.panes[self.active].search.clone();
                    self.advanced = Some(advanced_search::Editor {
                        tab: self.panes[self.active].identity,
                        draft,
                        error: None,
                    });
                    self.settings_open = false;
                    return iced::widget::operation::focus("advanced-name");
                }
            }
            Message::AdvancedChange(change) => {
                if let Some(editor) = &mut self.advanced {
                    editor.draft.edit(change);
                    editor.error = None;
                }
            }
            Message::AdvancedClose => self.advanced = None,
            Message::AdvancedClear => {
                if let Some(editor) = &mut self.advanced {
                    editor.draft = Default::default();
                    editor.error = None;
                }
            }
            Message::AdvancedApply => {
                if let Some(editor) = &mut self.advanced {
                    if let Err(error) = editor.draft.compile() {
                        editor.error = Some(error);
                        return Task::none();
                    }
                    let editor = self.advanced.take().unwrap();
                    for (id, panel) in self.panes[..self.pane_count].iter_mut().enumerate() {
                        if let Some(tab) = panel.tabs.iter_mut().find(|t| t.identity == editor.tab)
                        {
                            let reload = tab.filters.recursive || editor.draft.recursive;
                            tab.search = editor.draft.name.replace('"', "");
                            tab.filters = editor.draft;
                            tab.view_identity = next_identity();
                            if reload {
                                return tab.start(id, self.preferences.scan_mode);
                            }
                            tab.reorder();
                            break;
                        }
                    }
                }
            }
            Message::CancelSearch => {
                let tab: &mut Tab = &mut self.panes[self.active];
                tab.cancel.store(true, Ordering::Relaxed);
                tab.generation = next_identity();
                tab.busy = false;
                tab.backend = "Search stopped · partial results".into();
            }
            Message::AdvancedResult(identity, generation, event) => {
                let load_properties =
                    matches!(&event, advanced_search::SearchEvent::Finished { .. })
                        && file_properties::needed(&self.preferences.columns);
                if let Some(tab) = self.panes[..self.pane_count]
                    .iter_mut()
                    .flat_map(|p| p.tabs.iter_mut())
                    .find(|t| t.identity == identity && t.generation == generation)
                {
                    match event {
                        advanced_search::SearchEvent::Batch(entries) => {
                            tab.total_bytes = entries
                                .iter()
                                .filter_map(|e| e.bytes)
                                .fold(tab.total_bytes, u64::saturating_add);
                            tab.entries.extend(entries);
                            tab.view_identity = next_identity();
                            tab.reorder();
                            tab.restore_pending_selection();
                        }
                        advanced_search::SearchEvent::Progress { visited, skipped } => {
                            tab.backend = format!(
                                "Searching · {visited} checked · {skipped} unreadable folders"
                            )
                        }
                        advanced_search::SearchEvent::Finished {
                            visited,
                            skipped,
                            limited,
                        } => {
                            tab.busy = false;
                            tab.backend = format!(
                                "Search complete · {visited} checked · {skipped} unreadable folders{}",
                                if limited {
                                    " · 200,000 result limit reached"
                                } else {
                                    ""
                                }
                            );
                        }
                        advanced_search::SearchEvent::Failed(error) => {
                            tab.busy = false;
                            tab.error = Some(error);
                        }
                    }
                    if load_properties {
                        return file_properties::load(
                            tab.entries.iter().map(|e| e.path.clone()).collect(),
                            tab.cancel.clone(),
                        )
                        .map(|()| Message::ColumnPropertiesLoaded);
                    }
                }
            }

            Message::SessionSaved(result) => {
                if let Err(error) = result {
                    self.notice = format!("Could not save workspace: {error}");
                }
            }
            Message::LayoutSize(size) => self.files_layout.set_size(size),
            Message::Dock(id, identity, zone, size) => return self.dock(id, identity, zone, size),
            Message::ClosePanel(id) => self.close_panel(id),
            Message::Context(pane, index, point) => return self.open_context(pane, index, point),
            Message::RevealLocation => {
                self.context = None;
                let panel = &self.panes[self.active];
                let path = panel
                    .selected
                    .and_then(|i| panel.entries.get(i))
                    .map(|entry| {
                        if entry.directory {
                            entry.path.clone()
                        } else {
                            entry.path.parent().unwrap_or(&panel.path).to_path_buf()
                        }
                    })
                    .unwrap_or_else(|| panel.path.clone());
                if self.preferences.panel_trees {
                    let id = self.active;
                    return self.panel_locations[id]
                        .reveal(path)
                        .map(move |m| Message::PanelLocations(id, m));
                }
                return self.locations.reveal(path).map(Message::Locations);
            }
            Message::ScanModeChoice(mode) => {
                self.preferences.scan_mode = mode;
                self.persist_preferences();
                let mut tasks = Vec::new();
                for (id, panel) in self.panes.iter_mut().enumerate().take(self.pane_count) {
                    for tab in &mut panel.tabs {
                        if tab.generation != 0 {
                            tasks.push(tab.start(id, mode));
                        }
                    }
                }
                return Task::batch(tasks);
            }
            Message::CloseContext => {
                self.context = None;
                self.header_context = None;
            }
            Message::HeaderContext(pane, point) => {
                self.active = pane;
                self.context = None;
                self.header_context = Some(point);
            }
            Message::ColumnToggle(column) => {
                let was_needed = file_properties::needed(&self.preferences.columns);
                self.preferences.columns.toggle(column);
                self.persist_preferences();
                if !was_needed && file_properties::needed(&self.preferences.columns) {
                    return Task::batch(self.panes[..self.pane_count].iter().map(|pane| {
                        file_properties::load(
                            pane.entries.iter().map(|e| e.path.clone()).collect(),
                            pane.cancel.clone(),
                        )
                        .map(|()| Message::ColumnPropertiesLoaded)
                    }));
                }
            }
            Message::ColumnMove(from, to) => {
                self.preferences.columns.move_to(from, to);
                self.persist_preferences();
            }
            Message::CompletionNotifications(v) => {
                self.preferences.completion_notifications = v;
                self.persist_preferences();
            }
            Message::CompletionSound(choice) => {
                if self.sound_busy {
                    return Task::none();
                }
                if choice == preferences::SoundChoice::Custom
                    && self.preferences.custom_sound.is_empty()
                {
                    self.sound_busy = true;
                    return sounds::choose_task();
                }
                self.preferences.completion_sound = choice != preferences::SoundChoice::Off;
                if choice != preferences::SoundChoice::Off {
                    self.preferences.completion_tone = choice;
                }
                self.persist_preferences();
            }
            Message::ChooseSound => {
                if !self.sound_busy {
                    self.sound_busy = true;
                    return sounds::choose_task();
                }
            }
            Message::TestSound => {
                if !self.sound_busy
                    && self.preferences.sound_choice() != preferences::SoundChoice::Off
                {
                    self.sound_busy = true;
                    return sounds::preview_task(
                        self.preferences.sound_choice(),
                        self.preferences.custom_sound.clone(),
                    );
                }
            }
            Message::SoundChosen(result) => {
                self.sound_busy = false;
                match result {
                    Ok(Some(name)) => {
                        self.preferences.custom_sound = name;
                        self.preferences.completion_tone = preferences::SoundChoice::Custom;
                        self.preferences.completion_sound = true;
                        self.persist_preferences();
                        self.notice = "Custom completion sound imported.".into();
                    }
                    Ok(None) => {}
                    Err(e) => self.notice = e,
                }
            }
            Message::SoundPreviewed(result) => {
                self.sound_busy = false;
                if let Err(e) = result {
                    self.notice = e;
                }
            }
            Message::ClosePanelShortcut => {
                if !self.browsing_keys_blocked() {
                    return self.update(Message::ClosePanel(self.active));
                }
            }
            Message::CyclePanel(reverse) => {
                if self.browsing_keys_blocked() {
                    return if reverse {
                        iced::widget::operation::focus_previous()
                    } else {
                        iced::widget::operation::focus_next()
                    };
                }
                self.active =
                    (self.active + if reverse { self.pane_count - 1 } else { 1 }) % self.pane_count;
                self.focus_files();
                return iced::widget::operation::focus("file-panel-browsing");
            }
            Message::FocusSearch => {
                if self.licenses.is_some() {
                    return iced::widget::operation::focus("license-filter")
                        .chain(iced::widget::operation::select_all("license-filter"));
                }
                if !self.browsing_keys_blocked() {
                    self.focus_files();
                    return iced::widget::operation::focus("ribbon-search")
                        .chain(iced::widget::operation::select_all("ribbon-search"));
                }
            }
            Message::PanelShortcut => {
                if !self.browsing_keys_blocked() {
                    return self.update(Message::NewPanel(self.active));
                }
            }
            Message::PropertiesShortcut => {
                if !self.browsing_keys_blocked() {
                    return self.update(Message::ContextAction(None));
                }
            }
            Message::ArrowSelection(direction) => return self.arrow_selection(direction),
            Message::NewPanel(id) => {
                if !self.browsing_keys_blocked() {
                    return self.new_panel(id);
                }
            }
            Message::JumpInitial(c) => return self.jump_initial(c),
            Message::OpenFiles(v) => {
                self.preferences.open_files = v;
                self.persist_preferences();
            }
            Message::SizeUnits(v) => {
                self.preferences.size_units = v;
                self.persist_preferences();
            }
            Message::DesktopResult(result) => {
                if let Err(error) = result {
                    self.notice = error;
                }
            }
            Message::OpenContextEntry => {
                self.context = None;
                if let Some(index) = self.panes[self.active].selected {
                    return self.update(Message::OpenEntry(self.active, index));
                }
            }
            Message::OpenContextWith => {
                self.context = None;
                if let Some(index) = self.panes[self.active].selected {
                    return self.open_entry(self.active, index, true);
                }
            }
            Message::ContextAction(action) => {
                self.context = None;
                return if let Some(action) = action {
                    self.file_action(action)
                } else {
                    self.show_properties()
                };
            }
            Message::PropertiesLoaded(request, value) => {
                if self.properties_request == Some(request) {
                    self.properties_request = None;
                    self.properties = value;
                }
            }
            Message::CloseProperties => {
                self.properties = None;
                self.properties_request = None;
            }
            Message::FileDragStarted(pane, index) => self.begin_file_drag(pane, index),
            Message::FileDrop(pane, index, destination) => {
                self.drop_files(pane, index, destination)
            }
            Message::FolderTabDrop(panel, index, path, source) => {
                self.drag_files = None;
                if panel >= self.pane_count || path.as_os_str().is_empty() {
                    return Task::none();
                }
                let shadow = source
                    .and_then(|source| self.panes.get(source))
                    .and_then(|p| p.shadow.clone())
                    .filter(|copy| copy.contains(&path))
                    .or_else(|| {
                        self.panes
                            .iter()
                            .flat_map(|p| &p.tabs)
                            .filter_map(|tab| tab.shadow.as_ref())
                            .find(|copy| copy.contains(&path))
                            .cloned()
                    });
                let mut tab = Tab::new(path);
                tab.shadow = shadow;
                let index = index.min(self.panes[panel].tabs.len());
                self.panes[panel].drop_highlight = highlight::Pulse::new(self.motion);
                self.panes[panel].tabs.insert(index, tab);
                self.panes[panel].selected_tab = index;
                self.active = panel;
                return self.panes[panel].start(panel, self.preferences.scan_mode);
            }
            Message::CancelDrag => self.drag_files = None,
            Message::HistoryLoaded(generation, entry) => {
                if generation == self.history_generation
                    && !self.undo_busy
                    && self.running_job.is_none()
                {
                    self.undo_entry = entry;
                }
            }
            Message::Undo => return self.undo_operation(),
            Message::UndoFinished(paths, result, entry) => {
                self.undo_busy = false;
                let operation = next_identity();
                for path in &paths {
                    self.record_operation(
                        operation,
                        "Undo",
                        path.clone(),
                        None,
                        if result.is_ok() {
                            "Completed"
                        } else {
                            "Failed / incomplete"
                        },
                    );
                }
                if result.is_ok() {
                    if let Some((from, to)) = self.undo_rename.take() {
                        self.remap_renamed_tabs(&from, &to);
                    }
                } else {
                    self.undo_rename = None;
                }
                self.undo_entry = entry;
                self.notice = match result {
                    Ok(()) => "Undo completed".into(),
                    Err(e) => format!("Undo stopped: {e}"),
                };
                return self.refresh_affected(&paths);
            }
            Message::WindowReady(window) => {
                self.window = window;
                return self.style_chrome();
            }
            Message::ResizeTransfers(height) => {
                if height.is_finite() {
                    self.queue_height = height.clamp(60.0, 1600.0);
                }
            }
            Message::ToggleQueue => self.queue_expanded = !self.queue_expanded,
            Message::ResolveConflict(choice) => {
                if let Some((_, _, reply)) = self.conflict.take() {
                    self.dont_ask_again = false;
                    if let Some(job) = self
                        .jobs
                        .iter_mut()
                        .find(|j| Some(j.id) == self.running_job)
                    {
                        job.state = "Running";
                    }
                    let _ = reply.send(choice);
                }
            }
            Message::FileAction(action) => return self.file_action(action),
            Message::Confirm(accept) => {
                if self.licenses.is_some() {
                    if !accept {
                        self.licenses = None;
                    }
                    return Task::none();
                }
                if self.pending_skip_confirmation.is_some() {
                    return self.update(Message::ApproveSkipConfirmation(accept));
                }
                if self.sidebar_drawer && !accept {
                    self.focus_files();
                    self.sidebar_drawer = false;
                    return Task::none();
                }
                if self.rename.is_some() {
                    return self.confirm_rename(accept);
                }
                if self.advanced.is_some() {
                    return self.update(if accept {
                        Message::AdvancedApply
                    } else {
                        Message::AdvancedClose
                    });
                }
                if self.context.take().is_some() {
                    return Task::none();
                }
                if self.properties.take().is_some() {
                    self.properties_request = None;
                    return Task::none();
                }
                if self.settings_open {
                    self.settings_open = false;
                    return Task::none();
                }
                if self.conflict.is_some() {
                    if !accept {
                        return self
                            .update(Message::ResolveConflict(transfers::ConflictChoice::Skip));
                    }
                    return Task::none();
                }
                if self.confirmation.is_none()
                    && ((self.failed_view.is_some() || self.activity_view.is_some())
                        || self.shadow_picker.is_some())
                {
                    if !accept {
                        self.failed_view = None;
                        self.activity_view = None;
                        self.shadow_picker = None;
                    }
                    return Task::none();
                }
                if accept && self.tree_keyboard_focus && self.confirmation.is_none() {
                    let tree = if self.preferences.panel_trees {
                        &self.panel_locations[self.active]
                    } else {
                        &self.locations
                    };
                    if let Some(path) = tree.keyboard_cursor().cloned() {
                        return self.update(if self.preferences.panel_trees {
                            Message::PanelLocations(self.active, locations::Message::Navigate(path))
                        } else {
                            Message::Locations(locations::Message::Navigate(path))
                        });
                    }
                }
                if accept && !self.tree_keyboard_focus && !self.browsing_keys_blocked() {
                    if let Some(index) = self.panes[self.active].selected {
                        return self.update(Message::OpenEntry(self.active, index));
                    }
                }
                return self.confirm_operation(accept);
            }
            Message::Transfer(id, event) => return self.transfer_event(id, event),
            Message::ToggleTransferIssues(id) => {
                self.activity_view = None;
                self.failed_view = Some(utility_dialogs::FailedView {
                    job: id,
                    filter: String::new(),
                    identity: next_identity(),
                });
            }
            Message::RetryTransferIssue(id, index) => {
                let request = self.jobs.iter().find(|job| job.id == id).and_then(|job| {
                    job.report
                        .as_ref()?
                        .issues
                        .get(index)?
                        .retry_request(job.request.kind)
                });
                if let Some(request) = request {
                    return self.enqueue(request);
                }
            }
            Message::RetryAllTransferIssues(id) => {
                let requests = self
                    .jobs
                    .iter()
                    .find(|job| job.id == id)
                    .map(transfers::Job::failed_retry_requests)
                    .unwrap_or_default();
                let tasks: Vec<_> = requests
                    .into_iter()
                    .map(|request| self.enqueue(request))
                    .collect();
                return Task::batch(tasks);
            }
            Message::CancelTransfer(id) => {
                if let Some(job) = self.jobs.iter().find(|job| job.id == id) {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            Message::Modifiers(modifiers) => self.modifiers = modifiers,
            Message::OpenEntry(id, index) => {
                return self.open_entry(id, index, false);
            }
            Message::TabShortcut(action) => {
                let id = self.active;
                return self.update(match action {
                    0 => Message::NewTab(id),
                    1 => Message::CloseTab(id, self.panes[id].selected_tab),
                    _ => {
                        let panel = &self.panes[id];
                        let count = panel.tabs.len();
                        let next =
                            (panel.selected_tab + if action == 3 { count - 1 } else { 1 }) % count;
                        Message::SelectTab(id, next)
                    }
                });
            }
            Message::SidebarWindowResized(size) => {
                if self.window_width > 0.0 && size.width > 0.0 {
                    self.window_width = size.width;
                }
                if self.sidebar_collapses(size) && !self.sidebar_drawer {
                    self.focus_files();
                }
                if !self.sidebar_collapses(size) {
                    self.sidebar_drawer = false;
                }
            }
            Message::SidebarWidth(width) => {
                self.sidebar_width = width;
                self.preferences.sidebar = (width / self.window_width.max(1.)).clamp(0.10, 0.45);
            }
            Message::ToggleSidebar => {
                if !self.sidebar_collapses(iced::Size::new(self.window_width, 700.)) {
                    self.sidebar_drawer = false;
                    self.focus_files();
                    return Task::none();
                }
                self.sidebar_drawer = !self.sidebar_drawer;
                if self.sidebar_drawer {
                    self.tree_keyboard_focus = true;
                    self.locations
                        .set_keyboard_cursor(self.panes[self.active].path.clone());
                } else {
                    self.focus_files();
                }
            }
            Message::CloseSidebar => {
                self.sidebar_drawer = false;
                self.focus_files();
            }
            Message::MouseHistory(forward) => {
                return self.update(if forward {
                    Message::Forward(self.active)
                } else {
                    Message::Back(self.active)
                });
            }
            Message::FileDropInto(pane, index, destination, path) => {
                if let Some(drag) = self.drag_files.take() {
                    if drag.pane == pane
                        && drag.index == index
                        && destination < self.pane_count
                        && !drag
                            .paths
                            .iter()
                            .any(|source| path_utils::related(source, &path))
                    {
                        self.active = destination;
                        self.highlights.flash(&path, self.motion);
                        self.drop_confirmation = Some(transfers::Request {
                            kind: transfers::Kind::Copy,
                            sources: drag.paths,
                            destination: Some(path),
                        });
                    }
                }
            }
            Message::SettingsToggle => self.settings_open = !self.settings_open,
            Message::CloseSettings => self.settings_open = false,
            Message::UndoTick => self.undo_frame = self.undo_frame.wrapping_add(1),
            Message::StatusHover(panel) => self.status_hovered = panel,
            Message::DontAskAgain(value) => self.dont_ask_again = value,
            Message::ColumnPropertiesLoaded => {
                for pane in &mut self.panes[..self.pane_count] {
                    if matches!(pane.sort, Sort::Property(_)) {
                        pane.reorder();
                    }
                }
            }
            Message::SkipConfirmation(key, skip) => {
                if key != "recycle" {
                    return Task::none();
                }
                if skip {
                    self.settings_open = false;
                    self.pending_skip_confirmation = Some(key);
                } else {
                    self.preferences.skipped_confirmations.retain(|v| v != &key);
                    self.persist_preferences();
                }
            }
            Message::ApproveSkipConfirmation(approve) => {
                if let Some(key) = self.pending_skip_confirmation.take() {
                    if approve
                        && key == "recycle"
                        && !self.preferences.skipped_confirmations.contains(&key)
                    {
                        self.preferences.skipped_confirmations.push(key);
                        self.persist_preferences();
                    }
                }
            }
            Message::WindowUnfocused => {
                self.modifiers = iced::keyboard::Modifiers::default();
                self.settings_open = false;
                self.context = None;
                self.header_context = None;
            }
            Message::TintChoice(mode) => {
                self.preferences.tint = mode;
                self.persist_preferences();
                return self.apply_appearance();
            }
            Message::AccentChoice(accent) => {
                self.preferences.accent = accent;
                self.persist_preferences();
                return self.apply_appearance();
            }
            Message::LookChoice(look) => {
                self.look_override = None;
                self.preferences.look = look;
                self.persist_preferences();
                return self.apply_appearance();
            }
            Message::ThemeChoice(mode) => {
                self.preferences.theme = mode;
                self.persist_preferences();
                return self.apply_appearance();
            }
            Message::MotionSettingsChanged => {
                self.motion = motion::detect_policy();
                if !self.motion.enabled() {
                    self.highlights.clear();
                    for panel in &mut self.panes {
                        panel.drop_highlight = None;
                    }
                }
            }
            Message::SystemTheme(dark) => {
                self.system_dark = dark;
                return self.apply_appearance();
            }
            Message::DefaultLayoutChoice(layout) => {
                self.preferences.default_layout = layout;
                self.persist_preferences();
            }
            Message::LaunchAdmin(value) => {
                self.preferences.admin = value;
                self.persist_preferences();
            }
            Message::RestoreDefaults => {
                self.look_override = None;
                self.preferences = preferences::Preferences::default();
                self.pane_count = 2;
                self.stacked = true;
                self.grid = false;
                self.active = self.active.min(1);
                self.queue_expanded = false;
                self.sidebar_width = 0.15 * self.window_width;
                self.rebuild_file_layout();
                self.persist_preferences();
                self.notice = "Settings restored to defaults.".into();
                let mut tasks = vec![self.apply_appearance()];
                for id in 0..2 {
                    for tab in &mut self.panes[id].tabs {
                        if tab.generation != 0 {
                            tasks.push(tab.start(id, self.preferences.scan_mode));
                        }
                    }
                    if self.panes[id].generation == 0 {
                        tasks.push(self.panes[id].start(id, self.preferences.scan_mode));
                    }
                }
                return Task::batch(tasks);
            }
            Message::MoveTab(from, identity, to, index) => {
                return self.move_tab(from, identity, to, index);
            }
            Message::RelaunchAdmin => {
                if self.elevated || self.elevating || self.jobs.iter().any(|j| j.report.is_none()) {
                    return Task::none();
                }
                #[cfg(not(test))]
                if self.preferences.default_layout == preferences::DefaultLayout::Remember {
                    if let Err(error) = session::save_now(session::Session::capture(self)) {
                        self.notice = format!("Could not save workspace before relaunch: {error}");
                        return Task::none();
                    }
                }
                self.elevating = true;
                self.persist_preferences();
                return Task::perform(
                    async {
                        let (tx, rx) = iced::futures::channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let _ = tx.send(preferences::relaunch_elevated());
                        });
                        rx.await
                            .unwrap_or_else(|_| Err("Elevation worker stopped".into()))
                    },
                    Message::AdminRelaunched,
                );
            }
            Message::AdminRelaunched(result) => {
                self.elevating = false;
                match result {
                    Ok(()) => return iced::exit(),
                    Err(error) => {
                        self.notice = error;
                        self.queue_expanded = true;
                    }
                }
            }
            Message::ClickPanel(pane) => {
                self.focus_files();
                if let Some(&id) = self.files_layout.get(pane) {
                    self.active = id;
                }
            }
            Message::NewTab(id) => {
                self.focus_files();
                self.active = id;
                let path = self.panes[id].path.clone();
                let mut tab = Tab::new(path);
                tab.shadow = self.panes[id].shadow.clone();
                self.panes[id].tabs.push(tab);
                self.panes[id].selected_tab = self.panes[id].tabs.len() - 1;
                return self.panes[id].start(id, self.preferences.scan_mode);
            }
            Message::SelectTab(id, tab) => {
                self.focus_files();
                if tab < self.panes[id].tabs.len() {
                    self.active = id;
                    self.panes[id].selected_tab = tab;
                    if self.panes[id].generation == 0 && self.panes[id].entries.is_empty() {
                        return self.panes[id].start(id, self.preferences.scan_mode);
                    }
                }
            }
            Message::CloseTab(id, tab) => self.close_tab(id, tab),
            Message::PanelTrees(value) => {
                self.focus_files();
                self.preferences.panel_trees = value;
                self.persist_preferences();
                if value {
                    return Task::batch((0..self.pane_count).map(|id| {
                        self.panel_locations[id]
                            .reveal(self.panes[id].path.clone())
                            .map(move |m| Message::PanelLocations(id, m))
                    }));
                }
            }
            Message::PanelLocations(id, locations::Message::Navigate(path)) => {
                let task = self.update(Message::Navigate(id, path.clone()));
                self.tree_keyboard_focus = true;
                self.panel_locations[id].set_keyboard_cursor(path);
                return task;
            }
            Message::PanelLocations(id, message) => {
                if matches!(
                    &message,
                    locations::Message::Toggle(_)
                        | locations::Message::Refresh
                        | locations::Message::CollapseAll
                        | locations::Message::NetworkInput(_)
                        | locations::Message::AddNetwork
                ) {
                    self.active = id;
                    self.tree_keyboard_focus = true;
                }
                return self.panel_locations[id]
                    .update(message)
                    .map(move |m| Message::PanelLocations(id, m));
            }
            Message::SearchSubfolders(value) => {
                let id = self.active;
                self.preferences.search_subfolders = value;
                self.persist_preferences();
                let recursive = value && !self.panes[id].search.trim().is_empty();
                if self.panes[id].filters.recursive != recursive {
                    self.panes[id].filters.recursive = recursive;
                    return self.panes[id].start(id, self.preferences.scan_mode);
                }
            }
            Message::SearchSubmit(id) => {
                let tab = &mut self.panes[id];
                if tab.search.trim().is_empty() {
                    return Task::none();
                }
                let request = next_identity();
                tab.address_request = Some(request);
                return path_input::submit_search(
                    tab.search.clone(),
                    tab.path.clone(),
                    tab.identity,
                    request,
                );
            }
            Message::SearchPathValidated(identity, request, result) => {
                for (id, panel) in self.panes.iter_mut().enumerate() {
                    if let Some(tab) = panel
                        .tabs
                        .iter_mut()
                        .find(|t| t.identity == identity && t.address_request == Some(request))
                    {
                        tab.address_request = None;
                        if let Ok(path) = result {
                            tab.search.clear();
                            tab.filters = Default::default();
                            if tab.path != path {
                                tab.back.push(tab.path.clone());
                                tab.forward.clear();
                                tab.path = path;
                            }
                            return tab.start(id, self.preferences.scan_mode);
                        }
                        break;
                    }
                }
            }
            Message::Locations(locations::Message::Navigate(path)) => {
                let keep_tree_focus = !self.sidebar_drawer;
                let task = self.update(Message::Navigate(self.active, path.clone()));
                if keep_tree_focus {
                    self.tree_keyboard_focus = true;
                    self.locations.set_keyboard_cursor(path);
                }
                return task;
            }
            Message::Locations(message) => {
                if matches!(
                    &message,
                    locations::Message::Toggle(_)
                        | locations::Message::Refresh
                        | locations::Message::CollapseAll
                ) {
                    self.tree_keyboard_focus = true;
                }
                return self.locations.update(message).map(Message::Locations);
            }
            Message::ResizeFiles(event, size) => self.resize_file_panels(event, size),
            Message::ResizeColumn(id, column, width) => {
                if column == 4 {
                    self.panes[id].extension_width = width;
                } else if let Some(value) = self.panes[id].columns.get_mut(column) {
                    *value = width;
                }
            }
            Message::Sort(id, sort) => {
                self.focus_files();
                self.active = id;
                let pane: &mut Tab = &mut self.panes[id];
                pane.descending = pane.sort == sort && !pane.descending;
                pane.sort = sort;
                pane.view_identity = next_identity();
                pane.reorder();
            }
            Message::Scan(id, generation, event) => {
                let load_properties = matches!(&event, Event::Listed(Ok(_)))
                    && file_properties::needed(&self.preferences.columns);
                let owner = if self.panes[id]
                    .tabs
                    .iter()
                    .any(|t| t.generation == generation)
                {
                    Some(id)
                } else {
                    self.panes
                        .iter()
                        .position(|p| p.tabs.iter().any(|t| t.generation == generation))
                };
                let Some(pane) = owner.and_then(|owner| {
                    self.panes[owner]
                        .tabs
                        .iter_mut()
                        .find(|t| t.generation == generation)
                }) else {
                    return Task::none();
                };
                match event {
                    Event::Backend(backend) => pane.backend = backend,
                    Event::Listed(Ok(entries)) => {
                        self.highlights
                            .resolve(entries.iter().map(|e| e.path.as_path()), self.motion);
                        pane.total_bytes = entries
                            .iter()
                            .filter_map(|e| e.bytes)
                            .fold(0u64, u64::saturating_add);
                        pane.incomplete_entries = entries
                            .iter()
                            .filter(|e| e.partial || e.bytes.is_none())
                            .count();
                        pane.entries = entries;
                        pane.reorder();
                        pane.restore_pending_selection();
                    }
                    Event::Listed(Err(error)) => {
                        pane.error = Some(error);
                        pane.busy = false;
                    }
                    Event::Sizes(sizes) => {
                        for (i, bytes, partial) in sizes {
                            if let Some(entry) = pane.entries.get_mut(i) {
                                pane.total_bytes = pane
                                    .total_bytes
                                    .saturating_sub(entry.bytes.unwrap_or(0))
                                    .saturating_add(bytes);
                                if entry.partial || entry.bytes.is_none() {
                                    pane.incomplete_entries =
                                        pane.incomplete_entries.saturating_sub(1);
                                }
                                if partial {
                                    pane.incomplete_entries += 1;
                                }
                                entry.bytes = Some(bytes);
                                entry.partial = partial;
                            }
                        }
                        if matches!(pane.sort, Sort::Size | Sort::Share) || pane.filters.has_size()
                        {
                            pane.reorder();
                        }
                    }
                    Event::Finished => pane.busy = false,
                }
                if load_properties {
                    return file_properties::load(
                        pane.entries.iter().map(|e| e.path.clone()).collect(),
                        pane.cancel.clone(),
                    )
                    .map(|()| Message::ColumnPropertiesLoaded);
                }
            }
            Message::Address(id, s) => {
                self.focus_files();
                self.active = id;
                self.panes[id].address = s;
                self.panes[id].address_request = None;
            }
            Message::AddressBlur(id) => {
                if id < self.pane_count {
                    let tab = &mut self.panes[id];
                    tab.address = tab.path.display().to_string();
                    tab.address_request = None;
                }
            }
            Message::Density(value) => {
                self.preferences.density = preferences::density(value);
                self.persist_preferences();
            }
            Message::AddressValidated(identity, request, result) => {
                let owner = self.panes.iter().enumerate().find_map(|(id, p)| {
                    p.tabs
                        .iter()
                        .position(|t| t.identity == identity && t.address_request == Some(request))
                        .map(|i| (id, i))
                });
                let Some((id, index)) = owner else {
                    return Task::none();
                };
                let tab = &mut self.panes[id].tabs[index];
                tab.address_request = None;
                match result {
                    Ok(path) => {
                        if tab.path != path {
                            tab.back.push(tab.path.clone());
                            tab.forward.clear();
                            tab.path = path;
                        }
                        return tab.start(id, self.preferences.scan_mode);
                    }
                    Err(error) => {
                        tab.address = tab.path.display().to_string();
                        self.notice = format!("Directory unchanged: {error}");
                    }
                }
            }
            Message::Search(id, s) => {
                self.focus_files();
                self.active = id;
                self.panes[id].address_request = None;
                self.panes[id].search = s.replace('"', "");
                self.panes[id].view_identity = next_identity();
                let was_recursive = self.panes[id].filters.recursive;
                self.panes[id].filters.recursive =
                    self.preferences.search_subfolders && !self.panes[id].search.trim().is_empty();
                if was_recursive || self.panes[id].filters.recursive {
                    return self.panes[id].start(id, self.preferences.scan_mode);
                }
                self.panes[id].reorder();
            }
            Message::Select(id, i) => {
                self.focus_files();
                self.active = id;
                let modifiers = self.modifiers;
                let pane: &mut Tab = &mut self.panes[id];
                sk_iced::components::select(
                    &mut pane.selections,
                    &mut pane.selected,
                    &pane.order,
                    i,
                    modifiers.shift(),
                    modifiers.control(),
                );
            }
            Message::Go(id) => {
                let tab = &mut self.panes[id];
                let text = tab.address.trim().trim_matches('"');
                if text.is_empty() {
                    tab.address = tab.path.display().to_string();
                    tab.address_request = None;
                    return Task::none();
                }
                let input = text.to_string();
                let request = next_identity();
                tab.address_request = Some(request);
                return path_input::submit(input, tab.path.clone(), tab.identity, request);
            }
            Message::Up(id) => {
                if let Some(parent) = self.panes[id].parent() {
                    return self.update(Message::Navigate(id, parent));
                }
            }
            Message::Navigate(id, path) => {
                self.focus_files();
                self.sidebar_drawer = false;
                self.active = id;
                let pane: &mut Tab = &mut self.panes[id];
                if pane.path != path {
                    pane.back.push(pane.path.clone());
                    pane.forward.clear();
                    pane.path = path;
                }
                return pane.start(id, self.preferences.scan_mode);
            }
            Message::Back(id) => {
                self.focus_files();
                self.active = id;
                let pane: &mut Tab = &mut self.panes[id];
                if let Some(path) = pane.back.pop() {
                    pane.forward.push(pane.path.clone());
                    pane.path = path;
                    return pane.start(id, self.preferences.scan_mode);
                }
            }
            Message::Forward(id) => {
                self.focus_files();
                self.active = id;
                let pane: &mut Tab = &mut self.panes[id];
                if let Some(path) = pane.forward.pop() {
                    pane.back.push(pane.path.clone());
                    pane.path = path;
                    return pane.start(id, self.preferences.scan_mode);
                }
            }
            Message::Refresh(id) => {
                self.active = id;
                return self.panes[id].start_refresh(id, true, self.preferences.scan_mode);
            }
        }
        Task::none()
    }
    fn style_chrome(&self) -> Task<Message> {
        if let Some(id) = self.window {
            let t = palette::styled(
                self.dark,
                self.preferences.accent,
                self.preferences.tint,
                self.look(),
            );
            let dark = self.dark;
            iced::window::run(id, move |window| {
                chrome::apply(window, t, dark);
                Message::Noop
            })
        } else {
            Task::none()
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let base = container(responsive_cache::responsive(move |size| {
            self.adaptive_sidebar(size)
        }))
        .width(Fill)
        .height(Fill)
        .style(move |_| t.surface())
        .into();
        licenses::layer(
            self,
            tab_drag::layer(
                self.rename_layer(advanced_search::layer(
                    self,
                    self.interaction_layer(self.confirmation_layer(self.utility_layer(base))),
                )),
                self.licenses.is_none()
                    && !self.sidebar_drawer
                    && self.rename.is_none()
                    && !self.settings_open
                    && self.failed_view.is_none()
                    && self.activity_view.is_none()
                    && self.shadow_picker.is_none()
                    && self.advanced.is_none()
                    && self.context.is_none()
                    && self.header_context.is_none()
                    && self.properties.is_none()
                    && self.drop_confirmation.is_none()
                    && !self.undo_busy
                    && self.confirmation.is_none()
                    && self.conflict.is_none(),
            ),
        )
    }

    fn pane(&self, id: usize, width: f32) -> Element<'_, Message> {
        pane_view::FilePane::new(self, id, width).view()
    }
}
impl Drop for App {
    fn drop(&mut self) {
        #[cfg(not(test))]
        if self.preferences.default_layout == preferences::DefaultLayout::Remember {
            let _ = session::save_now(session::Session::capture(self));
        }
        for job in &self.jobs {
            job.cancel.store(true, Ordering::Relaxed);
        }
        for pane in &self.panes {
            pane.cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_recycle_confirmation_can_be_suppressed_after_approval() {
        let mut app = super::App::new("home".into(), "other".into(), vec![]);
        let _ = app.update(super::Message::SkipConfirmation("delete".into(), true));
        assert!(app.preferences.skipped_confirmations.is_empty());
        assert!(app.pending_skip_confirmation.is_none());
        let _ = app.update(super::Message::SkipConfirmation("recycle".into(), true));
        assert_eq!(app.pending_skip_confirmation.as_deref(), Some("recycle"));
        let _ = app.update(super::Message::ApproveSkipConfirmation(false));
        assert!(app.preferences.skipped_confirmations.is_empty());
        let _ = app.update(super::Message::SkipConfirmation("recycle".into(), true));
        let _ = app.update(super::Message::ApproveSkipConfirmation(true));
        let saved = super::preferences::Preferences::decode(&app.preferences.encode());
        assert_eq!(saved.skipped_confirmations, vec!["recycle"]);
        let _ = app.update(super::Message::SkipConfirmation("recycle".into(), false));
        assert!(app.preferences.skipped_confirmations.is_empty());
    }
    use super::*;
    fn selection_app() -> App {
        let mut app = App::new(PathBuf::from("."), PathBuf::from("."), vec![]);
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Listed(Ok(vec![
                entry("a", Some(1)),
                entry("b", Some(2)),
                entry("c", Some(3)),
            ])),
        ));
        app
    }
    #[test]
    fn default_vertical_layout_fits_ten_complete_rows_per_panel() {
        use iced::advanced::{layout, widget::Tree};
        let mut app = selection_app();
        let rows: Vec<_> = (0..30)
            .map(|n| entry(&format!("File {n}"), Some(1)))
            .collect();
        for panel in &mut app.panes {
            panel.entries = rows.clone();
            panel.reorder();
        }
        assert!(!app.queue_expanded);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut view = app.workspace();
        let mut tree = Tree::new(view.as_widget());
        let size = iced::Size::new(1120.0, 900.0);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        fn collect(node: &layout::Node, out: &mut Vec<f32>) {
            let b = node.bounds();
            if node.children().is_empty() && b.width > 600.0 && b.height >= 100.0 {
                out.push(b.height);
            }
            for child in node.children() {
                collect(child, out);
            }
        }
        let mut viewports = vec![];
        collect(&node, &mut viewports);
        assert_eq!(viewports.len(), 2, "{viewports:?}");
        assert!(
            viewports
                .iter()
                .all(|h| *h >= 10.0 * virtual_list::ROW_HEIGHT),
            "{viewports:?}"
        );
    }
    #[test]
    fn ctrl_and_shift_selection_and_unified_search_target_active_panel() {
        let mut app = selection_app();
        let _ = app.update(Message::Select(0, 0));
        let _ = app.update(Message::Modifiers(iced::keyboard::Modifiers::CTRL));
        let _ = app.update(Message::Select(0, 2));
        assert_eq!(
            app.panes[0].selections.iter().copied().collect::<Vec<_>>(),
            vec![0, 2]
        );
        let _ = app.update(Message::Modifiers(iced::keyboard::Modifiers::SHIFT));
        let _ = app.update(Message::Select(0, 0));
        assert_eq!(app.panes[0].selections.len(), 3);
        app.preferences.search_subfolders = false;
        let _ = app.update(Message::Search(0, "b".into()));
        assert_eq!(
            app.panes[0].selections.iter().copied().collect::<Vec<_>>(),
            vec![1]
        );
        assert!(app.panes[1].search.is_empty());
    }
    #[test]
    fn delete_and_cut_require_confirmation_and_cancel_has_no_side_effects() {
        let mut app = selection_app();
        let _ = app.update(Message::FileAction(FileAction::SelectAll));
        for action in [
            FileAction::Cut,
            FileAction::Delete,
            FileAction::DeletePermanent,
        ] {
            let _ = app.update(Message::FileAction(action));
            assert_eq!(app.confirmation.as_ref().unwrap().paths.len(), 3);
            assert!(app.jobs.is_empty());
            assert!(app.clipboard.is_none());
            let _ = app.update(Message::Confirm(false));
            assert!(app.confirmation.is_none());
            assert!(app.jobs.is_empty());
            assert!(app.clipboard.is_none());
        }
    }
    #[test]
    fn captured_text_edit_shortcuts_do_not_operate_on_files() {
        let event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character("c".into()),
            modified_key: iced::keyboard::Key::Character("c".into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        });
        assert!(
            keyboard_message(
                event.clone(),
                iced::event::Status::Captured,
                iced::window::Id::unique()
            )
            .is_none()
        );
        assert!(matches!(
            keyboard_message(
                event,
                iced::event::Status::Ignored,
                iced::window::Id::unique()
            ),
            Some(Message::FileAction(FileAction::Copy))
        ));
    }
    #[test]
    fn background_tab_updates_and_closure_keep_results_isolated() {
        let mut app = App::new(PathBuf::from("first"), PathBuf::from("other"), vec![]);
        let mut tab = Tab::new(PathBuf::from("second"));
        tab.generation = 42;
        app.panes[0].tabs.push(tab);
        app.panes[0].selected_tab = 1;
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Listed(Ok(vec![entry("background", Some(17))])),
        ));
        assert!(app.panes[0].entries.is_empty());
        assert_eq!(app.panes[0].tabs[0].total(), 17);
        let _ = app.update(Message::Scan(
            0,
            42,
            Event::Listed(Ok(vec![entry("active", Some(31))])),
        ));
        assert_eq!(app.panes[0].total(), 31);
        let _ = app.update(Message::CloseTab(0, 0));
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Listed(Err("late result".into())),
        ));
        assert!(app.panes[0].error.is_none());
        assert_eq!(app.panes[0].path, PathBuf::from("second"));
        let _ = app.update(Message::CloseTab(0, 0));
        assert_eq!(app.panes[0].tabs.len(), 1);
    }
    #[test]
    fn modified_sort_keeps_unknown_timestamps_last() {
        let mut app = App::new(PathBuf::from("."), PathBuf::from("."), vec![]);
        let mut old = entry("old", Some(0));
        old.modified = Some(std::time::UNIX_EPOCH);
        let mut new = entry("new", Some(0));
        new.modified = Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(100));
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Listed(Ok(vec![old, new, entry("unknown", None)])),
        ));
        let _ = app.update(Message::Sort(0, Sort::Modified));
        assert_eq!(app.panes[0].order, vec![0, 1, 2]);
        let _ = app.update(Message::Sort(0, Sort::Modified));
        assert_eq!(app.panes[0].order, vec![1, 0, 2]);
    }
    fn entry(name: &str, bytes: Option<u64>) -> Entry {
        Entry {
            hidden: false,
            path: PathBuf::from(name),
            name: name.into(),
            directory: false,
            bytes,
            partial: false,
            modified: None,
            modified_label: "—".into(),
            icon: None,
        }
    }
    #[test]
    fn sorting_toggles_and_size_updates_keep_selection_identity() {
        let mut app = App::new(PathBuf::from("."), PathBuf::from("."), vec![]);
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Listed(Ok(vec![
                entry("z", Some(5)),
                entry("a", Some(20)),
                entry("pending", None),
            ])),
        ));
        assert_eq!(app.panes[0].order, vec![1, 2, 0]);
        let _ = app.update(Message::Select(0, 0));
        let _ = app.update(Message::Sort(0, Sort::Size));
        assert_eq!(app.panes[0].order, vec![0, 1, 2]);
        let _ = app.update(Message::Sort(0, Sort::Size));
        assert_eq!(app.panes[0].order, vec![1, 0, 2]);
        let _ = app.update(Message::Scan(0, 0, Event::Sizes(vec![(2, 30, false)])));
        assert_eq!(app.panes[0].order, vec![2, 1, 0]);
        assert_eq!(app.panes[0].selected, Some(0));
        assert_eq!(app.panes[0].entries[0].name, "z");
        app.preferences.search_subfolders = false;
        let _ = app.update(Message::Search(0, "PEND".into()));
        assert_eq!(app.panes[0].order, vec![2]);
        assert!(app.panes[1].order.is_empty());
    }
    #[test]
    fn stale_results_do_not_replace_a_pane() {
        let path = PathBuf::from(".");
        let mut app = App::new(path.clone(), path, vec![]);
        app.panes[0].generation = 2;
        let _ = app.update(Message::Scan(0, 1, Event::Listed(Err("old error".into()))));
        assert!(app.panes[0].error.is_none());
        let _ = app.update(Message::Search(0, "left only".into()));
        assert!(app.panes[1].search.is_empty());
    }
}

#[cfg(test)]
mod workspace_tests {
    use super::*;
    #[test]
    fn tab_moves_preserve_identity_and_route_scan_results() {
        let mut app = App::new(
            PathBuf::from("source-test"),
            PathBuf::from("dest-test"),
            vec![],
        );
        app.pane_count = 4;
        app.rebuild_file_layout();
        assert_eq!(app.files_layout.len(), 4);
        let identity = app.panes[0].identity;
        let generation = u64::MAX - 7;
        app.panes[0].generation = generation;
        app.panes[0].search = "needle".into();
        let _ = app.update(Message::MoveTab(0, identity, 3, 1));
        assert_eq!(app.active, 2);
        assert_eq!(app.pane_count, 3);
        assert_eq!(app.panes[2].identity, identity);
        assert_eq!(app.panes[2].search, "needle");
        assert_eq!(app.panes[0].tabs.len(), 1);
        assert_ne!(app.panes[0].identity, identity);
        let _ = app.update(Message::Scan(
            0,
            generation,
            Event::Listed(Err("moved scan".into())),
        ));
        assert_eq!(app.panes[2].error.as_deref(), Some("moved scan"));
        assert!(app.panes[0].error.is_none());
    }
    #[test]
    fn tab_reorder_and_default_restore_keep_directory_state() {
        let mut app = App::new(PathBuf::from("a"), PathBuf::from("b"), vec![]);
        app.panes[0].tabs.push(Tab::new(PathBuf::from("c")));
        app.panes[0].tabs.push(Tab::new(PathBuf::from("d")));
        let identity = app.panes[0].identity;
        let _ = app.update(Message::MoveTab(0, identity, 0, 3));
        assert_eq!(app.panes[0].tabs[2].identity, identity);
        assert_eq!(app.panes[0].selected_tab, 2);
        app.preferences.admin = true;
        let _ = app.update(Message::ThemeChoice(preferences::ThemeMode::Dark));
        let _ = app.update(Message::SystemTheme(Some(false)));
        assert!(app.dark);
        let _ = app.update(Message::RestoreDefaults);
        assert!(!app.dark);
        assert!(!app.preferences.admin);
        assert_eq!(app.preferences.sidebar, 0.15);
        assert_eq!(app.panes[0].path, PathBuf::from("a"));
    }
}

#[cfg(test)]
mod ui_regression_tests {
    use super::*;
    #[test]
    fn cancelled_elevation_retains_current_session() {
        let mut app = App::new(PathBuf::from("keep"), PathBuf::from("other"), vec![]);
        app.elevating = true;
        let _ = app.update(Message::AdminRelaunched(Err("Cancelled".into())));
        assert!(!app.elevating);
        assert_eq!(app.panes[0].path, PathBuf::from("keep"));
        assert_eq!(app.notice, "Cancelled");
    }
}

#[cfg(test)]
mod interaction_sort_tests {
    use super::*;
    #[test]
    fn type_is_default_with_directories_first() {
        let mut tab = Tab::new(PathBuf::from("."));
        for (name, directory) in [
            ("a.txt", false),
            ("z-folder", true),
            ("b-folder", true),
            ("z.txt", false),
        ] {
            tab.entries.push(Entry {
                hidden: false,
                path: PathBuf::from(name),
                name: name.into(),
                directory,
                bytes: Some(0),
                partial: false,
                modified: None,
                modified_label: String::new(),
                icon: None,
            });
        }
        tab.reorder();
        assert_eq!(tab.sort, Sort::Type);
        assert_eq!(tab.order, vec![2, 1, 0, 3]);
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;

    #[test]
    fn narrow_panel_horizontal_scroll_preserves_header_sort_and_resize_coordinates() {
        use iced::advanced::{Layout, Shell, layout, widget::Tree};
        let app = App::new(PathBuf::from("home"), PathBuf::from("project"), vec![]);
        let mut element = app.pane(0, 320.0);
        let mut tree = Tree::new(&element);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = iced::Size::new(320.0, 400.0);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = vec![];
        let mut send = |event, point| {
            element.as_widget_mut().update(
                &mut tree,
                &iced::Event::Mouse(event),
                Layout::new(&node),
                iced::mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            )
        };
        send(
            iced::mouse::Event::WheelScrolled {
                delta: iced::mouse::ScrollDelta::Lines { x: -100.0, y: 0.0 },
            },
            iced::Point::new(150.0, 82.0),
        );
        for event in [
            iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
        ] {
            send(event, iced::Point::new(220.0, 82.0));
        }
        send(
            iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
            iced::Point::new(149.0, 82.0),
        );
        send(
            iced::mouse::Event::CursorMoved {
                position: iced::Point::new(139.0, 82.0),
            },
            iced::Point::new(139.0, 82.0),
        );
        send(
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
            iced::Point::new(139.0, 82.0),
        );
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::Sort(0, Sort::Share))),
            "{messages:?}"
        );
        assert!(messages.iter().any(|message|matches!(message,Message::ResizeColumn(0,3,width) if (*width-154.0).abs()<0.01)),"{messages:?}");
    }
    #[test]
    fn four_side_by_side_panels_render_all_columns_and_compact_tabs() {
        use iced::advanced::renderer::Headless as _;
        use iced::advanced::{Layout, Renderer as _, layout, widget::Tree};
        let mut app = App::new(PathBuf::from("Home"), PathBuf::from("Project"), vec![]);
        app.pane_count = 4;
        app.stacked = false;
        app.rebuild_file_layout();
        for pane in &mut app.panes {
            for i in 0..4 {
                pane.tabs
                    .push(Tab::new(PathBuf::from(format!("Folder {i}"))));
            }
            for i in 0..40 {
                pane.entries.push(Entry {
                    hidden: false,
                    path: PathBuf::from(format!("Item {i}")),
                    name: format!("Example folder {i}"),
                    directory: true,
                    bytes: Some(4096),
                    partial: false,
                    modified: None,
                    modified_label: "2026-10-22 12:30".into(),
                    icon: None,
                });
            }
            pane.reorder();
        }
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut element = app.view();
        let mut tree = Tree::new(&element);
        let size = iced::Size::new(1400.0, 900.0);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
            let bounds = iced::Rectangle::with_size(size);
            renderer.reset(bounds);
            element.as_widget().draw(
                &tree,
                &mut renderer,
                &app.app_theme,
                &iced::advanced::renderer::Style::default(),
                Layout::new(&node),
                iced::mouse::Cursor::Unavailable,
                &bounds,
            );
            std::fs::write(
                "target/four-horizontal.rgba",
                renderer.screenshot(iced::Size::new(1400, 900), 1.0, iced::Color::BLACK),
            )
            .unwrap();
        }
    }
    #[test]
    fn closing_last_tab_compacts_panels_and_routes_surviving_scan_results() {
        let mut app = App::new(PathBuf::from("left"), PathBuf::from("right"), vec![]);
        app.pane_count = 4;
        app.grid = true;
        app.stacked = false;

        app.rebuild_file_layout();
        for (id, panel) in app.panes.iter_mut().enumerate() {
            panel.tabs[0].generation = 100 + id as u64;
        }
        let identities = app.panes.iter().map(|p| p.identity).collect::<Vec<_>>();
        let cancel = app.panes[1].cancel.clone();
        app.active = 3;
        let _ = app.update(Message::CloseTab(1, 0));
        assert_eq!(app.pane_count, 3);
        assert!(app.grid);

        assert_eq!(app.active, 2);
        assert!(cancel.load(Ordering::Relaxed));
        assert_eq!(
            (
                app.panes[0].identity,
                app.panes[1].identity,
                app.panes[2].identity
            ),
            (identities[0], identities[2], identities[3])
        );
        let _ = app.update(Message::Scan(
            2,
            102,
            Event::Listed(Err("surviving scan".into())),
        ));
        assert_eq!(app.panes[1].error.as_deref(), Some("surviving scan"));
        assert!(app.panes[2].error.is_none());
        let _ = app.update(Message::CloseTab(0, 0));
        assert_eq!(app.pane_count, 2);
        assert!(!app.grid);
        assert!(!app.stacked);
        assert_eq!(app.active, 1);
        assert_eq!(app.panes[1].identity, identities[3]);
        let _ = app.update(Message::CloseTab(0, 0));
        assert_eq!(app.pane_count, 1);
        assert_eq!(app.panes[0].identity, identities[3]);
        assert_eq!(app.files_layout.len(), 1);
        let _ = app.update(Message::CloseTab(0, 0));
        assert_eq!(app.pane_count, 1);
        assert_eq!(app.panes[0].identity, identities[3]);
    }
    #[test]
    fn closing_one_of_multiple_tabs_keeps_panel_count_and_selected_tab() {
        let mut app = App::new(PathBuf::from("a"), PathBuf::from("b"), vec![]);
        app.panes[0].tabs.push(Tab::new(PathBuf::from("second")));
        app.panes[0].selected_tab = 1;
        let survivor = app.panes[0].identity;
        let _ = app.update(Message::CloseTab(0, 0));
        assert_eq!(app.pane_count, 2);
        assert_eq!(app.panes[0].identity, survivor);
        assert_eq!(app.panes[0].selected_tab, 0);
    }
    #[test]
    fn ribbon_file_actions_share_one_row() {
        use iced::advanced::{layout, widget::Tree};
        let app = App::new(PathBuf::from("."), PathBuf::from("."), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut view = app.ribbon_controls(1190.0);
        let mut tree = Tree::new(&view);
        let size = iced::Size::new(1190., 28.);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let files = &node.children()[0];
        assert_eq!(files.children().len(), 7);
        assert_eq!(files.bounds().height, RIBBON_HEIGHT);
        for control in files.children() {
            assert_eq!(control.bounds().height, RIBBON_HEIGHT);
            assert_eq!(control.bounds().width, FILE_ACTION_WIDTH);
        }
    }
    #[test]
    fn sidebar_and_workspace_gutters_share_one_flat_background() {
        use iced::advanced::renderer::Headless as _;
        use iced::advanced::{Renderer as _, layout, renderer, widget::Tree};
        let app = App::new(PathBuf::from("."), PathBuf::from("."), vec![]);
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut view = app.view();
        let mut tree = Tree::new(&view);
        let size = iced::Size::new(1400., 900.);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let bounds = node.bounds();
        renderer.reset(bounds);
        view.as_widget().draw(
            &tree,
            &mut renderer,
            &app.app_theme,
            &renderer::Style::default(),
            iced::advanced::Layout::new(&node),
            iced::mouse::Cursor::Unavailable,
            &bounds,
        );
        let pixels = renderer.screenshot(iced::Size::new(1400, 900), 1., iced::Color::BLACK);
        let expected =
            theme::color(palette::themed(true, Default::default()).navigation_background);
        let expected = [
            (expected.r * 255.).round() as u8,
            (expected.g * 255.).round() as u8,
            (expected.b * 255.).round() as u8,
            255,
        ];
        for (x, y) in [(211, 80), (218, 200), (222, 500), (1396, 850), (1000, 895)] {
            assert_eq!(
                &pixels[(y * 1400 + x) * 4..(y * 1400 + x) * 4 + 4],
                &expected,
                "background strip at {x},{y}"
            );
        }
    }
}

#[cfg(test)]
mod final_ui_checks {
    use super::*;
    use iced::advanced::renderer::Headless as _;
    use iced::advanced::{Layout, Renderer as _, Shell, layout, renderer, widget::Tree};
    pub(crate) fn render(app: &App, size: iced::Size<u32>, point: Option<iced::Point>) -> Vec<u8> {
        render_view(app.view(), app, size, point)
    }
    pub(crate) fn render_view(
        mut view: Element<'_, Message>,
        app: &App,
        size: iced::Size<u32>,
        point: Option<iced::Point>,
    ) -> Vec<u8> {
        let mut tree = Tree::new(&view);
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let logical = iced::Size::new(size.width as f32, size.height as f32);
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(logical, logical),
        );
        let cursor = point.map_or(
            iced::mouse::Cursor::Unavailable,
            iced::mouse::Cursor::Available,
        );
        let mut messages = vec![];
        if let Some(point) = point {
            view.as_widget_mut().update(
                &mut tree,
                &iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: point }),
                Layout::new(&node),
                cursor,
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            );
        }
        view.as_widget_mut().update(
            &mut tree,
            &iced::Event::Window(iced::window::Event::RedrawRequested(
                iced::time::Instant::now(),
            )),
            Layout::new(&node),
            cursor,
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut messages),
            &node.bounds(),
        );
        renderer.reset(node.bounds());
        view.as_widget().draw(
            &tree,
            &mut renderer,
            &app.app_theme,
            &renderer::Style::default(),
            Layout::new(&node),
            cursor,
            &node.bounds(),
        );
        renderer.screenshot(size, 1., app.app_theme.palette().background)
    }
    #[test]
    fn narrow_ribbon_keeps_settings_visible_without_scrolling() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for elevated in [false, true] {
            for width in [300.0, 480.0, 700.0, 900.0] {
                app.elevated = elevated;
                let mut view = app.workspace();
                let mut tree = Tree::new(&view);
                let size = iced::Size::new(width, 700.0);
                let node = view.as_widget_mut().layout(
                    &mut tree,
                    &renderer,
                    &layout::Limits::new(size, size),
                );
                let compact = width < app.single_row_required_width(&renderer);
                let point = iced::Point::new(
                    width - if compact { 26.0 } else { 50.0 },
                    if compact {
                        if elevated { 36.0 } else { 52.0 }
                    } else {
                        20.0
                    },
                );
                let mut messages = vec![];
                for event in [
                    iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
                    iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
                ] {
                    view.as_widget_mut().update(
                        &mut tree,
                        &iced::Event::Mouse(event),
                        Layout::new(&node),
                        iced::mouse::Cursor::Available(point),
                        &renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut Shell::new(&mut messages),
                        &node.bounds(),
                    );
                }
                assert!(
                    messages
                        .iter()
                        .any(|m| matches!(m, Message::SettingsToggle)),
                    "compact={compact} elevated={elevated} width={width}: {messages:?}"
                );
            }
        }
    }
    #[test]
    fn transfer_header_highlight_covers_the_full_card_width() {
        let app = App::new("home".into(), "other".into(), vec![]);
        let mut view = app.transfer_queue();
        let mut tree = Tree::new(&view);
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = iced::Size::new(1000., 40.);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = vec![];
        let mut paint = |point: Option<iced::Point>| {
            let cursor = point.map_or(
                iced::mouse::Cursor::Unavailable,
                iced::mouse::Cursor::Available,
            );
            if let Some(p) = point {
                view.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: p }),
                    Layout::new(&node),
                    cursor,
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &node.bounds(),
                );
            }
            view.as_widget_mut().update(
                &mut tree,
                &iced::Event::Window(iced::window::Event::RedrawRequested(
                    iced::time::Instant::now(),
                )),
                Layout::new(&node),
                cursor,
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            );
            renderer.reset(node.bounds());
            view.as_widget().draw(
                &tree,
                &mut renderer,
                &app.app_theme,
                &renderer::Style::default(),
                Layout::new(&node),
                cursor,
                &node.bounds(),
            );
            renderer.screenshot(
                iced::Size::new(1000, 40),
                1.,
                app.app_theme.palette().background,
            )
        };
        let idle = paint(None);
        let hover = paint(Some(iced::Point::new(500., 10.)));
        for x in [2, 8, 990, 997] {
            let i = (20 * 1000 + x) * 4;
            assert_ne!(&idle[i..i + 3], &hover[i..i + 3], "x {x}");
        }
    }
    #[test]
    fn compact_window_and_accent_changes_render_in_both_modes() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        for mode in [preferences::ThemeMode::Light, preferences::ThemeMode::Dark] {
            let _ = app.update(Message::ThemeChoice(mode));
            let before = render(&app, iced::Size::new(480, 480), None);
            let accent = palette::Accent::from_key("Purple");
            let _ = app.update(Message::AccentChoice(accent));
            let after = render(&app, iced::Size::new(480, 480), None);
            assert_eq!(app.preferences.accent, accent);
            assert_eq!(
                app.app_theme.palette().primary,
                theme::color(palette::themed(app.dark, accent).accent)
            );
            assert!(before != after, "accent should update the compact window");
            let settings = after;
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                tiny_skia::Pixmap::from_vec(
                    settings,
                    tiny_skia::IntSize::from_wh(480, 480).unwrap(),
                )
                .unwrap()
                .save_png(format!(
                    "target/compact-{}.png",
                    if app.dark { "dark" } else { "light" }
                ))
                .unwrap();
            }
            app.settings_open = false;
            let _ = app.update(Message::AccentChoice(Default::default()));
        }
    }
    #[test]
    fn capture_collapsed_sidebar_and_file_tree_drawer() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        for (name, open) in [("sidebar-collapsed", false), ("sidebar-drawer", true)] {
            app.sidebar_drawer = open;
            let pixels = render(&app, iced::Size::new(480, 480), None);
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(480, 480).unwrap())
                    .unwrap()
                    .save_png(format!("target/{name}.png"))
                    .unwrap();
            }
        }
    }
    #[test]
    fn capture_both_ribbon_layouts_and_shortcut_formatting() {
        let app = App::new("home".into(), "other".into(), vec![]);
        for (width, key) in [(900, "compact"), (1400, "single-row")] {
            let pixels = render(&app, iced::Size::new(width, 700), None);
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                tiny_skia::Pixmap::from_vec(
                    pixels,
                    tiny_skia::IntSize::from_wh(width, 700).unwrap(),
                )
                .unwrap()
                .save_png(format!("target/ribbon-{key}.png"))
                .unwrap();
            }
        }
        let t = palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        );
        let pixels = render_view(
            container(
                container(components::shortcut_hint("Copy"))
                    .padding(7)
                    .style(move |_| t.card(false)),
            )
            .center(Fill)
            .into(),
            &app,
            iced::Size::new(200, 60),
            None,
        );
        if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
            tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(200, 60).unwrap())
                .unwrap()
                .save_png("target/shortcut-tooltip.png")
                .unwrap();
        }
    }
    #[test]
    fn workflow_dialog_and_context_menu_render_in_compact_windows() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        for dark in [false, true] {
            let _ = app.update(Message::ThemeChoice(if dark {
                preferences::ThemeMode::Dark
            } else {
                preferences::ThemeMode::Light
            }));
            let _ = app.update(Message::FileAction(FileAction::NewFolder));
            let pixels = render(&app, iced::Size::new(480, 480), None);
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(480, 480).unwrap())
                    .unwrap()
                    .save_png(format!(
                        "target/new-folder-{}.png",
                        if dark { "dark" } else { "light" }
                    ))
                    .unwrap();
            }
            let _ = app.update(Message::Confirm(false));
            app.context = Some(iced::Point::new(300., 100.));
            let pixels = render_view(
                container(app.file_context_menu()).center(Fill).into(),
                &app,
                iced::Size::new(700, 700),
                None,
            );
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(700, 700).unwrap())
                    .unwrap()
                    .save_png(format!(
                        "target/workflow-menu-{}.png",
                        if dark { "dark" } else { "light" }
                    ))
                    .unwrap();
            }
            app.context = None;
        }
    }
    #[test]
    fn advanced_search_and_panel_trees_render_distinct_views() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        for panel in &mut app.panes {
            panel.entries = (0..60)
                .map(|i| Entry {
                    hidden: i % 5 == 0,
                    path: PathBuf::from(format!("file-{i}.txt")),
                    name: format!("file-{i}.txt"),
                    directory: false,
                    bytes: Some(2048),
                    partial: false,
                    modified: None,
                    modified_label: "2026-10-23 12:00".into(),
                    icon: None,
                })
                .collect();
            panel.total_bytes = panel.entries.iter().filter_map(|e| e.bytes).sum();
            panel.reorder();
        }
        let normal = render(&app, iced::Size::new(1400, 900), None);
        app.preferences.panel_trees = true;
        let panel_trees = render(&app, iced::Size::new(1400, 900), None);
        assert_ne!(normal, panel_trees);
        app.preferences.panel_trees = false;
        let _ = app.update(Message::AdvancedToggle);
        let advanced = render(&app, iced::Size::new(1400, 900), None);
        assert_ne!(normal, advanced);
    }
    #[test]
    fn column_changes_and_partial_sizes_update_the_rendered_view_before_finished() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let entry = Entry {
            path: "home/folder".into(),
            name: "folder".into(),
            directory: true,
            hidden: false,
            bytes: None,
            partial: false,
            modified: None,
            modified_label: "2026-10-23 12:00".into(),
            icon: None,
        };
        let _ = app.update(Message::Scan(0, 0, Event::Listed(Ok(vec![entry]))));
        app.panes[0].busy = true;
        let identity = app.panes[0].view_identity;
        let before = render(&app, iced::Size::new(1000, 700), None);
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Sizes(vec![(0, 1_048_576, true)]),
        ));
        assert!(app.panes[0].busy);
        assert_eq!(app.panes[0].total_bytes, 1_048_576);
        assert_eq!(app.panes[0].incomplete_entries, 1);
        assert_eq!(
            app.panes[0].view_identity, identity,
            "incremental results must preserve scroll identity"
        );
        let partial = render(&app, iced::Size::new(1000, 700), None);
        assert_ne!(before, partial);
        let _ = app.update(Message::Scan(
            0,
            0,
            Event::Sizes(vec![(0, 2_097_152, false)]),
        ));
        assert_eq!(app.panes[0].total_bytes, 2_097_152);
        assert_eq!(app.panes[0].incomplete_entries, 0);
        assert!(app.panes[0].busy);
        let _ = app.update(Message::Scan(0, 0, Event::Finished));
        assert!(!app.panes[0].busy);
        for column in columns::Column::OPTIONAL
            .into_iter()
            .filter(|c| !columns::Column::EXTRA.contains(c))
        {
            let _ = app.update(Message::ColumnToggle(column));
        }
        assert_eq!(
            app.preferences.columns.visible(),
            vec![columns::Column::Type, columns::Column::Name]
        );
        let hidden = render(&app, iced::Size::new(1000, 700), None);
        assert_ne!(hidden, partial);
        let _ = app.update(Message::ColumnMove(
            columns::Column::Name,
            columns::Column::Type,
        ));
        assert_eq!(
            app.preferences.columns.visible(),
            vec![columns::Column::Name, columns::Column::Type]
        );
        let reordered = render(&app, iced::Size::new(1000, 700), None);
        assert_ne!(hidden, reordered);
        if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
            std::fs::write("target/custom-columns-minimum.rgba", reordered).unwrap();
        }
    }
}

#[cfg(test)]
mod visual_theme_checks {
    use super::*;
    #[test]
    fn switching_looks_preserves_tabs_selection_scans_and_policy() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let identities: Vec<_> = app
            .panes
            .iter()
            .map(|p| {
                (
                    p.tabs.iter().map(|t| t.identity).collect::<Vec<_>>(),
                    p.selected_tab,
                    p.cancel.clone(),
                )
            })
            .collect();
        let policy = app.motion;
        for look in preferences::Look::ALL {
            app.preferences.look = look;
            let _ = app.apply_appearance();
            for (pane, (tabs, selected, scan)) in app.panes.iter().zip(identities.clone()) {
                assert_eq!(
                    pane.tabs.iter().map(|t| t.identity).collect::<Vec<_>>(),
                    tabs
                );
                assert_eq!(pane.selected_tab, selected);
                assert!(Arc::ptr_eq(&pane.cancel, &scan));
            }
            assert_eq!(app.motion, policy);
            assert_eq!(
                palette::from_theme(&app.app_theme).visual,
                palette::styled(app.dark, app.preferences.accent, app.preferences.tint, look)
                    .visual
            );
            let encoded = app.preferences.encode();
            assert_eq!(preferences::Preferences::decode(&encoded).look, look);
            assert_eq!(
                preferences::Preferences::decode("look=unknown\n").look,
                preferences::Look::Default
            );
        }
    }
}

#[cfg(test)]
mod shadow_paint_checks;

#[cfg(test)]
mod scroll_performance_checks;

#[cfg(test)]
mod requested_search_tree_tests {
    use super::*;
    #[test]
    fn subfolder_preference_defaults_on_and_controls_name_searches() {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        assert!(app.preferences.search_subfolders);
        assert!(
            !app.panes[0].filters.recursive,
            "Browsing must not scan the entire subtree"
        );
        let _ = app.update(Message::Search(0, "report".into()));
        assert!(app.panes[0].filters.recursive);
        let _ = app.update(Message::SearchSubfolders(false));
        assert!(!app.preferences.search_subfolders);
        assert!(!app.panes[0].filters.recursive);
        assert!(!preferences::Preferences::decode(&app.preferences.encode()).search_subfolders);
        let _ = app.update(Message::SearchSubfolders(true));
        assert!(app.panes[0].filters.recursive);
        let _ = app.update(Message::Search(0, String::new()));
        assert!(!app.panes[0].filters.recursive);
    }
    #[test]
    fn search_ignores_quotes_and_rejects_results_after_editing() {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        let _ = app.update(Message::Search(0, "\"report.txt\"".into()));
        assert_eq!(app.panes[0].search, "report.txt");
        let identity = app.panes[0].identity;
        app.panes[0].address_request = Some(77);
        let _ = app.update(Message::Search(0, "other".into()));
        let _ = app.update(Message::SearchPathValidated(
            identity,
            77,
            Ok("wrong".into()),
        ));
        assert_eq!(app.panes[0].path, PathBuf::from("left"));
        assert_eq!(app.panes[0].search, "other");
    }
    #[test]
    fn background_tree_updates_do_not_steal_the_active_panel() {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        assert!(!app.preferences.panel_trees);
        app.preferences.panel_trees = true;
        app.active = 1;
        let _ = app.update(Message::PanelLocations(
            0,
            locations::Message::DriveLabels(0, vec![]),
        ));
        assert_eq!(app.active, 1);
        let _ = app.update(Message::PanelLocations(
            0,
            locations::Message::Navigate("folder".into()),
        ));
        assert_eq!(app.panes[0].path, PathBuf::from("folder"));
        assert_eq!(app.panes[1].path, PathBuf::from("right"));
    }
}

#[cfg(test)]
mod readme_captures;
