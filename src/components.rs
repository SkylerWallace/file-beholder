//! File Beholder controls built on the shared sk-iced UI components.
use crate::{
    palette,
    theme::{Tokens, blend, color},
    typography::text,
};
use iced::widget::{button, container, row};
use iced::{Border, Element, Fill, Theme};

fn appearance() -> sk_iced::components::Appearance {
    sk_iced::components::Appearance::themed(crate::typography::font(), tokens)
}
fn tokens(theme: &Theme) -> Tokens {
    palette::from_theme(theme)
}
pub const RIBBON_HEIGHT: f32 = sk_iced::design::size::SMALL_BUTTON;
#[cfg(test)]
pub const FILE_ACTION_WIDTH: f32 = 28.0;
pub const NAVIGATION_WIDTH: f32 = sk_iced::design::size::SMALL_BUTTON;
pub fn compact_action<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: impl Into<std::borrow::Cow<'a, str>>,
    message: Option<M>,
    selected: bool,
) -> Element<'a, M> {
    action(
        policy,
        label,
        message,
        selected,
        sk_iced::design::ControlProfile::COMPACT.height,
        iced::Length::Shrink,
    )
}
pub fn navigation_action<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: &'static str,
    message: Option<M>,
) -> Element<'a, M> {
    let hint = match label {
        "←" => "Back",
        "→" => "Forward",
        "↑" => "Up",
        _ => "Refresh",
    };
    sk_iced::components::tooltip(
        action(
            policy,
            label,
            message,
            false,
            RIBBON_HEIGHT,
            iced::Length::Fixed(NAVIGATION_WIDTH),
        ),
        container(text(hint).size(12))
            .padding(7)
            .style(|theme| tokens(theme).card(false)),
        iced::widget::tooltip::Position::Bottom,
    )
    .delay(std::time::Duration::from_secs(1))
    .into()
}
fn action<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: impl Into<std::borrow::Cow<'a, str>>,
    message: Option<M>,
    selected: bool,
    height: f32,
    width: iced::Length,
) -> Element<'a, M> {
    let label = label.into();
    let refresh = match label.as_ref() {
        "↻" => crate::button_feedback::RefreshIcon::Center,
        "Refresh" => crate::button_feedback::RefreshIcon::Leading,
        _ => crate::button_feedback::RefreshIcon::None,
    };
    let content: Element<'a, M> = match refresh {
        crate::button_feedback::RefreshIcon::Center => {
            iced::widget::Space::new().width(12).height(16).into()
        }
        crate::button_feedback::RefreshIcon::Leading => {
            row![iced::widget::Space::new().width(14), text(label).size(12)]
                .spacing(5)
                .into()
        }
        _ => text(label)
            .size(12)
            .wrapping(iced::widget::text::Wrapping::None)
            .into(),
    };
    crate::typography::context(appearance(), policy)
        .button("")
        .content(content)
        .layout(sk_iced::components::ButtonLayout {
            height,
            width,
            padding: [0, 6].into(),
            center_x: Some(if matches!(width, iced::Length::Fixed(_)) {
                Fill
            } else {
                iced::Length::Shrink
            }),
        })
        .tone(if selected {
            sk_iced::components::ButtonTone::Important
        } else {
            sk_iced::components::ButtonTone::Default
        })
        .with_refresh(refresh)
        .on_press_maybe(message)
        .into()
}

