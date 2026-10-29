//! Shared semantic styles; File Beholder keeps its font/layout preferences.
pub use sk_iced::theme::*;
#[cfg(test)]
mod tests {
    use iced::widget::button;
    #[test]
    fn disabled_actions_share_the_neutral_cancel_style() {
        for dark in [false, true] {
            let t = crate::palette::palette(dark, false);
            let theme = t.theme();
            let cancel = t.button(false, true)(&theme, button::Status::Disabled);
            for primary in [false, true] {
                let style = t.button(primary, false)(&theme, button::Status::Disabled);
                assert_eq!(style.background, cancel.background);
                assert_eq!(style.text_color, cancel.text_color);
                assert_eq!(style.border, cancel.border);
            }
        }
    }
}
