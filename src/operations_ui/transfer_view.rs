//! Transfer queue/header/job presentation; queue state stays in App.
use super::*;
use iced::widget::{column, row, stack};
impl App {
    pub fn transfer_queue(&self) -> Element<'_, Message> {
        crate::responsive_cache::responsive(move |size| {
            self.transfer_queue_sized(size.width, size.height)
        })
        .height(iced::Length::Shrink)
        .into()
    }
    pub(super) fn transfer_queue_sized(
        &self,
        width: f32,
        available_height: f32,
    ) -> Element<'_, Message> {
        let available = (width - 76.0).max(324.0);
        let label_width = transfer_label_width(available);
        let show_label = label_width > 0.0;
        let maximum = (available_height - 200.0)
            .max(60.0)
            .min((available_height * 0.6).max(60.0));
        let list_height = self.queue_height.min(maximum);
        let meter_width = available - label_width;
        let t = palette::styled(
            self.dark,
            self.preferences.accent,
            self.preferences.tint,
            self.look(),
        );
        let pending: Vec<_> = self.jobs.iter().filter(|j| j.report.is_none()).collect();
        let batch: Vec<_> = self
            .jobs
            .iter()
            .filter(|j| j.id >= self.queue_batch)
            .collect();
        let percent = if batch.is_empty() {
            0.0
        } else {
            batch.iter().map(|j| j.progress.percent()).sum::<f32>() / batch.len() as f32
        };
        let eta = if pending.len() == 1 {
            pending[0].progress.eta
        } else {
            None
        };
        let state = if self.undo_busy {
            "Undoing"
        } else if self.conflict.is_some() {
            "Needs input"
        } else if self.running_job.is_some() {
            "Transferring"
        } else if self.jobs.back().is_some_and(|j| j.progress.failures > 0) {
            "Errors"
        } else if self.jobs.is_empty() {
            "Ready"
        } else {
            "Complete"
        };
        let mut aggregate = transfers::Progress::default();
        for job in &batch {
            aggregate.total_bytes = aggregate
                .total_bytes
                .saturating_add(job.progress.total_bytes);
            aggregate.done_bytes = aggregate.done_bytes.saturating_add(job.progress.done_bytes);
            aggregate.total_items += job.progress.total_items;
            aggregate.done_items += job.progress.done_items;
            if job.report.is_none() {
                aggregate.bytes_per_second += job.progress.bytes_per_second;
            }
        }
        let mut header_t = t;
        if self.status_hovered != Some(usize::MAX) {
            let dim = |rgb| {
                let c = theme::blend(t.surface, rgb, 0.65);
                (c.r * 255.0) as u32
                    | (((c.g * 255.0) as u32) << 8)
                    | (((c.b * 255.0) as u32) << 16)
            };
            header_t.text = dim(t.text);
            header_t.secondary = dim(t.secondary);
            header_t.accent = dim(t.accent);
            header_t.success = dim(t.success);
            header_t.failure = dim(t.failure);
            header_t.info = dim(t.info);
            header_t.meter_start = dim(t.meter_start);
            header_t.meter_end = dim(t.meter_end);
        }
        let mut header = row![].spacing(0).align_y(iced::Center);
        if show_label {
            header = header.push(transfer_label(
                format!("Transfers · {} pending", pending.len()),
                label_width,
                13.0,
            ));
        }
        header = header
            .push(queue_meter_sized(
                percent,
                state,
                eta,
                header_t,
                (!batch.is_empty()).then_some(&aggregate),
                self.preferences.size_units,
                meter_width,
            ))
            .push(Space::new().width(10))
            .push(crate::ui_icons::disclosure(
                self.queue_expanded,
                header_t.text,
                32.0,
            ));
        let heading = crate::components::quiet_button(header)
            .on_press(Message::ToggleQueue)
            .padding([4, 16])
            .height(40)
            .width(Fill)
            .style(move |theme, status| {
                let mut style = t.flat_dimmed()(theme, status);
                style.text_color = theme::color(header_t.text);
                style
            });
        let heading = iced::widget::mouse_area(heading)
            .on_enter(Message::StatusHover(Some(usize::MAX)))
            .on_exit(Message::StatusHover(None));
        let mut content = column![];

        content = content.push(heading);
        if self.notice.contains("Undo") {
            content = content.push(
                sk_iced::components::tooltip(
                    container(
                        text(&self.notice)
                            .size(11)
                            .color(theme::color(if self.notice.starts_with("Undo stopped") {
                                t.warning
                            } else {
                                t.secondary
                            }))
                            .wrapping(iced::widget::text::Wrapping::None),
                    )
                    .width(Fill)
                    .height(16)
                    .clip(true),
                    container(text(&self.notice).size(12))
                        .padding(8)
                        .width(600)
                        .style(move |_| t.card(false)),
                    iced::widget::tooltip::Position::Top,
                )
                .delay(std::time::Duration::from_secs(1)),
            );
        }
        content = content.push(sk_iced::components::ExpandableCard::reveal(
            self.queue_expanded,
            self.motion,
            move || {
                let mut jobs = column![compact_action(
                    self.motion,
                    "File operations history",
                    Some(Message::ShowActivity(None)),
                    false
                )]
                .spacing(8);
                if self.jobs.is_empty() {
                    jobs = jobs.push(
                        text(if self.notice.is_empty() {
                            "No transfers"
                        } else {
                            &self.notice
                        })
                        .size(12),
                    );
                }
                for job in &self.jobs {
                    jobs = jobs.push(self.transfer_job(job, width, t));
                }
                container(
                    sk_iced::components::scroll_area(
                        container(jobs).padding(t.visual.button.normal.shadow_padding()),
                    )
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        sk_iced::components::Scrollbar::new()
                            .width(6)
                            .scroller_width(6)
                            .spacing(4)
                            .build(),
                    ))
                    .style(t.scroll()),
                )
                .padding([0, 10])
                .height(iced::Length::Shrink)
                .height(iced::Length::Fixed(list_height))
                .into()
            },
        ));
        let card = sk_iced::components::Surface::new(
            sk_iced::components::Appearance::fixed(crate::typography::font(), t),
            container(content)
                .padding(1)
                .style(move |_| t.visual.disclosure.container_style(t.card(false))),
        )
        .role(sk_iced::components::SurfaceRole::Disclosure)
        .decorate();
        column![
            sk_iced::components::ExpandableCard::reveal(
                self.queue_expanded,
                self.motion,
                move || { crate::transfer_resize::handle(list_height, maximum) }
            ),
            card
        ]
        .into()
    }
    fn transfer_job<'a>(
        &'a self,
        job: &'a transfers::Job,
        width: f32,
        t: theme::Tokens,
    ) -> Element<'a, Message> {
        let job_available = (width - 104.0).max(324.0);
        let job_label = transfer_label_width(job_available);
        let mut summary = row![].spacing(0).align_y(iced::Center);
        if job_label > 0.0 {
            summary = summary.push(transfer_label(
                format!(
                    "{} · {} items",
                    job.request.kind.label(),
                    job.request.sources.len()
                ),
                job_label,
                12.0,
            ));
        }
        summary = summary
            .push(queue_meter_sized(
                job.progress.percent(),
                job.state,
                job.progress.eta,
                t,
                Some(&job.progress),
                self.preferences.size_units,
                job_available - job_label,
            ))
            .push(Space::new().width(10))
            .push(compact_action(
                self.motion,
                "Cancel",
                (job.report.is_none()).then_some(Message::CancelTransfer(job.id)),
                false,
            ));
        let mut details = column![
            summary,
            container(
                text(&job.progress.status)
                    .size(11)
                    .wrapping(iced::widget::text::Wrapping::None)
            )
            .width(Fill)
            .clip(true)
        ]
        .spacing(3);
        if let Some(report) = &job.report {
            if !report.completed_sources.is_empty() {
                details = details.push(compact_action(
                    self.motion,
                    "View successful transfers",
                    Some(Message::ShowActivity(Some(job.id))),
                    false,
                ));
            }
            if report.issues.is_empty() {
                for error in &report.failures {
                    details = details.push(text(error).size(11).color(theme::color(t.failure)));
                }
            }
            if let Some(warning) = &report.undo_warning {
                details = details.push(text(warning).size(11).color(theme::color(t.warning)));
            }
            if !report.issues.is_empty() {
                details = details.push(
                    row![
                        compact_action(
                            self.motion,
                            "View failed / skipped files",
                            Some(Message::ToggleTransferIssues(job.id)),
                            false
                        ),
                        text(format!("{} items", report.issues.len())).size(12),
                        compact_action(
                            self.motion,
                            "Retry All",
                            job.has_retryable_issues()
                                .then_some(Message::RetryAllTransferIssues(job.id)),
                            false
                        )
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                );
            }
        }

        container(details)
            .padding([6, 8])
            .style(move |_| t.card(false))
            .into()
    }
}
#[cfg(test)]
pub(super) fn queue_meter<'a>(
    percent: f32,
    status: &str,
    eta: Option<u64>,
    t: theme::Tokens,
    progress: Option<&transfers::Progress>,
    units: preferences::SizeUnits,
) -> Element<'a, Message> {
    queue_meter_sized(percent, status, eta, t, progress, units, 900.0)
}
pub(super) fn transfer_stats(
    p: &transfers::Progress,
    units: preferences::SizeUnits,
) -> (String, String) {
    let (divisor, label) = if units.metric() {
        (1_000_000.0, "MB")
    } else {
        (1_048_576.0, "MiB")
    };
    (
        format!(
            "{:.1} / {:.1} {label}",
            p.done_bytes as f64 / divisor,
            p.total_bytes as f64 / divisor
        ),
        format!(
            "{:.1} {label}/s · {} / {} items",
            p.bytes_per_second / divisor,
            p.done_items,
            p.total_items
        ),
    )
}
pub(super) fn transfer_label<'a>(label: String, width: f32, size: f32) -> Element<'a, Message> {
    container(
        container(
            text(label)
                .size(size)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width((width - 10.0).max(0.0))
        .clip(true),
    )
    .width(width)
    .clip(true)
    .into()
}
pub(super) const MIN_TRANSFER_BAR: f32 = 80.0;
// Spend surplus width on the bar first. After it reaches its minimum,
// continuously reduce other columns instead of giving released space back to it.
pub(super) fn transfer_label_width(available: f32) -> f32 {
    (available - (MIN_TRANSFER_BAR + 190.0 + 100.0 + 64.0 + 24.0)).clamp(0.0, 150.0)
}
#[derive(Clone, Copy, Debug)]
pub(super) struct MeterColumns {
    pub(super) stats: f32,
    pub(super) badge: f32,
    pub(super) eta: f32,
    pub(super) gap: f32,
    pub(super) bar: f32,
}
pub(super) fn meter_columns(width: f32) -> MeterColumns {
    let deficit = (458.0 - width).max(0.0);
    let stats = 190.0 - deficit.min(72.0);
    let badge = 100.0 - (deficit - 72.0).clamp(0.0, 32.0);
    let eta = 64.0 - (deficit - 104.0).clamp(0.0, 24.0);
    let gap = (24.0 - (deficit - 128.0).clamp(0.0, 6.0)) / 3.0;
    MeterColumns {
        stats,
        badge,
        eta,
        gap,
        bar: (width - 378.0).max(MIN_TRANSFER_BAR),
    }
}
pub(super) fn queue_meter_sized<'a>(
    percent: f32,
    status: &str,
    eta: Option<u64>,
    t: theme::Tokens,
    progress: Option<&transfers::Progress>,
    units: preferences::SizeUnits,
    width: f32,
) -> Element<'a, Message> {
    let columns = meter_columns(width);
    let stats_width = columns.stats;
    let badge_width = columns.badge;
    let eta_width = columns.eta;
    let details = stats_width >= 158.0;
    let narrow = eta_width < 58.0;
    let tone = if status.to_lowercase().contains("error") {
        t.warning
    } else if matches!(status, "Completed" | "Complete") {
        t.success
    } else {
        t.accent
    };
    let percent = percent.clamp(0.0, 100.0);
    let face = |ink| -> Element<'a, Message> {
        container(text(format!("{percent:.0}%")).size(11).color(ink))
            .padding([0, 8])
            .align_x(iced::Right)
            .center_y(20)
            .width(Fill)
            .clip(true)
            .into()
    };
    let bar = container(
        stack![
            sk_iced::components::progress(0.0..=100.0, percent)
                .girth(20)
                .style(t.progress(tone)),
            face(t.contrast(t.border)),
            crate::transfer_resize::filled_text(face(t.contrast(tone)), percent)
        ]
        .height(20)
        .width(Fill),
    )
    .width(columns.bar);
    let stats = if let Some(p) = progress {
        let (bytes, extra) = transfer_stats(p, units);
        let mut lines = column![
            text(bytes)
                .size(11)
                .wrapping(iced::widget::text::Wrapping::None)
        ];
        if details {
            lines = lines.push(
                text(extra)
                    .size(10)
                    .color(theme::color(t.secondary))
                    .wrapping(iced::widget::text::Wrapping::None),
            );
        }
        container(lines).width(stats_width).clip(true)
    } else {
        container(
            crate::typography::text_role(crate::typography::TextRole::Small, "—")
                .color(theme::color(t.secondary)),
        )
        .width(stats_width)
    };
    let badge = sk_iced::components::StatusBadge::custom(container(
        text(status.to_owned())
            .size(10)
            .wrapping(iced::widget::text::Wrapping::None),
    ))
    .with_appearance(sk_iced::components::Appearance::fixed(
        crate::typography::font(),
        t,
    ))
    .padding([0, 7])
    .center_y(20)
    .width(badge_width)
    .clip(true)
    .style(move |_| {
        t.visual.badge.container_style(container::Style {
            background: Some(theme::blend(t.surface, tone, 0.18).into()),
            text_color: Some(theme::color(tone)),
            border: iced::Border {
                radius: 11.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
    });
    let time = eta
        .map(|s| format!("{}:{:02}", s / 60, s % 60))
        .unwrap_or_else(|| "—".into());
    row![
        stats,
        bar,
        badge,
        container(
            text(if narrow { time } else { format!("ETA {time}") })
                .size(11)
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .center_y(20)
        .width(eta_width)
        .clip(true)
    ]
    .spacing(columns.gap)
    .height(32)
    .align_y(iced::Center)
    .into()
}
