//! Internal tab dragging. Geometry is collected from the current widget tree,
//! including scroll offsets and viewport clips; pointer motion stays local.
use crate::{Message, palette, theme::color};
use iced::advanced::{
    Clipboard, Layout, Renderer as _, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};
#[derive(Clone, Debug)]
pub struct Marker {
    pub pane: usize,
    pub identity: Option<u64>,
    pub index: usize,
    pub kind: u8,
    pub label: String,
    pub directory: Option<std::path::PathBuf>,
    pub layout: Option<std::sync::Arc<crate::container_layout::Layout>>,
}
impl Marker {
    pub fn folder(mut self, path: std::path::PathBuf) -> Self {
        self.directory = Some(path);
        self
    }
    pub fn tree_directory(path: std::path::PathBuf, label: String) -> Self {
        let mut marker = Self::file(usize::MAX, 0, label, 1).folder(path);
        marker.kind = 12;
        marker
    }
    pub fn docking_canvas(layout: &impl crate::container_layout::Snapshot) -> Self {
        let layout = layout.snapshot();
        let mut marker = Self::workspace_for(layout.len());
        marker.kind = 8;
        marker.layout = Some(std::sync::Arc::new(layout));
        marker
    }
    pub fn docking_panel(pane: usize) -> Self {
        let mut marker = Self::panel_grip(pane);
        marker.kind = 7;
        marker
    }
    pub fn grip_icon(pane: usize) -> Self {
        let mut marker = Self::panel_grip(pane);
        marker.kind = 9;
        marker
    }
    pub fn panel_grip(pane: usize) -> Self {
        Self {
            pane,
            identity: None,
            index: 0,
            kind: 4,
            layout: None,
            directory: None,
            label: format!("Panel {}", pane + 1),
        }
    }
    pub fn tab_control(pane: usize) -> Self {
        Self {
            pane,
            identity: None,
            index: 0,
            kind: 6,
            layout: None,
            directory: None,
            label: String::new(),
        }
    }
    pub fn workspace_for(count: usize) -> Self {
        let mut marker = Self::workspace();
        marker.index = count;
        marker
    }
    pub fn workspace() -> Self {
        Self {
            pane: 0,
            identity: None,
            index: 2,
            kind: 5,
            layout: None,
            directory: None,
            label: String::new(),
        }
    }