pub fn tab_width(panel_width: f32, count: usize) -> f32 {
    sk_iced::components::TabSizing::new()
        .reserved(134.0)
        .width(panel_width, count)
}
/// Small owned paint data only: never retain a closed Tab or its scan handle.
#[derive(Clone)]
pub struct TabVisual {
    pub label: String,
    pub selected: bool,
}
pub fn tab_visual<'a>(visual: &TabVisual, width: f32) -> Element<'a, crate::Message> {
    browser_tab_sized(
        crate::motion::Policy::DISABLED,
        visual.label.clone(),
        crate::Message::Noop,
        Some(crate::Message::Noop),
        visual.selected,
        width,
    )
}
#[cfg(test)]
pub fn browser_tab<'a, M: Clone + 'a>(
    label: String,
    select: M,
    close: Option<M>,
    selected: bool,
) -> Element<'a, M> {
    browser_tab_sized(
        crate::motion::Policy::DISABLED,
        label,
        select,
        close,
        selected,
        188.0,
    )
}
pub fn browser_tab_sized<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: String,
    select: M,
    close: Option<M>,
    selected: bool,
    width: f32,
) -> Element<'a, M> {
    let title: Element<'a, M> = sk_iced::components::Button::custom(
        appearance(),
        policy,
        button(
            container(
                text(label)
                    .size(13)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width((width - 48.0).max(0.0))
            .clip(true),
        )
        .on_press(select)
        .height(30)
        .padding([5, 10])
        .style(move |theme, _| {
            let t = tokens(theme);
            button::Style {
                text_color: color(if selected { t.text } else { t.secondary }),
                ..Default::default()
            }
        }),
    )
    .with_refresh(crate::button_feedback::RefreshIcon::None)
    .enabled(true)
    .into();
    let close_enabled = close.is_some();
    let close: Element<'a, M> = sk_iced::components::Button::custom(
        appearance(),
        policy,
        button(text("×").size(16))
            .width(28)
            .height(30)
            .on_press_maybe(close)
            .style(move |theme, status| panel_control_style(tokens(theme), status)),
    )
    .with_refresh(crate::button_feedback::RefreshIcon::None)
    .enabled(close_enabled)
    .into();
    let close = sk_iced::components::tooltip(
        close,
        container(shortcut_hint("Close tab"))
            .padding(7)
            .style(|theme| tokens(theme).card(false)),
        iced::widget::tooltip::Position::Bottom,
    )
    .delay(std::time::Duration::from_secs(1));
    crate::column_resize::tab_background(
        sk_iced::components::Tab::parts(title, close).into(),
        selected,
    )
}
pub fn file_row<'a, M: Clone + 'a>(
    name: &'a str,
    size: String,
    directory: bool,
    hidden: bool,
    share: f32,
    selected: bool,
    message: M,
    modified: Option<&'a str>,
    icon: Option<&'a iced::widget::image::Handle>,
    columns: [f32; 4],
    extension_width: f32,
    extension: &str,
    properties: [String; 6],
    visible: &[crate::columns::Column],
    row_height: f32,
    alternate: bool,
) -> Element<'a, M> {
    let compact = row_height < crate::virtual_list::ROW_HEIGHT;
    let scale = if compact {
        0.95
    } else {
        row_height / crate::virtual_list::ROW_HEIGHT
    };
    let spacing_scale = if compact { 0.6 } else { scale };
    let icon_size = if compact { 16.0 } else { 18. * scale };
    let font_size = 12. * scale;
    let line_height = iced::widget::text::LineHeight::Absolute(font_size.into());
    let icon: Element<'a, M> = if let Some(icon) = icon {
        sk_iced::icons::Icon::raster(appearance(), icon.clone())
            .width(icon_size)
            .height(icon_size)
            .opacity(if hidden { 0.45_f32 } else { 1.0_f32 })
            .into()
    } else {
        text(if directory { "▣" } else { "▤" })
            .size(16. * scale)
            .width(icon_size)
            .into()
    };
    use crate::columns::Column;
    let mut content = sk_iced::components::TableRow::new().spacing(4. * spacing_scale);
    let mut icon = Some(icon);
    for column in visible {
        let cell: Element<'a, M> =
            match column {
                Column::Name => {
                    row![
                        icon.take().expect("one Name column"),
                        container(
                            text(name)
                                .size(font_size)
                                .line_height(line_height)
                                .style(move |theme: &Theme| iced::widget::text::Style {
                                    color: Some(
                                        color(if hidden {
                                            tokens(theme).secondary
                                        } else {
                                            tokens(theme).text
                                        })
                                        .scale_alpha(if hidden { 0.6 } else { 1.0 })
                                    )
                                })
                                .width(Fill)
                                .wrapping(iced::widget::text::Wrapping::None)
                        )
                        .width(Fill)
                        .clip(true)
                    ]
                    .spacing(5. * spacing_scale)
                    .width(Fill)
                    .align_y(iced::Center)
                    .into()
                }
                Column::Share => row![
                    container(crate::share_bar::view(share, 8. * scale))
                        .width(((columns[3] - 47. * scale).max(0.) * 0.75).max(1.)),
                    text(format!("{share:.1}%"))
                        .size(font_size)
                        .line_height(line_height)
                        .width(43. * scale)
                        .align_x(iced::Right)
                ]
                .spacing(4. * scale)
                .align_y(iced::Center)
                .width(columns[3])
                .into(),
                _ => {
                    let value = match column {
                        Column::Type => {
                            if directory {
                                "DIR".into()
                            } else {
                                "FILE".into()
                            }
                        }
                        Column::Extension => extension.to_string(),
                        Column::Modified => modified.unwrap_or("—").to_string(),
                        Column::Size => size.clone(),
                        extra if extra.property_index().is_some() => {
                            properties[extra.property_index().unwrap()].clone()
                        }
                        _ => unreachable!(),
                    };
                    container(
                        text(value)
                            .size(font_size)
                            .line_height(line_height)
                            .wrapping(iced::widget::text::Wrapping::None),
                    )
                    .width(column.width(columns, extension_width))
                    .clip(true)
                    .into()
                }
            };
        content = content.push(cell);
    }
    sk_iced::components::Button::custom(
        appearance(),
        crate::motion::Policy::DISABLED,
        button(content)
            .height(row_height)
            .padding([if compact { 0.5 } else { scale }, 6. * spacing_scale])
            .width(Fill)
            .on_press(message)
            .style(move |theme, status| {
                let t = tokens(theme);
                let amount = if selected {
                    t.opacity_selected
                } else {
                    match status {
                        button::Status::Hovered => t.opacity_hover,
                        button::Status::Pressed => t.opacity_pressed,
                        _ => {
                            if alternate {
                                0.035
                            } else {
                                0.0
                            }
                        }
                    }
                };
                button::Style {
                    background: Some(
                        blend(t.surface, if selected { t.accent } else { t.text }, amount).into(),
                    ),
                    text_color: color(t.secondary).scale_alpha(if hidden { 0.6 } else { 1.0 }),
                    border: Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
    )
    .feedback(false)
    .into()
}

pub fn tab_add<'a, M: Clone + 'a>(policy: crate::motion::Policy, message: M) -> Element<'a, M> {
    sk_iced::components::Button::custom(
        appearance(),
        policy,
        button(container(text("+").size(21)).center(28))
            .width(28)
            .height(28)
            .padding(0)
            .on_press(message)
            .style(|theme, status| {
                let t = tokens(theme);
                button::Style {
                    background: (status == button::Status::Hovered)
                        .then(|| blend(t.navigation_background, t.text, t.opacity_hover).into()),
                    text_color: color(t.text),
                    border: Border {
                        radius: 14.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
    )
    .with_refresh(crate::button_feedback::RefreshIcon::None)
    .enabled(true)
    .into()
}

pub fn icon_action<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: &'static str,
    icon: crate::ui_icons::Icon,
    message: Option<M>,
    danger: bool,
) -> Element<'a, M> {
    icon_action_state(policy, label, icon, message, danger, false)
}
pub fn icon_toggle_action<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: &'static str,
    icon: crate::ui_icons::Icon,
    message: Option<M>,
    selected: bool,
) -> Element<'a, M> {
    icon_action_state(policy, label, icon, message, false, selected)
}

fn icon_action_state<'a, M: Clone + 'a>(
    policy: crate::motion::Policy,
    label: &'static str,
    icon: crate::ui_icons::Icon,
    message: Option<M>,
    danger: bool,
    selected: bool,
) -> Element<'a, M> {
    let enabled = message.is_some();
    let graphic: Element<'a, M> = if label.ends_with("Undoing…") {
        text(label.chars().next().unwrap().to_string())
            .size(18)
            .into()
    } else {
        crate::ui_icons::material(icon, danger, enabled)
    };
    sk_iced::components::tooltip(
        {
            let control = sk_iced::components::Button::new(appearance(), policy)
                .icon(graphic)
                .height(RIBBON_HEIGHT)
                .width(28)
                .padding(0)
                .center_x(Fill)
                .text_size(12)
                .tone(if danger {
                    sk_iced::components::ButtonTone::Danger
                } else {
                    sk_iced::components::ButtonTone::Default
                });
            control.selected(selected).on_press_maybe(message)
        },
        container(shortcut_hint(label))
            .padding(7)
            .style(|theme| tokens(theme).card(false)),
        iced::widget::tooltip::Position::Bottom,
    )
    .delay(std::time::Duration::from_secs(1))
    .into()
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    use iced::advanced::{layout, widget::Tree};
    #[test]
    fn tabs_shrink_without_title_measurement_and_keep_the_close_button() {
        assert_eq!(tab_width(1000.0, 2), 188.0);
        assert!(tab_width(420.0, 3) < tab_width(700.0, 3));
        assert_eq!(tab_width(320.0, 20), 72.0);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for width in [72.0, 120.0, 188.0] {
            let mut widget = browser_tab_sized(
                crate::motion::Policy::DISABLED,
                "A very long folder name".into(),
                (),
                Some(()),
                true,
                width,
            );
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(1000.0, 30.0)),
            );
            assert_eq!(node.bounds().width, width);
            assert_eq!(node.children()[1].bounds().width, 28.0);
        }
    }
    #[test]
    fn stem_arrows_up_and_refresh_have_identical_geometry() {
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for label in ["←", "→", "↑", "↻"] {
            let mut widget = navigation_action(crate::motion::Policy::DISABLED, label, Some(()));
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(200.0, 100.0)),
            );
            assert_eq!(
                node.bounds().size(),
                iced::Size::new(NAVIGATION_WIDTH, RIBBON_HEIGHT)
            );
        }
    }
}

