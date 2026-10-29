// SPDX-License-Identifier: MIT
//! Application typography using Iced's bundled Fira Sans.
fn theme_assets() -> &'static std::sync::RwLock<sk_iced::visual::ThemeAssets> {
    static ASSETS: std::sync::OnceLock<std::sync::RwLock<sk_iced::visual::ThemeAssets>> =
        std::sync::OnceLock::new();
    ASSETS.get_or_init(Default::default)
}
/// Active theme fonts for the application window.
pub fn set_theme(tokens: crate::theme::Tokens) {
    *theme_assets().write().unwrap_or_else(|e| e.into_inner()) = tokens.assets;
}
pub fn font() -> iced::Font {
    theme_assets()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .body
        .unwrap_or(iced::Font::with_name("Fira Sans"))
}
pub use sk_iced::typography::{TextRole, Typography};
pub fn profile() -> Typography {
    let assets = *theme_assets().read().unwrap_or_else(|e| e.into_inner());
    Typography::new(
        assets.body.unwrap_or(iced::Font::with_name("Fira Sans")),
        assets.monospace.unwrap_or(iced::Font::MONOSPACE),
    )
}
pub fn text_role<'a>(
    role: TextRole,
    content: impl iced::widget::text::IntoFragment<'a>,
) -> iced::widget::Text<'a> {
    profile().text(role, content)
}
pub fn context(
    appearance: sk_iced::components::Appearance,
    motion: sk_iced::motion::Policy,
) -> sk_iced::components::UiContext {
    sk_iced::components::UiContext::new(appearance, motion).with_typography(profile())
}
pub fn text<'a>(content: impl iced::widget::text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    profile().raw(content)
}
pub fn text_input<'a, M: Clone + 'a>(
    placeholder: &str,
    value: &str,
) -> sk_iced::components::TextInput<'a, M> {
    sk_iced::components::TextInput::new(
        sk_iced::components::Appearance::themed(font(), crate::palette::from_theme),
        placeholder,
        value,
    )
}

/// Native checkbox sizing with theme-owned colors.
pub fn checkbox<'a, M: 'a>(checked: bool) -> sk_iced::components::Checkbox<'a, M> {
    sk_iced::components::Checkbox::custom(iced::widget::checkbox(checked)).with_appearance(
        sk_iced::components::Appearance::themed(font(), crate::palette::from_theme),
    )
}
