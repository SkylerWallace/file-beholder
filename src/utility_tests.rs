use crate::*;

fn snapshot() -> shadow_copies::ShadowCopy {
    shadow_copies::ShadowCopy {
        device: r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy987654".into(),
        drive: "C:".into(),
        date: "2026-10-24 10:30:00".into(),
    }
}
fn opening_app() -> App {
    let mut app = App::new("source".into(), "destination".into(), vec![]);
    app.panes[0].entries = [("file.txt", false), ("folder", true)]
        .into_iter()
        .map(|(name, directory)| Entry {
            path: PathBuf::from("source").join(name),
            name: name.into(),
            directory,
            hidden: false,
            bytes: Some(1),
            partial: false,
            modified: None,
            modified_label: String::new(),
            icon: None,
        })
        .collect();
    app.panes[0].selected = Some(0);
    app.panes[0].selections.insert(0);
    app
}
#[test]
fn disabling_file_opening_blocks_default_and_chooser_actions_but_allows_folder_navigation() {
    let mut app = opening_app();
    let _ = app.update(Message::OpenFiles(false));
    for message in [
        Message::OpenEntry(0, 0),
        Message::OpenContextEntry,
        Message::OpenContextWith,
    ] {
        app.notice.clear();
        let _ = app.update(message);
        assert!(app.notice.contains("Allow opening files"));
        assert_eq!(app.panes[0].path, PathBuf::from("source"));
    }
    let _ = app.open_entry(0, 1, true);
    assert_eq!(app.panes[0].path, PathBuf::from("source"));
    let _ = app.update(Message::OpenEntry(0, 1));
    assert_eq!(app.panes[0].path, PathBuf::from("source/folder"));
    assert!(!preferences::Preferences::decode("open_files=false").open_files);
    assert!(preferences::Preferences::decode("open_files=true").open_files);
}
#[test]
fn context_menu_actions_are_horizontal_and_disabled_open_buttons_emit_no_commands() {
    use iced::advanced::{Layout, Shell, layout, widget::Tree};
    let mut app = opening_app();
    for enabled in [false, true] {
        app.preferences.open_files = enabled;
        let mut view = app.file_context_menu();
        let mut tree = Tree::new(&view);
        let renderer = iced::Renderer::new(crate::typography::font(), 13.0.into());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(iced::Size::ZERO, iced::Size::new(400., 700.)),
        );
        let contents = &node.children()[0];
        let actions = &contents.children()[0];
        assert_eq!(actions.children().len(), 5);
        let action_y = actions.children()[0].bounds().y;
        for action in actions.children() {
            assert_eq!(action.bounds().y, action_y);
        }
        let mut messages = Vec::new();
        for index in [2, 3] {
            let point = contents.children()[index].bounds().center();
            for event in [
                iced::mouse::Event::CursorMoved { position: point },
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
        }
        assert_eq!(
            messages
                .iter()
                .filter(|message| matches!(
                    message,
                    Message::OpenContextEntry | Message::OpenContextWith
                ))
                .count(),
            if enabled { 2 } else { 0 }
        );
    }
}
fn failures(app: &mut App) {
    let mut job = transfers::Job::new(
        10,
        transfers::Request {
            kind: transfers::Kind::Copy,
            sources: vec!["source".into()],
            destination: Some("dest".into()),
        },
    );
    job.state = "Completed with errors";
    job.report = Some(transfers::Report {
        cancelled: false,
        failures: vec![],
        failure_count: 250,
        succeeded: 1,
        completed_sources: vec![],
        undo_warning: None,
        history: None,
        issues: (0..500)
            .map(|i| transfers::Issue {
                source: format!(r"C:\Source\folder{}\file-{i:04}.txt", i % 10).into(),
                target: Some(format!(r"D:\Destination\folder{}\file-{i:04}.txt", i % 10).into()),
                reason: if i % 2 == 0 {
                    "Access denied after 4 attempts"
                } else {
                    "Skipped existing destination"
                }
                .into(),
                skipped: i % 2 != 0,
            })
            .collect(),
    });
    app.jobs.push_back(job);
    let _ = app.update(Message::ToggleTransferIssues(10));
}
#[test]
fn bulk_retry_keeps_all_500_paths_and_10_destinations_and_the_open_report() {
    let mut app = App::new("home".into(), "other".into(), vec![]);
    failures(&mut app);
    // Keep the queue paused so no real transfer worker is started by this routing test.
    app.undo_busy = true;
    let _ = app.update(Message::RetryAllTransferIssues(10));
    assert_eq!(app.jobs.len(), 11);
    assert_eq!(
        app.jobs
            .iter()
            .skip(1)
            .map(|job| job.request.sources.len())
            .sum::<usize>(),
        500
    );
    for job in app.jobs.iter().skip(1) {
        assert!(
            job.request
                .sources
                .iter()
                .all(|source| source.parent().unwrap().file_name()
                    == job.request.destination.as_ref().unwrap().file_name())
        );
    }
    assert_eq!(app.failed_view.as_ref().unwrap().job, 10);
    let _ = app.update(Message::FileAction(FileAction::Delete));
    assert!(app.confirmation.is_none());
    let _ = app.update(Message::Confirm(false));
    assert!(app.failed_view.is_none());
}
#[test]
fn shadow_tabs_open_in_the_chosen_panel_and_survive_move_and_session_restore() {
    let mut app = App::new("home".into(), "other".into(), vec![]);
    let copy = snapshot();
    app.shadow_picker = Some(shadow_copies::Picker {
        panel: 1,
        request: 42,
        result: Some(Ok(vec![copy.clone()])),
    });
    let _ = app.update(Message::OpenShadow(0));
    assert_eq!(app.active, 1);
    assert_eq!(app.panes[1].tabs.len(), 2);
    assert_eq!(app.panes[1].path, copy.root());
    assert_eq!(app.panes[1].shadow, Some(copy.clone()));
    assert!(app.panes[1].parent().is_none());
    app.panes[1].path = copy.root().join("Users");
    assert_eq!(app.panes[1].parent(), Some(copy.device.clone()));
    app.panes[1].path = copy.root();
    let tab = app.panes[1].identity;
    let _ = app.update(Message::MoveTab(1, tab, 0, 1));
    assert_eq!(app.panes[0].shadow, Some(copy.clone()));
    let saved = serde_json::to_vec(&session::Session::capture(&app)).unwrap();
    let saved: session::Session = serde_json::from_slice(&saved).unwrap();
    let mut restored = App::new("home".into(), "other".into(), vec![]);
    saved.restore(&mut restored).unwrap();
    assert_eq!(restored.panes[0].shadow, Some(copy.clone()));
    assert_eq!(restored.panes[0].path, copy.root());
    let _ = app.update(Message::ShadowsLoaded(42, Ok(vec![copy])));
    assert!(app.shadow_picker.is_none());
}
#[test]
fn dropped_snapshot_folders_preserve_dates_for_panel_and_tree_sources() {
    for source in [Some(1), None] {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let copy = snapshot();
        app.panes[1].shadow = Some(copy.clone());
        app.panes[1].path = copy.root();
        let folder = copy.root().join("Users");
        let _ = app.update(Message::FolderTabDrop(0, 0, folder.clone(), source));
        assert_eq!(app.panes[0].selected_tab, 0);
        assert_eq!(app.panes[0].path, folder);
        assert_eq!(app.panes[0].shadow, Some(copy));
        assert!(app.jobs.is_empty());
    }
}
#[test]
fn alternating_rows_default_on_and_roundtrip_the_disabled_setting() {
    assert!(preferences::Preferences::decode("").alternating_rows);
    let mut p = preferences::Preferences::default();
    p.alternating_rows = false;
    assert!(!preferences::Preferences::decode(&p.encode()).alternating_rows);
}
fn capture(name: &str, pixels: Vec<u8>, size: iced::Size<u32>) {
    if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
        tiny_skia::Pixmap::from_vec(
            pixels,
            tiny_skia::IntSize::from_wh(size.width, size.height).unwrap(),
        )
        .unwrap()
        .save_png(format!("target/{name}.png"))
        .unwrap();
    }
}
#[test]
fn file_menu_and_setting_render_enabled_disabled_and_folder_states() {
    let mut app = opening_app();
    let size = iced::Size::new(400, 360);
    let mut previous = None;
    for enabled in [false, true] {
        app.preferences.open_files = enabled;
        let pixels = final_ui_checks::render_view(
            container(app.file_context_menu()).center(Fill).into(),
            &app,
            size,
            None,
        );
        if let Some(previous) = previous {
            assert_ne!(pixels, previous);
        }
        previous = Some(pixels.clone());
        capture(
            if enabled {
                "context-file-enabled"
            } else {
                "context-file-disabled"
            },
            pixels,
            size,
        );
    }
    app.preferences.open_files = false;
    app.panes[0].selected = Some(1);
    capture(
        "context-folder",
        final_ui_checks::render_view(
            container(app.file_context_menu()).center(Fill).into(),
            &app,
            size,
            None,
        ),
        size,
    );
    let size = iced::Size::new(400, 540);
    capture(
        "allow-opening-files-setting",
        final_ui_checks::render_view(
            container(settings_ui::view(&app)).center(Fill).into(),
            &app,
            size,
            None,
        ),
        size,
    );
}
#[test]
fn large_failure_dialog_and_snapshot_picker_render_at_small_and_large_sizes() {
    let mut app = App::new("home".into(), "other".into(), vec![]);
    failures(&mut app);
    for size in [iced::Size::new(1000, 700), iced::Size::new(1400, 900)] {
        let full = final_ui_checks::render(&app, size, None);
        capture(
            &format!("failed-transfers-{}", size.width),
            full.clone(),
            size,
        );
        let _ = app.update(Message::TransferIssueFilter("file-0499".into()));
        let filtered = final_ui_checks::render(&app, size, None);
        assert_ne!(full, filtered);
        let _ = app.update(Message::TransferIssueFilter(String::new()));
    }
    app.failed_view = None;
    app.shadow_picker = Some(shadow_copies::Picker {
        panel: 1,
        request: 99,
        result: Some(Ok(vec![snapshot()])),
    });
    capture(
        "shadow-picker",
        final_ui_checks::render(&app, iced::Size::new(1400, 900), None),
        iced::Size::new(1400, 900),
    );
    app.shadow_picker.as_mut().unwrap().result =
        Some(Err("Administrator access may be required.".into()));
    let error = final_ui_checks::render(&app, iced::Size::new(1000, 700), None);
    app.shadow_picker.as_mut().unwrap().result = Some(Ok(vec![]));
    assert_ne!(
        error,
        final_ui_checks::render(&app, iced::Size::new(1000, 700), None)
    );
    app.shadow_picker = None;
    let copy = snapshot();
    app.panes[1].tabs.push(Tab::new(copy.root()));
    app.panes[1].selected_tab = 1;
    app.panes[1].shadow = Some(copy);
    for panel in &mut app.panes[..2] {
        panel.entries = (0..40)
            .map(|i| Entry {
                path: format!("home/file-{i:02}.txt").into(),
                name: format!("file-{i:02}.txt"),
                directory: false,
                hidden: false,
                bytes: Some(2048),
                partial: false,
                modified: None,
                modified_label: "2026-10-24 10:30".into(),
                icon: None,
            })
            .collect();
        panel.total_bytes = 40 * 2048;
        panel.reorder();
    }
    let size = iced::Size::new(1400, 900);
    let striped = final_ui_checks::render(&app, size, None);
    capture("striped-rows-shadow-tab", striped.clone(), size);
    let _ = app.update(Message::AlternatingRows(false));
    assert_ne!(striped, final_ui_checks::render(&app, size, None));
    let _ = app.update(Message::AlternatingRows(true));
    app.dark = false;
    app.app_theme = palette::palette(false, false).theme();
    capture(
        "striped-rows-light",
        final_ui_checks::render(&app, size, None),
        size,
    );
}

