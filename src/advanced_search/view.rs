//! Advanced-search dialog presentation.
use super::*;
use iced::widget::{column, row};
pub fn layer<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(editor) = &app.advanced else {
        return base;
    };
    let t = palette::styled(
        app.dark,
        app.preferences.accent,
        app.preferences.tint,
        app.look(),
    );
    let d = &editor.draft;
    let field = |label: &'static str,
                 value: &'a str,
                 change: fn(String) -> Change|
     -> Element<'a, Message> {
        sk_iced::components::FormField::new(
            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
            label,
            text_input(
                match label {
                    "Name / pattern" => "e.g. report*.pdf; budget*",
                    "Extensions" => "e.g. pdf, docx, txt",
                    "Minimum size" => "e.g. 500 KB",
                    "Maximum size" => "e.g. 2 GB",
                    "Modified from" | "Modified through" => "YYYY-MM-DD",
                    _ => "",
                },
                value,
            )
            .id(if label == "Name / pattern" {
                "advanced-name"
            } else {
                label
            })
            .on_input(move |s| Message::AdvancedChange(change(s)))
            .on_submit(Message::AdvancedApply)
            .size(13)
            .padding(6),
        )
        .text_size(13)
        .build()
    };
    let mut fields = column![
        crate::typography::text_role(crate::typography::TextRole::Subtitle, "Advanced search"),
        crate::typography::text_role(
            crate::typography::TextRole::Caption,
            "Combine any filters below. Blank fields match everything."
        ),
        text(format!(
            "In: {}",
            app.panes
                .iter()
                .flat_map(|p| p.tabs.iter())
                .find(|t| t.identity == editor.tab)
                .map(|t| t.path.display().to_string())
                .unwrap_or_default()
        ))
        .size(12),
        checkbox(d.recursive)
            .label("Include subfolders")
            .on_toggle(|v| Message::AdvancedChange(Change::Recursive(v))),
        field("Name / pattern", &d.name, Change::Name),
        text("Use * for any text, ? for one character, and ; for alternatives.").size(11),
        field("Extensions", &d.extensions, Change::Extensions),
        row![
            crate::typography::text_role(crate::typography::TextRole::Control, "Items").width(115),
            settings_ui::segmented(
                app.motion,
                t,
                &["All", "Files", "Folders"],
                d.kind as usize,
                280.,
                |i| Message::AdvancedChange(Change::Kind(i as u8))
            )
        ]
        .spacing(8)
        .align_y(iced::Center),
        field("Minimum size", &d.minimum, Change::Minimum),
        field("Maximum size", &d.maximum, Change::Maximum),
        crate::typography::text_role(
            crate::typography::TextRole::Small,
            "Examples: 500 KB, 2 MiB, 1 GB. Unknown folder sizes do not match size limits."
        ),
        field("Modified from", &d.after, Change::After),
        field("Modified through", &d.before, Change::Before),
        crate::typography::text_role(
            crate::typography::TextRole::Small,
            "Dates: YYYY-MM-DD, inclusive, using the displayed local date."
        ),
        checkbox(d.case_sensitive)
            .label("Match case")
            .on_toggle(|v| Message::AdvancedChange(Change::Case(v))),
    ]
    .spacing(10);
    if let Some(error) = &editor.error {
        fields = fields.push(
            sk_iced::components::Notice::new(
                sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                error.as_str(),
            )
            .text_size(12)
            .tone(sk_iced::design::ColorRole::Danger),
        );
    }
    fields = fields.push(
        row![
            crate::components::quiet_button(text("Reset filters"))
                .on_press(Message::AdvancedClear)
                .style(t.button(false, false)),
            Space::new().width(Fill),
            crate::components::quiet_button(text("Cancel"))
                .on_press(Message::AdvancedClose)
                .style(t.button(false, false)),
            crate::components::quiet_button(text("Search"))
                .on_press(Message::AdvancedApply)
                .style(t.button(true, false))
        ]
        .spacing(8),
    );
    let dialog = container(sk_iced::components::scroll_area(fields).height(iced::Length::Shrink))
        .padding(20)
        .width(560)
        .max_height(640)
        .style(move |_| t.card(false));
    sk_iced::components::Dialog::custom(
        base,
        sk_iced::components::DialogBackdrop::new(dialog)
            .width(Fill)
            .height(Fill)
            .padding(20)
            .scrim(iced::Color::from_rgba(0., 0., 0., 0.45)),
    )
    .into()
}
