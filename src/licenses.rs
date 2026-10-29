//! Embedded, offline application credits and dependency licenses.
use crate::*;
use iced::widget::{column, scrollable};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct Component {
    pub title: String,
    pub license: String,
    pub source: String,
    pub text: String,
}

pub fn components() -> &'static [Component] {
    static COMPONENTS: OnceLock<Vec<Component>> = OnceLock::new();
    COMPONENTS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Inventory { components: Vec<Component> }
        let embedded = |title: &str, license: &str, source: &str, text: String| Component {
            title: title.into(), license: license.into(), source: source.into(), text,
        };
        let mut entries = vec![
            embedded("File Beholder", "MIT", "", include_str!("../LICENSE").into()),
            embedded("Third-party notices", "Credits and attribution", "", include_str!("../THIRD_PARTY_NOTICES.md").into()),
            embedded("Material Design icons", "Apache-2.0", "https://github.com/google/material-design-icons", include_str!("../assets/material/LICENSE").into()),
            embedded("Fira Sans font", "SIL Open Font License 1.1", "https://github.com/mozilla/Fira", format!("Digitized data copyright 2012–2016, The Mozilla Foundation and Telefonica S.A.\n\n{}", include_str!("../docs/licenses/fonts/FiraSans-LICENSE"))),
            embedded("Iced icon font", "Distributed with Iced (MIT)", "https://github.com/iced-rs/iced/tree/3997291f318a8bc06fa522f5579836fb3feb94df/graphics/fonts", format!("Embedded font copyright: Copyright (C) 2020 by original authors @ fontello.com\n\n{}", include_str!("../docs/licenses/overrides/iced_graphics-0.14.0-LICENSE"))),
        ];
        entries.extend(serde_json::from_str::<Inventory>(include_str!("../docs/licenses/dependencies.json"))
            .expect("checked-in license inventory is valid JSON").components);
        entries
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct Choice {
    pub index: usize,
    title: String,
}
impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.title)
    }
}

#[derive(Default)]
pub struct Viewer {
    pub query: String,
    pub selected: usize,
}
impl Viewer {
    pub fn choices(&self) -> Vec<Choice> {
        let query = self.query.trim().to_lowercase();
        components()
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.title.to_lowercase().contains(&query) || c.license.to_lowercase().contains(&query)
            })
            .map(|(index, component)| Choice {
                index,
                title: component.title.clone(),
            })
            .collect()
    }
    pub fn filter(&mut self, query: String) {
        self.query = query;
        let choices = self.choices();
        if !choices.iter().any(|c| c.index == self.selected) {
            if let Some(first) = choices.first() {
                self.selected = first.index;
            }
        }
    }
}

pub fn reset_scroll() -> Task<Message> {
    iced::widget::operation::scroll_to("licenses-text", scrollable::AbsoluteOffset { x: 0., y: 0. })
}

fn component_picker(
    appearance: sk_iced::components::Appearance,
    choices: Vec<Choice>,
    selected: Option<Choice>,
) -> Element<'static, Message> {
    let enabled = selected.is_some();
    sk_iced::components::Dropdown::new(appearance, choices, selected, |c| {
        Message::LicenseSelect(c.index)
    })
    .width(Fill)
    .padding(8)
    .menu_height(240)
    .enabled(enabled)
    .into()
}

