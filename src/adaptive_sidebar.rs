//! Preserve the saved sidebar split; borrow its space only when the ribbon needs it.
use crate::*;
const RAIL_WIDTH: f32 = 18.;
use iced::widget::{button, column, container, mouse_area, opaque, row, tooltip};
impl App {
    pub fn sidebar_collapses(&self, size: iced::Size) -> bool {
        size.width - self.sidebar_width.round().max(160.) - 6.
            < self.compact_minimum_width() + 24. + 6. + 28.
    }
    fn sidebar_tray_handle(&self) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let open = self.sidebar_drawer;
        tooltip(
            sk_iced::components::Button::custom(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                self.motion,
                button(
                    container(text(if open { "‹" } else { "›" }).size(24))
                        .center_x(Fill)
                        .center_y(Fill),
                )
                .width(RAIL_WIDTH)
                .height(Fill)
                .padding(0)
                .on_press(Message::ToggleSidebar)
                .style(move |_, status| {
                    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
                    button::Style {
                        background: if open && !hover {
                            None
                        } else {
                            Some(
                                theme::blend(
                                    t.navigation_background,
                                    t.accent,
                                    if hover {
                                        0.16
                                    } else if open {
                                        0.09
                                    } else {
                                        0.035
                                    },
                                )
                                .into(),
                            )
                        },
                        text_color: theme::color(if hover { t.accent } else { t.secondary }),
                        border: iced::Border {
                            radius: iced::border::Radius {
                                top_left: 0.0,
                                top_right: 5.0,
                                bottom_left: 0.0,
                                bottom_right: 5.0,
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            )
            .feedback(false),
            container(
                text(if open {
                    "Close file tree"
                } else {
                    "Open file tree"
                })
                .size(12),
            )
            .padding(7)
            .style(move |_| t.card(false)),
            tooltip::Position::Right,
        )
        .delay(std::time::Duration::from_secs(1))
        .into()
    }
    pub fn adaptive_sidebar(&self, size: iced::Size) -> Element<'_, Message> {
        if self.preferences.panel_trees {
            return self.workspace();
        }
        let compact = self.sidebar_collapses(size);
        let visible = !compact;
        let open = compact && self.sidebar_drawer;
        let width = self.sidebar_width.round().max(160.);
        // Stable slots keep the workspace's widget tree and scroll offsets alive.
        let sidebar = sk_iced::components::InteractionGate::new(
            container(self.sidebar())
                .width(if visible { width } else { 0. })
                .height(Fill)
                .clip(true),
            visible,
        );
        let base: Element<'_, Message> = iced::widget::Row::from_vec(vec![
            sidebar.into(),
            if compact {
                self.sidebar_tray_handle()
            } else {
                sk_iced::components::ResizeGrip::new(
                    sk_iced::components::Appearance::themed(
                        crate::typography::font(),
                        palette::from_theme,
                    ),
                    width,
                )
                .horizontal()
                .indicator(false)
                .thickness(6.)
                .limits(160., (size.width * 0.45).max(160.))
                .on_resize(Message::SidebarWidth)
                .into()
            },
            self.workspace(),
        ])
        .width(Fill)
        .height(Fill)
        .into();
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let shade: Element<'_, Message> = if open {
            mouse_area(container(Space::new()).width(Fill).height(Fill).style(|_| {
                container::Style {
                    background: Some(iced::Color::from_rgba(0., 0., 0., 0.3).into()),
                    ..Default::default()
                }
            }))
            .on_press(Message::CloseSidebar)
            .into()
        } else {
            Space::new().into()
        };
        let shell =
            sk_iced::components::NavigationDrawer::new(base, open, self.motion, move || {
                let contents = container(
                    column![
                        row![
                            text("File tree").size(13),
                            Space::new().width(Fill),
                            sk_iced::components::Button::new(
                                sk_iced::components::Appearance::fixed(
                                    crate::typography::font(),
                                    t
                                ),
                                self.motion
                            )
                            .text("×")
                            .width(28)
                            .height(28)
                            .padding(0)
                            .on_press(Message::CloseSidebar)
                        ]
                        .align_y(iced::Center),
                        self.sidebar()
                    ]
                    .spacing(6),
                )
                .padding(8)
                .width(Fill)
                .height(Fill);
                let drawer = container(row![contents, self.sidebar_tray_handle()].height(Fill))
                    .padding(1)
                    .width((size.width - 48.0).min(300.0).max(1.0) + RAIL_WIDTH)
                    .height(Fill)
                    .clip(true)
                    .style(move |_| t.card(false));
                opaque(drawer).into()
            });
        shell.backdrop(shade).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{Layout, Shell, layout, widget::Tree};
    #[test]
    fn compact_tray_toggles_without_changing_saved_sidebar_width() {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        let width = app.sidebar_width;
        let _ = app.update(Message::SidebarWindowResized(iced::Size::new(480., 700.)));
        assert!(app.sidebar_collapses(iced::Size::new(480., 700.)));
        let _ = app.update(Message::ToggleSidebar);
        assert!(app.sidebar_drawer);
        let _ = app.update(Message::ToggleSidebar);
        assert!(!app.sidebar_drawer);
        assert_eq!(app.sidebar_width, width);
    }
    #[test]
    fn sidebar_width_is_exact_before_window_resize_messages_arrive() {
        let app = App::new("home".into(), "other".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let saved = app.preferences.sidebar;
        for width in [1400., 1398., 1200., 1198., 1000., 999.] {
            let size = iced::Size::new(width, 700.);
            let mut widget = app.adaptive_sidebar(size);
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            assert_eq!(node.children()[0].children()[0].size().width, 210.);
            assert!((node.children()[0].children()[2].size().width - (width - 216.)).abs() < 0.01);
            assert_eq!(
                app.window_width, 1400.,
                "rendering must not mutate the model"
            );
        }
        assert!(app.sidebar_collapses(iced::Size::new(480., 700.)));
        assert_eq!(app.preferences.sidebar, saved);
    }
    #[test]
    fn responsive_sidebar_preserves_workspace_scroll_position() {
        use iced::advanced::widget::Operation;
        #[derive(Default)]
        struct Probe {
            list: Option<iced::Rectangle>,
            rows: Vec<(usize, iced::Rectangle)>,
        }
        impl Operation for Probe {
            fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
                f(self);
            }
            fn custom(
                &mut self,
                _: Option<&iced::widget::Id>,
                b: iced::Rectangle,
                state: &mut dyn std::any::Any,
            ) {
                if let Some(m) = state.downcast_ref::<crate::tab_drag::Marker>() {
                    if m.pane == 0 && m.kind == 3 {
                        self.list = Some(b);
                    }
                    if m.pane == 0 && m.kind == 2 {
                        self.rows.push((m.index, b));
                    }
                }
            }
        }
        let mut app = App::new("left".into(), "right".into(), vec![]);
        app.motion = crate::motion::Policy::DISABLED;
        for pane in &mut app.panes {
            pane.entries = (0..200)
                .map(|i| Entry {
                    name: format!("Item {i:04}"),
                    path: format!("item-{i}").into(),
                    directory: false,
                    hidden: false,
                    bytes: Some(1024),
                    partial: false,
                    modified: None,
                    modified_label: String::new(),
                    icon: None,
                })
                .collect();
            pane.reorder();
        }
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());

        let mut tree = Tree::empty();
        let mut expected = None;
        for stage in 0..3 {
            let size = iced::Size::new(if stage == 1 { 480. } else { 1400. }, 900.);
            let bounds = iced::Rectangle::with_size(size);
            let _ = app.update(Message::SidebarWindowResized(size));
            let mut view = app.adaptive_sidebar(size);
            tree.diff(&view);
            let mut node =
                view.as_widget_mut()
                    .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
            let mut p = Probe::default();
            view.as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut p);
            if stage == 0 {
                let point = p.list.unwrap().center();
                let mut messages = vec![];
                let mut shell = Shell::new(&mut messages);
                view.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                        delta: iced::mouse::ScrollDelta::Lines { x: 0., y: -20. },
                    }),
                    Layout::new(&node),
                    iced::mouse::Cursor::Available(point),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut shell,
                    &bounds,
                );
                if shell.is_layout_invalid() {
                    node = view.as_widget_mut().layout(
                        &mut tree,
                        &renderer,
                        &layout::Limits::new(size, size),
                    );
                }
            }
            let mut p = Probe::default();
            view.as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut p);
            let list = p.list.unwrap();
            let first = p
                .rows
                .iter()
                .filter(|(_, b)| b.intersects(&list))
                .map(|(i, _)| *i)
                .min()
                .unwrap();
            assert!(
                first > 0,
                "scroll event must have moved the file list: stage={stage}"
            );
            if let Some(expected) = expected {
                assert_eq!(first, expected);
            } else {
                expected = Some(first);
            }
            drop(view);
        }
    }
}
