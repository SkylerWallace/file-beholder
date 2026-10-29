//! Rename and new-folder dialog presentation.
use super::*;
use iced::widget::{column, row};
impl App {
    pub fn rename_layer<'a>(&'a self, base: Element<'a, Message>) -> Element<'a, Message> {
        let Some(editor) = &self.rename else {
            return base;
        };
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let input = text_input("New name", &editor.name)
            .id("rename-name")
            .on_input_maybe((!editor.busy).then_some(Message::RenameName))
            .on_submit_maybe((!editor.busy).then_some(Message::RenameConfirm(true)))
            .padding(8)
            .size(14);
        let dialog = container(
            column![
                text(if editor.new_folder {
                    "New folder"
                } else {
                    "Rename"
                })
                .size(22),
                text(editor.source.display().to_string()).size(12),
                input,
                sk_iced::components::Notice::new(
                    sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                    editor.error.as_deref().unwrap_or("")
                )
                .text_size(12)
                .tone(sk_iced::design::ColorRole::Danger),
                row![
                    Space::new().width(Fill),
                    compact_action(
                        self.motion,
                        "Cancel",
                        (!editor.busy).then_some(Message::RenameConfirm(false)),
                        false
                    ),
                    compact_action(
                        self.motion,
                        if editor.busy {
                            "Working…"
                        } else if editor.new_folder {
                            "Create"
                        } else {
                            "Rename"
                        },
                        (!editor.busy).then_some(Message::RenameConfirm(true)),
                        true
                    )
                ]
                .spacing(8)
            ]
            .spacing(12),
        )
        .padding(20)
        .width(440)
        .style(move |_| t.card(false));
        sk_iced::components::Dialog::custom(
            base,
            sk_iced::components::DialogBackdrop::new(dialog).padding(12),
        )
        .into()
    }
}