#[cfg(test)]
mod density_rendering_tests {
    use super::*;
    use iced::advanced::{layout, widget::Tree};
    #[test]
    fn density_scales_actual_icon_and_font_layout_with_the_row() {
        let renderer = iced::Renderer::new(crate::typography::font(), 12.0.into());
        let image = iced::widget::image::Handle::from_rgba(40, 40, vec![255; 40 * 40 * 4]);
        for percent in [75, 100, 125, 150] {
            let height = crate::preferences::row_height(percent);
            let scale = crate::preferences::content_scale(percent);
            let mut view = file_row(
                "Example.txt",
                "2.0 KiB".into(),
                false,
                false,
                2.,
                false,
                (),
                Some("2026-10-23"),
                Some(&image),
                [44., 132., 82., 144.].map(|v| v * scale),
                84. * scale,
                "txt",
                Default::default(),
                &[crate::columns::Column::Name],
                height,
                true,
            );
            let mut tree = Tree::new(&view);
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(1000., 100.)),
            );
            assert!((node.size().height - height).abs() < 0.01);
            let name = &node.children()[0].children()[0];
            assert!(
                (name.children()[0].size().height - if percent == 75 { 16.0 } else { 18. * scale })
                    .abs()
                    < 0.01
            );
            let text = &name.children()[1].children()[0];
            assert!((text.size().height - 12. * scale).abs() < 0.01);
        }
    }
}

