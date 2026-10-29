use super::*;
use iced::widget::{column, row};
pub struct DraggedFiles {
    pub(crate) pane: usize,
    pub(crate) index: usize,
    pub(crate) paths: Vec<PathBuf>,
}
fn background<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    map: impl Fn(T) -> Message + Send + 'static,
) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    Task::perform(async move { rx.await.ok() }, move |value| {
        value.map(&map).unwrap_or(Message::Noop)
    })
}
pub fn history_task(generation: u64) -> Task<Message> {
    background(undo::latest, move |entry| {
        Message::HistoryLoaded(generation, entry)
    })
}
impl App {
    pub fn open_entry(&mut self, panel: usize, index: usize, choose_app: bool) -> Task<Message> {
        let Some(entry) = self.panes.get(panel).and_then(|p| p.entries.get(index)) else {
            return Task::none();
        };
        if entry.directory {
            return if choose_app {
                Task::none()
            } else {
                self.update(Message::Navigate(panel, entry.path.clone()))
            };
        }
        if !self.preferences.open_files {
            self.notice = "File opening is disabled. Enable Allow opening files in Settings to open user data.".into();
            return Task::none();
        }
        if choose_app {
            desktop::open_with_task(entry.path.clone())
        } else {
            desktop::open_task(entry.path.clone())
        }
    }
    pub fn open_context(
        &mut self,
        pane: usize,
        index: Option<usize>,
        point: iced::Point,
    ) -> Task<Message> {
        if pane >= self.pane_count {
            return Task::none();
        }
        self.focus_files();
        self.active = pane;
        self.header_context = None;
        let p = &mut self.panes[pane];
        if let Some(index) = index {
            if index >= p.entries.len() {
                return Task::none();
            }
            if !p.selections.contains(&index) {
                p.selections.clear();
                p.selections.insert(index);
            }
            p.selected = Some(index);
        } else {
            p.selections.clear();
            p.selected = None;
        }
        self.context = Some(point);
        self.settings_open = false;
        Task::none()
    }
    pub fn begin_file_drag(&mut self, pane: usize, index: usize) {
        self.dont_ask_again = false;
        if pane >= self.pane_count {
            return;
        }
        self.active = pane;
        let p = &mut self.panes[pane];
        if index >= p.entries.len() {
            return;
        }
        if !p.selections.contains(&index) {
            p.selections.clear();
            p.selections.insert(index);
            p.selected = Some(index);
        }
        let paths = p
            .order
            .iter()
            .filter(|i| p.selections.contains(i))
            .map(|i| p.entries[*i].path.clone())
            .collect::<Vec<_>>();
        self.drag_files = Some(DraggedFiles { pane, index, paths });
    }
    pub fn drop_files(&mut self, pane: usize, index: usize, destination: usize) {
        let Some(drag) = self.drag_files.take() else {
            return;
        };
        if drag.pane != pane
            || drag.index != index
            || pane == destination
            || destination >= self.pane_count
            || drag.paths.is_empty()
        {
            return;
        }
        let target = self.panes[destination].path.clone();
        self.active = destination;
        self.highlights.flash(&target, self.motion);
        self.drop_confirmation = Some(transfers::Request {
            kind: transfers::Kind::Copy,
            sources: drag.paths,
            destination: Some(target),
        });
    }
    pub fn undo_operation(&mut self) -> Task<Message> {
        self.context = None;
        if self.undo_busy || self.jobs.iter().any(|j| j.report.is_none()) {
            self.notice = "Finish or cancel queued transfers before Undo.".into();
            return Task::none();
        }
        let Some(entry) = self.undo_entry.take() else {
            return Task::none();
        };
        self.undo_rename = entry.actions.iter().find_map(|(_, a)| {
            if let undo::Action::RenameBack { from, to, .. } = a {
                Some((from.clone(), to.clone()))
            } else {
                None
            }
        });
        self.notice = format!("Undoing {}…", entry.label);
        self.undo_busy = true;
        self.history_generation = next_identity();
        self.drag_files = None;
        background(
            move || {
                let (paths, result) = undo::run(entry);
                (paths, result, undo::latest())
            },
            |(p, r, e)| Message::UndoFinished(p, r, e),
        )
    }
    pub fn refresh_affected(&mut self, paths: &[PathBuf]) -> Task<Message> {
        let mut tasks = vec![];
        for (id, panel) in self.panes.iter_mut().enumerate().take(self.pane_count) {
            for tab in &mut panel.tabs {
                if paths.iter().any(|p| path_utils::related(p, &tab.path)) {
                    tasks.push(tab.start_refresh(id, true, self.preferences.scan_mode));
                }
            }
        }
        Task::batch(tasks)
    }
    pub fn show_properties(&mut self) -> Task<Message> {
        let pane = &self.panes[self.active];
        let entries = pane
            .order
            .iter()
            .filter(|i| pane.selections.contains(i))
            .map(|i| pane.entries[*i].clone())
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Task::none();
        }
        let request = next_identity();
        self.properties_request = Some(request);
        self.properties = None;
        let units = self.preferences.size_units;
        background(
            move || properties_result_with_units(entries, crate::native_properties::show, units),
            move |value| Message::PropertiesLoaded(request, value),
        )
    }
    pub fn interaction_layer<'a>(&'a self, base: Element<'a, Message>) -> Element<'a, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        if let Some(value) = &self.properties {
            let dialog = container(
                sk_iced::components::DialogContent::new(
                    sk_iced::components::scroll_area(text(value).size(13)).height(340),
                )
                .title(text("Properties").size(22))
                .actions(row![
                    Space::new().width(Fill),
                    compact_action(self.motion, "Close", Some(Message::CloseProperties), true)
                ])
                .spacing(18),
            )
            .width(660)
            .padding(24)
            .style(move |_| t.card(false));
            return sk_iced::components::Dialog::custom(
                base,
                container(dialog).center(Fill).style(|_| container::Style {
                    background: Some(iced::Color::from_rgba(0., 0., 0., 0.55).into()),
                    ..Default::default()
                }),
            )
            .into();
        }
        if let Some(point) = self.header_context {
            let mut choices = column![text("Columns").size(13)].spacing(8);
            for column in columns::Column::OPTIONAL {
                if column == columns::Column::Created {
                    choices = choices.push(container(Space::new()).height(1).width(Fill).style(
                        move |_| container::Style {
                            background: Some(theme::color(t.border).into()),
                            ..Default::default()
                        },
                    ));
                }
                choices = choices.push(
                    crate::typography::checkbox(!self.preferences.columns.hidden.contains(&column))
                        .label(column.label())
                        .text_size(13)
                        .size(15)
                        .style(t.check())
                        .on_toggle(move |_| Message::ColumnToggle(column)),
                );
            }
            let menu = sk_iced::components::Menu::new(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                self.motion,
            )
            .content(choices)
            .clip(false)
            .padding(12)
            .width(210)
            .style(move |_| t.card(false));
            return context_popup::context_popup(base, menu, Some(point));
        }
        context_popup::context_popup(base, self.file_context_menu(), self.context)
    }
    pub fn file_context_menu(&self) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let panel = &self.panes[self.active];
        let selected = !panel.selections.is_empty();
        let entry = panel.selected.and_then(|i| panel.entries.get(i));
        let folder = entry.is_some_and(|entry| entry.directory);
        let item = |label, icon, action, danger, enabled| {
            crate::components::quiet_button(
                column![
                    ui_icons::material(icon, danger, enabled),
                    text(label).size(11)
                ]
                .spacing(4)
                .align_x(iced::Center)
                .width(Fill),
            )
            .width(Fill)
            .height(52)
            .padding([6, 4])
            .style(t.flat_dimmed())
            .on_press_maybe(enabled.then_some(Message::ContextAction(Some(action))))
        };
        let option = |label: &'static str, icon, message: Option<Message>| {
            let enabled = message.is_some();
            sk_iced::components::MenuItem::new(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                self.motion,
                label,
            )
            .icon(ui_icons::material(icon, false, enabled))
            .on_press_maybe(message)
            .feedback(false)
        };
        let mut choices = column![
            row![
                item("Cut", ui_icons::Icon::Cut, FileAction::Cut, false, selected),
                item(
                    "Copy",
                    ui_icons::Icon::Copy,
                    FileAction::Copy,
                    false,
                    selected
                ),
                item(
                    "Paste",
                    ui_icons::Icon::Paste,
                    FileAction::Paste,
                    false,
                    true
                ),
                item(
                    "Rename",
                    ui_icons::Icon::Rename,
                    FileAction::Rename,
                    false,
                    panel.selections.len() == 1
                ),
                item(
                    "Delete",
                    ui_icons::Icon::Delete,
                    FileAction::Delete,
                    false,
                    selected
                ),
            ]
            .spacing(4),
            iced::widget::rule::horizontal(1),
            option(
                if folder { "Open folder" } else { "Open file" },
                if folder {
                    ui_icons::Icon::OpenFolder
                } else {
                    ui_icons::Icon::OpenFile
                },
                (entry.is_some() && (folder || self.preferences.open_files))
                    .then_some(Message::OpenContextEntry)
            ),
        ]
        .spacing(2);
        if entry.is_some() && !folder {
            choices = choices.push(option(
                "Open file with…",
                ui_icons::Icon::OpenWith,
                self.preferences
                    .open_files
                    .then_some(Message::OpenContextWith),
            ));
        }
        choices = choices
            .push(option(
                "New folder",
                ui_icons::Icon::NewFolder,
                Some(Message::ContextAction(Some(FileAction::NewFolder))),
            ))
            .push(option(
                "Copy path",
                ui_icons::Icon::CopyPath,
                Some(Message::CopyPath),
            ))
            .push(option(
                "Open in terminal",
                ui_icons::Icon::Terminal,
                Some(Message::OpenTerminal),
            ));
        container(
            choices
                .push(iced::widget::rule::horizontal(1))
                .push(option(
                    "Reveal in File tree",
                    ui_icons::Icon::Reveal,
                    Some(Message::RevealLocation),
                ))
                .push(option(
                    "Properties",
                    ui_icons::Icon::Properties,
                    selected.then_some(Message::ContextAction(None)),
                )),
        )
        .padding(5)
        .width(300)
        .style(move |_| t.card(false))
        .into()
    }
}

