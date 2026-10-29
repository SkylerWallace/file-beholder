//! File share presentation through the shared stateless meter.
pub fn view<'a, M: 'a>(percent: f32, height: f32) -> iced::Element<'a, M> {
    sk_iced::components::ShareMeter::new(
        sk_iced::components::Appearance::themed(
            crate::typography::font(),
            crate::palette::from_theme,
        ),
        percent,
    )
    .height(height)
    .into()
}