    pub fn file(pane: usize, index: usize, name: String, count: usize) -> Self {
        Self {
            pane,
            index,
            identity: None,
            kind: 2,
            layout: None,
            directory: None,
            label: if count > 1 {
                format!("{count} items")
            } else {
                name
            },
        }
    }
    pub fn file_panel(pane: usize) -> Self {
        Self {
            pane,
            index: 0,
            identity: None,
            kind: 3,
            layout: None,
            directory: None,
            label: String::new(),
        }
    }
    pub fn tab(pane: usize, identity: u64, index: usize, label: String) -> Self {
        Self {
            pane,
            identity: Some(identity),
            index,
            kind: 0,
            layout: None,
            directory: None,
            label,
        }
    }
    pub fn address(pane: usize) -> Self {
        let mut marker = Self::strip(pane, 0);
        marker.kind = 11;
        marker
    }
    pub fn headers(pane: usize) -> Self {
        let mut marker = Self::strip(pane, 0);
        marker.kind = 10;
        marker
    }
    pub fn strip(pane: usize, count: usize) -> Self {
        Self {
            pane,
            identity: None,
            index: count,
            kind: 1,
            layout: None,
            directory: None,
            label: String::new(),
        }
    }
}
pub fn mark<'a>(content: Element<'a, Message>, marker: Marker) -> Element<'a, Message> {
    let hideable = marker.kind == 9;
    let interaction = if marker.kind == 4 {
        mouse::Interaction::Grab
    } else {
        mouse::Interaction::default()
    };
    sk_iced::components::DragHandle::new(content, marker)
        .hideable(hideable)
        .interaction(interaction)
        .build()
}
pub fn layer<'a>(content: Element<'a, Message>, enabled: bool) -> Element<'a, Message> {
    Element::new(Frame {
        content,
        drag: true,
        enabled,
    })
}
struct Frame<'a> {
    content: Element<'a, Message>,
    drag: bool,
    enabled: bool,
}
type PreviewParagraph = <Renderer as iced::advanced::text::Renderer>::Paragraph;
#[cfg(test)]
mod folder_drop_tests {
    use super::*;
    fn app() -> crate::App {
        let mut app = crate::App::new("home".into(), "other".into(), vec![]);
        app.panes[0].entries = [true, false]
            .into_iter()
            .map(|directory| crate::Entry {
                path: if directory {
                    "home/folder"
                } else {
                    "home/file.txt"
                }
                .into(),
                name: if directory { "folder" } else { "file.txt" }.into(),
                directory,
                hidden: false,
                bytes: Some(1),
                partial: false,
                modified: None,
                modified_label: String::new(),
                icon: None,
            })
            .collect();
        app.panes[0].reorder();
        app
    }
    fn geometry<'a>(
        element: &mut Element<'a, Message>,
        renderer: &Renderer,
    ) -> (Tree, layout::Node, Collect) {
        let mut tree = Tree::new(&*element);
        let size = Size::new(1400., 900.);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, renderer, &layout::Limits::new(size, size));
        let mut collect = Collect::new();
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), renderer, &mut collect);
        (tree, node, collect)
    }
    #[test]
    fn folders_from_tree_and_panels_drop_onto_either_tab_bar_without_transferring() {
        for source_kind in [12, 2] {
            for destination in [0, 1] {
                let mut app = app();
                let mut element = app.view();
                let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
                let (mut tree, node, collect) = geometry(&mut element, &renderer);
                let source = collect
                    .regions
                    .iter()
                    .find(|r| r.marker.kind == source_kind && r.marker.directory.is_some())
                    .unwrap();
                let path = source.marker.directory.clone().unwrap();
                let start = source.bounds.center();
                let end = collect
                    .regions
                    .iter()
                    .find(|r| r.marker.kind == 1 && r.marker.pane == destination)
                    .unwrap()
                    .bounds
                    .center();
                let mut messages = vec![];
                for (event, point) in [
                    (mouse::Event::ButtonPressed(mouse::Button::Left), start),
                    (mouse::Event::CursorMoved { position: end }, end),
                    (mouse::Event::ButtonReleased(mouse::Button::Left), end),
                ] {
                    element.as_widget_mut().update(
                        &mut tree,
                        &Event::Mouse(event),
                        Layout::new(&node),
                        mouse::Cursor::Available(point),
                        &renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut Shell::new(&mut messages),
                        &node.bounds(),
                    );
                }
                assert!(!messages.iter().any(|m| matches!(
                    m,
                    Message::FileDrop(..)
                        | Message::MoveTab(..)
                        | Message::Dock(..)
                        | Message::Locations(crate::locations::Message::Navigate(_))
                )));
                let message = messages.iter().find(|m| matches!(m, Message::FolderTabDrop(pane, _, p, _) if *pane == destination && *p == path)).expect("Folder drop must open a tab").clone();
                drop(element);
                let _ = app.update(message);
                assert_eq!(app.active, destination);
                assert_eq!(app.panes[destination].path, path);
                assert_eq!(app.panes[destination].tabs.len(), 2);
                assert!(app.jobs.is_empty());
                assert!(app.drop_confirmation.is_none());
            }
        }
    }
    #[test]
    fn files_do_not_open_as_folder_tabs() {
        let app = app();
        let mut element = app.view();
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let (mut tree, node, collect) = geometry(&mut element, &renderer);
        let start = collect
            .regions
            .iter()
            .find(|r| r.marker.kind == 2 && r.marker.directory.is_none())
            .unwrap()
            .bounds
            .center();
        let end = collect
            .regions
            .iter()
            .find(|r| r.marker.kind == 1 && r.marker.pane == 1)
            .unwrap()
            .bounds
            .center();
        let mut messages = vec![];
        for (event, point) in [
            (mouse::Event::ButtonPressed(mouse::Button::Left), start),
            (mouse::Event::CursorMoved { position: end }, end),
            (mouse::Event::ButtonReleased(mouse::Button::Left), end),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            );
        }
        assert!(
            !messages
                .iter()
                .any(|m| matches!(m, Message::FolderTabDrop(..) | Message::FileDrop(..)))
        );
        assert!(messages.iter().any(|m| matches!(m, Message::CancelDrag)));
    }
    #[test]
    fn disabled_open_menu_rows_above_panel_grips_do_not_show_drag_cursors() {
        let mut app = app();
        app.preferences.open_files = false;
        app.active = 0;
        app.panes[0].selected = Some(1);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let grip = {
            let mut element = app.view();
            let (tree, node, collect) = geometry(&mut element, &renderer);
            let grip = collect
                .regions
                .iter()
                .filter(|r| r.marker.kind == 4)
                .max_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
                .unwrap()
                .bounds
                .center();
            assert_eq!(
                element.as_widget().mouse_interaction(
                    &tree,
                    Layout::new(&node),
                    mouse::Cursor::Available(grip),
                    &node.bounds(),
                    &renderer
                ),
                mouse::Interaction::Grab
            );
            grip
        };
        for index in [2, 3] {
            let offset = {
                let mut menu = app.file_context_menu();
                let mut tree = Tree::new(&menu);
                let node = menu.as_widget_mut().layout(
                    &mut tree,
                    &renderer,
                    &layout::Limits::new(Size::ZERO, Size::new(400., 700.)),
                );
                node.children()[0].children()[index].bounds().center().y
            };
            app.context = Some(Point::new(grip.x - 20., grip.y - offset));
            let mut element = app.view();
            let (mut tree, node, _) = geometry(&mut element, &renderer);
            assert_eq!(
                element.as_widget().mouse_interaction(
                    &tree,
                    Layout::new(&node),
                    mouse::Cursor::Available(grip),
                    &node.bounds(),
                    &renderer
                ),
                mouse::Interaction::Idle
            );
            let mut popup = element
                .as_widget_mut()
                .overlay(
                    &mut tree,
                    Layout::new(&node),
                    &renderer,
                    &node.bounds(),
                    Vector::ZERO,
                )
                .unwrap();
            let popup_node = popup.as_overlay_mut().layout(&renderer, node.size());
            assert_eq!(
                popup.as_overlay().mouse_interaction(
                    Layout::new(&popup_node),
                    mouse::Cursor::Available(grip),
                    &renderer
                ),
                mouse::Interaction::Idle
            );
        }
    }
}
fn preview_label(label: &str) -> String {
    let mut chars = label.chars();
    let mut text = chars
        .by_ref()
        .take(80)
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                c
            }
        })
        .collect::<String>();
    if chars.next().is_some() {
        text.push('…');
    }
    text
}
#[derive(Default)]
struct State {
    source: Option<(Marker, Point)>,
    dragging: bool,
    position: Point,
    target: Option<(usize, usize, f32, f32)>,
    snap_bounds: Option<Rectangle>,
    snap_zone: Option<crate::docking::Zone>,
    snap_preview: Option<Rectangle>,
    snap_clip: Option<Rectangle>,
    preview_text: std::cell::RefCell<Option<(String, PreviewParagraph)>>,
    drop_label: Option<String>,
}
type Region = sk_iced::components::DragRegion<Marker>;
struct GripVisibility(bool);
impl Operation for GripVisibility {
    fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
        f(self);
    }
    fn custom(
        &mut self,
        _: Option<&iced::advanced::widget::Id>,
        _: Rectangle,
        value: &mut dyn std::any::Any,
    ) {
        if let Some(state) = value.downcast_mut::<sk_iced::components::DragVisibility>() {
            state.hidden = self.0;
        }
    }
}
type Collect = sk_iced::components::DragRegions<Marker>;
fn file_panel(regions: &[Region], point: Point) -> Option<&Region> {
    regions
        .iter()
        .find(|r| r.marker.kind == 3 && r.bounds.contains(point))
}
fn file_hit(regions: &[Region], point: Point) -> Option<&Region> {
    let panel = file_panel(regions, point)?;
    regions.iter().find(|r| {
        r.marker.kind == 2
            && r.marker.pane == panel.marker.pane
            && r.bounds.contains(point)
            && point.x < panel.bounds.x + panel.bounds.width - 16.0
    })
}
// Keep blank row space available for dropping into the panel root.
fn folder_hit(regions: &[Region], point: Point) -> Option<&Region> {
    let hit = file_hit(regions, point)?;
    (hit.marker.directory.is_some()
        && point.x
            < hit.bounds.x
                + (hit.marker.label.chars().count() as f32 * 7.0 + 90.0).clamp(120.0, 320.0))
    .then_some(hit)
}
fn target(regions: &[Region], position: Point) -> Option<(usize, usize, f32, f32)> {
    let strip = regions
        .iter()
        .find(|r| r.marker.kind == 1 && r.bounds.contains(position))?;
    let mut tabs: Vec<_> = regions
        .iter()
        .filter(|r| {
            r.marker.pane == strip.marker.pane
                && r.marker.identity.is_some()
                && r.bounds.intersects(&strip.bounds)
        })
        .collect();
    tabs.sort_by_key(|r| r.marker.index);
    for tab in &tabs {
        if position.x < tab.full.center_x() {
            return Some((
                strip.marker.pane,
                tab.marker.index,
                tab.bounds.x,
                strip.bounds.y,
            ));
        }
    }
    let x = tabs
        .last()
        .map_or(strip.bounds.x, |r| r.bounds.x + r.bounds.width);
    Some((strip.marker.pane, strip.marker.index, x, strip.bounds.y))
}
// Extreme edges dock against the whole workspace, including over the source panel.
fn workspace_target(regions: &[Region], point: Point) -> Option<(Rectangle, crate::docking::Zone)> {
    let bounds = regions.iter().find(|r| r.marker.kind == 5)?.bounds;
    if !bounds.contains(point) {
        return None;
    }
    let (distance, edge) = [
        (point.y - bounds.y, 1),
        (point.x - bounds.x, 3),
        (bounds.x + bounds.width - point.x, 5),
        (bounds.y + bounds.height - point.y, 7),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))?;
    (distance <= 16.0).then_some((bounds, crate::docking::Zone(edge).for_workspace()))
}
fn priority_target(
    regions: &[Region],
    _: &Marker,
    point: Point,
) -> Option<(Rectangle, crate::docking::Zone)> {
    workspace_target(regions, point)
}
fn dock_target(
    regions: &[Region],
    source: &Marker,
    point: Point,
) -> Option<(Rectangle, crate::docking::Zone)> {
    if let Some(target) = priority_target(regions, source, point) {
        return Some(target);
    }
    let tear_off = source.kind == 0
        && regions
            .iter()
            .any(|r| r.marker.kind == 1 && r.marker.pane == source.pane && r.marker.index > 1);
    let panel = regions.iter().find(|r| {
        r.marker.kind == 7 && r.bounds.contains(point) && (tear_off || r.marker.pane != source.pane)
    })?;
    let zone = crate::docking::Zone::at(panel.full, point)?.for_panel(panel.marker.pane);
    Some((panel.full, zone))
}
fn predicted_preview(
    regions: &[Region],
    source: &Marker,
    _: Rectangle,
    zone: crate::docking::Zone,
) -> Option<Rectangle> {
    let canvas = regions.iter().find(|r| r.marker.kind == 8)?;
    let mut layout = canvas.marker.layout.as_ref()?.as_ref().clone();
    layout.set_size(canvas.full.size());
    let tear_off = source.kind == 0
        && regions
            .iter()
            .any(|r| r.marker.kind == 1 && r.marker.pane == source.pane && r.marker.index > 1);
    let id = if tear_off { layout.len() } else { source.pane };
    let moved = if tear_off {
        layout.add_panel(id, zone)
    } else {
        layout.move_panel(id, zone)
    };
    if !moved {
        return None;
    }
    let viewport = regions
        .iter()
        .find(|r| r.marker.kind == 5)
        .map_or(canvas.full.size(), |r| {
            Size::new(r.bounds.width.max(0.0), r.bounds.height)
        });
    let size = layout.canvas_size(viewport);
    layout.set_size(size);
    let panes = layout.layout().pane_regions(6.0, 180.0, size);
    let pane = layout.iter().find(|(_, v)| **v == id)?.0;
    // A narrower rearrangement can clamp the workspace's existing horizontal offset.
    let origin_x = regions
        .iter()
        .find(|r| r.marker.kind == 5)
        .map_or(canvas.full.x, |r| {
            let left = r.bounds.x;
            let viewport = r.bounds.width.max(0.0);
            let offset = (left - canvas.full.x)
                .max(0.0)
                .min((size.width - viewport).max(0.0));
            left - offset
        });
    let rect = panes[pane] + Vector::new(origin_x, canvas.full.y);
    Some(rect)
}
impl Widget<Message, Theme, Renderer> for Frame<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, t: &mut Tree) {
        t.diff_children(&[&self.content]);
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(&mut self, t: &mut Tree, r: &Renderer, l: &layout::Limits) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut t.children[0], r, l)
    }
    fn draw(
        &self,
        t: &Tree,
        r: &mut Renderer,
        th: &Theme,
        s: &renderer::Style,
        l: Layout<'_>,
        c: mouse::Cursor,
        v: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &t.children[0],
            r,
            th,
            s,
            l,
            if self.drag && t.state.downcast_ref::<State>().dragging {
                mouse::Cursor::Unavailable
            } else {
                c
            },
            v,
        );
        let state = t.state.downcast_ref::<State>();
        if self.drag && self.enabled && state.dragging {
            let tokens = palette::from_theme(th);
            if let Some((_, _, x, y)) = state.target {
                sk_iced::components::DropIndicator::new(
                    Rectangle {
                        x: x - 1.0,
                        y,
                        width: 3.0,
                        height: 32.0,
                    },
                    color(tokens.accent),
                )
                .draw(r);
            }
            if let Some(preview) = state.snap_preview {
                sk_iced::components::DropIndicator::new(
                    preview,
                    color(tokens.accent).scale_alpha(0.18),
                )
                .border(color(tokens.accent), 2.0, 6.0)
                .clip(state.snap_clip)
                .draw(r);
            }
            let label = state
                .drop_label
                .as_deref()
                .or_else(|| state.source.as_ref().map(|(m, _)| m.label.as_str()))
                .unwrap_or("");
            let width = (label.chars().take(40).count() as f32 * 7.0 + 28.0).clamp(120.0, 300.0);
            let preview = Rectangle {
                x: (state.position.x + 12.0).min((l.bounds().width - width).max(0.0)),
                y: (state.position.y + 14.0).min((l.bounds().height - 38.0).max(0.0)),
                width,
                height: 36.0,
            };
            // Isolate the preview above file text/images and invalidate its entire
            // old/new bounds. Blurred shadows bypass this backend's damage clip.
            r.with_layer(preview.expand(1.0), |r| {
                r.fill_quad(
                    renderer::Quad {
                        bounds: preview,
                        border: iced::Border {
                            width: 1.0,
                            color: color(tokens.accent),
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    },
                    color(tokens.surface),
                );
                use iced::advanced::text::{Paragraph as _, Renderer as _};
                let label = preview_label(label);
                let mut cached = state.preview_text.borrow_mut();
                if cached.as_ref().is_none_or(|(text, _)| *text != label) {
                    let paragraph = PreviewParagraph::with_text(iced::advanced::text::Text {
                        content: label.as_str(),
                        bounds: Size::new(width - 20., 18.),
                        size: 13.0.into(),
                        line_height: iced::widget::text::LineHeight::Absolute(16.0.into()),
                        font: crate::typography::font(),
                        align_x: iced::alignment::Horizontal::Left.into(),
                        align_y: iced::alignment::Vertical::Center,
                        shaping: iced::widget::text::Shaping::Advanced,
                        wrapping: iced::widget::text::Wrapping::None,
                    });
                    *cached = Some((label, paragraph));
                }
                let clip = Rectangle {
                    x: preview.x + 10.,
                    y: preview.y + 2.,
                    width: width - 20.,
                    height: preview.height - 4.,
                };
                r.with_layer(clip, |r| {
                    r.fill_paragraph(
                        &cached.as_ref().unwrap().1,
                        Point::new(preview.x + 10., preview.center_y()),
                        color(tokens.text),
                        clip,
                    )
                });
            });
        }
    }
    fn operate(&mut self, t: &mut Tree, l: Layout<'_>, r: &Renderer, o: &mut dyn Operation) {
        self.content
            .as_widget_mut()
            .operate(&mut t.children[0], l, r, o);
    }
    fn update(
        &mut self,
        t: &mut Tree,
        e: &Event,
        l: Layout<'_>,
        c: mouse::Cursor,
        r: &Renderer,
        cb: &mut dyn Clipboard,
        s: &mut Shell<'_, Message>,
        v: &Rectangle,
    ) {
        if self.drag {
            if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = e {
                if modifiers.control()
                    && matches!(key.as_ref(), iced::keyboard::Key::Character("z"))
                {
                    let mut collect = Collect::new();
                    self.content
                        .as_widget_mut()
                        .operate(&mut t.children[0], l, r, &mut collect);
                    if collect.focused_input {
                        s.capture_event();
                        return;
                    }
                }
            }
        }
        if self.drag && self.enabled {
            let collect_now = |content: &mut Element<'_, Message>, tree: &mut Tree| {
                let mut collect = Collect::new();
                content.as_widget_mut().operate(tree, l, r, &mut collect);
                collect
            };
            if matches!(
                e,
                Event::Mouse(mouse::Event::ButtonPressed(_))
                    | Event::Window(iced::window::Event::Unfocused)
            ) {
                let collected = collect_now(&mut self.content, &mut t.children[0]);
                for region in collected.regions.iter().filter(|r| r.marker.kind == 11) {
                    if !c.is_over(region.bounds)
                        || matches!(e, Event::Window(iced::window::Event::Unfocused))
                    {
                        s.publish(Message::AddressBlur(region.marker.pane));
                    }
                }
            }
            match e {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                    if let Some(p) = c.position() {
                        let collected = collect_now(&mut self.content, &mut t.children[0]);
                        if let Some(header) = collected
                            .regions
                            .iter()
                            .find(|r| r.marker.kind == 10 && r.bounds.contains(p))
                        {
                            s.publish(Message::HeaderContext(header.marker.pane, p));
                            s.capture_event();
                            return;
                        }
                        if let Some(panel) = file_panel(&collected.regions, p) {
                            let index = file_hit(&collected.regions, p).map(|h| h.marker.index);
                            s.publish(Message::Context(panel.marker.pane, index, p));
                            s.capture_event();
                            return;
                        }
                    }
                }
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    if let Some(p) = c.position() {
                        let collected = collect_now(&mut self.content, &mut t.children[0]);
                        let tab_hit = collected
                            .regions
                            .iter()
                            .find(|r| matches!(r.marker.kind, 0 | 6) && r.bounds.contains(p));
                        let interactive = tab_hit.is_some();
                        let hit = collected
                            .regions
                            .iter()
                            .find(|r| r.marker.kind == 12 && r.bounds.contains(p))
                            .or_else(|| file_hit(&collected.regions, p))
                            .or_else(|| {
                                tab_hit.filter(|r| {
                                    r.marker.kind == 0
                                        && r.bounds.contains(p)
                                        && p.x < r.full.x + r.full.width - 28.0
                                })
                            })
                            .or_else(|| {
                                (!interactive)
                                    .then(|| {
                                        collected
                                            .regions
                                            .iter()
                                            .find(|r| r.marker.kind == 4 && r.bounds.contains(p))
                                    })
                                    .flatten()
                            });
                        let state = t.state.downcast_mut::<State>();
                        state.source = hit.map(|h| (h.marker.clone(), p));
                        state.position = p;
                        state.dragging = false;
                        state.target = None;
                        state.snap_bounds = None;
                        state.snap_zone = None;
                        state.snap_preview = None;
                        state.snap_clip = None;
                    }
                }
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    let state = t.state.downcast_mut::<State>();
                    if let Some((source, origin)) = state.source.as_ref() {
                        let first = !state.dragging
                            && sk_iced::components::DragGesture::crossed(*origin, *position, 5.0);
                        if first && source.kind == 2 {
                            s.publish(Message::FileDragStarted(source.pane, source.index));
                        }
                        state.dragging |= first;
                        state.position = *position;
                        if state.dragging {
                            let collected = collect_now(&mut self.content, &mut t.children[0]);
                            state.target = if source.kind == 0
                                && priority_target(&collected.regions, source, *position).is_none()
                            {
                                target(&collected.regions, *position)
                            } else if source.directory.is_some() {
                                target(&collected.regions, *position).or_else(|| {
                                    (source.kind == 2)
                                        .then(|| {
                                            file_panel(&collected.regions, *position)
                                                .map(|p| (p.marker.pane, 0, p.bounds.x, p.bounds.y))
                                        })
                                        .flatten()
                                })
                            } else if source.kind == 2 {
                                file_panel(&collected.regions, *position)
                                    .map(|p| (p.marker.pane, 0, p.bounds.x, p.bounds.y))
                            } else {
                                None
                            };
                            if source.kind == 2 {
                                state.drop_label = folder_hit(&collected.regions, *position)
                                    .map(|folder| format!("Drop into {}", folder.marker.label))
                                    .or_else(|| {
                                        file_panel(&collected.regions, *position)
                                            .map(|_| "Drop into current folder".into())
                                    });
                                state.snap_preview = folder_hit(&collected.regions, *position)
                                    .map(|folder| Rectangle {
                                        width: (folder.marker.label.chars().count() as f32 * 7.0
                                            + 90.0)
                                            .clamp(120.0, 320.0)
                                            .min(folder.bounds.width),
                                        ..folder.bounds
                                    })
                                    .or_else(|| {
                                        file_panel(&collected.regions, *position)
                                            .map(|panel| panel.bounds)
                                    });
                            }
                            let docking = if source.kind == 4
                                || (source.kind == 0 && state.target.is_none())
                            {
                                dock_target(&collected.regions, source, *position)
                            } else {
                                None
                            };
                            state.snap_clip = collected
                                .regions
                                .iter()
                                .find(|r| r.marker.kind == 5)
                                .map(|r| r.bounds);
                            state.snap_bounds = docking.map(|d| d.0);
                            state.snap_zone = docking.map(|d| d.1);
                            state.snap_preview = if source.kind == 2 {
                                state.snap_preview
                            } else {
                                docking.and_then(|(bounds, zone)| {
                                    predicted_preview(&collected.regions, source, bounds, zone)
                                })
                            };
                            self.content.as_widget_mut().operate(
                                &mut t.children[0],
                                l,
                                r,
                                &mut GripVisibility(source.kind == 4),
                            );
                            s.request_redraw();
                            s.capture_event();
                            return;
                        }
                    }
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    let state = t.state.downcast_mut::<State>();
                    let was_dragging = state.dragging;
                    let source = state.source.take();
                    state.dragging = false;
                    state.target = None;
                    state.snap_bounds = None;
                    state.snap_zone = None;
                    state.snap_preview = None;
                    state.snap_clip = None;
                    self.content.as_widget_mut().operate(
                        &mut t.children[0],
                        l,
                        r,
                        &mut GripVisibility(false),
                    );
                    if was_dragging {
                        let collected = collect_now(&mut self.content, &mut t.children[0]);
                        if let (Some((source, _)), Some(position)) = (source, c.position()) {
                            if source.kind == 0 || source.kind == 4 {
                                if source.kind == 0
                                    && priority_target(&collected.regions, &source, position)
                                        .is_none()
                                    && let Some((pane, index, _, _)) =
                                        target(&collected.regions, position)
                                {
                                    s.publish(Message::MoveTab(
                                        source.pane,
                                        source.identity.unwrap(),
                                        pane,
                                        index,
                                    ));
                                } else if let Some((bounds, zone)) =
                                    dock_target(&collected.regions, &source, position)
                                    && predicted_preview(&collected.regions, &source, bounds, zone)
                                        .is_some()
                                {
                                    if let Some(canvas) =
                                        collected.regions.iter().find(|r| r.marker.kind == 8)
                                    {
                                        s.publish(Message::Dock(
                                            source.pane,
                                            if source.kind == 0 {
                                                source.identity
                                            } else {
                                                None
                                            },
                                            zone,
                                            Size::new(
                                                collected
                                                    .regions
                                                    .iter()
                                                    .find(|r| r.marker.kind == 5)
                                                    .map_or(canvas.full.width, |r| {
                                                        r.bounds.width.max(0.0)
                                                    }),
                                                canvas.full.height,
                                            ),
                                        ));
                                    }
                                }
                            } else if let Some(path) = source.directory.as_ref()
                                && let Some((pane, index, _, _)) =
                                    target(&collected.regions, position)
                            {
                                s.publish(Message::FolderTabDrop(
                                    pane,
                                    index,
                                    path.clone(),
                                    (source.kind == 2).then_some(source.pane),
                                ));
                            } else if source.kind == 2
                                && let Some(folder) = folder_hit(&collected.regions, position)
                            {
                                s.publish(Message::FileDropInto(
                                    source.pane,
                                    source.index,
                                    folder.marker.pane,
                                    folder.marker.directory.clone().unwrap(),
                                ));
                            } else if source.kind == 2
                                && let Some(panel) = file_panel(&collected.regions, position)
                            {
                                s.publish(Message::FileDrop(
                                    source.pane,
                                    source.index,
                                    panel.marker.pane,
                                ));
                            } else {
                                s.publish(Message::CancelDrag);
                            }
                        }
                        self.content.as_widget_mut().update(
                            &mut t.children[0],
                            e,
                            l,
                            mouse::Cursor::Unavailable,
                            r,
                            cb,
                            s,
                            v,
                        );
                        s.request_redraw();
                        s.capture_event();
                        return;
                    }
                }
                Event::Window(iced::window::Event::Unfocused)
                | Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => {
                    let state = t.state.downcast_mut::<State>();
                    if state.source.take().is_some() {
                        state.dragging = false;
                        state.target = None;
                        state.snap_bounds = None;
                        state.snap_zone = None;
                        state.snap_preview = None;
                        state.snap_clip = None;
                        self.content.as_widget_mut().operate(
                            &mut t.children[0],
                            l,
                            r,
                            &mut GripVisibility(false),
                        );
                        s.publish(Message::CancelDrag);
                        s.request_redraw();
                        s.capture_event();
                        return;
                    }
                }
                _ => {}
            }
        } else if self.drag {
            *t.state.downcast_mut::<State>() = State::default();
            self.content.as_widget_mut().operate(
                &mut t.children[0],
                l,
                r,
                &mut GripVisibility(false),
            );
        }

        self.content.as_widget_mut().update(
            &mut t.children[0],
            e,
            l,
            if self.drag && t.state.downcast_ref::<State>().dragging {
                mouse::Cursor::Unavailable
            } else {
                c
            },
            r,
            cb,
            s,
            v,
        )
    }
    fn mouse_interaction(
        &self,
        t: &Tree,
        l: Layout<'_>,
        c: mouse::Cursor,
        v: &Rectangle,
        r: &Renderer,
    ) -> mouse::Interaction {
        if self.drag && !self.enabled {
            return mouse::Interaction::Idle;
        }
        if self.drag && t.state.downcast_ref::<State>().dragging {
            if t.state
                .downcast_ref::<State>()
                .source
                .as_ref()
                .is_some_and(|(m, _)| m.kind == 4)
            {
                mouse::Interaction::Idle
            } else {
                mouse::Interaction::Grabbing
            }
        } else {
            self.content
                .as_widget()
                .mouse_interaction(&t.children[0], l, c, v, r)
        }
    }

    fn overlay<'b>(
        &'b mut self,
        t: &'b mut Tree,
        l: Layout<'b>,
        r: &Renderer,
        v: &Rectangle,
        tr: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if self.drag && t.state.downcast_ref::<State>().dragging {
            return None;
        }
        self.content
            .as_widget_mut()
            .overlay(&mut t.children[0], l, r, v, tr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_tabs_behind_add_control_do_not_steal_its_drag_hit() {
        fn strip(keys: &[u64], width: f32) -> Element<'static, Message> {
            use crate::motion_strip::{Item, Key};
            let mut items: Vec<_> = keys
                .iter()
                .enumerate()
                .map(|(index, key)| Item {
                    key: Key::Item(*key),
                    snapshot: Some(()),
                    content: mark(
                        iced::widget::button(iced::widget::Space::new())
                            .width(width)
                            .height(30)
                            .on_press(Message::Noop)
                            .into(),
                        Marker::tab(0, *key, index, key.to_string()),
                    ),
                })
                .collect();
            items.push(Item {
                key: Key::Control,
                snapshot: None,
                content: mark(
                    iced::widget::button(iced::widget::Space::new())
                        .width(28)
                        .height(28)
                        .on_press(Message::Noop)
                        .into(),
                    Marker::tab_control(0),
                ),
            });
            layer(
                mark(
                    crate::motion_strip::view(
                        items,
                        crate::motion::Policy {
                            system_enabled: true,
                        },
                        |_, width| iced::widget::Space::new().width(width).height(30).into(),
                    ),
                    Marker::strip(0, keys.len()),
                ),
                true,
            )
        }
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 30.0));
        let mut view = strip(&[1, 2, 3], 80.0);
        let mut tree = Tree::new(&view);
        view.as_widget_mut().layout(&mut tree, &renderer, &limits);
        view = strip(&[2, 3], 120.0);
        tree.diff(&view);
        let node = view.as_widget_mut().layout(&mut tree, &renderer, &limits);
        let point = Point::new(250.0, 15.0);
        view.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(point),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut vec![]),
            &node.bounds(),
        );
        assert!(tree.state.downcast_ref::<State>().source.is_none());
    }
    #[test]
    fn folder_target_leaves_blank_row_space_for_the_panel_root() {
        let bounds = Rectangle {
            x: 10.,
            y: 10.,
            width: 600.,
            height: 300.,
        };
        let row = Rectangle {
            height: 24.,
            ..bounds
        };
        let regions = vec![
            Region {
                marker: Marker::file_panel(0),
                bounds,
                full: bounds,
            },
            Region {
                marker: Marker::file(0, 0, "folder".into(), 1).folder("folder".into()),
                bounds: row,
                full: row,
            },
        ];
        assert!(folder_hit(&regions, Point::new(90., 20.)).is_some());
        assert!(folder_hit(&regions, Point::new(500., 20.)).is_none());
        assert!(file_panel(&regions, Point::new(500., 20.)).is_some());
    }
    use iced::widget::{Space, column, container, row};
    #[test]
    fn close_panel_button_has_a_background_highlight_on_hover() {
        use iced::advanced::renderer::Headless as _;
        let app = crate::App::new("home".into(), "other".into(), vec![]);
        let mut view = app.pane(0, 800.);
        let mut tree = Tree::new(&view);
        let mut renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(800., 400.);
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut collect = Collect::new();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
        let close = collect
            .regions
            .iter()
            .filter(|r| r.marker.kind == 6)
            .max_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x))
            .unwrap()
            .bounds;
        let mut messages = vec![];
        let mut paint = |cursor| {
            view.as_widget_mut().update(
                &mut tree,
                &Event::Window(iced::window::Event::RedrawRequested(
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
            renderer.screenshot(Size::new(800, 400), 1., app.app_theme.palette().background)
        };
        let idle = paint(mouse::Cursor::Unavailable);
        let hover = paint(mouse::Cursor::Available(close.center()));
        let x = (close.x + 2.) as usize;
        let y = close.center_y() as usize;
        let i = (y * 800 + x) * 4;
        assert_ne!(&idle[i..i + 3], &hover[i..i + 3]);
    }
    #[test]
    fn panel_grips_hide_during_drag_and_return_after_cancellation() {
        struct Probe(Vec<bool>);
        impl Operation for Probe {
            fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
                f(self);
            }
            fn custom(
                &mut self,
                _: Option<&iced::advanced::widget::Id>,
                _: Rectangle,
                value: &mut dyn std::any::Any,
            ) {
                if let Some(state) = value.downcast_ref::<sk_iced::components::DragVisibility>() {
                    self.0.push(state.hidden);
                }
            }
        }
        let app = crate::App::new("home".into(), "other".into(), vec![]);
        let mut element = app.view();
        let mut tree = Tree::new(&element);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(1400., 900.);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut collect = Collect::new();
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
        let start = collect
            .regions
            .iter()
            .find(|r| r.marker.kind == 9 && r.marker.pane == 0)
            .unwrap()
            .bounds
            .center();
        let end = Point::new(start.x + 100., start.y + 100.);
        let mut messages = vec![];
        for (event, p) in [
            (
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                start,
            ),
            (
                Event::Mouse(mouse::Event::CursorMoved { position: end }),
                end,
            ),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(p),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            );
        }
        let mut probe = Probe(vec![]);
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut probe);
        assert_eq!(probe.0, vec![true, true]);
        assert_eq!(
            element.as_widget().mouse_interaction(
                &tree,
                Layout::new(&node),
                mouse::Cursor::Available(end),
                &node.bounds(),
                &renderer
            ),
            mouse::Interaction::Idle
        );
        element.as_widget_mut().update(
            &mut tree,
            &Event::Window(iced::window::Event::Unfocused),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut messages),
            &node.bounds(),
        );
        let mut probe = Probe(vec![]);
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut probe);
        assert_eq!(probe.0, vec![false, false]);
    }
    #[test]
    fn dropping_in_row_gutter_does_not_publish_docking() {
        let layout = iced::widget::pane_grid::State::with_configuration(
            iced::widget::pane_grid::Configuration::Split {
                axis: iced::widget::pane_grid::Axis::Horizontal,
                ratio: 1. / 3.,
                a: Box::new(iced::widget::pane_grid::Configuration::Pane(0)),
                b: Box::new(iced::widget::pane_grid::Configuration::Split {
                    axis: iced::widget::pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(iced::widget::pane_grid::Configuration::Pane(1)),
                    b: Box::new(iced::widget::pane_grid::Configuration::Pane(2)),
                }),
            },
        );
        let source = mark(
            column![
                mark(
                    Space::new().width(600).height(30).into(),
                    Marker::panel_grip(0)
                ),
                Space::new().height(30)
            ]
            .into(),
            Marker::docking_panel(0),
        );
        let middle = mark(
            Space::new().width(600).height(64).into(),
            Marker::docking_panel(1),
        );
        let bottom = mark(
            Space::new().width(600).height(64).into(),
            Marker::docking_panel(2),
        );
        let element = layer(
            mark(
                mark(
                    column![source, middle, bottom].spacing(6).into(),
                    Marker::docking_canvas(&layout),
                ),
                Marker::workspace_for(3),
            ),
            true,
        );
        let start = Point::new(300., 15.);
        let end = Point::new(300., 133.);
        let messages = gesture_on(
            element,
            &[
                (mouse::Event::ButtonPressed(mouse::Button::Left), start),
                (mouse::Event::CursorMoved { position: end }, end),
                (mouse::Event::ButtonReleased(mouse::Button::Left), end),
            ],
        );
        assert!(!messages.iter().any(|m| matches!(m, Message::Dock(..))));
        assert!(
            !messages
                .iter()
                .any(|m| matches!(m, Message::MoveTab(..) | Message::FileDrop(..)))
        );
    }
    #[test]
    fn edge_drag_release_targets_existing_panel_and_center_cancels() {
        for (end, edge) in [
            (Point::new(10., 150.), Some(3)),
            (Point::new(590., 150.), Some(5)),
            (Point::new(300., 195.), Some(7)),
            (Point::new(300., 150.), None),
        ] {
            let layout = iced::widget::pane_grid::State::with_configuration(
                iced::widget::pane_grid::Configuration::Split {
                    axis: iced::widget::pane_grid::Axis::Horizontal,
                    ratio: 0.5,
                    a: Box::new(iced::widget::pane_grid::Configuration::Pane(0)),
                    b: Box::new(iced::widget::pane_grid::Configuration::Pane(1)),
                },
            );
            let source = mark(
                column![
                    mark(
                        Space::new().width(600).height(30).into(),
                        Marker::panel_grip(0)
                    ),
                    Space::new().height(70)
                ]
                .into(),
                Marker::docking_panel(0),
            );
            let destination = mark(
                Space::new().width(600).height(100).into(),
                Marker::docking_panel(1),
            );
            let element = layer(
                mark(
                    column![source, destination].into(),
                    Marker::docking_canvas(&layout),
                ),
                true,
            );
            let start = Point::new(300., 15.);
            let messages = gesture_on(
                element,
                &[
                    (mouse::Event::ButtonPressed(mouse::Button::Left), start),
                    (mouse::Event::CursorMoved { position: end }, end),
                    (mouse::Event::ButtonReleased(mouse::Button::Left), end),
                ],
            );
            let dropped = messages.iter().find_map(|m| {
                if let Message::Dock(0, None, zone, _) = m {
                    Some(*zone)
                } else {
                    None
                }
            });
            assert_eq!(dropped, edge.map(|e| crate::docking::Zone(e).for_panel(1)));
        }
    }
    fn fixture() -> Element<'static, Message> {
        let tab = |pane, identity, index| {
            mark(
                crate::components::browser_tab(
                    format!("Tab {identity}"),
                    Message::SelectTab(pane, index),
                    Some(Message::CloseTab(pane, index)),
                    index == 0,
                ),
                Marker::tab(pane, identity, index, format!("Tab {identity}")),
            )
        };
        layer(
            column![
                mark(
                    container(row![tab(0, 11, 0), tab(0, 12, 1), Space::new().width(100)])
                        .width(600)
                        .height(30)
                        .into(),
                    Marker::strip(0, 2)
                ),
                mark(
                    container(row![tab(1, 21, 0), Space::new().width(300)])
                        .width(600)
                        .height(30)
                        .into(),
                    Marker::strip(1, 1)
                )
            ]
            .spacing(20)
            .into(),
            true,
        )
    }
    fn gesture(points: &[(mouse::Event, Point)]) -> Vec<Message> {
        gesture_on(fixture(), points)
    }
    fn gesture_on(
        mut element: Element<'static, Message>,
        points: &[(mouse::Event, Point)],
    ) -> Vec<Message> {
        let mut tree = Tree::new(&element);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(600.0, 200.0);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = vec![];
        for (event, point) in points {
            let mut shell = Shell::new(&mut messages);
            element.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(*event),
                Layout::new(&node),
                mouse::Cursor::Available(*point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut shell,
                &Rectangle::with_size(size),
            );
        }
        messages
    }
    #[test]
    fn dragging_reorders_and_moves_between_strips() {
        for (position, pane, index) in [
            (Point::new(370.0, 10.0), 0, 2),
            (Point::new(300.0, 60.0), 1, 1),
        ] {
            let messages = gesture(&[
                (
                    mouse::Event::ButtonPressed(mouse::Button::Left),
                    Point::new(30.0, 10.0),
                ),
                (mouse::Event::CursorMoved { position }, position),
                (mouse::Event::ButtonReleased(mouse::Button::Left), position),
            ]);
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m,Message::MoveTab(0,11,p,i) if *p==pane&&*i==index)),
                "{messages:?}"
            );
        }
    }
    #[test]
    fn close_button_is_never_a_drag_handle() {
        let p = Point::new(175.0, 10.0);
        let messages = gesture(&[
            (mouse::Event::ButtonPressed(mouse::Button::Left), p),
            (mouse::Event::ButtonReleased(mouse::Button::Left), p),
        ]);
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::CloseTab(0, 0))),
            "{messages:?}"
        );
        assert!(!messages.iter().any(|m| matches!(m, Message::MoveTab(..))));
    }
    #[test]
    fn outside_drop_cancels_without_moving() {
        let p = Point::new(400.0, 150.0);
        let messages = gesture(&[
            (
                mouse::Event::ButtonPressed(mouse::Button::Left),
                Point::new(30.0, 10.0),
            ),
            (mouse::Event::CursorMoved { position: p }, p),
            (mouse::Event::ButtonReleased(mouse::Button::Left), p),
        ]);
        assert!(!messages.iter().any(|m| matches!(m, Message::MoveTab(..))));
    }

    fn snap_fixture() -> Element<'static, Message> {
        let title = mark(
            crate::components::browser_tab(
                "Folder".into(),
                Message::Noop,
                Some(Message::Noop),
                true,
            ),
            Marker::tab(0, 11, 0, "Folder".into()),
        );
        let strip = mark(
            iced::widget::container(iced::widget::row![title])
                .width(550)
                .height(30)
                .into(),
            Marker::strip(0, 1),
        );
        let add = mark(
            crate::components::tab_add(crate::motion::Policy::DISABLED, Message::Noop),
            Marker::tab_control(0),
        );
        let bar = mark(
            iced::widget::row![strip, add, iced::widget::Space::new().width(22)]
                .width(600)
                .height(30)
                .into(),
            Marker::panel_grip(0),
        );
        layer(
            mark(
                iced::widget::column![bar, iced::widget::Space::new().height(iced::Fill)]
                    .width(600)
                    .height(200)
                    .into(),
                Marker::workspace(),
            ),
            true,
        )
    }
    #[test]
    fn center_and_outside_drops_do_not_snap() {
        for end in [Point::new(300.0, 100.0), Point::new(700.0, 100.0)] {
            let start = Point::new(400.0, 15.0);
            let messages = gesture_on(
                snap_fixture(),
                &[
                    (mouse::Event::ButtonPressed(mouse::Button::Left), start),
                    (mouse::Event::CursorMoved { position: end }, end),
                    (mouse::Event::ButtonReleased(mouse::Button::Left), end),
                ],
            );
            assert!(
                !messages
                    .iter()
                    .any(|m| matches!(m, Message::Dock(..) | Message::MoveTab(..)))
            );
        }
    }

    #[test]
    fn full_window_grip_preview_and_escape_use_the_visible_workspace() {
        use iced::advanced::renderer::Headless as _;
        let app = crate::App::new(
            std::path::PathBuf::from("Home"),
            std::path::PathBuf::from("Project"),
            vec![],
        );
        let mut element = app.view();
        let mut tree = Tree::new(&element);
        let mut renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(1400.0, 900.0);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let bounds = Rectangle::with_size(size);
        let mut collect = Collect::new();
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
        let workspace = collect
            .regions
            .iter()
            .find(|r| r.marker.kind == 5)
            .unwrap()
            .bounds;
        let grip = collect
            .regions
            .iter()
            .find(|r| r.marker.kind == 4 && r.marker.pane == 0)
            .unwrap()
            .bounds
            .center();
        let end = Point::new(
            workspace.x + workspace.width * 0.85,
            workspace.y + workspace.height * 0.85,
        );
        let mut messages = vec![];
        for (event, point) in [
            (
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                grip,
            ),
            (
                Event::Mouse(mouse::Event::CursorMoved { position: end }),
                end,
            ),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &bounds,
            );
        }
        let state = tree.state.downcast_ref::<State>();
        assert!(state.dragging);
        assert!(state.snap_bounds.is_some());
        assert_eq!(state.snap_zone.unwrap().target(), 1);
        renderer.reset(bounds);
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &app.app_theme,
            &renderer::Style::default(),
            Layout::new(&node),
            mouse::Cursor::Available(end),
            &bounds,
        );
        if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
            std::fs::write(
                "target/snap-preview.rgba",
                renderer.screenshot(iced::Size::new(1400, 900), 1.0, iced::Color::BLACK),
            )
            .unwrap();
        }
        element.as_widget_mut().update(
            &mut tree,
            &Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                physical_key: iced::keyboard::key::Physical::Code(
                    iced::keyboard::key::Code::Escape,
                ),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::empty(),
                text: None,
                repeat: false,
            }),
            Layout::new(&node),
            mouse::Cursor::Available(end),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut messages),
            &bounds,
        );
        assert!(!tree.state.downcast_ref::<State>().dragging);
        assert!(!messages.iter().any(|m| matches!(m, Message::Dock(..))));
    }

    #[test]
    fn panel_close_is_at_the_right_and_is_absent_for_the_final_panel() {
        for count in [1, 2] {
            let mut app = crate::App::new(
                std::path::PathBuf::from("Home"),
                std::path::PathBuf::from("Project"),
                vec![],
            );
            app.preferences.default_layout = if count == 1 {
                crate::preferences::DefaultLayout::Single
            } else {
                crate::preferences::DefaultLayout::Vertical
            };
            app.initialize_default_layout();
            let mut element = app.view();
            let mut tree = Tree::new(&element);
            let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
            let size = Size::new(1400.0, 900.0);
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let mut collect = Collect::new();
            element
                .as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
            let bar = collect
                .regions
                .iter()
                .find(|r| r.marker.kind == 4 && r.marker.pane == 0)
                .unwrap();
            let tab = collect
                .regions
                .iter()
                .find(|r| r.marker.kind == 0 && r.marker.pane == 0)
                .unwrap();
            assert!(tab.bounds.x >= bar.bounds.x + 28.0);
            let controls = collect
                .regions
                .iter()
                .filter(|r| r.marker.kind == 6 && r.marker.pane == 0)
                .collect::<Vec<_>>();
            assert_eq!(controls.len(), if count == 1 { 2 } else { 3 });
            let mut ordered = controls.clone();
            ordered.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x));
            let add = ordered[1].bounds.center();
            assert!(add.x > tab.bounds.x + tab.bounds.width);
            if count == 1 {
                assert!(
                    add.x > bar.bounds.x + bar.bounds.width - 32.0,
                    "New panel should sit at the right edge without a close-button gap"
                );
            }
            let mut messages = vec![];
            for event in [
                mouse::Event::ButtonPressed(mouse::Button::Left),
                mouse::Event::ButtonReleased(mouse::Button::Left),
            ] {
                element.as_widget_mut().update(
                    &mut tree,
                    &Event::Mouse(event),
                    Layout::new(&node),
                    mouse::Cursor::Available(add),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &Rectangle::with_size(size),
                );
            }
            assert!(messages.iter().any(|m| matches!(m, Message::NewPanel(0))));
            assert!(!messages.iter().any(|m| matches!(m, Message::Dock(..))));
            if count > 1 {
                let close = controls
                    .iter()
                    .max_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x))
                    .unwrap()
                    .bounds
                    .center();
                assert!(close.x > bar.bounds.x + bar.bounds.width - 32.0);
                let mut messages = vec![];
                for event in [
                    mouse::Event::ButtonPressed(mouse::Button::Left),
                    mouse::Event::ButtonReleased(mouse::Button::Left),
                ] {
                    element.as_widget_mut().update(
                        &mut tree,
                        &Event::Mouse(event),
                        Layout::new(&node),
                        mouse::Cursor::Available(close),
                        &renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut Shell::new(&mut messages),
                        &Rectangle::with_size(size),
                    );
                }
                assert!(messages.iter().any(|m| matches!(m, Message::ClosePanel(0))));
                assert!(!messages.iter().any(|m| matches!(m, Message::Dock(..))));
            }
        }
    }

    #[test]
    fn opening_and_switching_tabs_reveal_the_actual_selected_tab() {
        let mut app = crate::App::new(
            std::path::PathBuf::from("Home"),
            std::path::PathBuf::from("Project"),
            vec![],
        );
        for index in 0..30 {
            app.panes[0]
                .tabs
                .push(crate::Tab::new(std::path::PathBuf::from(format!(
                    "Folder {index}"
                ))));
        }
        let check = |app: &crate::App| {
            let mut element = app.pane(0, 320.0);
            let mut tree = Tree::new(&element);
            let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
            let size = Size::new(320.0, 500.0);
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let mut collect = Collect::new();
            element
                .as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
            let selected = app.panes[0].identity;
            let tab = collect
                .regions
                .iter()
                .find(|r| r.marker.kind == 0 && r.marker.identity == Some(selected))
                .expect("selected tab must be visible");
            assert!(
                (tab.bounds.width - tab.full.width).abs() < 0.01,
                "selected tab is clipped: {:?} {:?}",
                tab.bounds,
                tab.full
            );
        };
        let _ = app.update(Message::NewTab(0));
        check(&app);
        let _ = app.update(Message::SelectTab(0, 0));
        check(&app);
        let _ = app.update(Message::SelectTab(0, 20));
        check(&app);
    }
    fn files_fixture() -> Element<'static, Message> {
        let listing = crate::virtual_list::file_list(
            100_000,
            1,
            |index| {
                mark(
                    iced::widget::button(Space::new().height(24))
                        .height(24)
                        .width(iced::Fill)
                        .on_press(Message::Select(0, index))
                        .into(),
                    Marker::file(0, index, format!("file-{index}"), 1),
                )
            },
            |index| Message::OpenEntry(0, index),
        );
        layer(
            column![
                mark(
                    container(listing).width(600).height(60).into(),
                    Marker::file_panel(0)
                ),
                mark(
                    Space::new().width(600).height(60).into(),
                    Marker::file_panel(1)
                )
            ]
            .spacing(20)
            .into(),
            true,
        )
    }
    #[test]
    fn scrolled_file_drag_targets_visible_row_and_blank_destination() {
        let first = Point::new(40., 10.);
        let last = Point::new(300., 100.);
        let messages = gesture_on(
            files_fixture(),
            &[
                (
                    mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Lines { x: 0., y: -1. },
                    },
                    first,
                ),
                (mouse::Event::ButtonPressed(mouse::Button::Left), first),
                (mouse::Event::CursorMoved { position: last }, last),
                (mouse::Event::ButtonReleased(mouse::Button::Left), last),
            ],
        );
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::FileDragStarted(0, 3))),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::FileDrop(0, 3, 1))),
            "{messages:?}"
        );
        assert!(
            !messages
                .iter()
                .any(|m| matches!(m, Message::Select(..) | Message::OpenEntry(..)))
        );
    }
    #[test]
    fn cached_rows_outside_their_panel_never_receive_context_click() {
        let messages = gesture_on(
            files_fixture(),
            &[(
                mouse::Event::ButtonPressed(mouse::Button::Right),
                Point::new(40., 100.),
            )],
        );
        assert!(
            matches!(messages.as_slice(), [Message::Context(1, None, _)]),
            "{messages:?}"
        );
    }
    #[test]
    fn preview_labels_describe_tab_single_item_and_multiple_selection() {
        assert_eq!(Marker::tab(0, 1, 0, "Downloads".into()).label, "Downloads");
        assert_eq!(Marker::file(0, 1, "photo.png".into(), 1).label, "photo.png");
        assert_eq!(Marker::file(0, 1, "photo.png".into(), 12).label, "12 items");
    }
    #[test]
    fn ctrl_z_never_undoes_files_while_a_text_input_is_focused() {
        let mut element = layer(
            iced::widget::text_input("Path", "value")
                .on_input(|_| Message::Noop)
                .into(),
            true,
        );
        let mut tree = Tree::new(&element);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let size = Size::new(600., 200.);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut messages = vec![];
        let mut clipboard = iced::advanced::clipboard::Null;
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            let mut shell = Shell::new(&mut messages);
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(20., 10.)),
                &renderer,
                &mut clipboard,
                &mut shell,
                &node.bounds(),
            );
        }
        let mut shell = Shell::new(&mut messages);
        let event = Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character("z".into()),
            modified_key: iced::keyboard::Key::Character("z".into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        });
        element.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut clipboard,
            &mut shell,
            &node.bounds(),
        );
        assert!(shell.is_event_captured());
        assert!(messages.is_empty());
    }
    fn paint_fixture() -> Element<'static, Message> {
        let rows = iced::widget::column((0..12).map(|i| {
            crate::typography::text(format!(
                "UNDERLYING FILE {i} WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"
            ))
            .size(18)
            .color(iced::Color::from_rgb(1., 0.2, 0.2))
            .into()
        }));
        layer(
            container(rows)
                .width(iced::Fill)
                .height(iced::Fill)
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Color::from_rgb(0.04, 0.05, 0.08).into()),
                    ..Default::default()
                })
                .into(),
            true,
        )
    }
    fn record_frame(
        element: &Element<'_, Message>,
        tree: &Tree,
        node: &layout::Node,
        renderer: &mut Renderer,
    ) {
        let bounds = node.bounds();
        renderer.reset(bounds);
        element.as_widget().draw(
            tree,
            renderer,
            &Theme::Dark,
            &renderer::Style::default(),
            Layout::new(node),
            mouse::Cursor::Unavailable,
            &bounds,
        );
    }
    #[test]
    fn incremental_drag_frames_match_fresh_paint_after_moving_and_removal() {
        use iced_tiny_skia::{
            Layer,
            graphics::{Viewport, damage},
        };
        for scale in [1.0, 1.25, 2.0] {
            let mut element = paint_fixture();
            let mut tree = Tree::new(&element);
            let mut renderer = Renderer::new(crate::typography::font(), 13.0.into());
            let size = Size::new(640., 240.);
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let bounds = node.bounds();
            let physical = Size::new((size.width * scale) as u32, (size.height * scale) as u32);
            let viewport = Viewport::with_physical_size(physical, scale);
            let mut pixels = tiny_skia::Pixmap::new(physical.width, physical.height).unwrap();
            let mut mask = tiny_skia::Mask::new(physical.width, physical.height).unwrap();
            let mut previous: Option<Vec<Layer>> = None;
            for position in [
                None,
                Some(Point::new(12., 15.)),
                Some(Point::new(150.25, 35.5)),
                Some(Point::new(520., 200.)),
                Some(Point::new(40., 65.)),
                None,
            ] {
                let state = tree.state.downcast_mut::<State>();
                state.source = position.map(|p| {
                    (
                        Marker::file(
                            0,
                            0,
                            "very long filename with spaces 資料 and unicode é ".repeat(12),
                            1,
                        ),
                        p,
                    )
                });
                state.dragging = position.is_some();
                state.position = position.unwrap_or_default();
                record_frame(&element, &tree, &node, &mut renderer);
                if position.is_some() {
                    use iced::advanced::text::Paragraph as _;
                    let cache = tree.state.downcast_ref::<State>().preview_text.borrow();
                    assert!(
                        cache.as_ref().unwrap().1.min_height() <= 16.01,
                        "drag preview must remain a single line"
                    );
                }

                let regions = previous
                    .as_ref()
                    .map(|old| {
                        damage::diff(old, renderer.layers(), |l| vec![l.bounds], Layer::damage)
                    })
                    .unwrap_or_else(|| vec![bounds]);
                let regions = iced_tiny_skia::window::compositor::coalesce_damage(damage::group(
                    regions, bounds,
                ));
                let current = renderer.layers().to_vec();
                renderer.draw(
                    &mut pixels.as_mut(),
                    &mut mask,
                    &viewport,
                    &regions,
                    iced::Color::BLACK,
                );
                let mut clean = tiny_skia::Pixmap::new(physical.width, physical.height).unwrap();
                let mut clean_mask = tiny_skia::Mask::new(physical.width, physical.height).unwrap();
                renderer.draw(
                    &mut clean.as_mut(),
                    &mut clean_mask,
                    &viewport,
                    &[bounds],
                    iced::Color::BLACK,
                );
                let differences = pixels
                    .data()
                    .iter()
                    .zip(clean.data())
                    .filter(|(a, b)| a != b)
                    .count();
                assert_eq!(
                    differences, 0,
                    "stale/overpainted pixels at {position:?}, scale {scale}"
                );
                previous = Some(current);
            }
        }
    }
    #[test]
    fn circular_panel_drag_frames_match_fresh_paint_without_ghost_lines() {
        use iced_tiny_skia::{
            Layer,
            graphics::{Viewport, damage},
        };
        for scale in [1.0, 1.25, 2.0] {
            let mut element = paint_fixture();
            let mut tree = Tree::new(&element);
            let mut renderer = Renderer::new(crate::typography::font(), 13.0.into());
            let size = Size::new(640., 240.);
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let bounds = node.bounds();
            let physical = Size::new((size.width * scale) as u32, (size.height * scale) as u32);
            let viewport = Viewport::with_physical_size(physical, scale);
            let mut pixels = tiny_skia::Pixmap::new(physical.width, physical.height).unwrap();
            let mut mask = tiny_skia::Mask::new(physical.width, physical.height).unwrap();
            let mut previous: Option<Vec<Layer>> = None;
            for zone in [
                None,
                Some(0),
                Some(1),
                Some(2),
                Some(5),
                Some(8),
                Some(7),
                Some(6),
                Some(3),
                Some(0),
                None,
                Some(8),
                Some(2),
                Some(6),
                None,
            ] {
                let state = tree.state.downcast_mut::<State>();
                state.source = zone.map(|_| (Marker::panel_grip(0), Point::ORIGIN));
                state.dragging = zone.is_some();
                state.snap_preview = zone.map(|zone| Rectangle {
                    x: if zone % 3 == 2 {
                        bounds.width / 2.0 + 3.0
                    } else {
                        0.0
                    },
                    y: if zone > 5 {
                        bounds.height / 2.0 + 3.0
                    } else {
                        0.0
                    },
                    width: bounds.width / 2.0 - 3.0,
                    height: bounds.height / 2.0 - 3.0,
                });
                record_frame(&element, &tree, &node, &mut renderer);
                let regions = previous
                    .as_ref()
                    .map(|old| {
                        damage::diff(old, renderer.layers(), |l| vec![l.bounds], Layer::damage)
                    })
                    .unwrap_or_else(|| vec![bounds]);
                let regions = iced_tiny_skia::window::compositor::coalesce_damage(damage::group(
                    regions, bounds,
                ));
                let current = renderer.layers().to_vec();
                renderer.draw(
                    &mut pixels.as_mut(),
                    &mut mask,
                    &viewport,
                    &regions,
                    iced::Color::BLACK,
                );
                let mut clean = tiny_skia::Pixmap::new(physical.width, physical.height).unwrap();
                let mut clean_mask = tiny_skia::Mask::new(physical.width, physical.height).unwrap();
                renderer.draw(
                    &mut clean.as_mut(),
                    &mut clean_mask,
                    &viewport,
                    &[bounds],
                    iced::Color::BLACK,
                );
                let differences = pixels
                    .data()
                    .iter()
                    .zip(clean.data())
                    .filter(|(a, b)| a != b)
                    .count();
                assert_eq!(
                    differences, 0,
                    "stale/overpainted pixels at {zone:?}, scale {scale}"
                );
                previous = Some(current);
            }
        }
    }
    #[test]
    fn drag_preview_is_opaque_above_underlying_file_text() {
        use iced::advanced::renderer::Headless as _;
        let mut element = paint_fixture();
        let mut tree = Tree::new(&element);
        let mut renderer = Renderer::new(crate::typography::font(), 13.0.into());
        let size = Size::new(640., 240.);
        let node =
            element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let state = tree.state.downcast_mut::<State>();
        state.source = Some((Marker::file(0, 0, "Report.pdf".into(), 3), Point::ORIGIN));
        state.dragging = true;
        state.position = Point::new(150., 40.);
        record_frame(&element, &tree, &node, &mut renderer);
        let shot = renderer.screenshot(Size::new(640, 240), 1., iced::Color::BLACK);
        let preview = renderer
            .layers()
            .iter()
            .find(|layer| (layer.bounds.height - 38.).abs() < 0.01)
            .expect("opaque preview background layer")
            .bounds;
        let tokens = palette::themed(true, Default::default());
        let ink = color(tokens.surface);
        let expected = [
            (ink.r * 255.).round() as u8,
            (ink.g * 255.).round() as u8,
            (ink.b * 255.).round() as u8,
            255,
        ];
        let y = (preview.y + preview.height - 7.) as usize;
        for x in (preview.x + 12.) as usize..(preview.x + preview.width - 12.) as usize {
            assert_eq!(
                &shot[(y * 640 + x) * 4..(y * 640 + x) * 4 + 4],
                &expected,
                "file text bleeds through preview at {x},{y}"
            );
        }
        let out = std::env::current_dir()
            .unwrap()
            .join("target/render-regressions");
        std::fs::create_dir_all(&out).unwrap();
        tiny_skia::Pixmap::from_vec(shot, tiny_skia::IntSize::from_wh(640, 240).unwrap())
            .unwrap()
            .save_png(out.join("drag-preview.png"))
            .unwrap();
    }
}

