use crate::*;
use iced::widget::{column, tooltip};

pub struct FailedView {
    pub job: u64,
    pub filter: String,
    pub identity: u64,
}
pub(crate) fn cell<'a>(
    value: impl Into<std::borrow::Cow<'a, str>>,
    width: iced::Length,
) -> Element<'a, Message> {
    container(
        text(value.into())
            .size(12)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(width)
    .clip(true)
    .into()
}
impl App {
    pub fn utility_layer<'a>(&'a self, base: Element<'a, Message>) -> Element<'a, Message> {
        if self.failed_view.is_none()
            && self.activity_view.is_none()
            && self.shadow_picker.is_none()
        {
            return base;
        }
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let dialog = responsive(move |size| {
            let body: Element<'_, Message> = if self.activity_view.is_some() {
                column![self.activity_body(size.width - 44.0)].into()
            } else if let Some(view) = &self.failed_view {
                self.failed_items_body(view)
            } else {
                self.shadow_picker_body(self.shadow_picker.as_ref().unwrap())
            };
            let shadow = self.shadow_picker.is_some();
            container(
                container(body)
                    .padding(22)
                    .width(if shadow {
                        size.width.min(900.)
                    } else {
                        size.width
                    })
                    .height(if shadow {
                        size.height.min(560.)
                    } else {
                        size.height
                    })
                    .style(move |_| t.card(false)),
            )
            .center(Fill)
            .into()
        });
        sk_iced::components::Dialog::custom(
            base,
            sk_iced::components::DialogBackdrop::custom(
                container(dialog).padding(32).width(Fill).height(Fill),
            )
            .scrim(iced::Color::from_rgba(0., 0., 0., 0.55)),
        )
        .into()
    }

    fn failed_items_body<'a>(&'a self, view: &'a FailedView) -> Element<'a, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );

        let Some(job) = self.jobs.iter().find(|job| job.id == view.job) else {
            return text("Transfer no longer available").into();
        };
        let Some(report) = &job.report else {
            return text("Transfer is still running").into();
        };
        let filter = view.filter.to_lowercase();
        let indices: Vec<_> = report
            .issues
            .iter()
            .enumerate()
            .filter(|(_, issue)| {
                filter.is_empty()
                    || issue
                        .source
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&filter)
                    || issue.reason.to_lowercase().contains(&filter)
            })
            .map(|(i, _)| i)
            .collect();
        let shown = indices.len();
        let rows = virtual_list::file_list_sized(
            indices.len(),
            view.identity,
            34.,
            move |row| {
                let index = indices[row];
                let issue = &report.issues[index];
                let info = format!(
                    "{}\n{}\n{}",
                    issue.source.display(),
                    issue
                        .target
                        .as_ref()
                        .map(|target| format!("Destination: {}", target.display()))
                        .unwrap_or_default(),
                    issue.reason
                );
                let fields = row![
                    cell(
                        if issue.skipped { "Skipped" } else { "Failed" },
                        iced::Length::Fixed(62.)
                    ),
                    cell(
                        issue.source.display().to_string(),
                        iced::Length::FillPortion(4)
                    ),
                    cell(issue.reason.as_str(), iced::Length::FillPortion(3)),
                ]
                .spacing(8)
                .align_y(iced::Center)
                .width(Fill);
                let fields = sk_iced::components::tooltip(
                    fields,
                    container(text(info).size(12))
                        .padding(10)
                        .max_width(750)
                        .style(move |_| t.card(false)),
                    tooltip::Position::Bottom,
                )
                .delay(std::time::Duration::from_secs(1));
                container(
                    row![
                        fields,
                        compact_action(
                            self.motion,
                            "Retry",
                            issue
                                .retry_request(job.request.kind)
                                .map(|_| Message::RetryTransferIssue(job.id, index)),
                            false
                        ),
                        compact_action(
                            self.motion,
                            "Location",
                            issue
                                .source
                                .parent()
                                .map(|_| Message::TransferIssueLocation(job.id, index)),
                            false
                        ),
                    ]
                    .spacing(8)
                    .align_y(iced::Center),
                )
                .padding([3, 8])
                .height(34)
                .width(Fill)
                .style(move |_| container::Style {
                    background: Some(
                        theme::blend(t.surface, t.text, if row % 2 == 1 { 0.035 } else { 0.0 })
                            .into(),
                    ),
                    ..Default::default()
                })
                .into()
            },
            |_| Message::Noop,
        );
        column![
            row![
                text("Failed / skipped transfers").size(21),
                Space::new().width(Fill),
                compact_action(
                    self.motion,
                    "Retry All",
                    job.has_retryable_issues()
                        .then_some(Message::RetryAllTransferIssues(job.id)),
                    false
                ),
                compact_action(self.motion, "Close", Some(Message::CloseUtility), false)
            ]
            .spacing(12)
            .align_y(iced::Center),
            text(format!(
                "{} · {shown} of {} items · Hover a path or reason for full details",
                job.request.kind.label(),
                report.issues.len()
            ))
            .size(12),
            text_input("Filter by path or reason…", &view.filter)
                .on_input(Message::TransferIssueFilter)
                .padding(8)
                .size(13),
            row![
                cell("Status", iced::Length::Fixed(62.)),
                cell("File", iced::Length::FillPortion(4)),
                cell("Reason", iced::Length::FillPortion(3)),
                Space::new().width(150)
            ]
            .spacing(8)
            .padding([0, 8]),
            rows,
        ]
        .spacing(10)
        .into()
    }
    fn shadow_picker_body<'a>(&'a self, picker: &'a shadow_copies::Picker) -> Element<'a, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );

        let mut contents = column![
                    row![
                        text("Open shadow copy").size(21),
                        Space::new().width(Fill),
                        compact_action(self.motion,
                            "Refresh",
                            picker.result.as_ref().map(|_| Message::RefreshShadows),
                            false
                        ),
                        compact_action(self.motion, "Close", Some(Message::CloseUtility), false)
                    ]
                    .spacing(12)
                    .align_y(iced::Center),
                    crate::typography::text_role(crate::typography::TextRole::Control, "A shadow copy is a snapshot of a drive from an earlier time. Browse it to find and copy older versions of files."),
                    text(format!(
                        "Open an existing snapshot in panel {} · Dates use local time",
                        picker.panel + 1
                    ))
                    .size(12),
                ]
                .spacing(12);
        match &picker.result {
            None => {
                contents = contents.push(crate::typography::text_role(
                    crate::typography::TextRole::Control,
                    "Looking for shadow copies on local drives…",
                ))
            }
            Some(Err(error)) => {
                contents = contents.push(
                    sk_iced::components::Notice::new(
                        sk_iced::components::Appearance::fixed(crate::typography::font(), t),
                        error.as_str(),
                    )
                    .text_size(13)
                    .tone(sk_iced::design::ColorRole::Danger),
                )
            }
            Some(Ok(copies)) if copies.is_empty() => {
                contents = contents.push(crate::typography::text_role(
                    crate::typography::TextRole::Control,
                    "No shadow copies were found on local drives.",
                ))
            }
            Some(Ok(copies)) => {
                contents = contents.push(
                    row![text("Drive").width(140), text("Snapshot date and time")].spacing(16),
                );
                contents = contents.push(virtual_list::file_list_sized(
                    copies.len(),
                    picker.request,
                    42.,
                    move |index| {
                        let copy = &copies[index];
                        sk_iced::components::tooltip(
                            crate::components::quiet_button(
                                container(
                                    row![
                                        cell(copy.drive.as_str(), iced::Length::Fixed(140.)),
                                        cell(copy.date.as_str(), Fill),
                                        crate::typography::text_role(
                                            crate::typography::TextRole::Caption,
                                            "Open tab →"
                                        )
                                    ]
                                    .spacing(16)
                                    .align_y(iced::Center),
                                )
                                .width(Fill)
                                .center_y(Fill),
                            )
                            .height(42)
                            .width(Fill)
                            .style(t.button(false, false))
                            .on_press(Message::OpenShadow(index)),
                            container(text(copy.device.display().to_string()).size(12))
                                .padding(8)
                                .style(move |_| t.card(false)),
                            tooltip::Position::Bottom,
                        )
                        .delay(std::time::Duration::from_secs(1))
                        .into()
                    },
                    Message::OpenShadow,
                ));
            }
        }
        contents.into()
    }
}
