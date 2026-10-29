//! Session operation history records selected roots, independently of queue retention.
use crate::*;
use iced::widget::column;
#[derive(Clone)]
pub struct Record {
    pub operation: u64,
    pub time: String,
    pub kind: String,
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub status: String,
    pub completed: bool,
}
pub struct View {
    pub job: Option<u64>,
    pub filter: String,
    pub identity: u64,
}
impl App {
    pub fn record_operation(
        &mut self,
        operation: u64,
        kind: &str,
        source: PathBuf,
        target: Option<PathBuf>,
        status: &str,
    ) {
        self.activities.push(Record {
            operation,
            time: browser::format_modified(Some(std::time::SystemTime::now())),
            kind: kind.into(),
            source,
            target,
            status: status.into(),
            completed: status == "Completed",
        });
    }
    pub fn finish_transfer_log(&mut self, id: u64, report: &transfers::Report) {
        for record in self.activities.iter_mut().filter(|r| r.operation == id) {
            if let Some((_, target)) = report
                .completed_sources
                .iter()
                .find(|(source, _)| path_utils::same(source, &record.source))
            {
                record.status = "Completed".into();
                record.completed = true;
                record.target = target.clone();
            } else {
                let issues = report
                    .issues
                    .iter()
                    .filter(|i| path_utils::related(&i.source, &record.source))
                    .collect::<Vec<_>>();
                record.status = if let Some(issue) = issues.first() {
                    format!(
                        "{}: {}",
                        if issue.skipped {
                            "Skipped / incomplete"
                        } else {
                            "Failed / incomplete"
                        },
                        issue.reason
                    )
                } else if report.cancelled {
                    "Cancelled / incomplete".into()
                } else {
                    "Not completed".into()
                };
            }
        }
    }
    pub fn activity_body(&self, width: f32) -> Element<'_, Message> {
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let view = self.activity_view.as_ref().unwrap();
        let needle = view.filter.to_lowercase();
        let records = self
            .activities
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                view.job.is_none_or(|id| r.operation == id && r.completed)
                    && (needle.is_empty()
                        || format!(
                            "{} {} {} {}",
                            r.kind,
                            r.source.display(),
                            r.target
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default(),
                            r.status
                        )
                        .to_lowercase()
                        .contains(&needle))
            })
            .collect::<Vec<_>>();
        let count = records.len();
        let narrow = width < 700.0;
        let row_height = if narrow { 52.0 } else { 38.0 };
        let rows = virtual_list::file_list_sized(
            count,
            view.identity,
            row_height,
            move |row| {
                let (index, r) = records[row];
                let info = format!(
                    "{} · {} · {}\n{}\n{}",
                    r.time,
                    r.kind,
                    r.status,
                    r.source.display(),
                    r.target
                        .as_ref()
                        .map(|p| format!("Destination: {}", p.display()))
                        .unwrap_or_default()
                );
                let fields: Element<'_, Message> = if narrow {
                    column![
                        utility_dialogs::cell(r.source.display().to_string(), Fill),
                        row![
                            utility_dialogs::cell(r.kind.as_str(), iced::Length::Fixed(100.0)),
                            utility_dialogs::cell(r.status.as_str(), Fill)
                        ]
                        .spacing(8)
                    ]
                    .spacing(3)
                    .into()
                } else {
                    row![
                        utility_dialogs::cell(
                            format!("{} · {}", index + 1, r.time),
                            iced::Length::Fixed(160.0)
                        ),
                        utility_dialogs::cell(r.kind.as_str(), iced::Length::Fixed(110.0)),
                        utility_dialogs::cell(
                            r.source.display().to_string(),
                            iced::Length::FillPortion(3)
                        ),
                        utility_dialogs::cell(
                            r.target
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "—".into()),
                            iced::Length::FillPortion(3)
                        ),
                        utility_dialogs::cell(r.status.as_str(), iced::Length::FillPortion(2))
                    ]
                    .spacing(8)
                    .align_y(iced::Center)
                    .into()
                };
                sk_iced::components::tooltip(
                    container(fields)
                        .padding([3, 8])
                        .height(row_height)
                        .width(Fill)
                        .style(move |_| container::Style {
                            background: Some(
                                theme::blend(
                                    t.surface,
                                    t.text,
                                    if row % 2 == 1 { 0.035 } else { 0.0 },
                                )
                                .into(),
                            ),
                            ..Default::default()
                        }),
                    container(text(info).size(12))
                        .padding(10)
                        .max_width(750)
                        .style(move |_| t.card(false)),
                    iced::widget::tooltip::Position::Bottom,
                )
                .delay(std::time::Duration::from_secs(1))
                .into()
            },
            |_| Message::Noop,
        );
        let header: Element<'_, Message> = if narrow {
            crate::typography::text_role(
                crate::typography::TextRole::Small,
                "Selected item · Operation / status · Hover for destination and time",
            )
            .into()
        } else {
            row![
                utility_dialogs::cell("Order / local time", iced::Length::Fixed(160.0)),
                utility_dialogs::cell("Operation", iced::Length::Fixed(110.0)),
                utility_dialogs::cell("Selected item", iced::Length::FillPortion(3)),
                utility_dialogs::cell("Destination", iced::Length::FillPortion(3)),
                utility_dialogs::cell("Status", iced::Length::FillPortion(2))
            ]
            .spacing(8)
            .padding([0, 8])
            .into()
        };
        column![
            row![
                text(if view.job.is_some() {
                    "Successful transfers"
                } else {
                    "File operations"
                })
                .size(21),
                Space::new().width(Fill),
                compact_action(self.motion, "Close", Some(Message::CloseUtility), false)
            ]
            .spacing(12)
            .align_y(iced::Center),
            text(format!(
                "{count} selected items · Current session · Oldest first · Hover for full details"
            ))
            .size(12),
            text_input("Filter by operation, path or status…", &view.filter)
                .on_input(Message::ActivityFilter)
                .padding(8)
                .size(13),
            header,
            rows
        ]
        .spacing(10)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_retains_selected_roots_and_modal_actions_are_isolated() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        for id in 0..45 {
            app.record_operation(
                id,
                "Copy",
                PathBuf::from(format!("source{id}")),
                Some("destination".into()),
                "Queued",
            );
        }
        let report = transfers::Report {
            issues: vec![],
            cancelled: false,
            failures: vec![],
            failure_count: 0,
            succeeded: 500,
            completed_sources: vec![("source0".into(), Some("destination/root".into()))],
            undo_warning: None,
            history: None,
        };
        app.finish_transfer_log(0, &report);
        assert_eq!(app.activities.len(), 45);
        assert!(app.activities[0].completed);
        assert_eq!(
            app.activities[0].target,
            Some(PathBuf::from("destination/root"))
        );
        assert!(
            app.activities
                .windows(2)
                .all(|pair| pair[0].operation < pair[1].operation)
        );
        let _ = app.update(Message::ShowActivity(Some(0)));
        assert_eq!(app.activity_view.as_ref().unwrap().job, Some(0));
        let active = app.active;
        let _ = app.update(Message::PanelShortcut);
        assert_eq!(app.active, active);
        let _ = app.update(Message::ActivityFilter("SOURCE".into()));
        assert_eq!(app.activity_view.as_ref().unwrap().filter, "SOURCE");
        let _ = app.update(Message::Confirm(false));
        assert!(app.activity_view.is_none());
        let _ = app.update(Message::ShowActivity(None));
        let _ = app.update(Message::CloseUtility);
        assert!(app.activity_view.is_none());
        assert_eq!(app.activities.len(), 45);
    }
    #[test]
    fn activity_dialogs_render_many_roots_at_small_and_large_window_sizes() {
        let mut app = App::new("home".into(), "other".into(), vec![]);
        for id in 0..1000 {
            app.record_operation(
                id / 10,
                "Copy",
                PathBuf::from(format!("C:/Users/Selected folder {id}")),
                Some(PathBuf::from(format!("D:/Backup/Selected folder {id}"))),
                "Completed",
            );
        }
        for job in [None, Some(0)] {
            let _ = app.update(Message::ShowActivity(job));
            for (w, h) in [(480, 480), (1400, 900)] {
                let pixels = crate::final_ui_checks::render_view(
                    app.utility_layer(container(Space::new()).width(Fill).height(Fill).into()),
                    &app,
                    iced::Size::new(w, h),
                    None,
                );
                assert_eq!(pixels.len(), (w * h * 4) as usize);
                if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                    tiny_skia::Pixmap::from_vec(pixels, tiny_skia::IntSize::from_wh(w, h).unwrap())
                        .unwrap()
                        .save_png(format!(
                            "target/activity-{}-{w}.png",
                            if job.is_some() { "success" } else { "history" }
                        ))
                        .unwrap();
                }
            }
        }
    }
}
