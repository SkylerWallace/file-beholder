// SPDX-License-Identifier: MIT
//! Settings sections, control choices and popup composition.
use crate::preferences;
use crate::{
    App, Message,
    preferences::{DefaultLayout, ThemeMode},
    theme::{Tokens, color},
    typography::text,
};
use iced::widget::{button, column, container, image, row};
use iced::{Element, Fill};
pub fn segmented<'a>(
    policy: crate::motion::Policy,
    t: Tokens,
    labels: &[&str],
    selected: usize,
    width: f32,
    choose: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    use sk_iced::components::{Appearance, Segment};
    let options = labels.iter().map(|label| {
        let hint = match *label {
            "Auto" => Some("Follow Windows appearance"),
            "Dark only" => Some("Tint backgrounds only in dark mode"),
            _ => None,
        };
        let option = Segment::new((*label).to_owned());
        if let Some(hint) = hint {
            option.tooltip(hint)
        } else {
            option
        }
    });
    crate::typography::context(Appearance::fixed(crate::typography::font(), t), policy)
        .segmented()
        .options(options)
        .selected(selected)
        .width(width)
        .text_size(13.333)
        .feedback(false)
        .on_select(choose)
        .into()
}

fn layout_choice_style(
    t: Tokens,
    selected: bool,
    theme: &iced::Theme,
    status: button::Status,
) -> button::Style {
    if t.visual.segment.is_some() {
        return t.segment(selected, false)(theme, status);
    }
    let mut style = t.button(false, false)(theme, status);
    if selected {
        style.background = Some(crate::theme::blend(t.surface, t.accent, 0.16).into());
        style.border.color = color(t.accent);
        style.border.width = 1.5;
    }
    style
}

