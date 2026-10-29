//! App-specific file pane components. Borrow live state; never own scans or tabs.
use crate::*;
use iced::widget::{column, row};
#[derive(Clone, Copy)]
pub(super) struct FilePane<'a> {
    app: &'a App,
    id: usize,
    width: f32,
}
impl<'a> FilePane<'a> {
    pub(super) fn new(app: &'a App, id: usize, width: f32) -> Self {
        Self { app, id, width }
    }
    pub(super) fn view(self) -> Element<'a, Message> {
        let app = self.app;
        let id = self.id;

        let p = &app.panes[id];
        let t = palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        );
        // Reserve elevation inside the scroll viewport, including unselected panes.
        let shadow_space = t
            .visual
            .selected_panel
            .unwrap_or_default()
            .inherit(t.visual.panel)
            .shadow_padding();
        let width = (self.width - shadow_space.left - shadow_space.right).max(0.);
        let total = p.total();
        let incomplete = p.busy || p.incomplete_entries > 0;
        let scale = preferences::content_scale(app.preferences.density);
        let columns = p.columns.map(|width| width * scale);
        let extension_width = p.extension_width * scale;
        let table_width = (width - 16.0).max(
            app.preferences
                .columns
                .visible()
                .iter()
                .map(|c| {
                    if c.property_index().is_some() {
                        160.0
                    } else {
                        c.width_index()
                            .map_or(0.0, |i| if i == 4 { extension_width } else { columns[i] })
                    }
                })
                .sum::<f32>()
                + 180.0 * scale
                + 48.0 * scale,
        );
        let visible = app.preferences.columns.visible();
        let headers = self.headers(&visible, columns, extension_width, scale);
        let listing = self.listing(visible, columns, extension_width, total);
        let table = container(horizontal_rail::view(
            sk_iced::components::scroll_area(
                sk_iced::components::DataTable::new(headers, listing)
                    .width(table_width)
                    .height(Fill)
                    .build(),
            )
            .direction(iced::widget::scrollable::Direction::Horizontal(
                sk_iced::components::Scrollbar::hidden().build(),
            ))
            .width(Fill)
            .height(Fill)
            .style(t.scroll())
            .into(),
            table_width,
        ))
        .padding([0, 6]);
        let status = if let Some(error) = &p.error {
            format!("Cannot read directory: {error}")
        } else {
            format!(
                "{} items · {}{} · {} items selected",
                p.order.len(),
                app.preferences.size_units.format(total),
                if incomplete { " (partial)" } else { "" },
                p.selections.len()
            )
        };
        let title = Self { width, ..self }.tabs();
        let address = text_input("Directory path or %VARIABLE%", &p.address)
            .on_input(move |s| Message::Address(id, s))
            .on_submit(Message::Go(id))
            .size(13)
            .line_height(iced::widget::text::LineHeight::Absolute(16.0.into()))
            .padding([6, 7]);
        let address = tab_drag::mark(address.into(), tab_drag::Marker::address(id));
        tab_drag::mark(
            container(
                sk_iced::components::Panel::custom(container(
                    column![
                        title,
                        container(
                            row![
                                navigation_action(
                                    app.motion,
                                    "←",
                                    (!p.back.is_empty()).then_some(Message::Back(id))
                                ),
                                navigation_action(
                                    app.motion,
                                    "→",
                                    (!p.forward.is_empty()).then_some(Message::Forward(id))
                                ),
                                navigation_action(
                                    app.motion,
                                    "↑",
                                    p.parent().map(|_| Message::Up(id))
                                ),
                                navigation_action(app.motion, "↻", Some(Message::Refresh(id))),
                                address
                            ]
                            .spacing(3)
                            .align_y(iced::Center)
                        )
                        .padding([3, 4]),
                        table,
                        iced::widget::mouse_area(
                            text(status)
                                .size(11)
                                .color(
                                    theme::color(if p.error.is_some() {
                                        t.failure
                                    } else {
                                        t.secondary
                                    })
                                    .scale_alpha(
                                        if app.status_hovered == Some(id) {
                                            1.0
                                        } else {
                                            0.65
                                        }
                                    )
                                )
                                .wrapping(iced::widget::text::Wrapping::None)
                        )
                        .on_enter(Message::StatusHover(Some(id)))
                        .on_exit(Message::StatusHover(None)),
                    ]
                    .spacing(0),
                ))
                .padding(2)
                .height(Fill)
                .width(Fill)
                .clip(true)
                .style(move |_| t.panel(app.active == id))
                .with_appearance(sk_iced::components::Appearance::fixed(
                    crate::typography::font(),
                    t,
                )),
            )
            .padding(shadow_space)
            .width(Fill)
            .height(Fill)
            .into(),
            tab_drag::Marker::docking_panel(id),
        )
    }
    fn headers(
        self,
        visible: &[columns::Column],
        columns: [f32; 4],
        extension_width: f32,
        scale: f32,
    ) -> Element<'a, Message> {
        let app = self.app;
        let id = self.id;
        let p = &app.panes[id];
        let mut headers = row![]
            .align_y(iced::Center)
            .spacing(4. * scale)
            .padding(iced::Padding {
                left: 6. * scale,
                right: 16. + 6. * scale,
                ..Default::default()
            });
        for column in visible {
            let suffix = if p.sort == column.sort() {
                if p.descending { " ▾" } else { " ▴" }
            } else {
                ""
            };
            let label = container(
                text(format!("{}{suffix}", column.label()))
                    .size(12. * scale)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .padding([2. * scale, 4. * scale])
            .width(Fill)
            .clip(true);
            let cell: Element<'_, Message> = if let Some(index) = column.width_index() {
                let width = if index == 4 {
                    extension_width
                } else {
                    columns[index]
                };
                let handle = column_resize::handle_scaled(id, index, width, 400. * scale, scale);
                if index == 0 {
                    row![label, handle].width(width).into()
                } else {
                    row![handle, label].width(width).into()
                }
            } else {
                label.width(column.width(columns, extension_width)).into()
            };
            headers = headers.push(cell);
        }
        tab_drag::mark(
            header_drag::view(headers.into(), id, visible.to_vec(), scale),
            tab_drag::Marker::headers(id),
        )
    }
    fn listing(
        self,
        visible: Vec<columns::Column>,
        columns: [f32; 4],
        extension_width: f32,
        total: u64,
    ) -> Element<'a, Message> {
        let app = self.app;
        let id = self.id;
        let p = &app.panes[id];
        let t = palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        );
        let units = app.preferences.size_units;
        let row_height = preferences::row_height(app.preferences.density);
        let listing = virtual_list::file_list_revealed(
            p.order.len(),
            p.view_identity,
            row_height,
            p.reveal_selection,
            move |row| {
                let index = p.order[row];
                let e = &p.entries[index];
                let size = match e.bytes {
                    Some(n) => format!("{}{}", units.format(n), if e.partial { "*" } else { "" }),
                    None => {
                        if e.partial {
                            "Unavailable".into()
                        } else if p.busy {
                            "Scanning…".into()
                        } else {
                            "—".into()
                        }
                    }
                };
                let share = if total == 0 {
                    0.0
                } else {
                    e.bytes.unwrap_or(0) as f64 / total as f64 * 100.0
                } as f32;
                highlight::wrap(
                    tab_drag::mark(
                        file_row(
                            &e.name,
                            size,
                            e.directory,
                            e.hidden,
                            share,
                            p.selections.contains(&index),
                            Message::Select(id, index),
                            Some(e.modified_label.as_str()),
                            e.icon.as_ref(),
                            columns,
                            extension_width,
                            &columns::extension(&e.path, e.directory),
                            if visible.iter().any(|c| c.property_index().is_some()) {
                                file_properties::get(&e.path)
                            } else {
                                Default::default()
                            },
                            &visible,
                            row_height,
                            app.preferences.alternating_rows && row % 2 == 1,
                        ),
                        {
                            let mut marker = tab_drag::Marker::file(
                                id,
                                index,
                                e.name.clone(),
                                if p.selections.contains(&index) {
                                    p.selections.len()
                                } else {
                                    1
                                },
                            );
                            marker.directory = e.directory.then(|| e.path.clone());
                            marker
                        },
                    ),
                    app.highlights.pulse(&e.path),
                    app.motion,
                    highlight::Style::Item,
                )
            },
            move |row| Message::OpenEntry(id, p.order[row]),
        );
        let listing: Element<'_, Message> = if p.order.is_empty() {
            sk_iced::components::EmptyState::new(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                if p.error.is_some() {
                    "Directory unavailable"
                } else if p.busy && p.entries.is_empty() {
                    "Loading directory…"
                } else {
                    "No matching files"
                },
            )
            .text_size(13)
            .center(Fill)
            .build()
        } else {
            listing
        };
        highlight::wrap(
            tab_drag::mark(listing, tab_drag::Marker::file_panel(id)),
            app.highlights.pulse(&p.path),
            app.motion,
            highlight::Style::Destination,
        )
    }
    fn tabs(self) -> Element<'a, Message> {
        let app = self.app;
        let id = self.id;
        let width = self.width;
        let p = &app.panes[id];
        let t = palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        );
        let mut tabstrip = Vec::new();
        for (index, tab) in p.tabs.iter().enumerate() {
            let folder = tab
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| tab.path.display().to_string());
            let label =
                if let Some(copy) = tab.shadow.as_ref().filter(|copy| copy.contains(&tab.path)) {
                    if tab.path == copy.root() {
                        copy.tab_label()
                    } else {
                        format!("{} · {folder}", copy.tab_label())
                    }
                } else {
                    folder
                };
            tabstrip.push(motion_strip::Item {
                key: motion_strip::Key::Item(tab.identity),
                snapshot: Some(components::TabVisual {
                    label: label.clone(),
                    selected: p.selected_tab == index,
                }),
                content: tab_drag::mark(
                    sk_iced::components::tooltip(
                        browser_tab_sized(
                            app.motion,
                            label.clone(),
                            Message::SelectTab(id, index),
                            (p.tabs.len() > 1 || app.pane_count > 1)
                                .then_some(Message::CloseTab(id, index)),
                            p.selected_tab == index,
                            tab_width(width, p.tabs.len()),
                        ),
                        container(
                            text(format!(
                                "{}{}",
                                tab.shadow
                                    .as_ref()
                                    .filter(|copy| copy.contains(&tab.path))
                                    .map(|copy| format!("{}\n", copy.description()))
                                    .unwrap_or_default(),
                                tab.path.display()
                            ))
                            .size(12),
                        )
                        .padding(8)
                        .style(move |_| t.card(false)),
                        iced::widget::tooltip::Position::Bottom,
                    )
                    .delay(std::time::Duration::from_secs(1))
                    .into(),
                    tab_drag::Marker::tab(id, tab.identity, index, label),
                ),
            });
        }
        tabstrip.push(motion_strip::Item {
            key: motion_strip::Key::Control,
            snapshot: None,
            content: tab_drag::mark(
                sk_iced::components::tooltip(
                    tab_add(app.motion, Message::NewTab(id)),
                    container(components::shortcut_hint("New tab"))
                        .padding(7)
                        .style(move |_| t.card(false)),
                    iced::widget::tooltip::Position::Bottom,
                )
                .delay(std::time::Duration::from_secs(1))
                .into(),
                tab_drag::Marker::tab_control(id),
            ),
        });
        let tabstrip: Element<'_, Message> =
            sk_iced::components::TabBar::new(app.motion, components::tab_visual)
                .items(tabstrip)
                .overflow(
                    p.tabs[p.selected_tab].identity,
                    p.selected_tab as f32 * (tab_width(width, p.tabs.len()) + 2.0),
                    p.selected_tab as f32 * (tab_width(width, p.tabs.len()) + 2.0)
                        + tab_width(width, p.tabs.len()),
                )
                .into();
        let title = tab_drag::mark(tabstrip, tab_drag::Marker::strip(id, p.tabs.len()));
        let grip = tab_drag::mark(
            container(text("⠿").size(18).color(theme::color(t.secondary)))
                .width(28)
                .height(28)
                .align_x(iced::Center)
                .align_y(iced::Center)
                .into(),
            tab_drag::Marker::grip_icon(id),
        );
        let close: Element<'_, Message> = if app.pane_count > 1 {
            tab_drag::mark(
                sk_iced::components::tooltip(
                    crate::button_feedback::wrap(
                        button(container(text("×").size(18)).center(28))
                            .width(28)
                            .height(28)
                            .padding(0)
                            .style(move |_, status| components::panel_control_style(t, status))
                            .on_press(Message::ClosePanel(id)),
                        app.motion,
                        crate::button_feedback::RefreshIcon::None,
                        true,
                    ),
                    container(components::shortcut_hint("Close panel"))
                        .padding(8)
                        .style(move |_| t.card(false)),
                    iced::widget::tooltip::Position::Bottom,
                )
                .delay(std::time::Duration::from_secs(1))
                .into(),
                tab_drag::Marker::tab_control(id),
            )
        } else {
            Space::new().width(28).height(28).into()
        };
        let mut title_row = row![
            grip,
            title,
            Space::new().width(6),
            container(Space::new())
                .width(1)
                .height(16)
                .style(move |_| container::Style {
                    background: Some(theme::color(t.border).into()),
                    ..Default::default()
                }),
            tab_drag::mark(
                sk_iced::components::tooltip(
                    crate::button_feedback::wrap(
                        button(
                            container(ui_icons::material(
                                ui_icons::Icon::NewPanel,
                                false,
                                app.pane_count < 4
                            ))
                            .center(28)
                        )
                        .width(28)
                        .height(28)
                        .padding(0)
                        .on_press_maybe((app.pane_count < 4).then_some(Message::NewPanel(id)))
                        .style(move |_, status| components::panel_control_style(t, status)),
                        app.motion,
                        crate::button_feedback::RefreshIcon::None,
                        app.pane_count < 4
                    ),
                    container(components::shortcut_hint("New panel"))
                        .padding(8)
                        .style(move |_| t.card(false)),
                    iced::widget::tooltip::Position::Bottom
                )
                .delay(std::time::Duration::from_secs(1))
                .into(),
                tab_drag::Marker::tab_control(id)
            ),
        ]
        .spacing(4)
        .align_y(iced::Center);
        if app.pane_count > 1 {
            title_row = title_row.push(close);
        }
        tab_drag::mark(
            container(title_row)
                .padding(iced::Padding {
                    top: 2.0,
                    right: 3.0,
                    bottom: 0.0,
                    left: 3.0,
                })
                .style(move |_| {
                    t.visual
                        .tab_bar
                        .unwrap_or_default()
                        .container_style(t.surface())
                })
                .into(),
            tab_drag::Marker::panel_grip(id),
        )
    }
}