#[cfg(test)]
mod edge_tests {
    use super::*;
    fn collect(app: &crate::App, size: Size) -> Vec<Region> {
        let mut view = app.view();
        let mut tree = Tree::new(&view);
        let renderer = Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let node =
            view.as_widget_mut()
                .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
        let mut collected = Collect::new();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut collected);
        collected.regions
    }
    #[test]
    fn all_normal_edges_match_actual_widget_bounds_with_and_without_new_scrollbars() {
        for window_width in [1100., 1400., 1700.] {
            for tear_off in [false, true] {
                for workspace_edge in [false, true] {
                    for edge in [1, 3, 5, 7] {
                        let mut app = crate::App::new("one".into(), "two".into(), vec![]);
                        app.preferences.default_layout = crate::preferences::DefaultLayout::Three;
                        app.initialize_default_layout();
                        if tear_off {
                            app.panes[0].tabs.push(crate::Tab::new("tearoff".into()));
                            app.panes[0].selected_tab = 1;
                        }
                        let identity = app.panes[0].identity;
                        let size = Size::new(window_width, 900.);
                        let before = collect(&app, size);
                        let workspace = before.iter().find(|r| r.marker.kind == 5).unwrap();
                        let zone = if workspace_edge {
                            crate::docking::Zone(edge).for_workspace()
                        } else {
                            crate::docking::Zone(edge).for_panel(1)
                        };
                        let source = if tear_off {
                            Marker::tab(0, identity, 1, "tearoff".into())
                        } else {
                            Marker::panel_grip(0)
                        };
                        let preview =
                            predicted_preview(&before, &source, workspace.bounds, zone).unwrap();
                        let _ = app.update(Message::Dock(
                            0,
                            if tear_off { Some(identity) } else { None },
                            zone,
                            Size::new(workspace.bounds.width - 12., workspace.bounds.height),
                        ));
                        let after = collect(&app, size);
                        let moved = if tear_off { 3 } else { 0 };
                        if let Some(actual) = after
                            .iter()
                            .find(|r| r.marker.kind == 7 && r.marker.pane == moved)
                        {
                            assert_eq!(
                                preview, actual.full,
                                "width={window_width} tearoff={tear_off} workspace={workspace_edge} edge={edge}"
                            );
                        } else {
                            assert!(
                                preview.intersection(&workspace.bounds).is_none(),
                                "offscreen width={window_width} tearoff={tear_off} workspace={workspace_edge} edge={edge}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn preview_matches_actual_widget_after_tearoff_into_a_split_column() {
        for window_width in [1100., 1400., 1700.] {
            let mut app = crate::App::new("one".into(), "two".into(), vec![]);
            app.preferences.default_layout = crate::preferences::DefaultLayout::Three;
            app.initialize_default_layout();
            let _ = app.update(Message::Dock(
                0,
                None,
                crate::docking::Zone(5).for_workspace(),
                Size::new(1150., 780.),
            ));
            app.panes[0].tabs.push(crate::Tab::new("tearoff".into()));
            app.panes[0].selected_tab = 1;
            let identity = app.panes[0].identity;
            let size = Size::new(window_width, 900.);
            let before = collect(&app, size);
            let workspace = before.iter().find(|r| r.marker.kind == 5).unwrap();
            let target = before
                .iter()
                .find(|r| r.marker.kind == 7 && r.marker.pane == 1)
                .unwrap();
            let zone = crate::docking::Zone(5).for_panel(1);
            let preview = predicted_preview(
                &before,
                &Marker::tab(0, identity, 1, "tearoff".into()),
                target.full,
                zone,
            )
            .unwrap();
            let _ = app.update(Message::Dock(
                0,
                Some(identity),
                zone,
                Size::new(workspace.bounds.width - 12., workspace.bounds.height),
            ));
            let after = collect(&app, size);
            let actual = after
                .iter()
                .find(|r| r.marker.kind == 7 && r.marker.pane == 3)
                .unwrap()
                .full;
            assert_eq!(preview, actual, "width {window_width}");
        }
    }
    #[test]
    fn preview_matches_commit_when_overflowing_workspace_shrinks_and_scroll_offset_clamps() {
        let mut app = crate::App::new("one".into(), "two".into(), vec![]);
        app.pane_count = 4;
        app.grid = false;
        app.stacked = false;
        app.rebuild_file_layout();
        let width = app.files_layout.minimum_width().ceil();
        app.files_layout.set_size(Size::new(width, 900.));
        let viewport = Rectangle {
            x: 20.,
            y: 40.,
            width: 600.,
            height: 900.,
        };
        let canvas = Rectangle {
            x: 20. - 600.,
            y: 40.,
            width,
            height: 900.,
        };
        let regions = vec![
            Region {
                marker: Marker::workspace_for(4),
                bounds: viewport,
                full: viewport,
            },
            Region {
                marker: Marker::docking_canvas(&app.files_layout),
                bounds: viewport,
                full: canvas,
            },
        ];
        let zone = crate::docking::Zone(7).for_workspace();
        let preview = predicted_preview(&regions, &Marker::panel_grip(0), canvas, zone).unwrap();
        let _ = app.update(Message::Dock(0, None, zone, Size::new(600., 900.)));
        let new_width = app.files_layout.minimum_width().ceil().max(600.);
        let actual = app.files_layout.layout().pane_regions(
            6.,
            180.,
            app.files_layout.canvas_size(Size::new(600., 900.)),
        );
        let pane = app.files_layout.iter().find(|(_, id)| **id == 0).unwrap().0;
        assert!(new_width < width);
        assert_eq!(
            preview,
            actual[pane] + Vector::new(20. - 600_f32.min(new_width - 600.), 40.)
        );
    }
    #[test]
    fn row_insertions_match_preview_and_evenly_redistribute_every_source_and_gap() {
        for count in 2..=4 {
            for source in 0..count {
                for boundary in 1..count {
                    let mut app = crate::App::new("one".into(), "two".into(), vec![]);
                    app.pane_count = count;
                    app.stacked = true;
                    app.grid = false;
                    app.rebuild_file_layout();
                    let bounds = Rectangle {
                        x: 40.,
                        y: 60.,
                        width: 1200.,
                        height: 900.,
                    };
                    // Distort initial heights to prove insertion normalizes the entire stack.
                    if let iced::widget::pane_grid::Node::Split { id, .. } =
                        app.files_layout.layout()
                    {
                        app.files_layout.resize(*id, 0.43);
                    }
                    let old = app
                        .files_layout
                        .layout()
                        .pane_regions(6., 180., bounds.size());
                    let mut collected = vec![
                        Region {
                            marker: Marker::docking_canvas(&app.files_layout),
                            bounds,
                            full: bounds,
                        },
                        Region {
                            marker: Marker::workspace_for(count),
                            bounds,
                            full: bounds,
                        },
                    ];
                    for (pane, id) in app.files_layout.iter() {
                        let rect = old[pane] + Vector::new(bounds.x, bounds.y);
                        collected.push(Region {
                            marker: Marker::docking_panel(*id),
                            bounds: rect,
                            full: rect,
                        });
                    }
                    let lower = collected
                        .iter()
                        .find(|r| r.marker.kind == 7 && r.marker.pane == boundary)
                        .unwrap()
                        .full;
                    let point = if boundary == source {
                        let upper = collected
                            .iter()
                            .find(|r| r.marker.kind == 7 && r.marker.pane == boundary - 1)
                            .unwrap()
                            .full;
                        Point::new(upper.center_x(), upper.y + upper.height - 35.0)
                    } else {
                        Point::new(lower.center_x(), lower.y + 35.0)
                    };
                    let (_, zone) =
                        dock_target(&collected, &Marker::panel_grip(source), point).unwrap();
                    assert!(!zone.workspace());
                    let preview =
                        predicted_preview(&collected, &Marker::panel_grip(source), bounds, zone)
                            .unwrap();
                    let ids = app
                        .panes
                        .iter()
                        .flat_map(|p| p.tabs.iter().map(|t| t.identity))
                        .collect::<Vec<_>>();
                    let cancels = app
                        .panes
                        .iter()
                        .map(|p| p.cancel.clone())
                        .collect::<Vec<_>>();
                    let _ = app.update(Message::Dock(source, None, zone, bounds.size()));
                    assert_eq!(app.pane_count, count);
                    assert_eq!(
                        ids,
                        app.panes
                            .iter()
                            .flat_map(|p| p.tabs.iter().map(|t| t.identity))
                            .collect::<Vec<_>>()
                    );
                    assert!(
                        cancels
                            .iter()
                            .all(|c| !c.load(std::sync::atomic::Ordering::Relaxed))
                    );
                    let actual = app
                        .files_layout
                        .layout()
                        .pane_regions(6., 180., bounds.size());
                    let pane = app
                        .files_layout
                        .iter()
                        .find(|(_, id)| **id == source)
                        .unwrap()
                        .0;
                    assert_eq!(preview, actual[pane] + Vector::new(bounds.x, bounds.y));
                    let expected_height = (bounds.height - 6. * (count - 1) as f32) / count as f32;
                    let mut rows = app
                        .files_layout
                        .iter()
                        .map(|(p, id)| (*id, actual[p]))
                        .collect::<Vec<_>>();
                    rows.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
                    for (_, rect) in &rows {
                        assert!((rect.height - expected_height).abs() <= 1., "{rows:?}");
                    }
                    let mut expected = (0..count).filter(|i| *i != source).collect::<Vec<_>>();
                    let index = (0..boundary).filter(|i| *i != source).count();
                    expected.insert(index, source);
                    assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), expected);
                }
            }
        }
    }
    #[test]
    fn gutters_are_neutral_and_normal_edges_cover_row_insertion() {
        let bounds = Rectangle {
            x: 0.,
            y: 0.,
            width: 1000.,
            height: 800.,
        };
        let upper = Rectangle {
            height: 397.,
            ..bounds
        };
        let lower = Rectangle {
            y: 403.,
            height: 397.,
            ..bounds
        };
        let regions = vec![
            Region {
                marker: Marker::workspace_for(3),
                bounds,
                full: bounds,
            },
            Region {
                marker: Marker::docking_panel(0),
                bounds: upper,
                full: upper,
            },
            Region {
                marker: Marker::docking_panel(1),
                bounds: lower,
                full: lower,
            },
        ];
        let source = Marker::panel_grip(2);
        assert!(dock_target(&regions, &source, Point::new(500., 400.)).is_none());
        assert_eq!(
            dock_target(&regions, &source, Point::new(500., 390.))
                .unwrap()
                .1,
            crate::docking::Zone(7).for_panel(0)
        );
        assert_eq!(
            dock_target(&regions, &source, Point::new(500., 410.))
                .unwrap()
                .1,
            crate::docking::Zone(1).for_panel(1)
        );
        assert!(
            priority_target(&regions, &source, Point::new(1., 400.))
                .unwrap()
                .1
                .workspace()
        );
    }
    #[test]
    fn row_insertion_balances_its_column_without_resizing_the_other_column() {
        let mut app = crate::App::new("one".into(), "two".into(), vec![]);
        app.preferences.default_layout = crate::preferences::DefaultLayout::Grid;
        app.initialize_default_layout();
        let size = Size::new(1400., 900.);
        let before = app.files_layout.layout().pane_regions(6., 180., size);
        let preserved = app
            .files_layout
            .iter()
            .filter(|(_, id)| **id == 1 || **id == 3)
            .map(|(p, id)| (*id, before[p]))
            .collect::<Vec<_>>();
        let _ = app.update(Message::Dock(
            0,
            None,
            crate::docking::Zone(7).for_panel(2),
            size,
        ));
        let after = app.files_layout.layout().pane_regions(6., 180., size);
        for (id, rect) in preserved {
            let pane = app.files_layout.iter().find(|(_, v)| **v == id).unwrap().0;
            assert_eq!(after[pane], rect);
        }
        assert_eq!(app.pane_count, 4);
    }
    #[test]
    fn tab_tear_off_into_row_gap_preserves_pending_scan_and_preview() {
        let mut app = crate::App::new("one".into(), "two".into(), vec![]);
        app.pane_count = 3;
        app.stacked = true;
        app.grid = false;
        app.rebuild_file_layout();
        app.panes[0].tabs.push(crate::Tab::new("pending".into()));
        app.panes[0].selected_tab = 1;
        app.panes[0].generation = 321;
        let identity = app.panes[0].identity;
        let bounds = Rectangle::with_size(Size::new(1200., 900.));
        let regions = vec![
            Region {
                marker: Marker::docking_canvas(&app.files_layout),
                bounds,
                full: bounds,
            },
            Region {
                marker: Marker::strip(0, 2),
                bounds,
                full: bounds,
            },
        ];
        let zone = crate::docking::Zone(1).for_panel(2);
        let preview = predicted_preview(
            &regions,
            &Marker::tab(0, identity, 1, "pending".into()),
            bounds,
            zone,
        )
        .unwrap();
        let _ = app.update(Message::Dock(0, Some(identity), zone, bounds.size()));
        assert_eq!(app.pane_count, 4);
        assert_eq!(app.panes[3].identity, identity);
        let actual = app
            .files_layout
            .layout()
            .pane_regions(6., 180., bounds.size());
        let pane = app.files_layout.iter().find(|(_, v)| **v == 3).unwrap().0;
        assert_eq!(preview, actual[pane]);
        let _ = app.update(Message::Scan(
            0,
            321,
            crate::Event::Listed(Err("pending result".into())),
        ));
        assert_eq!(app.panes[3].error.as_deref(), Some("pending result"));
    }
    #[test]
    fn workspace_edges_take_priority_over_source_panel_and_local_edges() {
        let bounds = Rectangle {
            x: 100.,
            y: 100.,
            width: 1000.,
            height: 800.,
        };
        let regions = vec![
            Region {
                marker: Marker::workspace_for(4),
                bounds,
                full: bounds,
            },
            Region {
                marker: Marker::docking_panel(0),
                bounds,
                full: bounds,
            },
        ];
        for (point, edge) in [
            (Point::new(101., 500.), 3),
            (Point::new(1099., 500.), 5),
            (Point::new(600., 101.), 1),
            (Point::new(600., 899.), 7),
        ] {
            assert_eq!(
                dock_target(&regions, &Marker::panel_grip(0), point)
                    .unwrap()
                    .1,
                crate::docking::Zone(edge).for_workspace()
            );
        }
        assert!(dock_target(&regions, &Marker::panel_grip(0), bounds.center()).is_none());
        assert!(workspace_target(&regions, Point::new(120., 500.)).is_none());
    }
    #[test]
    fn workspace_preview_matches_drop_for_all_presets_and_edges() {
        for preset in crate::preferences::DefaultLayout::ALL {
            let count = preset.configuration().0;
            for source in 0..count {
                for edge in [1, 3, 5, 7] {
                    let mut app = crate::App::new("one".into(), "two".into(), vec![]);
                    app.preferences.default_layout = preset;
                    app.initialize_default_layout();
                    let bounds = Rectangle {
                        x: 20.,
                        y: 40.,
                        width: 1600.,
                        height: 800.,
                    };
                    let region = Region {
                        marker: Marker::docking_canvas(&app.files_layout),
                        bounds,
                        full: bounds,
                    };
                    let zone = crate::docking::Zone(edge).for_workspace();
                    let preview =
                        predicted_preview(&[region], &Marker::panel_grip(source), bounds, zone);
                    let _ = app.update(Message::Dock(source, None, zone, bounds.size()));
                    assert_eq!(app.pane_count, count);
                    if count == 1 {
                        assert!(preview.is_none());
                        continue;
                    }
                    let size = app.files_layout.canvas_size(bounds.size());
                    let regions = app.files_layout.layout().pane_regions(6., 180., size);
                    let pane = app
                        .files_layout
                        .iter()
                        .find(|(_, v)| **v == source)
                        .unwrap()
                        .0;
                    assert_eq!(
                        preview,
                        Some(regions[pane] + Vector::new(bounds.x, bounds.y))
                    );
                }
            }
        }
    }
    #[test]
    fn preview_equals_committed_geometry_for_all_presets_edges_and_sources() {
        for preset in crate::preferences::DefaultLayout::ALL {
            for edge in [1, 3, 5, 7] {
                let mut app = crate::App::new("one".into(), "two".into(), vec![]);
                app.preferences.default_layout = preset;
                app.initialize_default_layout();
                for source in 0..app.pane_count {
                    for target in 0..app.pane_count {
                        if source == target {
                            continue;
                        }
                        let mut app = crate::App::new("one".into(), "two".into(), vec![]);
                        app.preferences.default_layout = preset;
                        app.initialize_default_layout();
                        let bounds = Rectangle {
                            x: 20.,
                            y: 40.,
                            width: 1000.,
                            height: 800.,
                        };
                        let region = Region {
                            marker: Marker::docking_canvas(&app.files_layout),
                            bounds,
                            full: bounds,
                        };
                        let zone = crate::docking::Zone(edge).for_panel(target);
                        let preview =
                            predicted_preview(&[region], &Marker::panel_grip(source), bounds, zone)
                                .unwrap();
                        let count = app.pane_count;
                        let identities = app.panes.iter().map(|p| p.identity).collect::<Vec<_>>();
                        let _ = app.update(Message::Dock(source, None, zone, bounds.size()));
                        assert_eq!(app.pane_count, count);
                        assert_eq!(
                            identities,
                            app.panes.iter().map(|p| p.identity).collect::<Vec<_>>()
                        );
                        assert_eq!(app.preferences.default_layout, preset);
                        let size = app.files_layout.canvas_size(bounds.size());
                        let regions = app.files_layout.layout().pane_regions(6., 180., size);
                        let pane = app
                            .files_layout
                            .iter()
                            .find(|(_, id)| **id == source)
                            .unwrap()
                            .0;
                        assert_eq!(preview, regions[pane] + Vector::new(bounds.x, bounds.y));
                    }
                }
            }
        }
    }
    #[test]
    fn hit_testing_uses_target_panel_edges_and_ignores_self_center_and_outside() {
        let bounds = Rectangle {
            x: 100.,
            y: 200.,
            width: 400.,
            height: 400.,
        };
        let regions = vec![Region {
            marker: Marker::docking_panel(1),
            bounds,
            full: bounds,
        }];
        let source = Marker::panel_grip(0);
        for (point, edge) in [
            (Point::new(110., 400.), 3),
            (Point::new(490., 400.), 5),
            (Point::new(300., 210.), 1),
            (Point::new(300., 590.), 7),
        ] {
            assert_eq!(
                dock_target(&regions, &source, point).unwrap().1,
                crate::docking::Zone(edge).for_panel(1)
            );
        }
        assert!(dock_target(&regions, &source, bounds.center()).is_none());
        assert!(dock_target(&regions, &source, Point::ORIGIN).is_none());
        assert!(dock_target(&regions, &Marker::panel_grip(1), Point::new(110., 400.)).is_none());
    }
}

#[cfg(test)]
mod address_input_regressions {
    use super::*;
    #[test]
    fn typing_into_the_address_box_works_and_clicking_the_listing_reverts_the_draft() {
        let mut app = crate::App::new("original".into(), "other".into(), vec![]);
        let mut messages = Vec::new();
        {
            let mut view = app.view();
            let mut tree = Tree::new(&view);
            let renderer = Renderer::new(crate::typography::font(), 13.0.into());
            let size = Size::new(1400., 900.);
            let node =
                view.as_widget_mut()
                    .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
            let mut collect = Collect::new();
            view.as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
            let address = collect
                .regions
                .iter()
                .find(|r| r.marker.kind == 11 && r.marker.pane == 0)
                .unwrap()
                .bounds
                .center();
            let listing = collect
                .regions
                .iter()
                .find(|r| r.marker.kind == 3 && r.marker.pane == 0)
                .unwrap()
                .bounds
                .center();
            for (event, point) in [
                (
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    address,
                ),
                (
                    Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Character("x".into()),
                        modified_key: iced::keyboard::Key::Character("x".into()),
                        physical_key: iced::keyboard::key::Physical::Unidentified(
                            iced::keyboard::key::NativeCode::Unidentified,
                        ),
                        location: iced::keyboard::Location::Standard,
                        modifiers: iced::keyboard::Modifiers::empty(),
                        text: Some("x".into()),
                        repeat: false,
                    }),
                    address,
                ),
                (
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    listing,
                ),
            ] {
                view.as_widget_mut().update(
                    &mut tree,
                    &event,
                    Layout::new(&node),
                    mouse::Cursor::Available(point),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &node.bounds(),
                );
            }
        }
        assert!(
            messages
                .iter()
                .any(|m| matches!(m,Message::Address(0,value) if value.contains('x'))),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::AddressBlur(0)))
        );
        for message in messages {
            let _ = app.update(message);
        }
        assert_eq!(app.panes[0].address, "original");
        assert_eq!(app.panes[0].path, std::path::PathBuf::from("original"));
    }
}
