//! First-character navigation follows the last explicitly selected browsing surface.
use crate::*;
impl App {
    pub fn focus_files(&mut self) {
        self.tree_keyboard_focus = false;
        self.locations.clear_keyboard_cursor();
        for tree in &mut self.panel_locations {
            tree.clear_keyboard_cursor();
        }
    }
    pub fn jump_initial(&mut self, c: char) -> Task<Message> {
        if !c.is_alphanumeric() || self.browsing_keys_blocked() {
            return Task::none();
        }
        if self.tree_keyboard_focus {
            if self.preferences.panel_trees {
                self.panel_locations[self.active].jump_initial(c);
            } else {
                self.locations.jump_initial(c);
            }
            return Task::none();
        }
        let pane = &mut self.panes[self.active];
        let start = pane
            .selected
            .filter(|i| starts_with_initial(&pane.entries[*i].name, c))
            .and_then(|i| pane.order.iter().position(|v| *v == i))
            .map_or(0, |r| r + 1);
        let target = (start..pane.order.len()).chain(0..start).find_map(|row| {
            let index = pane.order[row];
            starts_with_initial(&pane.entries[index].name, c).then_some((row, index))
        });
        if let Some((row, index)) = target {
            pane.selections.clear();
            pane.selections.insert(index);
            pane.selected = Some(index);
            pane.reveal_selection = Some((next_identity(), row));
        }
        Task::none()
    }
    pub fn browsing_keys_blocked(&self) -> bool {
        self.settings_open
            || self.licenses.is_some()
            || self.rename.is_some()
            || self.advanced.is_some()
            || (self.failed_view.is_some() || self.activity_view.is_some())
            || self.shadow_picker.is_some()
            || self.confirmation.is_some()
            || self.conflict.is_some()
            || self.properties.is_some()
            || self.drop_confirmation.is_some()
            || self.context.is_some()
            || self.header_context.is_some()
            || self.undo_busy
    }
    pub fn arrow_selection(&mut self, direction: i8) -> Task<Message> {
        if self.browsing_keys_blocked() {
            return Task::none();
        }
        if self.tree_keyboard_focus {
            if self.preferences.panel_trees {
                let id = self.active;
                return self.panel_locations[id]
                    .arrow_selection(direction)
                    .map(move |m| Message::PanelLocations(id, m));
            }
            return self
                .locations
                .arrow_selection(direction)
                .map(Message::Locations);
        }
        let pane = &mut self.panes[self.active];
        if pane.order.is_empty() {
            return Task::none();
        }
        let current = pane
            .selected
            .and_then(|i| pane.order.iter().position(|v| *v == i));
        let row = match current {
            Some(r) if direction < 0 => r.saturating_sub(1),
            Some(r) => (r + 1).min(pane.order.len() - 1),
            None if direction < 0 => pane.order.len() - 1,
            None => 0,
        };
        let index = pane.order[row];
        pane.selected = Some(index);
        pane.selections.clear();
        pane.selections.insert(index);
        pane.reveal_selection = Some((next_identity(), row));
        Task::none()
    }
}
pub fn starts_with_initial(name: &str, c: char) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.to_lowercase().to_string() == c.to_lowercase().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(name: &str) -> Entry {
        Entry {
            path: PathBuf::from("home").join(name),
            name: name.into(),
            directory: false,
            hidden: false,
            bytes: Some(1),
            partial: false,
            modified: None,
            modified_label: String::new(),
            icon: None,
        }
    }
    #[test]
    fn panel_jump_uses_visible_order_and_does_not_open_or_navigate() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        app.panes[0].entries = vec![
            entry("apple.txt"),
            entry("Alpha.txt"),
            entry("7zip.txt"),
            entry("banana.txt"),
        ];
        app.panes[0].order = vec![1, 0, 2, 3];
        let _ = app.update(Message::JumpInitial('a'));
        assert_eq!(app.panes[0].selected, Some(1));
        assert_eq!(app.panes[0].reveal_selection.unwrap().1, 0);
        let _ = app.update(Message::JumpInitial('a'));
        assert_eq!(app.panes[0].selected, Some(0));
        let _ = app.update(Message::JumpInitial('A'));
        assert_eq!(app.panes[0].selected, Some(1));
        let _ = app.update(Message::JumpInitial('7'));
        assert_eq!(app.panes[0].selected, Some(2));
        app.panes[0].order = vec![0, 3];
        let _ = app.update(Message::JumpInitial('A'));
        assert_eq!(app.panes[0].selected, Some(0));
        assert_eq!(app.panes[0].path, PathBuf::from("home"));
        let _ = app.update(Message::JumpInitial('z'));
        assert_eq!(app.panes[0].selected, Some(0));
        app.settings_open = true;
        let _ = app.update(Message::JumpInitial('b'));
        assert_eq!(app.panes[0].selected, Some(0));
    }
    #[test]
    fn arrows_follow_visible_order_and_enter_opens_selected_directory() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let mut directory = entry("folder");
        directory.directory = true;
        app.panes[0].entries = vec![entry("file"), directory];
        app.panes[0].order = vec![1, 0];
        let _ = app.update(Message::ArrowSelection(1));
        assert_eq!(app.panes[0].selected, Some(1));
        let _ = app.update(Message::ArrowSelection(1));
        assert_eq!(app.panes[0].selected, Some(0));
        let _ = app.update(Message::ArrowSelection(1));
        assert_eq!(app.panes[0].selected, Some(0));
        let _ = app.update(Message::ArrowSelection(-1));
        assert_eq!(app.panes[0].selected, Some(1));
        assert_eq!(app.panes[0].reveal_selection.unwrap().1, 0);
        let _ = app.update(Message::Confirm(true));
        assert_eq!(app.panes[0].path, PathBuf::from("home").join("folder"));
    }
    #[test]
    fn new_panels_preserve_tabs_and_fill_four_panel_grid() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        app.pane_count = 1;
        app.rebuild_file_layout();
        let original = app.panes[0].identity;
        for count in 2..=4 {
            let _ = app.update(Message::PanelShortcut);
            assert_eq!(app.pane_count, count);
            assert_eq!(app.active, count - 1);
            assert_eq!(app.panes[0].identity, original);
        }
        let regions =
            app.files_layout
                .layout()
                .pane_regions(0.0, 0.0, iced::Size::new(1000.0, 800.0));
        assert!(
            regions
                .values()
                .all(|r| r.width == 500.0 && r.height == 400.0)
        );
        let _ = app.update(Message::PanelShortcut);
        assert_eq!(app.pane_count, 4);
        app.settings_open = true;
        let _ = app.update(Message::NewPanel(0));
        assert_eq!(app.pane_count, 4);
    }
    #[test]
    fn panel_cycle_wraps_and_close_preserves_final_panel_and_modal_state() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        app.pane_count = 3;
        app.rebuild_file_layout();
        app.active = 2;
        let identities = app.panes[..3]
            .iter()
            .map(|p| p.identity)
            .collect::<Vec<_>>();
        let _ = app.update(Message::CyclePanel(false));
        assert_eq!(app.active, 0);
        let _ = app.update(Message::CyclePanel(true));
        assert_eq!(app.active, 2);
        app.settings_open = true;
        let _ = app.update(Message::CyclePanel(false));
        let _ = app.update(Message::ClosePanelShortcut);
        assert_eq!(app.active, 2);
        assert_eq!(app.pane_count, 3);
        app.settings_open = false;
        let _ = app.update(Message::ClosePanelShortcut);
        assert_eq!(app.pane_count, 2);
        assert_eq!(app.panes[0].identity, identities[0]);
        assert_eq!(app.panes[1].identity, identities[1]);
        let _ = app.update(Message::ClosePanelShortcut);
        let _ = app.update(Message::ClosePanelShortcut);
        assert_eq!(app.pane_count, 1);
    }
    #[test]
    fn shortcuts_route_only_when_not_captured() {
        use iced::keyboard::{Key, Modifiers, key::Named};
        let cases = [
            (
                Key::Character("c".into()),
                Modifiers::CTRL | Modifiers::SHIFT,
                0,
            ),
            (Key::Named(Named::Enter), Modifiers::ALT, 1),
            (Key::Character("n".into()), Modifiers::CTRL, 2),
            (
                Key::Character("N".into()),
                Modifiers::CTRL | Modifiers::SHIFT,
                3,
            ),
            (Key::Named(Named::ArrowDown), Modifiers::empty(), 4),
            (Key::Named(Named::Tab), Modifiers::empty(), 5),
            (Key::Named(Named::Tab), Modifiers::SHIFT, 6),
            (
                Key::Character("W".into()),
                Modifiers::CTRL | Modifiers::SHIFT,
                7,
            ),
            (Key::Character("f".into()), Modifiers::CTRL, 8),
        ];
        for (key, modifiers, expected) in cases {
            for captured in [false, true] {
                let event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: key.clone(),
                    modified_key: key.clone(),
                    physical_key: iced::keyboard::key::Physical::Unidentified(
                        iced::keyboard::key::NativeCode::Unidentified,
                    ),
                    location: iced::keyboard::Location::Standard,
                    modifiers,
                    text: None,
                    repeat: false,
                });
                let message = keyboard_message(
                    event,
                    if captured {
                        iced::event::Status::Captured
                    } else {
                        iced::event::Status::Ignored
                    },
                    iced::window::Id::unique(),
                );
                if captured && expected < 7 {
                    assert!(message.is_none());
                    continue;
                }
                assert!(matches!(
                    (expected, message),
                    (0, Some(Message::CopyPath))
                        | (1, Some(Message::PropertiesShortcut))
                        | (2, Some(Message::PanelShortcut))
                        | (3, Some(Message::FileAction(FileAction::NewFolder)))
                        | (4, Some(Message::ArrowSelection(1)))
                        | (5, Some(Message::CyclePanel(false)))
                        | (6, Some(Message::CyclePanel(true)))
                        | (7, Some(Message::ClosePanelShortcut))
                        | (8, Some(Message::FocusSearch))
                ));
            }
        }
    }
    #[test]
    fn text_inputs_and_modified_shortcuts_do_not_jump() {
        for (status, modifiers, should_jump) in [
            (
                iced::event::Status::Captured,
                iced::keyboard::Modifiers::empty(),
                false,
            ),
            (
                iced::event::Status::Ignored,
                iced::keyboard::Modifiers::CTRL,
                false,
            ),
            (
                iced::event::Status::Ignored,
                iced::keyboard::Modifiers::ALT,
                false,
            ),
            (
                iced::event::Status::Ignored,
                iced::keyboard::Modifiers::empty(),
                true,
            ),
        ] {
            let key = iced::keyboard::Key::Character("a".into());
            let event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key: iced::keyboard::key::Physical::Unidentified(
                    iced::keyboard::key::NativeCode::Unidentified,
                ),
                location: iced::keyboard::Location::Standard,
                modifiers,
                text: Some("a".into()),
                repeat: false,
            });
            let message = keyboard_message(event, status, iced::window::Id::unique());
            assert_eq!(
                matches!(message, Some(Message::JumpInitial('a'))),
                should_jump
            );
        }
        assert!(starts_with_initial("Éclair", 'é'));
        assert!(!starts_with_initial("", 'a'));
    }
    #[test]
    fn tree_focus_routes_jumps_and_enter_without_changing_panel_selection() {
        let mut app = App::new("home".into(), "other".into(), vec![PathBuf::from("Z:")]);
        app.panes[0].entries = vec![entry("zebra")];
        app.panes[0].order = vec![0];
        let _ = app.update(Message::Locations(locations::Message::Navigate(
            "home".into(),
        )));
        let _ = app.update(Message::JumpInitial('z'));
        assert!(app.tree_keyboard_focus);
        assert_eq!(app.locations.keyboard_cursor(), Some(&PathBuf::from("Z:")));
        assert_eq!(app.panes[0].selected, None);
        app.sidebar_drawer = true;
        let _ = app.update(Message::Confirm(true));
        assert_eq!(app.panes[0].path, PathBuf::from("Z:"));
        assert!(!app.sidebar_drawer);
        app.panes[0].entries = vec![entry("zebra")];
        app.panes[0].order = vec![0];
        app.focus_files();
        let _ = app.update(Message::JumpInitial('z'));
        assert_eq!(app.panes[0].selected, Some(0));
        assert!(app.locations.keyboard_cursor().is_none());
    }
}
