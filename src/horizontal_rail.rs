//! Inset rail backed by the shared widget; app-specific table layout stays local.
use iced::Element;
pub fn view<'a, M: 'a>(content: Element<'a, M>, content_width: f32) -> Element<'a, M> {
    sk_iced::components::HorizontalScrollRail::new(
        sk_iced::components::Appearance::themed(
            crate::typography::font(),
            crate::palette::from_theme,
        ),
        content,
        content_width,
    )
    .into()
}
