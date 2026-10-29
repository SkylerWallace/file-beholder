use super::*;
pub const MIN_PANEL_WIDTH: f32 = 320.0;
pub(super) fn minimum_grid_width(node: &pane_grid::Node) -> f32 {
    match node {
        pane_grid::Node::Pane(_) => MIN_PANEL_WIDTH,
        pane_grid::Node::Split {
            axis, ratio, a, b, ..
        } => {
            let (a, b) = (minimum_grid_width(a), minimum_grid_width(b));
            if *axis == pane_grid::Axis::Horizontal {
                a.max(b)
            } else {
                let ratio = ratio.clamp(0.001, 0.999);
                ((a + 3.0) / ratio).max((b + 3.0) / (1.0 - ratio))
            }
        }
    }
}
impl App {
    pub(super) fn new_panel(&mut self, source: usize) -> Task<Message> {
        if self.pane_count >= 4 || source >= self.pane_count {
            return Task::none();
        }
        let id = self.pane_count;
        let mut panel = Panel::new(self.panes[source].path.clone());
        panel.shadow = self.panes[source].shadow.clone();
        self.panes[id] = panel;
        self.pane_count += 1;
        self.active = id;
        self.stacked = false;
        self.grid = self.pane_count >= 3;
        self.rebuild_file_layout();
        self.focus_files();
        self.panes[id].start(id, self.preferences.scan_mode)
    }

    pub(super) fn resize_file_panels(&mut self, event: pane_grid::ResizeEvent, size: iced::Size) {
        self.files_layout.set_size(size);
        fn children(
            node: &pane_grid::Node,
            split: pane_grid::Split,
        ) -> Option<(&pane_grid::Node, &pane_grid::Node)> {
            match node {
                pane_grid::Node::Split { id, a, b, .. } if *id == split => Some((a, b)),
                pane_grid::Node::Split { a, b, .. } => {
                    children(a, split).or_else(|| children(b, split))
                }
                _ => None,
            }
        }
        let splits = self.files_layout.layout().split_regions(6.0, 180.0, size);
        let Some((axis, bounds, _)) = splits.get(&event.split) else {
            return;
        };
        let ratio = if *axis == pane_grid::Axis::Vertical {
            let Some((a, b)) = children(self.files_layout.layout(), event.split) else {
                return;
            };
            let minimum = (minimum_grid_width(a) + 3.0) / bounds.width;
            let maximum = 1.0 - (minimum_grid_width(b) + 3.0) / bounds.width;
            if minimum <= maximum {
                event.ratio.clamp(minimum, maximum)
            } else {
                (minimum + maximum) * 0.5
            }
        } else {
            event.ratio.clamp(0.20, 0.80)
        };
        self.files_layout.resize(event.split, ratio);
    }
    pub(super) fn initialize_default_layout(&mut self) {
        let (count, stacked, grid) = self.preferences.default_layout.configuration();
        self.pane_count = count;
        self.stacked = stacked;
        self.grid = grid;
        self.active = self.active.min(count - 1);
        self.rebuild_file_layout();
    }

    pub(super) fn close_tab(&mut self, id: usize, tab: usize) {
        if id >= self.pane_count || tab >= self.panes[id].tabs.len() {
            return;
        }
        self.context = None;
        if self.panes[id].tabs.len() > 1 {
            let panel = &mut self.panes[id];
            panel.tabs.remove(tab);
            if tab < panel.selected_tab {
                panel.selected_tab -= 1;
            }
            panel.selected_tab = panel.selected_tab.min(panel.tabs.len() - 1);
        } else {
            self.close_panel(id);
        }
    }
    pub(super) fn close_panel(&mut self, id: usize) {
        if self.pane_count <= 1 || id >= self.pane_count {
            return;
        }
        self.context = None;
        let closed_pane = self
            .files_layout
            .iter()
            .find(|(_, value)| **value == id)
            .map(|(pane, _)| *pane);
        if let Some(pane) = closed_pane {
            self.files_layout.close(pane);
        }
        self.files_layout.renumber_after(id);
        let path = self.panes[id]
            .tabs
            .get(self.panes[id].selected_tab)
            .map(|tab| tab.path.clone())
            .unwrap_or_else(|| self.panes[self.active].path.clone());
        self.panes[id] = Panel::new(path);
        self.panes[id..self.pane_count].rotate_left(1);
        self.pane_count -= 1;
        self.active = if self.active > id {
            self.active - 1
        } else if self.active == id {
            id.min(self.pane_count - 1)
        } else {
            self.active
        };
        if self.pane_count < 3 {
            self.grid = false;
        }

        self.drag_files = None;
    }

