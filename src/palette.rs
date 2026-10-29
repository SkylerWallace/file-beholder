//! File Beholder preference adapter for the shared palette.
#[cfg(test)]
pub use sk_iced::palette::palette;
pub use sk_iced::palette::{Accent, from_theme};
pub fn styled(
    dark: bool,
    accent: Accent,
    tint: crate::preferences::TintMode,
    look: crate::preferences::Look,
) -> crate::theme::Tokens {
    let tokens = with_tint(dark, accent, tint);
    let mut tokens = match look {
        crate::preferences::Look::Default => tokens,
        crate::preferences::Look::Material => sk_iced::themes::material(tokens).tokens,
        crate::preferences::Look::Metro => sk_iced::themes::metro(tokens).tokens,
        crate::preferences::Look::Neumorphic => {
            sk_iced::themes::neumorphic(tokens)
                .fonts(iced::Font::with_name("Fira Sans"), iced::Font::MONOSPACE)
                .icons(crate::ui_icons::material_pack)
                .tokens
        }
    };
    if look == crate::preferences::Look::Neumorphic {
        // Keep the raised/recessed lighting, but give every track state pill geometry.
        for states in [&mut tokens.visual.switch_off, &mut tokens.visual.switch_on]
            .into_iter()
            .flatten()
        {
            states.normal.shape = Some(sk_iced::visual::Shape::Pill);
            for surface in [
                &mut states.hovered,
                &mut states.pressed,
                &mut states.disabled,
                &mut states.focused,
            ]
            .into_iter()
            .flatten()
            {
                surface.shape = Some(sk_iced::visual::Shape::Pill);
            }
        }
    }
    if tint.enabled(dark) && look != crate::preferences::Look::Default {
        tokens = sk_iced::visual::ThemeDefinition::new(tokens)
            .tint_surfaces(accent.rgb(), 0.045)
            .tokens;
    }
    tokens
}
pub fn with_tint(
    dark: bool,
    accent: Accent,
    tint: crate::preferences::TintMode,
) -> crate::theme::Tokens {
    sk_iced::palette::with_tint(dark, accent, tint.enabled(dark))
}
#[cfg(test)]
pub fn themed(dark: bool, accent: Accent) -> crate::theme::Tokens {
    with_tint(dark, accent, crate::preferences::TintMode::default())
}
#[cfg(test)]
mod accent_tests {
    use super::*;
    #[test]
    fn metro_tiles_keep_text_and_standard_icon_ink_readable_for_every_accent() {
        let luminance = |c: iced::Color| {
            let linear = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
        };
        for dark in [false, true] {
            for accent in Accent::ALL {
                for tint in [
                    crate::preferences::TintMode::On,
                    crate::preferences::TintMode::Off,
                ] {
                    let t = styled(dark, accent, tint, crate::preferences::Look::Metro);
                    let s = (t.button(false, false))(
                        &iced::Theme::Dark,
                        iced::widget::button::Status::Active,
                    );
                    let Some(iced::Background::Color(bg)) = s.background else {
                        panic!("solid tile required")
                    };
                    let a = luminance(bg);
                    let b = luminance(s.text_color);
                    assert!(
                        (a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5,
                        "dark={dark} accent={accent:?}"
                    );
                    assert_eq!(s.text_color, crate::theme::color(t.text));
                }
            }
        }
    }
    #[test]
    fn surface_tint_applies_to_all_active_styles_without_changing_ink_or_semantics() {
        use crate::preferences::{Look, TintMode};
        for look in [Look::Default, Look::Neumorphic, Look::Material, Look::Metro] {
            for dark in [false, true] {
                let off = styled(dark, Accent::ALL[0], TintMode::Off, look);
                let on = styled(dark, Accent::ALL[0], TintMode::On, look);
                assert_ne!(off.background, on.background, "{look:?} dark={dark}");
                assert_eq!(off.text, on.text);
                assert_eq!(off.accent, on.accent);
                assert_eq!(off.failure, on.failure);
                assert_eq!(
                    on,
                    styled(
                        dark,
                        Accent::ALL[0],
                        if dark {
                            TintMode::DarkOnly
                        } else {
                            TintMode::On
                        },
                        look
                    )
                );
            }
        }
    }
    #[test]
    fn tint_modes_roundtrip_and_shared_controls_match_all_surface_roles() {
        use crate::preferences::{Preferences, TintMode};
        assert_eq!(Preferences::decode("").tint, TintMode::DarkOnly);
        assert_eq!(Preferences::decode("tint=invalid").tint, TintMode::DarkOnly);
        for mode in TintMode::ALL {
            let mut p = Preferences::default();
            p.tint = mode;
            assert_eq!(Preferences::decode(&p.encode()).tint, mode);
            for dark in [false, true] {
                for accent in Accent::ALL {
                    let t = with_tint(dark, accent, mode);
                    let recovered = from_theme(&t.theme());
                    for (actual, shared) in [
                        (t.background, recovered.background),
                        (t.surface, recovered.surface),
                        (t.navigation_background, recovered.navigation_background),
                        (t.border, recovered.border),
                    ] {
                        assert_eq!(actual, shared);
                        if !mode.enabled(dark) {
                            assert_eq!(actual & 255, (actual >> 8) & 255);
                            assert_eq!(actual & 255, (actual >> 16) & 255);
                        }
                    }
                    assert_eq!(t.accent, themed(dark, accent).accent);
                    assert_eq!(
                        mode.enabled(dark),
                        mode == TintMode::On || (mode == TintMode::DarkOnly && dark)
                    );
                }
            }
        }
    }
    #[test]
    fn all_accents_roundtrip_and_keep_contrasting_labels_in_both_modes() {
        for accent in Accent::ALL {
            let mut preferences = crate::preferences::Preferences::default();
            preferences.accent = accent;
            assert_eq!(
                crate::preferences::Preferences::decode(&preferences.encode()),
                preferences
            );
            for dark in [false, true] {
                let t = themed(dark, accent);
                assert_eq!(from_theme(&t.theme()).accent, t.accent);
                let linear = |v: f32| {
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                };
                let c = crate::theme::color(t.accent);
                let l = 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b);
                let ratio = if t.accent_text == 0 {
                    (l + 0.05) / 0.05
                } else {
                    1.05 / (l + 0.05)
                };
                assert!(ratio >= 4.5, "{} dark={dark}: {ratio}", accent.key());
            }
        }
        assert_eq!(Accent::from_key("unknown"), Accent::default());
    }
}