#[cfg(test)]
fn properties_result(
    entries: Vec<Entry>,
    native: impl FnOnce(&[PathBuf]) -> Result<(), String>,
) -> Option<String> {
    properties_result_with_units(entries, native, preferences::SizeUnits::Binary)
}
fn properties_result_with_units(
    entries: Vec<Entry>,
    native: impl FnOnce(&[PathBuf]) -> Result<(), String>,
    units: preferences::SizeUnits,
) -> Option<String> {
    let paths = entries.iter().map(|e| e.path.clone()).collect::<Vec<_>>();
    let error = match native(&paths) {
        Ok(()) => return None,
        Err(error) => error,
    };
    let mut lines = vec![format!(
        "Windows properties unavailable: {error}\nShowing File Beholder properties instead."
    )];
    for e in entries {
        let mut line = format!(
            "{}\nLocation: {}\nType: {}\nSize: {}{}\nModified: {}",
            e.name,
            e.path.display(),
            if e.directory { "Folder" } else { "File" },
            e.bytes
                .map(|n| units.format(n))
                .unwrap_or_else(|| "Not calculated".into()),
            if e.partial { " (partial)" } else { "" },
            e.modified_label
        );
        match std::fs::metadata(&e.path) {
            Ok(meta) => line.push_str(&format!(
                "\nRead-only: {}",
                if meta.permissions().readonly() {
                    "Yes"
                } else {
                    "No"
                }
            )),
            Err(err) => line.push_str(&format!("\nMetadata unavailable: {err}")),
        }
        lines.push(line);
    }
    Some(lines.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new(
            PathBuf::from("source"),
            PathBuf::from("destination"),
            vec![],
        );
        let p = &mut app.panes[0];
        for name in ["one", "two", "three"] {
            p.entries.push(Entry {
                hidden: false,
                path: PathBuf::from(format!("source/{name}")),
                name: name.into(),
                directory: false,
                bytes: Some(1),
                partial: false,
                modified: None,
                modified_label: "—".into(),
                icon: None,
            });
        }
        p.reorder();
        app
    }
    #[test]
    fn context_preserves_multi_selection_and_selects_unselected_item() {
        let mut a = app();
        a.panes[0].selections.extend([0, 1]);
        let _ = a.open_context(0, Some(1), iced::Point::ORIGIN);
        assert_eq!(a.panes[0].selections.len(), 2);
        let _ = a.open_context(0, Some(2), iced::Point::ORIGIN);
        assert_eq!(
            a.panes[0].selections.iter().copied().collect::<Vec<_>>(),
            vec![2]
        );
        let _ = a.open_context(1, None, iced::Point::ORIGIN);
        assert_eq!(a.active, 1);
        assert!(a.panes[1].selections.is_empty());
    }
    #[test]
    fn drop_copies_frozen_paths_without_confirmation() {
        let mut a = app();
        a.panes[0].selections.extend([0, 1]);
        a.begin_file_drag(0, 0);
        a.panes[0].selections.clear();
        a.drop_files(0, 0, 1);
        let request = a.drop_confirmation.as_ref().unwrap();
        assert_eq!(request.sources.len(), 2);
        assert_eq!(request.destination, Some(PathBuf::from("destination")));
        assert!(a.clipboard.is_none());
        assert!(a.jobs.is_empty());
        a.undo_busy = true;
        let _ = a.update(Message::StatusHover(None));
        assert!(a.drop_confirmation.is_none());
        assert_eq!(a.jobs.back().unwrap().request.sources.len(), 2);
        assert_eq!(a.jobs.back().unwrap().request.kind, transfers::Kind::Copy);
    }
    #[test]
    fn dropping_unselected_file_selects_only_that_file_and_enqueues_copy() {
        let mut a = app();
        a.panes[0].selections.insert(0);
        a.begin_file_drag(0, 2);
        assert_eq!(
            a.drag_files.as_ref().unwrap().paths,
            vec![PathBuf::from("source/three")]
        );
        a.undo_busy = true;
        a.drop_files(0, 2, 1);
        let _ = a.update(Message::StatusHover(None));
        let job = a.jobs.back().unwrap();
        assert_eq!(job.request.kind, transfers::Kind::Copy);
        assert_eq!(job.request.sources.len(), 1);
        assert!(a.clipboard.is_none());
    }
    #[test]
    fn properties_worker_cannot_reopen_closed_dialog() {
        let mut a = app();
        a.properties = None;
        let _ = a.update(Message::PropertiesLoaded(42, Some("stale".into())));
        assert!(a.properties.is_none());
    }
    #[test]
    fn shell_failure_uses_existing_properties_and_success_does_not_open_fallback() {
        let a = app();
        let entries = a.panes[0].entries.clone();
        let fallback = properties_result(entries.clone(), |paths| {
            assert_eq!(paths.len(), 3);
            Err("Shell unavailable".into())
        })
        .unwrap();
        assert!(fallback.contains("Shell unavailable"));
        assert!(fallback.contains("Location: source"));
        assert!(fallback.contains("Modified: —"));
        assert!(properties_result(entries, |_| Ok(())).is_none());
    }
    #[test]
    fn cancelled_properties_request_rejects_late_fallback() {
        let mut a = app();
        a.properties_request = Some(77);
        let _ = a.update(Message::CloseProperties);
        let _ = a.update(Message::PropertiesLoaded(77, Some("late fallback".into())));
        assert!(a.properties.is_none());
    }
}