fn layout_choices(t: Tokens, selected: DefaultLayout) -> Element<'static, Message> {
    let mut choices = row![].spacing(4);
    for (index, layout) in DefaultLayout::ALL.into_iter().enumerate() {
        static ICONS: std::sync::OnceLock<[iced::widget::svg::Handle; 6]> =
            std::sync::OnceLock::new();
        let icons = ICONS.get_or_init(|| {
            [
                iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/layouts/single.svg"
                )),
                iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/layouts/vertical.svg"
                )),
                iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/layouts/horizontal.svg"
                )),
                iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/layouts/three.svg"
                )),
                iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/layouts/grid.svg"
                )),
                iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/material/undo.svg"
                )),
            ]
        });
        let active = layout == selected;
        let icon = iced::widget::svg(icons[index].clone())
            .width(24)
            .height(18)
            .style(move |theme, _| iced::widget::svg::Style {
                color: Some(
                    layout_choice_style(t, active, theme, button::Status::Active).text_color,
                ),
            });
        choices = choices.push(
            sk_iced::components::Button::new(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                crate::motion::Policy::DISABLED,
            )
            .icon(icon)
            .width(Fill)
            .height(32)
            .padding(0)
            .surface_role(sk_iced::components::SurfaceRole::Segment(active))
            .style(move |theme, status| layout_choice_style(t, active, theme, status))
            .on_press(Message::DefaultLayoutChoice(layout))
            .feedback(false),
        );
    }
    choices.width(198).into()
}
fn category(t: Tokens, label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(15)
        .color(color(t.text))
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..crate::typography::font()
        })
        .into()
}
fn divider(t: Tokens) -> Element<'static, Message> {
    container(iced::widget::Space::new())
        .height(1)
        .width(Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(color(t.border).into()),
            ..Default::default()
        })
        .into()
}
fn accent_choices(t: Tokens, selected: crate::palette::Accent) -> Element<'static, Message> {
    let mut grid = sk_iced::components::ChoiceGrid::new(6).spacing(4);
    for &accent in &crate::palette::Accent::ALL {
        let active = accent == selected;
        let swatch = container(iced::widget::Space::new())
            .width(Fill)
            .height(14)
            .style(move |_| iced::widget::container::Style {
                background: Some(color(accent.rgb()).into()),
                border: iced::Border {
                    radius: 3.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
        grid = grid.push(
            sk_iced::components::Button::custom(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                crate::motion::Policy::DISABLED,
                button(swatch)
                    .width(Fill)
                    .padding(3)
                    .style(move |_, _| iced::widget::button::Style {
                        background: Some(color(t.surface).into()),
                        border: iced::Border {
                            color: color(if active { t.text } else { t.border }),
                            width: if active { 2.0 } else { 1.0 },
                            radius: 5.into(),
                        },
                        ..Default::default()
                    })
                    .on_press(Message::AccentChoice(accent)),
            )
            .feedback(false),
        );
    }
    container(grid).width(198).into()
}
fn setting_card<'a>(
    t: Tokens,
    title: &'static str,
    description: &'static str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    sk_iced::components::SettingsCard::new(
        sk_iced::components::Appearance::fixed(crate::typography::font(), t),
        title,
        control,
    )
    .description(description)
    .into()
}
fn switch_control<'a>(
    app: &App,
    t: Tokens,
    value: bool,
    action: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(if value { "On" } else { "Off" })
            .size(12.5)
            .color(color(t.secondary))
            .width(26),
        sk_iced::components::ToggleSwitch::new(
            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
            app.motion,
            value
        )
        .on_toggle(action)
    ]
    .spacing(8)
    .align_y(iced::Center)
    .into()
}
fn sound_controls<'a>(app: &'a App, t: Tokens) -> Element<'a, Message> {
    column![
        sk_iced::components::Dropdown::new(
            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
            preferences::SoundChoice::ALL.to_vec(),
            Some(app.preferences.sound_choice()),
            Message::CompletionSound
        )
        .width(198)
        .padding([4, 7])
        .enabled(!app.sound_busy),
        row![
            crate::components::quiet_button(
                container(crate::typography::text_role(
                    crate::typography::TextRole::Caption,
                    "Browse…"
                ))
                .center(Fill)
            )
            .width(Fill)
            .height(28)
            .style(t.button(false, false))
            .on_press_maybe((!app.sound_busy).then_some(Message::ChooseSound)),
            crate::components::quiet_button(
                container(crate::typography::text_role(
                    crate::typography::TextRole::Caption,
                    "Test sound"
                ))
                .center(Fill)
            )
            .width(Fill)
            .height(28)
            .style(t.button(false, false))
            .on_press_maybe(
                (!app.sound_busy
                    && app.preferences.sound_choice() != preferences::SoundChoice::Off)
                    .then_some(Message::TestSound)
            )
        ]
        .spacing(8),
        text(if app.preferences.custom_sound.is_empty() {
            "No custom WAV imported"
        } else {
            &app.preferences.custom_sound
        })
        .size(11)
        .color(color(t.secondary))
    ]
    .spacing(6)
    .width(198)
    .into()
}
pub fn view(app: &App) -> Element<'_, Message> {
    let t = crate::palette::styled(
        app.dark,
        app.preferences.accent,
        app.preferences.tint,
        app.look(),
    );
    let title = text("Settings").size(18).font(iced::Font {
        weight: iced::font::Weight::Bold,
        ..crate::typography::font()
    });
    let appearance = sk_iced::components::Appearance::fixed(crate::typography::font(), t);
    let content = column![
        category(t, "Appearance"),
        setting_card(
            t,
            "Style",
            "Choose the visual style of the app.",
            sk_iced::components::Dropdown::new(
                appearance,
                preferences::Look::ALL.to_vec(),
                Some(app.look()),
                Message::LookChoice
            )
            .width(198)
            .padding([5, 8])
        ),
        setting_card(
            t,
            "Theme",
            "Use a light or dark appearance, or follow Windows.",
            segmented(
                app.motion,
                t,
                &["Auto", "Dark", "Light"],
                match app.preferences.theme {
                    ThemeMode::Auto => 0,
                    ThemeMode::Dark => 1,
                    ThemeMode::Light => 2,
                },
                198.,
                |i| Message::ThemeChoice([ThemeMode::Auto, ThemeMode::Dark, ThemeMode::Light][i])
            )
        ),
        setting_card(
            t,
            "Background tint",
            "Apply the accent color to background surfaces.",
            sk_iced::components::Dropdown::new(
                appearance,
                preferences::TintMode::ALL.to_vec(),
                Some(app.preferences.tint),
                Message::TintChoice
            )
            .width(198)
            .padding([5, 8])
        ),
        setting_card(
            t,
            "Accent color",
            "Choose a color for highlights and selection.",
            accent_choices(t, app.preferences.accent)
        ),
        setting_card(
            t,
            "Item density",
            "Adjust file row spacing and size.",
            container(
                column![
                    text(format!("{}%", app.preferences.density)).size(13),
                    sk_iced::components::Slider::new(
                        75u16..=150u16,
                        app.preferences.density,
                        Message::Density
                    )
                    .with_appearance(appearance)
                    .step(25u16)
                ]
                .spacing(8)
            )
            .width(198)
        ),
        setting_card(
            t,
            "Alternate row colors",
            "Make long file lists easier to read",
            switch_control(
                app,
                t,
                app.preferences.alternating_rows,
                Message::AlternatingRows
            )
        ),
        category(t, "Files"),
        setting_card(
            t,
            "Search subfolders",
            "Include subfolders when searching by name.",
            switch_control(
                app,
                t,
                app.preferences.search_subfolders,
                Message::SearchSubfolders
            )
        ),
        setting_card(
            t,
            "File trees",
            "Use one contextual tree, or a tree beside each panel.",
            sk_iced::components::Dropdown::new(
                appearance,
                vec![
                    preferences::FileTreeMode::Contextual,
                    preferences::FileTreeMode::PerPanel
                ],
                Some(if app.preferences.panel_trees {
                    preferences::FileTreeMode::PerPanel
                } else {
                    preferences::FileTreeMode::Contextual
                }),
                |mode| Message::PanelTrees(mode == preferences::FileTreeMode::PerPanel)
            )
            .width(198)
            .padding([5, 8])
        ),
        setting_card(
            t,
            "Recycle Bin confirmation",
            "Ask before sending files to the Recycle Bin.",
            switch_control(
                app,
                t,
                !app.preferences
                    .skipped_confirmations
                    .iter()
                    .any(|k| k == "recycle"),
                |ask| Message::SkipConfirmation("recycle".into(), !ask)
            )
        ),
        setting_card(
            t,
            "Directory sizes",
            "Choose how folder totals are calculated.",
            sk_iced::components::Dropdown::new(
                appearance,
                preferences::ScanMode::ALL.to_vec(),
                Some(app.preferences.scan_mode),
                Message::ScanModeChoice
            )
            .width(198)
            .padding([5, 8])
        ),
        setting_card(
            t,
            "File size units",
            "Choose how file and folder sizes are displayed.",
            sk_iced::components::Dropdown::new(
                appearance,
                preferences::SizeUnits::ALL.to_vec(),
                Some(app.preferences.size_units),
                Message::SizeUnits
            )
            .width(198)
            .padding([5, 8])
        ),
        setting_card(
            t,
            "Allow opening files",
            "Launch files through double-click and Open commands.",
            switch_control(app, t, app.preferences.open_files, Message::OpenFiles)
        ),
        category(t, "Workspace & startup"),
        setting_card(
            t,
            "Default layout",
            "Choose the startup arrangement, or restore your last workspace.",
            layout_choices(t, app.preferences.default_layout)
        ),
        setting_card(
            t,
            "Administrator on launch",
            "Request Windows administrator privileges when the app starts.",
            switch_control(app, t, app.preferences.admin, Message::LaunchAdmin)
        ),
        category(t, "Transfer feedback"),
        setting_card(
            t,
            "Windows notifications",
            "Show a notification when a transfer batch finishes.",
            switch_control(
                app,
                t,
                app.preferences.completion_notifications,
                Message::CompletionNotifications
            )
        ),
        sk_iced::components::SettingsCard::new(
            appearance,
            "Completion sound",
            sound_controls(app, t)
        )
        .description("Play a sound when all queued transfers finish."),
    ]
    .spacing(8)
    .width(Fill);
    let reset = crate::components::quiet_button(
        container(
            row![
                crate::ui_icons::material(crate::ui_icons::Icon::Undo, false, true),
                crate::typography::text_role(
                    crate::typography::TextRole::Caption,
                    "Restore default settings"
                )
            ]
            .spacing(8)
            .align_y(iced::Center),
        )
        .center_x(Fill)
        .center_y(Fill),
    )
    .height(32)
    .padding([0, 10])
    .width(Fill)
    .style(t.button(false, false))
    .on_press(Message::RestoreDefaults);
    let body = container(
        sk_iced::components::scroll_area(
            container(content)
                .style(move |_| iced::widget::container::Style {
                    background: Some(color(t.background).into()),
                    ..Default::default()
                })
                .padding(iced::Padding {
                    top: 0.0,
                    right: 12.0,
                    bottom: 0.0,
                    left: 12.0,
                }),
        )
        .height(Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            sk_iced::components::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .spacing(4)
                .build(),
        ))
        .style(t.scroll()),
    )
    .padding(iced::Padding {
        top: 6.0,
        right: 6.0,
        bottom: 6.0,
        left: 1.0,
    });
    sk_iced::components::Surface::new(
        sk_iced::components::Appearance::fixed(crate::typography::font(), t),
        container(column![
            container(
                row![
                    title.width(Fill),
                    crate::components::quiet_button(text("×").size(18))
                        .padding([0, 8])
                        .style(t.button(false, false))
                        .on_press(Message::CloseSettings)
                ]
                .align_y(iced::Center)
            )
            .padding(12)
            .width(Fill),
            body,
            container(
                column![
                    divider(t),
                    row![
                        crate::components::quiet_button(
                            container(
                                row![
                                    crate::ui_icons::material(
                                        crate::ui_icons::Icon::Licenses,
                                        false,
                                        true
                                    ),
                                    crate::typography::text_role(
                                        crate::typography::TextRole::Caption,
                                        "Show licenses"
                                    )
                                ]
                                .spacing(6)
                                .align_y(iced::Center)
                            )
                            .center_y(Fill)
                        )
                        .height(32)
                        .padding([0, 12])
                        .style(t.button(false, false))
                        .on_press(Message::LicensesOpen),
                        reset
                    ]
                    .spacing(8)
                ]
                .spacing(10)
            )
            .padding(12)
            .width(Fill)
        ])
        .width(580)
        .max_height(760)
        .clip(true)
        .style(move |_| t.card(false)),
    )
    .role(sk_iced::components::SurfaceRole::Custom(t.visual.card))
    .decorate()
    .into()
}