pub fn shortcut_hint<'a, M: 'a>(label: &'static str) -> Element<'a, M> {
    sk_iced::components::shortcut_hint(appearance(), label, crate::ui_icons::shortcut(label))
}

pub use sk_iced::components::panel_control_style;

/// A small paint-only closing panel card, without retained directory or tab state.
pub fn panel_visual(visual: &TabVisual, width: f32) -> Element<'static, crate::Message> {
    sk_iced::components::Panel::custom(sk_iced::components::card(
        appearance(),
        text(visual.label.clone()).size(12),
        sk_iced::components::CardLayout {
            padding: 12.into(),
            width: iced::Length::Fixed(width),
            height: Some(Fill),
            clip: false,
            selected: false,
        },
    ))
    .into()
}

/// Wrap a native control without adding ripple feedback.
pub fn quiet_button<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
) -> sk_iced::components::Button<'a, M> {
    sk_iced::components::Button::custom(
        appearance(),
        crate::motion::Policy::DISABLED,
        button(content),
    )
    .feedback(false)
}

/// Retain native sizing/style while opting standard controls into all theme shadow layers.
pub fn themed_native<'a, M: Clone + 'a>(
    inner: iced::widget::Button<'a, M>,
    policy: crate::motion::Policy,
    enabled: bool,
) -> Element<'a, M> {
    themed_toggle(inner, policy, enabled, false)
}

/// Preserve native layout while sharing persistent toggle material.
pub fn themed_toggle<'a, M: Clone + 'a>(
    inner: iced::widget::Button<'a, M>,
    policy: crate::motion::Policy,
    enabled: bool,
    selected: bool,
) -> Element<'a, M> {
    sk_iced::components::Button::custom(appearance(), policy, inner)
        .surface_role(sk_iced::components::SurfaceRole::Toggle(selected))
        .enabled(enabled)
        .feedback(false)
        .into()
}
