//! Local operation/conflict dialog composition.
use super::*;
use iced::widget::{column, row};
impl App {
    pub fn confirmation_layer<'a>(&'a self, base: Element<'a, Message>) -> Element<'a, Message> {
        if self.pending_skip_confirmation.is_some() {
            let action = "Files will be sent to the Recycle Bin without asking";
            let t = palette::styled(
                self.dark,
                self.preferences.accent,
                self.preferences.tint,
                self.look(),
            );
            let dialog = container(
                column![
                    crate::typography::text_role(
                        crate::typography::TextRole::Title,
                        "Disable this confirmation?"
                    ),
                    text(format!(
                        "{action}. You can restore confirmation in Settings."
                    ))
                    .size(13),
                    row![
                        compact_action(
                            self.motion,
                            "Cancel",
                            Some(Message::ApproveSkipConfirmation(false)),
                            false
                        ),
                        compact_action(
                            self.motion,
                            "Don't ask again",
                            Some(Message::ApproveSkipConfirmation(true)),
                            true
                        )
                    ]
                    .spacing(8)
                ]
                .spacing(16),
            )
            .padding(24)
            .width(500)
            .style(move |_| t.card(false));
            return sk_iced::components::Dialog::custom(
                base,
                sk_iced::components::DialogBackdrop::new(dialog),
            )
            .into();
        }
        if let Some((source, target, _)) = &self.conflict {
            let t = palette::styled(
                self.dark,
                self.preferences.accent,
                self.preferences.tint,
                self.look(),
            );
            let dialog = container(
                column![
                    crate::typography::text_role(crate::typography::TextRole::Title, "File already exists"),
                    text(format!(
                        "Source: {}\nDestination: {}",
                        source.display(),
                        target.display()
                    ))
                    .size(13),
                    crate::typography::text_role(crate::typography::TextRole::Control, "Choose how to handle this filename collision."),
                    crate::typography::text_role(crate::typography::TextRole::Caption, "Save as - Copy adds a suffix before the extension. Existing copies and .old backups are numbered automatically."),
                    column([
                        ("Overwrite", transfers::ConflictChoice::Replace),
                        ("Skip", transfers::ConflictChoice::Skip),
                        ("Save as - Copy", transfers::ConflictChoice::KeepBoth),
                        ("Rename existing to .old", transfers::ConflictChoice::RenameExisting),
                    ].into_iter().map(|(label, choice)| {
                        crate::components::quiet_button(container(text(label).size(13)).center(Fill))
                            .width(Fill).height(32).padding([0, 12])
                            .style(t.button(choice == transfers::ConflictChoice::KeepBoth, false))
                            .on_press(Message::ResolveConflict(choice)).into()
                    })).width(220)
                    .spacing(10)
                ]
                .spacing(18),
            )
            .width(640)
            .padding(24)
            .style(move |_| t.card(false));
            return sk_iced::components::Dialog::custom(
                base,
                sk_iced::components::DialogBackdrop::new(dialog),
            )
            .into();
        }
        let Some(confirmation) = &self.confirmation else {
            return base;
        };
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let deleting = matches!(
            confirmation.action,
            Action::Delete | Action::DeletePermanent
        );
        let permanent = confirmation.action == Action::DeletePermanent;
        let heading = format!(
            "{} {} selected items?",
            if permanent {
                "Permanently delete"
            } else if deleting {
                "Send to Recycle Bin:"
            } else {
                "Cut"
            },
            confirmation.paths.len()
        );
        let detail = if permanent {
            "This permanently removes the listed files and folders. It does not use the Recycle Bin and cannot be undone."
        } else if deleting {
            "Move the listed files and folders to the Recycle Bin. Network or unsupported drives will report an error instead of silently deleting permanently."
        } else {
            "Mark these items to move. Nothing is removed now; originals are removed only after Paste successfully copies them."
        };
        let paths = column(
            confirmation
                .paths
                .iter()
                .map(|p| text(p.display().to_string()).size(12).into()),
        )
        .spacing(4);
        let confirm = crate::components::quiet_button(
            text(if permanent {
                "Delete permanently"
            } else if deleting {
                "Recycle"
            } else {
                "Confirm Cut"
            })
            .size(13),
        )
        .on_press(Message::Confirm(true))
        .width(160)
        .height(34)
        .padding([8, 14])
        .style(t.button(true, deleting));
        let dialog = container(
            column![
                text(heading).size(22),
                text(detail).size(13),
                sk_iced::components::scroll_area(paths).height(160),
                if confirmation.action == Action::Delete {
                    Element::from(
                        crate::typography::checkbox(self.dont_ask_again)
                            .label("Don't ask me again")
                            .text_size(13)
                            .size(15)
                            .style(t.check())
                            .on_toggle(Message::DontAskAgain),
                    )
                } else {
                    Element::from(Space::new().height(0))
                },
                row![
                    Space::new().width(Fill),
                    crate::components::quiet_button(
                        container(crate::typography::text_role(
                            crate::typography::TextRole::Control,
                            "Cancel"
                        ))
                        .center(Fill)
                    )
                    .width(160)
                    .height(34)
                    .style(t.button(false, false))
                    .on_press(Message::Confirm(false)),
                    confirm
                ]
                .spacing(8)
            ]
            .spacing(16),
        )
        .width(620)
        .padding(24)
        .style(move |_| t.card(false));
        let shade = sk_iced::components::DialogBackdrop::new(dialog);
        sk_iced::components::Dialog::custom(base, shade).into()
    }
}
