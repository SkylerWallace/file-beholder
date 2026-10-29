//! App-owned workspace/sidebar composition; models remain in App.
use crate::*;
use iced::widget::{column, row};
impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        static COLLAPSE_ICON: std::sync::OnceLock<iced::widget::svg::Handle> =
            std::sync::OnceLock::new();
        let target = format!("panel {}", self.active + 1);
        let heading = column![
            row![
                iced::widget::image(iced::widget::image::Handle::from_rgba(
                    64,
                    64,
                    include_bytes!("../assets/app-icon.rgba").to_vec(),
                ))
                .width(30)
                .height(30),
                text("File Beholder")
                    .wrapping(iced::widget::text::Wrapping::None)
                    .size(25)
                    .color(theme::color(t.navigation_text)),
            ]
            .spacing(8)
            .align_y(iced::Center),
            Space::new().height(18),
            row![
                crate::typography::text_role(crate::typography::TextRole::Caption, "FILE TREE"),
                Space::new().width(Fill),
                row![
                    sk_iced::components::Tooltip::new(
                        sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                        sk_iced::components::Button::new(
                            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                            self.motion
                        )
                        .icon(
                            sk_iced::icons::Icon::svg(
                                sk_iced::components::Appearance::fixed(
                                    crate::typography::font(),
                                    t
                                ),
                                COLLAPSE_ICON
                                    .get_or_init(|| iced::widget::svg::Handle::from_memory(
                                        include_bytes!("../assets/collapse-tree.svg")
                                    ))
                                    .clone()
                            )
                            .size(16)
                        )
                        .on_press(Message::Locations(locations::Message::CollapseAll)),
                        "Collapse all folders"
                    )
                    .position(iced::widget::tooltip::Position::Bottom),
                    sk_iced::components::Tooltip::new(
                        sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                        sk_iced::components::Button::new(
                            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                            self.motion
                        )
                        .icon(Space::new().width(16).height(16))
                        .with_refresh(crate::button_feedback::RefreshIcon::Center)
                        .on_press(Message::Locations(locations::Message::Refresh)),
                        "Refresh file tree"
                    )
                    .position(iced::widget::tooltip::Position::Bottom)
                ]
                .spacing(4)
                .align_y(iced::Center)
            ]
            .align_y(iced::Center),
            text(format!("Open in {target}"))
                .size(11)
                .color(theme::color(t.secondary)),
        ]
        .spacing(5);
        container(
            sk_iced::components::Navigation::vertical()
                .push(heading)
                .push(
                    self.locations
                        .view(&self.panes[self.active].path, t, self.motion),
                )
                .spacing(10),
        )
        .padding(iced::Padding {
            top: 4.0,
            right: 4.0,
            bottom: 10.0,
            left: 12.0,
        })
        .width(Fill)
        .height(Fill)
        .style(move |_| t.surface())
        .into()
    }
    fn panel_tree(&self, id: usize, t: theme::Tokens) -> Element<'_, Message> {
        let header = row![
            text("File tree").size(12),
            Space::new().width(Fill),
            components::quiet_button(text("−").size(14))
                .padding(3)
                .on_press(Message::PanelLocations(id, locations::Message::CollapseAll))
                .style(t.button(false, false)),
            components::quiet_button(text("↻").size(14))
                .padding(3)
                .on_press(Message::PanelLocations(id, locations::Message::Refresh))
                .style(t.button(false, false)),
        ]
        .spacing(4)
        .align_y(iced::Center);
        container(
            column![
                header,
                self.panel_locations[id]
                    .view(&self.panes[id].path, t, self.motion)
                    .map(move |message| match message {
                        Message::Locations(m) => Message::PanelLocations(id, m),
                        other => other,
                    })
            ]
            .spacing(6),
        )
        .padding(4)
        .width(160)
        .height(Fill)
        .clip(true)
        .into()
    }
    pub(super) fn workspace(&self) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let ribbon = self.ribbon_view();
        let panes: Element<'_, Message> = responsive(move |size| {
            let grid_width = size
                .width
                .max(0.0)
                .max(self.files_layout.minimum_width().ceil())
                .max(if self.preferences.panel_trees {
                    self.files_layout.minimum_width().ceil() * 1.52
                } else {
                    0.
                });
            let grid_size = iced::Size::new(grid_width, size.height);
            // Keep the same keyed host even for a single panel. The empty grid
            // underneath owns split resizing; live panel trees own all content.
            let surface = sk_iced::components::SplitLayout::new(&self.files_layout, |_, _, _| {
                pane_grid::Content::new(iced::widget::Space::new().width(0).height(0))
            })
            .width(grid_width)
            .spacing(6)
            .min_size(180)
            .on_resize(7, move |event| Message::ResizeFiles(event, grid_size));
            let mut items = vec![motion_strip::Item {
                key: motion_strip::Key::Control,
                content: surface.into(),
                snapshot: None,
            }];
            for (pane, id) in self.files_layout.iter() {
                let p = &self.panes[*id];
                items.push(motion_strip::Item {
                    key: motion_strip::Key::Item(p.widget_key),
                    snapshot: Some(components::TabVisual {
                        label: p.path.display().to_string(),
                        selected: self.active == *id,
                    }),
                    content: iced::widget::mouse_area(highlight::wrap(
                        responsive(move |size| {
                            if self.preferences.panel_trees {
                                row![
                                    self.panel_tree(*id, t),
                                    self.pane(*id, (size.width - 166.).max(180.))
                                ]
                                .spacing(6)
                                .into()
                            } else {
                                self.pane(*id, size.width)
                            }
                        }),
                        p.drop_highlight,
                        self.motion,
                        highlight::Style::Destination,
                    ))
                    .on_press(Message::ClickPanel(*pane))
                    .into(),
                });
            }
            let panes = sk_iced::components::Panel::grid(
                items,
                move |size| {
                    let regions = self.files_layout.layout().pane_regions(6.0, 180.0, size);
                    std::iter::once(iced::Rectangle::with_size(size))
                        .chain(self.files_layout.iter().map(|(pane, _)| regions[pane]))
                        .collect()
                },
                grid_size,
                self.files_layout.motion_topology(),
                self.motion,
                |visual, width| components::panel_visual(visual, width),
            );
            container(
                sk_iced::components::scroll_area(
                    iced::widget::sensor(
                        container(tab_drag::mark(
                            panes,
                            tab_drag::Marker::docking_canvas(&self.files_layout),
                        ))
                        .width(grid_width)
                        .height(Fill),
                    )
                    .on_show(Message::LayoutSize)
                    .on_resize(Message::LayoutSize),
                )
                .direction(iced::widget::scrollable::Direction::Horizontal(
                    sk_iced::components::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(4)
                        .build(),
                ))
                .width(Fill)
                .height(Fill)
                .style(t.scroll()),
            )
            .into()
        })
        .into();
        // Keep every gutter on the same flat navigation surface.
        let panes = tab_drag::mark(panes, tab_drag::Marker::workspace_for(self.pane_count));
        let content = container(column![panes,].spacing(5))
            .padding(iced::Padding {
                top: 6.0,
                right: 12.0,
                bottom: 6.0,
                left: 12.0,
            })
            .height(Fill)
            .width(Fill)
            .clip(true)
            .style(move |_| {
                t.visual.background.container_style(container::Style {
                    background: Some(theme::color(t.navigation_background).into()),
                    text_color: Some(theme::color(t.text)),
                    ..Default::default()
                })
            });
        let shadow_space = t
            .visual
            .selected_panel
            .unwrap_or_default()
            .inherit(t.visual.panel)
            .shadow_padding();
        let transfers = container(self.transfer_queue()).padding(iced::Padding {
            top: 0.0,
            right: 12.0 + shadow_space.right,
            bottom: 12.0,
            left: 12.0 + shadow_space.left,
        });
        column![ribbon, content, transfers]
            .height(Fill)
            .spacing(6)
            .into()
    }
}

#[cfg(test)]
mod sidebar_control_checks {
    use super::*;
    #[test]
    fn tree_header_actions_use_equal_default_squares_with_a_gap() {
        let app = App::new("left".into(), "right".into(), vec![]);
        let mut widget = app.sidebar();
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut tree = iced::advanced::widget::Tree::new(&widget);
        let node = widget.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(210., 700.)),
        );
        fn visit(
            n: &iced::advanced::layout::Node,
            origin: iced::Point,
            found: &mut std::collections::BTreeSet<(i32, i32)>,
        ) {
            let b = n.bounds();
            let p = iced::Point::new(origin.x + b.x, origin.y + b.y);
            if b.size() == iced::Size::new(32., 32.) {
                found.insert((p.x.round() as i32, p.y.round() as i32));
            }
            for c in n.children() {
                visit(c, p, found);
            }
        }
        let mut found = std::collections::BTreeSet::new();
        visit(&node, iced::Point::ORIGIN, &mut found);
        let found = found.into_iter().collect::<Vec<_>>();
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].1, found[1].1);
        assert_eq!(found[1].0 - found[0].0, 36);
    }
}