    pub(super) fn rebuild_file_layout(&mut self) {
        self.files_layout =
            container_layout::Layout::preset(self.pane_count, self.stacked, self.grid, self.active);
    }

    pub(super) fn persist_preferences(&mut self) {
        #[cfg(not(test))]
        if let Err(error) = self.preferences.save() {
            self.notice = format!("Settings changed for this session; could not save: {error}");
        }
    }
    pub(super) fn apply_appearance(&mut self) -> Task<Message> {
        let dark = self.preferences.theme.dark(self.system_dark);
        self.dark = dark;
        self.app_theme = palette::styled(
            dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        )
        .theme();
        crate::typography::set_theme(palette::from_theme(&self.app_theme));
        self.style_chrome()
    }
    pub(super) fn move_tab(
        &mut self,
        from: usize,
        identity: u64,
        to: usize,
        index: usize,
    ) -> Task<Message> {
        if from >= self.pane_count || to >= self.pane_count {
            return Task::none();
        }
        let Some(source) = self.panes[from]
            .tabs
            .iter()
            .position(|t| t.identity == identity)
        else {
            return Task::none();
        };
        let mut index = index.min(self.panes[to].tabs.len());
        let tab = self.panes[from].tabs.remove(source);
        if from == to && index > source {
            index -= 1;
        }
        if from != to {
            let selected = self.panes[from].selected_tab;
            self.panes[from].selected_tab = if source < selected {
                selected - 1
            } else {
                selected.min(self.panes[from].tabs.len().saturating_sub(1))
            };
        }
        self.panes[to].tabs.insert(index, tab);
        self.panes[to].selected_tab = index;
        self.panes[to].drop_highlight = highlight::Pulse::new(self.motion);
        self.active = to;
        if from != to && self.panes[from].tabs.is_empty() {
            self.close_panel(from);
        }
        Task::none()
    }
}
pub(super) fn ribbon_divider<'a>(t: theme::Tokens) -> Element<'a, Message> {
    container(Space::new())
        .width(1)
        .height(28)
        .style(move |_| container::Style {
            background: Some(theme::color(t.border).into()),
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod default_layout_tests {
    use super::*;
    #[test]
    fn all_layouts_keep_a_320_dip_panel_minimum_without_increasing_height_limits() {
        for stacked in [true, false] {
            for count in 1..=4 {
                let mut app = App::new(PathBuf::from("home"), PathBuf::from("project"), vec![]);
                app.pane_count = count;
                app.stacked = stacked;
                app.rebuild_file_layout();
                let width = minimum_grid_width(app.files_layout.layout()).ceil();
                let regions = app.files_layout.layout().pane_regions(
                    6.0,
                    180.0,
                    iced::Size::new(width, 1000.0),
                );
                assert!(
                    regions
                        .values()
                        .all(|region| region.width >= MIN_PANEL_WIDTH - 0.01),
                    "{regions:?}"
                );
                if stacked {
                    assert_eq!(width, MIN_PANEL_WIDTH);
                }
            }
        }
        let mut app = App::new(PathBuf::from("home"), PathBuf::from("project"), vec![]);
        app.stacked = false;
        app.rebuild_file_layout();
        let pane_grid::Node::Split { id, .. } = app.files_layout.layout() else {
            panic!()
        };
        let split = *id;
        for ratio in [0.01, 0.99] {
            app.resize_file_panels(
                pane_grid::ResizeEvent { split, ratio },
                iced::Size::new(1000.0, 800.0),
            );
            let r =
                app.files_layout
                    .layout()
                    .pane_regions(6.0, 180.0, iced::Size::new(1000.0, 800.0));
            assert!(r.values().all(|region| region.width >= MIN_PANEL_WIDTH));
        }
    }
    #[test]
    fn closing_a_whole_panel_cancels_every_tab_and_keeps_surviving_scans() {
        let mut app = App::new(PathBuf::from("home"), PathBuf::from("project"), vec![]);
        app.preferences.default_layout = preferences::DefaultLayout::Three;
        app.initialize_default_layout();
        app.panes[0]
            .tabs
            .push(Tab::new(PathBuf::from("background")));
        app.panes[0].tabs[0].generation = 101;
        app.panes[0].tabs[1].generation = 102;
        app.panes[1].generation = 103;
        let flags = app.panes[0]
            .tabs
            .iter()
            .map(|tab| tab.cancel.clone())
            .collect::<Vec<_>>();
        let survivor = app.panes[1].identity;
        let _ = app.update(Message::ClosePanel(0));
        assert_eq!(app.pane_count, 2);
        assert_eq!(app.files_layout.len(), 2);
        assert!(flags.iter().all(|flag| flag.load(Ordering::Relaxed)));
        assert_eq!(app.panes[0].identity, survivor);
        for generation in [101, 102] {
            let _ = app.update(Message::Scan(
                0,
                generation,
                Event::Listed(Err("closed".into())),
            ));
        }
        assert!(app.panes[0].error.is_none());
        let _ = app.update(Message::Scan(1, 103, Event::Listed(Err("survived".into()))));
        assert_eq!(app.panes[0].error.as_deref(), Some("survived"));
        assert_eq!(
            app.preferences.default_layout,
            preferences::DefaultLayout::Three
        );
        let _ = app.update(Message::ClosePanel(1));
        let last = app.panes[0].identity;
        let _ = app.update(Message::ClosePanel(0));
        assert_eq!(app.pane_count, 1);
        assert_eq!(app.panes[0].identity, last);
    }
    #[test]
    fn saved_choice_controls_startup_and_runtime_snapping_does_not_change_it() {
        use preferences::DefaultLayout;
        for choice in DefaultLayout::ALL {
            let mut app = App::new(PathBuf::from("home"), PathBuf::from("project"), vec![]);
            let original = app.panes[0].identity;
            let _ = app.update(Message::DefaultLayoutChoice(choice));
            assert_eq!(app.pane_count, 2);
            assert!(app.stacked);
            assert_eq!(app.panes[0].identity, original);
            app.preferences = preferences::Preferences::decode(&app.preferences.encode());
            app.initialize_default_layout();
            let raw =
                app.files_layout
                    .layout()
                    .pane_regions(0.0, 0.0, iced::Size::new(1000.0, 800.0));
            let r: std::collections::BTreeMap<_, _> = app
                .files_layout
                .iter()
                .map(|(pane, id)| (*id, raw[pane]))
                .collect();
            match choice {
                DefaultLayout::Single => {
                    assert_eq!(app.pane_count, 1);
                    assert_eq!(r[&0].width, 1000.0);
                    assert_eq!(r[&0].height, 800.0);
                }
                DefaultLayout::Vertical | DefaultLayout::Remember => {
                    assert_eq!(app.pane_count, 2);
                    assert_eq!(r[&0].height, 400.0);
                    assert_eq!(r[&1].y, 400.0);
                }
                DefaultLayout::Horizontal => {
                    assert_eq!(app.pane_count, 2);
                    assert_eq!(r[&0].width, 500.0);
                    assert_eq!(r[&1].x, 500.0);
                }
                DefaultLayout::Three => {
                    assert_eq!(app.pane_count, 3);
                    assert_eq!(r[&0].height, 800.0);
                    assert_eq!(r[&0].width, 500.0);
                    assert_eq!(r[&1].x, 500.0);
                    assert_eq!(r[&2].y, 400.0);
                }
                DefaultLayout::Grid => {
                    assert_eq!(app.pane_count, 4);
                    assert!(r.values().all(|r| r.width == 500.0 && r.height == 400.0));
                }
            }
            let _ = app.snap_panel(0, docking::Zone(5));
            assert_eq!(app.preferences.default_layout, choice);
            let _ = app.update(Message::RestoreDefaults);
            assert_eq!(app.preferences.default_layout, DefaultLayout::Vertical);
        }
    }
}

#[cfg(test)]
mod last_tab_move_tests {
    use super::*;
    #[test]
    fn moving_final_tab_closes_source_without_duplicating_or_restarting_it() {
        for count in 2..=4 {
            for from in 0..count {
                for to in 0..count {
                    if from == to {
                        continue;
                    }
                    let mut app = App::new("source".into(), "destination".into(), vec![]);
                    app.pane_count = count;
                    app.rebuild_file_layout();
                    app.panes[from].generation = 500 + from as u64;
                    app.panes[from].search = "retained search".into();
                    app.panes[from].address = "retained address".into();
                    let identity = app.panes[from].identity;
                    let cancel = app.panes[from].cancel.clone();
                    let generation = app.panes[from].generation;
                    let target_identity = app.panes[to].identity;
                    let target_key = *app
                        .files_layout
                        .iter()
                        .find(|(_, id)| **id == to)
                        .unwrap()
                        .0;
                    let identities = app.panes[..count]
                        .iter()
                        .flat_map(|p| p.tabs.iter().map(|t| t.identity))
                        .collect::<std::collections::BTreeSet<_>>();
                    let preset = app.preferences.default_layout;
                    let _ = app.update(Message::MoveTab(from, identity, to, 1));
                    let destination = to - usize::from(from < to);
                    assert_eq!(app.pane_count, count - 1);
                    assert_eq!(app.files_layout.len(), count - 1);
                    assert_eq!(app.active, destination);
                    assert_eq!(app.panes[destination].selected_tab, 1);
                    assert_eq!(app.panes[destination].identity, identity);
                    assert_eq!(app.panes[destination].tabs[0].identity, target_identity);
                    assert_eq!(app.panes[destination].search, "retained search");
                    assert_eq!(app.panes[destination].address, "retained address");
                    assert!(!cancel.load(Ordering::Relaxed));

                    assert_eq!(app.preferences.default_layout, preset);
                    assert_eq!(
                        *app.files_layout
                            .iter()
                            .find(|(_, id)| **id == destination)
                            .unwrap()
                            .0,
                        target_key
                    );
                    let visible = app.panes[..app.pane_count]
                        .iter()
                        .flat_map(|p| p.tabs.iter().map(|t| t.identity))
                        .collect::<Vec<_>>();
                    assert_eq!(visible.len(), identities.len());
                    assert_eq!(
                        visible
                            .iter()
                            .copied()
                            .collect::<std::collections::BTreeSet<_>>(),
                        identities
                    );
                    let _ = app.update(Message::Scan(
                        from,
                        generation,
                        Event::Listed(Err("moved result".into())),
                    ));
                    assert_eq!(
                        app.panes[destination].error.as_deref(),
                        Some("moved result")
                    );
                    let _ = app.view();
                }
            }
        }
    }
    #[test]
    fn moving_one_of_multiple_tabs_keeps_source_panel_and_same_panel_reorder_is_safe() {
        let mut app = App::new("one".into(), "two".into(), vec![]);
        app.panes[0].tabs.push(Tab::new("three".into()));
        app.panes[0].selected_tab = 1;
        let identity = app.panes[0].identity;
        let _ = app.update(Message::MoveTab(0, identity, 1, 0));
        assert_eq!(app.pane_count, 2);
        assert_eq!(app.active, 1);
        assert_eq!(app.panes[0].tabs.len(), 1);
        assert_eq!(app.panes[0].selected_tab, 0);
        let _ = app.update(Message::MoveTab(0, app.panes[0].identity, 0, 1));
        assert_eq!(app.pane_count, 2);
        assert_eq!(app.panes[0].tabs.len(), 1);
        assert_eq!(app.panes[0].selected_tab, 0);
    }
    #[test]
    fn panel_widget_keys_survive_tab_selection_docking_and_compaction() {
        let mut app = App::new("one".into(), "two".into(), vec![]);
        let first = app.panes[0].widget_key;
        let second = app.panes[1].widget_key;
        let cancel = app.panes[1].cancel.clone();
        let tab = app.panes[1].identity;
        app.panes[1].search = "preserved".into();
        app.panes[0].tabs.push(Tab::new("another".into()));
        app.panes[0].selected_tab = 1;
        assert_eq!(app.panes[0].widget_key, first);
        let _ = app.update(Message::Dock(
            1,
            None,
            crate::docking::Zone(3).for_workspace(),
            iced::Size::new(1200.0, 700.0),
        ));
        assert_eq!(app.panes[1].widget_key, second);
        assert_eq!(app.panes[1].identity, tab);
        let closed = app.panes[0].cancel.clone();
        let _ = app.update(Message::ClosePanel(0));
        assert!(closed.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(app.pane_count, 1);
        assert_eq!(app.panes[0].widget_key, second);
        assert_eq!(app.panes[0].search, "preserved");
        assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
        let _ = app.update(Message::NewPanel(0));
        assert_eq!(app.panes[0].widget_key, second);
        assert_ne!(app.panes[1].widget_key, first);
        assert_ne!(app.panes[1].widget_key, second);
    }
    #[test]
    fn panel_motion_host_keeps_split_resize_immediate() {
        use iced::advanced::{
            Layout, Shell, layout,
            widget::{Operation, Tree},
        };
        #[derive(Default)]
        struct Panels(Vec<iced::Rectangle>);
        impl Operation for Panels {
            fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
                f(self);
            }
            fn custom(
                &mut self,
                _: Option<&iced::advanced::widget::Id>,
                bounds: iced::Rectangle,
                state: &mut dyn std::any::Any,
            ) {
                if state
                    .downcast_ref::<crate::tab_drag::Marker>()
                    .is_some_and(|m| m.kind == 7)
                {
                    self.0.push(bounds);
                }
            }
        }
        let mut app = App::new("one".into(), "two".into(), vec![]);
        app.motion = crate::motion::Policy::DISABLED;
        let mut widget = app.workspace();
        let mut tree = Tree::new(&widget);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = iced::Size::new(1200.0, 900.0);
        let node =
            widget
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut panels = Panels::default();
        widget
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut panels);
        assert_eq!(panels.0.len(), 2);
        panels
            .0
            .sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        let a = panels.0[0];
        let b = panels.0[1];
        let point = if b.y > a.y {
            iced::Point::new(a.center_x(), (a.y + a.height + b.y) / 2.0)
        } else {
            iced::Point::new((a.x + a.width + b.x) / 2.0, a.center_y())
        };
        let moved = point + iced::Vector::new(24.0, 24.0);
        let mut messages = vec![];
        for (event, point) in [
            (
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
                point,
            ),
            (iced::mouse::Event::CursorMoved { position: moved }, moved),
        ] {
            widget.as_widget_mut().update(
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
                .any(|m| matches!(m, Message::ResizeFiles(..))),
            "{messages:?}"
        );
    }
}