pub fn layer<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(viewer) = &app.licenses else {
        return base;
    };
    let t = palette::styled(
        app.dark,
        app.preferences.accent,
        app.preferences.tint,
        app.look(),
    );
    let appearance = sk_iced::components::Appearance::fixed(typography::font(), t);
    let choices = viewer.choices();
    let selected = choices.iter().find(|c| c.index == viewer.selected).cloned();
    let component = selected.as_ref().map(|choice| &components()[choice.index]);
    let heading = row![
        column![
            text("Licenses and credits").size(22),
            text(concat!("File Beholder ", env!("CARGO_PKG_VERSION"))).size(12)
        ]
        .spacing(4),
        Space::new().width(Fill),
        components::quiet_button(text("×").size(20))
            .padding([4, 10])
            .style(t.button(false, false))
            .on_press(Message::LicensesClose)
    ]
    .align_y(iced::Center);
    let controls = column![
        text_input("Filter components or licenses…", &viewer.query)
            .id("license-filter")
            .on_input(Message::LicenseQuery)
            .padding(8)
            .size(13),
        component_picker(appearance, choices, selected),
    ]
    .spacing(8);
    let content = if let Some(c) = component {
        column![
            text(&c.license).size(13),
            text(&c.source).size(12).color(theme::color(t.secondary)),
            text(&c.text).size(13)
        ]
        .spacing(12)
    } else {
        column![text("No components match your filter.").size(14)]
    };
    let body = sk_iced::components::scroll_area(container(content).padding(12).width(Fill))
        .id("licenses-text")
        .direction(scrollable::Direction::Vertical(
            sk_iced::components::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .spacing(4)
                .build(),
        ))
        .height(Fill)
        .width(Fill)
        .style(t.scroll());
    let footer = row![
        Space::new().width(Fill),
        components::quiet_button(text("Close"))
            .padding([6, 16])
            .style(t.button(false, false))
            .on_press(Message::LicensesClose)
    ];
    let dialog = container(column![heading, controls, body, footer].spacing(14))
        .padding(20)
        .width(Fill)
        .max_width(1000)
        .height(Fill)
        .max_height(720)
        .clip(true)
        .style(move |_| t.card(false));
    sk_iced::components::Dialog::custom(
        base,
        sk_iced::components::DialogBackdrop::new(dialog)
            .width(Fill)
            .height(Fill)
            .padding(20)
            .scrim(iced::Color::from_rgba(0., 0., 0., 0.5)),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_menu_stays_inside_the_window_with_the_full_inventory() {
        use iced::advanced::{Layout, Shell, layout, widget::Tree};
        for look in preferences::Look::ALL {
            let mut app = App::new("left".into(), "right".into(), vec![]);
            app.preferences.look = look;
            let _ = app.apply_appearance();
            app.motion = motion::Policy::DISABLED;
            let t = palette::styled(app.dark, app.preferences.accent, app.preferences.tint, look);
            let appearance = sk_iced::components::Appearance::fixed(typography::font(), t);
            let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
            for (width, height, top) in
                [(480., 480., 140.), (1400., 900., 210.), (480., 480., 410.)]
            {
                let viewer = Viewer::default();
                let choices = viewer.choices();
                let selected = choices.first().cloned();
                let mut widget: Element<'_, Message> = iced::widget::column![
                    Space::new().height(top),
                    component_picker(appearance, choices, selected)
                ]
                .width(Fill)
                .into();
                let mut tree = Tree::new(&widget);
                let size = iced::Size::new(width, height);
                let node = widget.as_widget_mut().layout(
                    &mut tree,
                    &renderer,
                    &layout::Limits::new(size, size),
                );
                let point = node.children()[1].bounds().center();
                widget.as_widget_mut().update(
                    &mut tree,
                    &iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Left,
                    )),
                    Layout::new(&node),
                    iced::mouse::Cursor::Available(point),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut vec![]),
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
                    .expect("component menu opens");
                let popup_node = popup.as_overlay_mut().layout(&renderer, size);
                let mut viewport = Layout::new(&popup_node);
                while viewport.children().count() == 1 {
                    let child = viewport.children().next().unwrap();
                    if child.bounds().height > viewport.bounds().height + 0.5 {
                        break;
                    }
                    viewport = child;
                }
                let menu = viewport.bounds();
                assert!(
                    menu.y >= 0. && menu.y + menu.height <= height + 0.5,
                    "{look:?}: {menu:?}"
                );
                assert!(
                    menu.height <= 242.,
                    "full inventory must scroll inside a bounded menu: {menu:?}"
                );
            }
        }
    }
    #[test]
    fn embedded_inventory_has_text_and_filtering_selects_a_visible_component() {
        assert!(components().len() > 100);
        assert!(
            components()
                .iter()
                .all(|c| !c.text.trim().is_empty() && !c.license.is_empty())
        );
        let mut viewer = Viewer::default();
        viewer.filter("clipboard-win".into());
        assert_eq!(viewer.choices().len(), 1);
        assert!(
            components()[viewer.selected]
                .text
                .contains("Boost Software License")
        );
        viewer.filter("no-such-component".into());
        assert!(viewer.choices().is_empty());
        viewer.filter("Fira Sans".into());
        assert!(
            components()[viewer.selected]
                .text
                .contains("SIL OPEN FONT LICENSE")
        );
    }
    #[test]
    fn viewer_blocks_browsing_and_escape_closes_it() {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        app.settings_open = true;
        let _ = app.update(Message::LicensesOpen);
        assert!(!app.settings_open);
        assert!(app.browsing_keys_blocked());
        let _ = app.update(Message::Navigate(0, "wrong".into()));
        assert_eq!(app.panes[0].path, PathBuf::from("left"));
        let _ = app.update(Message::Confirm(false));
        assert!(app.licenses.is_none());
    }
    #[test]
    fn viewer_renders_in_small_and_large_windows() {
        let mut app = App::new("left".into(), "right".into(), vec![]);
        let _ = app.update(Message::LicensesOpen);
        for (width, height) in [(480, 480), (1400, 900)] {
            let pixels = final_ui_checks::render(&app, iced::Size::new(width, height), None);
            if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                tiny_skia::Pixmap::from_vec(
                    pixels,
                    tiny_skia::IntSize::from_wh(width, height).unwrap(),
                )
                .unwrap()
                .save_png(format!("target/licenses-{width}.png"))
                .unwrap();
            }
        }
        if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
            let pixels = final_ui_checks::render_view(
                settings_ui::view(&app),
                &app,
                iced::Size::new(580, 760),
                None,
            );
            tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(580, 760).unwrap())
                .unwrap()
                .save_png("target/settings-license-button.png")
                .unwrap();
        }
    }
}
