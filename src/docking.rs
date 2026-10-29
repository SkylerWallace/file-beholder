//! Dock panels against the edges of an existing panel; the center is neutral.
use crate::*;
use iced::{Point, Rectangle};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone(pub u8);
impl Zone {
    pub fn balances_rows(self) -> bool {
        self.workspace() && matches!(self.edge(), 1 | 7)
    }
    pub fn workspace(self) -> bool {
        self.target() == 4
    }
    pub fn for_workspace(self) -> Self {
        Self(36 + self.edge())
    }
    pub fn target(self) -> usize {
        (self.0 / 9) as usize
    }
    pub fn edge(self) -> u8 {
        self.0 % 9
    }
    pub fn for_panel(self, panel: usize) -> Self {
        Self(panel as u8 * 9 + self.edge())
    }
    pub fn at(bounds: Rectangle, point: Point) -> Option<Self> {
        use sk_iced::components::DockEdge;
        DockEdge::at(bounds, point).map(|edge| {
            Self(match edge {
                DockEdge::Top => 1,
                DockEdge::Left => 3,
                DockEdge::Right => 5,
                DockEdge::Bottom => 7,
            })
        })
    }
    pub fn valid(self) -> bool {
        self.target() <= 4 && matches!(self.edge(), 1 | 3 | 5 | 7)
    }
}
pub fn relocate(layout: &mut container_layout::Layout, source: usize, zone: Zone) -> bool {
    layout.move_panel(source, zone)
}

