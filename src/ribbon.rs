//! Both ribbon layouts share commands, tooltip styling, and the Settings anchor.
use crate::*;
use iced::widget::{column, row, tooltip};
const ROW: f32 = 28.0;
const GAP: f32 = 4.0;
const HEIGHT: f32 = ROW * 2.0 + GAP;
fn action<'a>(
    policy: motion::Policy,
    t: theme::Tokens,
    label: &'static str,
    icon: ui_icons::Icon,
    message: Option<Message>,
    width: f32,
    height: f32,
    selected: bool,
    danger: bool,
) -> Element<'a, Message> {
    let enabled = message.is_some();
    let graphic: Element<'a, Message> = if label.ends_with("Undoing…") {
        text(label.chars().next().unwrap().to_string())
            .size(18)
            .into()
    } else {
        ui_icons::material(icon, danger, message.is_some())
    };
    sk_iced::components::tooltip(
        crate::button_feedback::wrap(
            crate::components::themed_toggle(
                button(container(graphic).center(Fill))
                    .width(width)
                    .height(height)
                    .padding(0)
                    .style(move |theme, status| {
                        if danger {
                            t.button(false, true)(theme, status)
                        } else {
                            t.toggle_button(selected)(theme, status)
                        }
                    })
                    .on_press_maybe(message),
                policy,
                enabled,
                selected,
            ),
            policy,
            crate::button_feedback::RefreshIcon::None,
            enabled,
        ),
        container(components::shortcut_hint(label))
            .padding(7)
            .style(move |_| t.card(false)),
        tooltip::Position::Bottom,
    )
    .delay(std::time::Duration::from_secs(1))
    .into()
}
fn divider(t: theme::Tokens) -> Element<'static, Message> {
    container(Space::new())
        .width(1)
        .height(HEIGHT)
        .style(move |_| container::Style {
            background: Some(theme::color(t.border).into()),
            ..Default::default()
        })
        .into()
}
impl App {
    pub fn ribbon_view(&self) -> Element<'_, Message> {
        Element::new(FitRibbon::new(self.single_row_probe(), move |compact| {
            self.ribbon_for_layout(compact)
        }))
    }
    fn ribbon_for_layout(&self, compact: bool) -> Element<'_, Message> {
        let height = if compact { HEIGHT + 12.0 } else { ROW + 12.0 };
        let controls: Element<'_, Message> = responsive(move |size| {
            let width = size.width.max(if compact {
                self.compact_minimum_width()
            } else {
                0.0
            });
            let content = if compact {
                self.compact_ribbon(width)
            } else {
                self.ribbon_controls(width)
            };
            sk_iced::components::scroll_area(container(content).padding([6, 0]))
                .direction(iced::widget::scrollable::Direction::Horizontal(
                    sk_iced::components::Scrollbar::new()
                        .width(4)
                        .scroller_width(4)
                        .spacing(3)
                        .build(),
                ))
                .width(Fill)
                .into()
        })
        .height(height)
        .into();
        let tools: Element<'_, Message> = if compact {
            let t = palette::styled(
                self.dark,
                self.preferences.accent,
                self.preferences.tint,
                self.look(),
            );
            if self.elevated {
                settings_ui::compact_button(self, HEIGHT)
            } else {
                let idle = self.jobs.iter().all(|j| j.report.is_some());
                column![
                    action(
                        self.motion,
                        t,
                        if self.elevating {
                            "Requesting administrator…"
                        } else {
                            "Run as administrator"
                        },
                        ui_icons::Icon::Admin,
                        (!self.elevating && idle).then_some(Message::RelaunchAdmin),
                        ROW,
                        ROW,
                        false,
                        false
                    ),
                    settings_ui::compact_button(self, ROW)
                ]
                .spacing(GAP)
                .into()
            }
        } else {
            settings_ui::button_view(self)
        };
        container(
            sk_iced::components::Toolbar::new()
                .push(controls)
                .push(container(tools).padding([6, 0]))
                .spacing(6)
                .align_y(iced::Top),
        )
        .padding([0, 12])
        .into()
    }
    pub(crate) fn compact_minimum_width(&self) -> f32 {
        // Fixed 28px file buttons and a fully labeled Advanced search control.
        4.0 * ROW
            + 3.0 * GAP
            + ROW
            + 2.0
            + 4.0 * 6.0
            + if self.panes[self.active].filters.recursive && self.panes[self.active].busy {
                190.0
            } else {
                140.0
            }
    }
    fn single_row_probe(&self) -> Element<'_, Message> {
        container(
            row![
                self.single_row_controls(iced::Length::Shrink, true),
                settings_ui::button_view(self)
            ]
            .spacing(6)
            .align_y(iced::Top),
        )
        .padding([6, 12])
        .into()
    }
    #[cfg(test)]
    pub fn single_row_required_width(&self, renderer: &iced::Renderer) -> f32 {
        use iced::advanced::{layout, widget::Tree};
        let mut probe = self.single_row_probe();
        let mut tree = Tree::new(&probe);
        probe
            .as_widget_mut()
            .layout(
                &mut tree,
                renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(100000.0, 200.0)),
            )
            .size()
            .width
    }
    pub fn ribbon_search_probe(&self) -> Element<'_, Message> {
        // Measure the placeholder at the real font size, icon size, and padding.
        // Entered text may scroll within the input and does not resize the ribbon.
        container(
            row![
                ui_icons::material(ui_icons::Icon::Search, false, true),
                container(text("Search...").size(13)).padding([0, 4])
            ]
            .spacing(4)
            .align_y(iced::Center),
        )
        .padding([1, 7])
        .height(ROW)
        .into()
    }
    pub fn ribbon_search_field(&self) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let input = text_input("Search...", &self.panes[self.active].search)
            .without_decoration()
            .id("ribbon-search")
            .on_input(move |s| Message::Search(self.active, s))
            .on_submit(Message::SearchSubmit(self.active))
            .size(13)
            .line_height(iced::widget::text::LineHeight::Absolute(16.0.into()))
            .padding([5, 4])
            .style(move |theme, status| {
                let mut s = iced::widget::text_input::default(theme, status);
                s.background = iced::Color::TRANSPARENT.into();
                s.border = iced::Border::default();
                s.value = theme::color(t.text);
                s.placeholder = theme::color(t.secondary);
                s.selection = theme::color(t.accent);
                s
            });
        let field = container(
            row![
                ui_icons::material(ui_icons::Icon::Search, false, true),
                input
            ]
            .spacing(4)
            .align_y(iced::Center),
        )
        .padding([1, 7])
        .height(ROW)
        .width(Fill)
        .style(move |_| {
            t.visual
                .search_field
                .unwrap_or_default()
                .container_style(container::Style {
                    background: Some(theme::color(t.background).into()),
                    border: iced::Border {
                        width: 1.0,
                        color: theme::color(t.border),
                        radius: 5.0.into(),
                    },
                    ..Default::default()
                })
        });
        let field = sk_iced::components::Surface::new(
            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
            field,
        )
        .role(sk_iced::components::SurfaceRole::SearchField)
        .decorate();
        sk_iced::components::tooltip(
            field,
            container(components::shortcut_hint("Search"))
                .padding(7)
                .style(move |_| t.card(false)),
            tooltip::Position::Bottom,
        )
        .delay(std::time::Duration::from_secs(1))
        .into()
    }
    pub fn compact_ribbon(&self, width: f32) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let cell = ROW;
        let selected = !self.panes[self.active].selections.is_empty();
        let copy = selected.then_some(Message::FileAction(FileAction::Copy));
        let cut = selected.then_some(Message::FileAction(FileAction::Cut));
        let rename = (self.panes[self.active].selections.len() == 1)
            .then_some(Message::FileAction(FileAction::Rename));
        let delete = selected.then_some(Message::FileAction(FileAction::Delete));
        let undo = (!self.undo_busy
            && self.undo_entry.is_some()
            && self.jobs.iter().all(|j| j.report.is_some()))
        .then_some(Message::Undo);
        let files = row![
            action(
                self.motion,
                t,
                "New folder",
                ui_icons::Icon::NewFolder,
                Some(Message::FileAction(FileAction::NewFolder)),
                cell,
                HEIGHT,
                false,
                false
            ),
            column![
                row![
                    action(
                        self.motion,
                        t,
                        "Copy",
                        ui_icons::Icon::Copy,
                        copy,
                        cell,
                        ROW,
                        false,
                        false
                    ),
                    action(
                        self.motion,
                        t,
                        "Cut",
                        ui_icons::Icon::Cut,
                        cut,
                        cell,
                        ROW,
                        false,
                        false
                    ),
                    action(
                        self.motion,
                        t,
                        "Paste",
                        ui_icons::Icon::Paste,
                        Some(Message::FileAction(FileAction::Paste)),
                        cell,
                        ROW,
                        false,
                        false
                    )
                ]
                .spacing(GAP),
                row![
                    action(
                        self.motion,
                        t,
                        "Rename",
                        ui_icons::Icon::Rename,
                        rename,
                        cell,
                        ROW,
                        self.rename.is_some(),
                        false
                    ),
                    action(
                        self.motion,
                        t,
                        "Delete",
                        ui_icons::Icon::Delete,
                        delete,
                        cell,
                        ROW,
                        false,
                        true
                    ),
                    action(
                        self.motion,
                        t,
                        self.undo_label(),
                        ui_icons::Icon::Undo,
                        undo,
                        cell,
                        ROW,
                        false,
                        false
                    )
                ]
                .spacing(GAP)
            ]
            .spacing(GAP)
        ]
        .spacing(GAP);
        let snapshot = action(
            self.motion,
            t,
            "Shadow copies",
            ui_icons::Icon::Restore,
            Some(Message::ShowShadows),
            cell,
            HEIGHT,
            self.shadow_picker.is_some(),
            false,
        );
        let input = self.ribbon_search_field();
        let stopping = self.panes[self.active].filters.recursive && self.panes[self.active].busy;
        let advanced = sk_iced::components::tooltip(
            crate::button_feedback::wrap(
                crate::components::themed_toggle(
                    button(
                        container(
                            row![
                                ui_icons::toggle_material(
                                    ui_icons::Icon::AdvancedSearch,
                                    t,
                                    self.advanced.is_some()
                                ),
                                text(if self.panes[self.active].filters.active() {
                                    "Edit filters"
                                } else {
                                    "Advanced search"
                                })
                                .size(12)
                            ]
                            .spacing(5)
                            .align_y(iced::Center),
                        )
                        .center(Fill),
                    )
                    .height(ROW)
                    .width(Fill)
                    .padding(0)
                    .style(t.toggle_button(self.advanced.is_some()))
                    .on_press(Message::AdvancedToggle),
                    self.motion,
                    true,
                    self.advanced.is_some(),
                ),
                self.motion,
                crate::button_feedback::RefreshIcon::None,
                true,
            ),
            container(
                text(if self.panes[self.active].filters.active() {
                    "Edit filters"
                } else {
                    "Advanced search"
                })
                .size(12),
            )
            .padding(7)
            .style(move |_| t.card(false)),
            tooltip::Position::Bottom,
        )
        .delay(std::time::Duration::from_secs(1));
        let mut advanced_row = row![advanced].spacing(4).width(Fill);
        if stopping {
            advanced_row = advanced_row.push(compact_action(
                self.motion,
                "Stop",
                Some(Message::CancelSearch),
                false,
            ));
        }
        let search = column![input, advanced_row].spacing(GAP).width(Fill);
        row![files, divider(t), snapshot, divider(t), search]
            .spacing(6)
            .height(HEIGHT)
            .width(width)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{layout, widget::Tree};
    #[test]
    fn resizing_switches_layout_and_reclaims_the_extra_row() {
        let app = App::new("home".into(), "other".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut view = app.ribbon_view();
        let mut tree = Tree::new(&view);
        let required = app.single_row_required_width(&renderer);
        for width in [
            required + 100.0,
            500.0,
            required,
            required - 0.5,
            required + 0.5,
        ] {
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(width, 200.0)),
            );
            assert_eq!(node.size().width, width);
            assert_eq!(
                node.size().height,
                if width < required { 72.0 } else { 40.0 }
            );
        }
    }
    #[test]
    fn overflow_threshold_tracks_the_visible_single_row_contents() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let idle = app.single_row_required_width(&renderer);
        assert!(idle > 600.0 && idle < 1200.0, "measured width {idle}");
        app.elevated = true;
        let elevated = app.single_row_required_width(&renderer);
        assert!(elevated < idle);
        app.panes[0].filters.recursive = true;
        app.panes[0].busy = true;
        let searching = app.single_row_required_width(&renderer);
        assert!(searching > elevated);
        assert!(app.compact_minimum_width() > 318.0);
    }
    #[test]
    fn compact_file_buttons_form_a_complete_rectangle_at_both_widths() {
        let app = App::new("home".into(), "other".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for width in [318.0, 600.0] {
            let mut view = app.compact_ribbon(width);
            let mut tree = Tree::new(&view);
            let size = iced::Size::new(width, HEIGHT);
            let node =
                view.as_widget_mut()
                    .layout(&mut tree, &renderer, &layout::Limits::new(size, size));
            let files = &node.children()[0];
            let tall = &files.children()[0];
            let rows = &files.children()[1];
            assert_eq!(tall.bounds().height, HEIGHT);
            assert_eq!(rows.children().len(), 2);
            let top = &rows.children()[0];
            let bottom = &rows.children()[1];
            assert_eq!(top.children().len(), 3);
            assert_eq!(bottom.children().len(), 3);
            assert_eq!(top.bounds().width, bottom.bounds().width);
            assert_eq!(top.bounds().y, tall.bounds().y);
            assert_eq!(
                bottom.bounds().y + bottom.bounds().height,
                tall.bounds().y + HEIGHT
            );
            for (a, b) in top.children().iter().zip(bottom.children()) {
                assert_eq!(a.bounds().x, b.bounds().x);
                assert_eq!(a.bounds().width, b.bounds().width);
                assert_eq!(a.bounds().height, ROW);
                assert_eq!(b.bounds().height, ROW);
            }
            assert_eq!(node.children()[2].bounds().height, HEIGHT);
            assert_eq!(node.children()[4].bounds().height, HEIGHT);
        }
    }
}