#[cfg(test)]
mod undo_refresh_regression {
    use super::*;
    #[test]
    fn undoing_copy_refreshes_an_ordinary_panel_path_for_a_canonical_destination() {
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("undo-refresh-{}", next_identity()));
        let destination = root.join("destination");
        let history = root.join("history");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::create_dir_all(&history).unwrap();
        std::fs::write(history.join("actions.jsonl"), b"").unwrap();
        let target = destination.join("copy.txt");
        std::fs::write(&target, b"copied").unwrap();
        let canonical = std::fs::canonicalize(&target).unwrap();
        let entry = undo::Entry {
            path: history,
            label: "Copy".into(),
            actions: vec![(
                0,
                undo::Action::RemoveCopy {
                    path: canonical,
                    stamp: undo::stamp(&target).unwrap(),
                },
            )],
        };
        let mut app = App::new(destination.clone(), root.join("unrelated"), vec![]);
        app.preferences.scan_mode = preferences::ScanMode::Off;
        app.panes[0].entries = browser::list(&destination).unwrap();
        app.panes[0].reorder();
        app.panes[0].generation = 17;
        assert_eq!(app.panes[0].entries.len(), 1);
        let (paths, result) = undo::run(entry);
        assert!(result.is_ok());
        assert!(!target.exists());
        let task = app.update(Message::UndoFinished(paths, result, None));
        assert_ne!(app.panes[0].generation, 17);
        assert!(app.panes[0].entries.is_empty());
        assert!(app.panes[0].busy);
        assert_eq!(app.panes[1].generation, 0);
        drop(task);
        drop(app);
        assert!(root.starts_with(std::env::current_dir().unwrap().join("target")));
        let _ = std::fs::remove_dir_all(root);
    }
}

impl App {
    pub fn copy_path_text(&self) -> String {
        let p = &self.panes[self.active];
        let paths: Vec<_> = p
            .order
            .iter()
            .filter(|i| p.selections.contains(i))
            .map(|i| p.entries[*i].path.display().to_string())
            .collect();
        if paths.is_empty() {
            p.path.display().to_string()
        } else {
            paths.join("\r\n")
        }
    }
    pub fn terminal_path(&self) -> PathBuf {
        let p = &self.panes[self.active];
        if p.selections.len() == 1 {
            p.selections
                .iter()
                .next()
                .and_then(|i| p.entries.get(*i))
                .filter(|e| e.directory)
                .map(|e| e.path.clone())
                .unwrap_or_else(|| p.path.clone())
        } else {
            p.path.clone()
        }
    }
}