impl App {
    pub(super) fn dock(
        &mut self,
        from: usize,
        identity: Option<u64>,
        zone: Zone,
        size: iced::Size,
    ) -> Task<Message> {
        if from >= self.pane_count
            || !zone.valid()
            || (!zone.workspace() && zone.target() >= self.pane_count)
        {
            return Task::none();
        }
        self.files_layout.set_size(size);
        let task = if let Some(identity) = identity {
            self.snap_tab(from, identity, zone)
        } else {
            self.snap_panel(from, zone)
        };
        self.files_layout
            .set_size(self.files_layout.canvas_size(size));
        task
    }
    pub(super) fn snap_panel(&mut self, id: usize, zone: Zone) -> Task<Message> {
        if id >= self.pane_count
            || (!zone.workspace() && zone.target() >= self.pane_count)
            || !relocate(&mut self.files_layout, id, zone)
        {
            return Task::none();
        }
        self.context = None;
        self.drag_files = None;
        self.active = id;
        self.panes[id].drop_highlight = highlight::Pulse::new(self.motion);

        Task::none()
    }
    pub(super) fn snap_tab(&mut self, from: usize, identity: u64, zone: Zone) -> Task<Message> {
        if from >= self.pane_count
            || (!zone.workspace() && zone.target() >= self.pane_count)
            || !zone.valid()
        {
            return Task::none();
        }
        let Some(index) = self.panes[from]
            .tabs
            .iter()
            .position(|tab| tab.identity == identity)
        else {
            return Task::none();
        };
        if self.panes[from].tabs.len() == 1 {
            return self.snap_panel(from, zone);
        }
        if self.pane_count == 4 {
            self.notice="Four panels are already open. Move the tab to an existing panel, or close a panel first.".into();
            return Task::none();
        }
        let destination = self.pane_count;
        if !self.files_layout.add_panel(destination, zone) {
            return Task::none();
        }
        let tab = self.panes[from].tabs.remove(index);
        let selected = self.panes[from].selected_tab;
        self.panes[from].selected_tab = if index < selected {
            selected - 1
        } else {
            selected.min(self.panes[from].tabs.len() - 1)
        };
        if self.panes[destination].tabs.len() == 1 && self.panes[destination].generation == 0 {
            self.panes[destination].tabs.clear();
        }
        self.panes[destination].tabs.push(tab);
        self.panes[destination].selected_tab = self.panes[destination].tabs.len() - 1;
        self.pane_count += 1;
        self.context = None;
        self.drag_files = None;
        self.active = destination;
        self.panes[destination].drop_highlight = highlight::Pulse::new(self.motion);

        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_edges_change_two_panel_orientation_from_either_source() {
        for preset in [
            preferences::DefaultLayout::Horizontal,
            preferences::DefaultLayout::Vertical,
        ] {
            for source in 0..2 {
                for edge in [1, 3, 5, 7] {
                    let mut app = App::new("one".into(), "two".into(), vec![]);
                    app.preferences.default_layout = preset;
                    app.initialize_default_layout();
                    let identities = app.panes.iter().map(|p| p.identity).collect::<Vec<_>>();
                    let _ = app.snap_panel(source, Zone(edge).for_workspace());
                    assert_eq!(app.pane_count, 2);
                    assert_eq!(
                        identities,
                        app.panes.iter().map(|p| p.identity).collect::<Vec<_>>()
                    );
                    let pane_grid::Node::Split { axis, .. } = app.files_layout.layout() else {
                        panic!("split")
                    };
                    assert_eq!(
                        *axis,
                        if matches!(edge, 1 | 7) {
                            pane_grid::Axis::Horizontal
                        } else {
                            pane_grid::Axis::Vertical
                        }
                    );
                    assert_eq!(app.preferences.default_layout, preset);
                }
            }
        }
    }
    #[test]
    fn four_panels_allow_two_tall_columns_beside_a_stacked_pair_and_back_to_grid() {
        let mut app = App::new("one".into(), "two".into(), vec![]);
        app.preferences.default_layout = preferences::DefaultLayout::Grid;
        app.initialize_default_layout();
        let _ = app.snap_panel(0, Zone(3).for_workspace());
        let _ = app.snap_panel(2, Zone(3).for_workspace());
        let size = iced::Size::new(1600., 800.);
        let regions = app.files_layout.layout().pane_regions(6., 180., size);
        let rect = |id| regions[app.files_layout.iter().find(|(_, v)| **v == id).unwrap().0];
        assert_eq!(rect(0).height, 800.);
        assert_eq!(rect(2).height, 800.);
        assert_eq!(rect(1).x, rect(3).x);
        assert_eq!(rect(1).width, rect(3).width);
        assert_eq!(rect(1).height, 397.);
        assert_eq!(rect(3).height, 397.);
        assert_eq!(app.pane_count, 4);
        let _ = app.snap_panel(2, Zone(7).for_panel(0));
        let regions = app.files_layout.layout().pane_regions(6., 180., size);
        for rect in regions.values() {
            assert_eq!(rect.height, 397.);
        }
        assert_eq!(app.pane_count, 4);
    }
    #[test]
    fn tab_tear_off_preserves_identity_and_scan_and_rejects_fifth_panel() {
        let mut app = App::new("one".into(), "two".into(), vec![]);
        app.panes[0].tabs.push(Tab::new("pending".into()));
        app.panes[0].selected_tab = 1;
        app.panes[0].generation = 123;
        let identity = app.panes[0].identity;
        let cancel = app.panes[0].cancel.clone();
        let _ = app.snap_tab(0, identity, Zone(5).for_panel(1));
        assert_eq!(app.pane_count, 3);
        assert_eq!(app.panes[2].identity, identity);
        assert!(!cancel.load(Ordering::Relaxed));
        let _ = app.update(Message::Scan(0, 123, Event::Listed(Err("pending".into()))));
        assert_eq!(app.panes[2].error.as_deref(), Some("pending"));
        app.panes[0].tabs.push(Tab::new("fourth".into()));
        let id = app.panes[0].tabs.last().unwrap().identity;
        let _ = app.snap_tab(0, id, Zone(7).for_panel(2));
        assert_eq!(app.pane_count, 4);
        app.panes[0].tabs.push(Tab::new("fifth".into()));
        let id = app.panes[0].tabs.last().unwrap().identity;
        let _ = app.snap_tab(0, id, Zone(3).for_panel(1));
        assert_eq!(app.pane_count, 4);
        assert!(app.panes[0].tabs.iter().any(|t| t.identity == id));
    }
    #[test]
    fn moving_only_panel_or_dropping_on_self_creates_nothing() {
        let mut app = App::new("one".into(), "two".into(), vec![]);
        app.preferences.default_layout = preferences::DefaultLayout::Single;
        app.initialize_default_layout();
        for edge in [1, 3, 5, 7] {
            let _ = app.snap_panel(0, Zone(edge));
            assert_eq!(app.pane_count, 1);
        }
    }
}