pub fn button_view(app: &App) -> Element<'_, Message> {
    settings_button(app, false, 28.0)
}
pub fn compact_button(app: &App, height: f32) -> Element<'_, Message> {
    settings_button(app, true, height)
}
fn settings_button(app: &App, compact: bool, height: f32) -> Element<'_, Message> {
    let t = crate::palette::styled(
        app.dark,
        app.preferences.accent,
        app.preferences.tint,
        app.look(),
    );
    let ink_color =
        t.toggle_button(app.settings_open)(&app.app_theme, iced::widget::button::Status::Active)
            .text_color;
    let ink = (ink_color.r * 255.).round() as u32
        | ((ink_color.g * 255.).round() as u32) << 8
        | ((ink_color.b * 255.).round() as u32) << 16;
    let mut content = row![image(icon(ink)).width(16).height(16)]
        .spacing(6)
        .align_y(iced::Center);
    if !compact {
        content = content.push(
            crate::typography::text_role(crate::typography::TextRole::Caption, "Settings")
                .color(color(ink)),
        );
    }
    let base: Element<'_, Message> = crate::button_feedback::wrap(
        crate::components::themed_toggle(
            button(container(content).center_y(Fill).center_x(Fill))
                .width(if compact {
                    iced::Length::Fixed(28.0)
                } else {
                    iced::Length::Shrink
                })
                .padding([0, if compact { 0 } else { 10 }])
                .height(height)
                .style(t.toggle_button(app.settings_open))
                .on_press(Message::SettingsToggle),
            app.motion,
            true,
            app.settings_open,
        ),
        app.motion,
        crate::button_feedback::RefreshIcon::None,
        true,
    );
    crate::settings_popup::settings_popup(base, view(app), app.settings_open, app.motion)
}
fn icon(tint: u32) -> iced::widget::image::Handle {
    static DARK: std::sync::OnceLock<iced::widget::image::Handle> = std::sync::OnceLock::new();
    static LIGHT: std::sync::OnceLock<iced::widget::image::Handle> = std::sync::OnceLock::new();
    let cache = if tint & 255 > 128 { &DARK } else { &LIGHT };
    cache
        .get_or_init(|| {
            let alpha = include_bytes!("../assets/settings-alpha.bin");
            let mut rgba = Vec::with_capacity(1600);
            for &a in alpha {
                rgba.extend_from_slice(&[tint as u8, (tint >> 8) as u8, (tint >> 16) as u8, a]);
            }
            iced::widget::image::Handle::from_rgba(20, 20, rgba)
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{Layout, Shell, layout, widget::Tree};
    #[test]
    fn sound_choices_publish_all_modes_and_controls_render_without_playing() {
        let app = App::new("home".into(), "other".into(), vec![]);
        let t = crate::palette::styled(
            app.dark,
            app.preferences.accent,
            app.preferences.tint,
            app.look(),
        );
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for (index, choice) in preferences::SoundChoice::ALL.into_iter().enumerate() {
            let mut widget = sound_controls(&app, t);
            let mut tree = Tree::new(&widget);
            let size = iced::Size::new(332., 180.);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, size),
            );
            let bounds = node.children()[0].bounds();
            let mut messages = vec![];
            let point = bounds.center();
            widget.as_widget_mut().update(
                &mut tree,
                &iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                Layout::new(&node),
                iced::mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &iced::Rectangle::with_size(size),
            );
            let mut popup = widget
                .as_widget_mut()
                .overlay(
                    &mut tree,
                    Layout::new(&node),
                    &renderer,
                    &iced::Rectangle::with_size(size),
                    iced::Vector::ZERO,
                )
                .expect("combobox menu");
            let popup_node = popup.as_overlay_mut().layout(&renderer, size);
            let mut menu = Layout::new(&popup_node);
            while menu.children().count() == 1 {
                menu = menu.children().next().unwrap();
            }
            let menu_bounds = menu.bounds();
            let point = iced::Point::new(
                menu_bounds.x + 20.,
                menu_bounds.y + (index as f32 + 0.5) * menu_bounds.height / 4.,
            );
            popup.as_overlay_mut().update(
                &iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: point }),
                Layout::new(&popup_node),
                iced::mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
            );
            popup.as_overlay_mut().update(
                &iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                Layout::new(&popup_node),
                iced::mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
            );
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m,Message::CompletionSound(value) if *value==choice)),
                "{messages:?} choice={choice:?}"
            );
        }
        if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
            let pixels = crate::final_ui_checks::render_view(
                container(sound_controls(&app, t))
                    .padding(12)
                    .style(move |_| t.card(false))
                    .into(),
                &app,
                iced::Size::new(376, 180),
                None,
            );
            tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(376, 180).unwrap())
                .unwrap()
                .save_png("target/sound-settings.png")
                .unwrap();
        }
    }
    #[test]
    fn color_grid_fills_the_selector_width() {
        let t = crate::palette::themed(true, Default::default());
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut widget = accent_choices(t, Default::default());
        let mut tree = Tree::new(&widget);
        let node = widget.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(iced::Size::ZERO, iced::Size::new(198., 100.)),
        );
        for row in node.children()[0].children() {
            assert_eq!(row.size().width, 198.0);
            let last = row.children().last().unwrap().bounds();
            assert!((last.x + last.width - 198.0).abs() < 0.01);
        }
    }
    #[test]
    fn all_eighteen_swatch_buttons_publish_their_color() {
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for (index, accent) in crate::palette::Accent::ALL.into_iter().enumerate() {
            let mut widget = accent_choices(crate::palette::themed(true, accent), accent);
            let mut tree = Tree::new(&widget);
            let size = iced::Size::new(198.0, 100.0);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, size),
            );
            let cursor = iced::mouse::Cursor::Available(iced::Point::new(
                (198.0 - 5.0 * 4.0) / 6.0 / 2.0 + (index % 6) as f32 * ((198.0 + 4.0) / 6.0),
                10.0 + (index / 6) as f32 * 24.0,
            ));
            let mut messages = vec![];
            for event in [
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
            ] {
                widget.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Mouse(event),
                    Layout::new(&node),
                    cursor,
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &iced::Rectangle::with_size(size),
                );
            }
            assert!(
                messages.iter().any(
                    |message| matches!(message, Message::AccentChoice(choice) if *choice == accent)
                ),
                "{}: {messages:?}",
                accent.key()
            );
        }
    }
    #[test]
    fn layout_icon_choices_publish_all_four_presets() {
        let t = crate::palette::palette(true, false);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for (index, layout) in DefaultLayout::ALL.into_iter().enumerate() {
            let mut widget = layout_choices(t, DefaultLayout::Vertical);
            let mut tree = Tree::new(&widget);
            let size = iced::Size::new(198.0, 32.0);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            fn check_centers(n: &iced::advanced::layout::Node, y: f32, count: &mut usize) {
                let y = y + n.bounds().y;
                if (n.size().height - 18.).abs() < 0.01
                    && (n.size().width - 24.).abs() < 0.01
                    && n.children().is_empty()
                {
                    assert!((y + 9. - 16.).abs() < 0.01, "icon center={}", y + 9.);
                    *count += 1;
                }
                for c in n.children() {
                    check_centers(c, y, count);
                }
            }
            let mut count = 0;
            check_centers(&node, 0., &mut count);
            assert_eq!(count, 6);
            let cursor = iced::mouse::Cursor::Available(iced::Point::new(
                index as f32 * (202.0 / DefaultLayout::ALL.len() as f32)
                    + (198.0 - 4.0 * (DefaultLayout::ALL.len() - 1) as f32)
                        / DefaultLayout::ALL.len() as f32
                        / 2.0,
                16.0,
            ));
            let mut messages = vec![];
            for event in [
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
            ] {
                widget.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Mouse(event),
                    Layout::new(&node),
                    cursor,
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &iced::Rectangle::with_size(size),
                );
            }
            assert!(messages.iter().any(|message|matches!(message,Message::DefaultLayoutChoice(choice) if *choice==layout)),"{messages:?}");
        }
    }
    #[test]
    fn settings_header_footer_and_scrollbar_have_separate_bounds() {
        let app = App::new("home".into(), "project".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for height in [240.0, 400.0, 760.0] {
            let mut widget = view(&app);
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(376.0, height)),
            );
            let parts = Layout::new(&node)
                .children()
                .next()
                .unwrap()
                .children()
                .collect::<Vec<_>>();
            assert_eq!(parts.len(), 3);
            let header = parts[0].bounds();
            let body = parts[1].bounds();
            let footer = parts[2].bounds();
            let scroll = parts[1].children().next().unwrap().bounds();
            assert!(scroll.y >= body.y + 6.0);
            assert!(scroll.y + scroll.height <= body.y + body.height - 6.0 + 0.01);
            assert!(header.y + header.height <= body.y);
            assert!(body.y + body.height <= footer.y + 0.01);
            assert!(footer.y + footer.height <= height + 0.01);
            let reset = parts[2]
                .children()
                .next()
                .unwrap()
                .children()
                .nth(1)
                .unwrap()
                .bounds()
                .center();
            let mut messages = vec![];
            for event in [
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
            ] {
                widget.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Mouse(event),
                    Layout::new(&node),
                    iced::mouse::Cursor::Available(reset),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &iced::Rectangle::with_size(node.size()),
                );
            }
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::RestoreDefaults))
            );
        }
    }
    #[test]
    fn settings_height_adapts_to_short_and_tall_windows() {
        let app = App::new("home".into(), "project".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for height in [240.0, 400.0, 760.0, 900.0] {
            let mut widget = view(&app);
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(376.0, height)),
            );
            assert!(node.size().height <= height.min(760.0));
            if height >= 760.0 {
                assert!(node.size().height > 500.0);
            }
        }
    }
    #[test]
    fn categorized_settings_stay_within_scrollable_popup_bounds() {
        use iced::advanced::Renderer as _;
        use iced::advanced::renderer::Headless as _;
        for dark in [true, false] {
            let mut app = App::new(
                std::path::PathBuf::from("home"),
                std::path::PathBuf::from("project"),
                vec![],
            );
            app.dark = dark;
            app.app_theme =
                crate::palette::with_tint(dark, app.preferences.accent, app.preferences.tint)
                    .theme();
            let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
            let mut widget = view(&app);
            let mut tree = Tree::new(&widget);
            let size = iced::Size::new(376.0, 760.0);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, size),
            );
            assert!(node.bounds().height <= 760.0);
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                let bounds = iced::Rectangle::with_size(size);
                widget.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Window(iced::window::Event::RedrawRequested(
                        iced::time::Instant::now(),
                    )),
                    Layout::new(&node),
                    iced::mouse::Cursor::Unavailable,
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut vec![]),
                    &bounds,
                );
                renderer.reset(bounds);
                widget.as_widget().draw(
                    &tree,
                    &mut renderer,
                    &app.app_theme,
                    &iced::advanced::renderer::Style::default(),
                    Layout::new(&node),
                    iced::mouse::Cursor::Unavailable,
                    &bounds,
                );
                tiny_skia::Pixmap::from_vec(
                    renderer.screenshot(
                        iced::Size::new(376, 760),
                        1.0,
                        app.app_theme.palette().background,
                    ),
                    tiny_skia::IntSize::from_wh(376, 760).unwrap(),
                )
                .unwrap()
                .save_png(format!(
                    "target/settings-{}.png",
                    if dark { "dark" } else { "light" }
                ))
                .unwrap();
                std::fs::write(
                    format!(
                        "target/settings-layout-{}.rgba",
                        if dark { "dark" } else { "light" }
                    ),
                    renderer.screenshot(
                        iced::Size::new(376, 760),
                        1.0,
                        app.app_theme.palette().background,
                    ),
                )
                .unwrap();
            }
        }
    }
}