#[test]
fn rename_requires_one_selection_and_cancel_preserves_the_original() {
    let mut app = opening_app();
    let _ = app.update(Message::ContextAction(Some(FileAction::Rename)));
    assert_eq!(
        app.rename.as_ref().unwrap().source,
        PathBuf::from("source/file.txt")
    );
    let _ = app.update(Message::RenameName("new.txt".into()));
    let _ = app.update(Message::Confirm(false));
    assert!(app.rename.is_none());
    assert_eq!(app.panes[0].entries[0].name, "file.txt");
    app.panes[0].selections.insert(1);
    let _ = app.update(Message::FileAction(FileAction::Rename));
    assert!(app.rename.is_none());
    app.panes[0].selections.clear();
    let _ = app.update(Message::FileAction(FileAction::Rename));
    assert!(app.rename.is_none());
}

#[test]
fn renamed_item_is_selected_after_listing_refresh_and_new_folder_is_cancellable() {
    let mut app = opening_app();
    let _ = app.update(Message::FileAction(FileAction::Rename));
    let id = app.rename.as_ref().unwrap().id;
    let _ = app.update(Message::RenameName("updated.txt".into()));
    let _ = app.update(Message::RenameFinished(id, Ok(None)));
    let mut entry = app.panes[0].entries.first().cloned();
    // Refresh clears old entries, so supply the newly listed item.
    if entry.is_none() {
        let fixture = opening_app();
        entry = fixture.panes[0].entries.first().cloned();
    }
    let mut entry = entry.unwrap();
    entry.name = "updated.txt".into();
    entry.path = "source/updated.txt".into();
    let generation = app.panes[0].generation;
    let _ = app.update(Message::Scan(0, generation, Event::Listed(Ok(vec![entry]))));
    assert_eq!(app.panes[0].selected, Some(0));
    assert!(app.panes[0].selections.contains(&0));
    assert_eq!(app.panes[0].reveal_selection.unwrap().1, 0);
    let _ = app.update(Message::FileAction(FileAction::NewFolder));
    assert!(app.rename.as_ref().unwrap().new_folder);
    let _ = app.update(Message::Confirm(false));
    assert!(app.rename.is_none());
}

#[test]
fn copy_path_and_terminal_use_selected_items_or_the_current_folder() {
    let mut app = opening_app();
    app.panes[0].reorder();
    assert_eq!(
        app.copy_path_text(),
        PathBuf::from("source")
            .join("file.txt")
            .display()
            .to_string()
    );
    assert_eq!(app.terminal_path(), PathBuf::from("source"));
    app.panes[0].selections.clear();
    app.panes[0].selections.insert(1);
    assert_eq!(app.terminal_path(), PathBuf::from("source/folder"));
    app.panes[0].selections.insert(0);
    assert_eq!(app.copy_path_text().lines().count(), 2);
    assert_eq!(app.terminal_path(), PathBuf::from("source"));
    app.panes[0].selections.clear();
    assert_eq!(app.copy_path_text(), "source");
    assert_eq!(app.terminal_path(), PathBuf::from("source"));
}
