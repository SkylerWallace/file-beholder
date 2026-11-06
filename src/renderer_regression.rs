// SPDX-License-Identifier: MIT
//! Run the common renderer contract with File Beholder's actual UI dependency.
use crate::{
    Message, palette,
    preferences::{Look, TintMode},
};
use iced::widget::{column, container};
use iced::{Element, Font};
use sk_iced::components::{Appearance, Dropdown};
#[test]
fn open_themed_dropdowns_remain_pixel_identical_after_partial_repaints() {
    for look in [Look::Neumorphic, Look::Material] {
        for dark in [false, true] {
            for scale in [1.0, 1.5] {
                let t = palette::styled(dark, Default::default(), TintMode::default(), look);
                let font = Font::with_name("Fira Sans");
                let a = Appearance::fixed(font, t);
                let view: Element<'_, Message> = container(
                    column![
                        Dropdown::new(a, Look::ALL.to_vec(), Some(look), Message::LookChoice)
                            .width(198)
                            .padding([5, 8]),
                        Dropdown::new(
                            a,
                            TintMode::ALL.to_vec(),
                            Some(TintMode::DarkOnly),
                            Message::TintChoice
                        )
                        .width(198)
                        .padding([5, 8]),
                    ]
                    .spacing(20),
                )
                .padding([24, 40])
                .width(420)
                .height(260)
                .into();
                sk_iced_renderer_regression::assert_open_dropdown_repaints(
                    view,
                    &t.theme(),
                    crate::theme::color(t.background),
                    font,
                    iced::advanced::graphics::text::FIRA_SANS_REGULAR,
                    &format!("file-beholder-{look:?}-{dark}"),
                    scale,
                );
            }
        }
    }
}