use sk_iced::components::AdaptiveToolbar as FitRibbon;

impl App {
    pub(super) fn ribbon_controls(&self, width: f32) -> Element<'_, Message> {
        self.single_row_controls(iced::Length::Fixed(width), false)
    }
    pub(super) fn single_row_controls(
        &self,
        width: iced::Length,
        measure: bool,
    ) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let has_selection = !self.panes[self.active].selections.is_empty();
        let files = row![
            icon_action(
                self.motion,
                "New folder",
                ui_icons::Icon::NewFolder,
                Some(Message::FileAction(FileAction::NewFolder)),
                false
            ),
            icon_action(
                self.motion,
                "Copy",
                ui_icons::Icon::Copy,
                has_selection.then_some(Message::FileAction(FileAction::Copy)),
                false
            ),
            icon_action(
                self.motion,
                "Cut",
                ui_icons::Icon::Cut,
                has_selection.then_some(Message::FileAction(FileAction::Cut)),
                false
            ),
            icon_action(
                self.motion,
                "Paste",
                ui_icons::Icon::Paste,
                Some(Message::FileAction(FileAction::Paste)),
                false
            ),
            crate::components::icon_toggle_action(
                self.motion,
                "Rename",
                ui_icons::Icon::Rename,
                (self.panes[self.active].selections.len() == 1)
                    .then_some(Message::FileAction(FileAction::Rename)),
                self.rename.is_some()
            ),
            icon_action(
                self.motion,
                "Delete",
                ui_icons::Icon::Delete,
                has_selection.then_some(Message::FileAction(FileAction::Delete)),
                true
            ),
            icon_action(
                self.motion,
                self.undo_label(),
                ui_icons::Icon::Undo,
                (!self.undo_busy
                    && self.undo_entry.is_some()
                    && self.jobs.iter().all(|j| j.report.is_some()))
                .then_some(Message::Undo),
                false
            )
        ]
        .spacing(4)
        .align_y(iced::Center);
        let search = container(if measure {
            self.ribbon_search_probe()
        } else {
            self.ribbon_search_field()
        })
        .width(if measure { iced::Length::Shrink } else { Fill });
        let advanced_search = sk_iced::components::tooltip(
            crate::components::themed_toggle(
                button(
                    row![
                        ui_icons::toggle_material(
                            ui_icons::Icon::AdvancedSearch,
                            t,
                            self.advanced.is_some()
                        ),
                        crate::typography::text_role(
                            crate::typography::TextRole::Caption,
                            if self.panes[self.active].filters.active() {
                                "Edit filters"
                            } else {
                                "Advanced search"
                            }
                        )
                    ]
                    .spacing(5)
                    .align_y(iced::Center),
                )
                .width(iced::Length::Shrink)
                .height(28)
                .padding(6)
                .style(t.toggle_button(self.advanced.is_some()))
                .on_press(Message::AdvancedToggle),
                self.motion,
                true,
                self.advanced.is_some(),
            ),
            container(crate::typography::text_role(
                crate::typography::TextRole::Caption,
                "Advanced search",
            ))
            .padding(8)
            .style(move |_| t.card(false)),
            iced::widget::tooltip::Position::Bottom,
        )
        .delay(std::time::Duration::from_secs(1));
        let stop: Element<'_, Message> =
            if self.panes[self.active].filters.recursive && self.panes[self.active].busy {
                compact_action(self.motion, "Stop", Some(Message::CancelSearch), false)
            } else {
                Space::new().width(0).into()
            };
        let search = row![search, advanced_search, stop]
            .spacing(4)
            .width(Fill)
            .align_y(iced::Center);
        let mut controls = row![
            files,
            ribbon_divider(t),
            sk_iced::components::tooltip(
                crate::components::themed_toggle(
                    button(
                        container(
                            row![
                                ui_icons::material(ui_icons::Icon::Restore, false, true),
                                crate::typography::text_role(
                                    crate::typography::TextRole::Caption,
                                    "Shadow copies"
                                )
                            ]
                            .spacing(5)
                            .align_y(iced::Center)
                        )
                        .center_y(Fill)
                    )
                    .height(components::RIBBON_HEIGHT)
                    .padding([0, 7])
                    .style(t.toggle_button(self.shadow_picker.is_some()))
                    .on_press(Message::ShowShadows),
                    self.motion,
                    true,
                    self.shadow_picker.is_some()
                ),
                container(crate::typography::text_role(
                    crate::typography::TextRole::Caption,
                    "Browse earlier snapshots of your drives and copy older files."
                ))
                .padding(8)
                .style(move |_| t.card(false)),
                iced::widget::tooltip::Position::Bottom
            )
            .delay(std::time::Duration::from_secs(1)),
            ribbon_divider(t),
            search,
            ribbon_divider(t)
        ]
        .spacing(8)
        .align_y(iced::Center);
        if !self.elevated {
            let idle = self.jobs.iter().all(|j| j.report.is_some());
            let action = crate::components::themed_native(
                button(
                    container(
                        row![
                            ui_icons::material(
                                ui_icons::Icon::Admin,
                                false,
                                !self.elevating && idle
                            ),
                            text(if self.elevating {
                                "Requesting…"
                            } else {
                                "Run as admin"
                            })
                            .size(12)
                        ]
                        .spacing(5)
                        .align_y(iced::Center),
                    )
                    .center_y(Fill),
                )
                .height(components::RIBBON_HEIGHT)
                .padding([0, 7])
                .style(t.button(false, false))
                .on_press_maybe((!self.elevating && idle).then_some(Message::RelaunchAdmin)),
                self.motion,
                !self.elevating && idle,
            );
            controls = controls.push(
                sk_iced::components::tooltip(
                    action,
                    container(
                        text(if idle {
                            "Relaunch with administrator privileges"
                        } else {
                            "Finish or cancel queued transfers before relaunching"
                        })
                        .size(12),
                    )
                    .padding(8)
                    .style(move |_| t.card(false)),
                    iced::widget::tooltip::Position::Bottom,
                )
                .delay(std::time::Duration::from_secs(1)),
            );
        }
        controls.width(width).into()
    }
}
