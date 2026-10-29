use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    SelectAll,
    Rename,
    NewFolder,
    Copy,
    Cut,
    Paste,
    Delete,
    DeletePermanent,
}
pub struct Clipboard {
    pub native: bool,
    pub kind: transfers::Kind,
    pub paths: Vec<PathBuf>,
}
pub struct Confirmation {
    pub action: Action,
    pub paths: Vec<PathBuf>,
}
impl Action {
    pub fn confirmation_key(self) -> &'static str {
        match self {
            Self::Delete => "recycle",
            _ => "",
        }
    }
}
impl App {
    pub fn file_action(&mut self, action: Action) -> Task<Message> {
        if self.confirmation.is_some() || self.undo_busy {
            return Task::none();
        }
        if action == Action::NewFolder {
            return self.begin_new_folder();
        }
        if action == Action::Rename {
            return self.begin_rename();
        }
        if action == Action::SelectAll {
            let pane = &mut self.panes[self.active];
            pane.selections = pane.order.iter().copied().collect();
            pane.selected = pane.order.first().copied();
            return Task::none();
        }
        if action == Action::Paste {
            match file_clipboard::read() {
                Ok(Some((kind, paths))) => {
                    self.clipboard = Some(Clipboard {
                        kind,
                        paths,
                        native: true,
                    })
                }
                Ok(None) => {
                    if self.clipboard.as_ref().is_some_and(|c| c.native) {
                        self.clipboard = None;
                    }
                }
                Err(()) => {}
            }
            if let Some(clipboard) = &self.clipboard {
                let request = transfers::Request {
                    kind: clipboard.kind,
                    sources: clipboard.paths.clone(),
                    destination: Some(self.panes[self.active].path.clone()),
                };
                if clipboard.kind == transfers::Kind::Move {
                    if clipboard.native {
                        file_clipboard::clear();
                    }
                    self.clipboard = None;
                }
                return self.enqueue(request);
            }
            self.notice = "Copy or cut files before pasting.".into();
            return Task::none();
        }
        let pane = &self.panes[self.active];
        let paths: Vec<_> = pane
            .order
            .iter()
            .filter(|i| pane.selections.contains(i))
            .filter_map(|i| pane.entries.get(*i))
            .map(|e| e.path.clone())
            .collect();
        if paths.is_empty() {
            self.notice = "Select one or more files or folders first.".into();
            return Task::none();
        }
        if action == Action::Copy {
            self.notice = format!(
                "{} items copied to File Beholder clipboard. Select a destination and Paste.",
                paths.len()
            );
            let native = file_clipboard::write(&paths, transfers::Kind::Copy);
            self.clipboard = Some(Clipboard {
                native,
                kind: transfers::Kind::Copy,
                paths,
            });
        } else {
            self.confirmation = Some(Confirmation { action, paths });
            self.dont_ask_again = false;
            if self
                .preferences
                .skipped_confirmations
                .iter()
                .any(|key| action == Action::Delete && key == action.confirmation_key())
            {
                return self.confirm_operation(true);
            }
        }
        Task::none()
    }
    pub fn confirm_operation(&mut self, accept: bool) -> Task<Message> {
        let Some(confirmation) = self.confirmation.take() else {
            return Task::none();
        };
        if accept && self.dont_ask_again && confirmation.action == Action::Delete {
            let key = confirmation.action.confirmation_key().to_owned();
            if !self.preferences.skipped_confirmations.contains(&key) {
                self.preferences.skipped_confirmations.push(key);
                self.persist_preferences();
            }
        }
        self.dont_ask_again = false;
        if !accept {
            return Task::none();
        }
        if confirmation.action == Action::Cut {
            self.notice = format!(
                "{} items cut. Sources remain until Paste completes successfully.",
                confirmation.paths.len()
            );
            let native = file_clipboard::write(&confirmation.paths, transfers::Kind::Move);
            self.clipboard = Some(Clipboard {
                native,
                kind: transfers::Kind::Move,
                paths: confirmation.paths,
            });
            Task::none()
        } else {
            self.enqueue(transfers::Request {
                kind: if confirmation.action == Action::DeletePermanent {
                    transfers::Kind::Delete
                } else {
                    transfers::Kind::Recycle
                },
                sources: confirmation.paths,
                destination: None,
            })
        }
    }
    pub(crate) fn enqueue(&mut self, request: transfers::Request) -> Task<Message> {
        let id = next_identity();
        if self.jobs.iter().all(|j| j.report.is_some()) {
            self.queue_batch = id;
        }
        for source in &request.sources {
            let source = std::path::absolute(source).unwrap_or_else(|_| source.clone());
            let target = request
                .destination
                .as_ref()
                .and_then(|d| source.file_name().map(|n| d.join(n)));
            self.record_operation(id, request.kind.label(), source, target, "Queued");
        }
        self.jobs.push_back(transfers::Job::new(id, request));
        self.notice = "Added to queue".into();
        // Keep a bounded session history without removing active/pending work.
        while self.jobs.len() > 30
            && self.jobs.front().is_some_and(|job| {
                job.report.is_some()
                    && self
                        .failed_view
                        .as_ref()
                        .is_none_or(|view| view.job != job.id)
            })
        {
            self.jobs.pop_front();
        }
        self.start_next_job()
    }
    fn start_next_job(&mut self) -> Task<Message> {
        if self.running_job.is_some() || self.undo_busy {
            return Task::none();
        }
        let Some(job) = self.jobs.iter_mut().find(|job| job.state == "Queued") else {
            return Task::none();
        };
        job.state = "Running";
        let id = job.id;
        self.running_job = Some(id);
        for record in self.activities.iter_mut().filter(|r| r.operation == id) {
            record.status = "Running".into();
        }
        transfers::run(job.request.clone(), job.cancel.clone())
            .map(move |event| Message::Transfer(id, event))
    }
    pub fn transfer_event(&mut self, id: u64, event: transfers::Event) -> Task<Message> {
        let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) else {
            return Task::none();
        };
        if job.report.is_some() {
            return Task::none();
        }
        match event {
            transfers::Event::Conflict(source, target, reply) => {
                self.dont_ask_again = false;
                self.conflict = Some((source, target, reply));
                job.state = "Needs input";
            }
            transfers::Event::Progress(progress) => job.progress = progress,
            transfers::Event::Finished(report) => {
                job.state = if report.cancelled {
                    "Cancelled"
                } else if report.failure_count > 0 {
                    "Completed with errors"
                } else if !report.issues.is_empty() {
                    "Completed with skipped files"
                } else {
                    "Completed"
                };
                self.notice = format!(
                    "{} · {} successful, {} failed, {} skipped",
                    job.state,
                    report.succeeded,
                    report.failure_count,
                    report.issues.iter().filter(|issue| issue.skipped).count()
                );
                if let Some(warning) = &report.undo_warning {
                    self.notice.push_str(&format!(" · {warning}"));
                }
                let request = job.request.clone();
                job.report = Some(report.clone());
                if matches!(request.kind, transfers::Kind::Copy | transfers::Kind::Move) {
                    for (_, target) in report.completed_sources.iter().take(64) {
                        if let Some(target) = target {
                            self.highlights.request(target, self.motion);
                        }
                    }
                }
                self.finish_transfer_log(id, &report);
                self.running_job = None;
                self.conflict = None;
                let mut tasks = vec![];
                for (panel_id, panel) in self.panes.iter_mut().enumerate().take(self.pane_count) {
                    for tab in &mut panel.tabs {
                        if request
                            .sources
                            .iter()
                            .any(|p| path_utils::related(p, &tab.path))
                            || request
                                .destination
                                .as_ref()
                                .is_some_and(|p| path_utils::related(p, &tab.path))
                        {
                            tasks.push(tab.start_refresh(
                                panel_id,
                                true,
                                self.preferences.scan_mode,
                            ));
                        }
                    }
                }
                if self.preferences.completion_notifications || self.preferences.completion_sound {
                    if let Some(summary) = completion_summary(&self.jobs, self.queue_batch) {
                        tasks.push(desktop::completion_task(
                            self.preferences.completion_notifications,
                            self.preferences.sound_choice(),
                            self.preferences.custom_sound.clone(),
                            summary,
                        ));
                    }
                }
                tasks.push(self.start_next_job());
                self.history_generation = next_identity();
                tasks.push(interactions::history_task(self.history_generation));
                return Task::batch(tasks);
            }
        }
        Task::none()
    }
}
fn completion_summary(
    jobs: &std::collections::VecDeque<transfers::Job>,
    batch: u64,
) -> Option<String> {
    if jobs.is_empty() || jobs.iter().any(|j| j.report.is_none()) {
        return None;
    }
    let reports = jobs
        .iter()
        .filter(|j| j.id >= batch)
        .filter_map(|j| j.report.as_ref())
        .collect::<Vec<_>>();
    if reports.is_empty() || reports.iter().any(|r| r.cancelled) {
        return None;
    }
    let successful: usize = reports.iter().map(|r| r.succeeded).sum();
    let failed: usize = reports.iter().map(|r| r.failure_count).sum();
    Some(format!(
        "Transfer queue complete · {successful} successful, {failed} failed"
    ))
}
#[cfg(test)]
mod completion_tests {
    use super::*;
    fn report(success: usize, fail: usize, cancelled: bool) -> transfers::Report {
        transfers::Report {
            cancelled,
            issues: vec![],
            failures: vec![],
            failure_count: fail,
            succeeded: success,
            completed_sources: vec![],
            undo_warning: None,
            history: None,
        }
    }
    #[test]
    fn transfer_badge_matches_bar_top_and_height_with_or_without_stats() {
        use iced::advanced::{layout, widget::Tree};
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let progress = transfers::Progress::default();
        for stats in [None, Some(&progress)] {
            let mut widget = queue_meter(
                100.0,
                "Errors",
                None,
                palette::themed(true, Default::default()),
                stats,
                preferences::SizeUnits::default(),
            );
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(900.0, 100.0)),
            );
            let parts = iced::advanced::Layout::new(&node)
                .children()
                .collect::<Vec<_>>();
            let bar = parts[1].bounds();
            let badge = parts[2].bounds();
            assert_eq!(bar.y, badge.y);
            assert_eq!(bar.height, 20.0);
            assert_eq!(badge.height, bar.height);
        }
    }
    #[test]
    fn shrinking_transfer_header_never_grows_the_bar_at_column_transitions() {
        use iced::advanced::{Layout, layout, widget::Tree};
        let app = App::new("home".into(), "other".into(), vec![]);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let mut previous = f32::INFINITY;
        for width in (400..=1600).rev() {
            let width = width as f32;
            let available = (width - 76.0).max(324.0);
            let label = transfer_label_width(available);
            let mut widget = app.transfer_queue_sized(width, 900.0);
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(width, 900.0)),
            );
            let heading = Layout::new(&node)
                .children()
                .nth(1)
                .unwrap()
                .children()
                .next()
                .unwrap()
                .children()
                .next()
                .unwrap();
            let row = heading.children().next().unwrap();
            let children = row.children().collect::<Vec<_>>();
            let meter = children[usize::from(label > 0.0)];
            let bar = meter.children().nth(1).unwrap().bounds();
            assert!(
                bar.width <= previous + 0.01,
                "bar grew at width {width}: {} -> {}",
                previous,
                bar.width
            );
            assert!(bar.width >= MIN_TRANSFER_BAR - 0.01);
            if label < 150.0 {
                assert!(
                    (bar.width - MIN_TRANSFER_BAR).abs() < 0.01,
                    "label compressed before bar minimum at {width}"
                );
            }
            let end = children.last().unwrap().bounds();
            assert!(end.x + end.width <= width + 0.01);
            previous = bar.width;
        }
        for width in 324..=458 {
            let c = meter_columns(width as f32);
            assert_eq!(c.bar, MIN_TRANSFER_BAR);
            assert!((c.stats + c.badge + c.eta + 3.0 * c.gap + c.bar - width as f32).abs() < 0.01);
        }
    }
    #[test]
    fn transfer_columns_fit_narrow_widths_and_percentage_stays_at_right_edge() {
        use iced::advanced::{Layout, layout, widget::Tree};
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        let progress = transfers::Progress {
            done_bytes: 10 * 1_048_576,
            total_bytes: 10 * 1_048_576,
            done_items: 10,
            total_items: 20,
            ..Default::default()
        };
        let (bytes, _) = transfer_stats(&progress, preferences::SizeUnits::default());
        assert_eq!(bytes, "10.0 / 10.0 MiB");
        for width in [324.0, 380.0, 500.0, 650.0, 1000.0] {
            let mut widget = queue_meter_sized(
                42.0,
                "Completed with skipped files",
                Some(95),
                palette::themed(true, Default::default()),
                Some(&progress),
                preferences::SizeUnits::default(),
                width,
            );
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(width, 32.0)),
            );
            let parts = Layout::new(&node).children().collect::<Vec<_>>();
            let bar = parts[1];
            let badge = parts[2];
            assert!(bar.bounds().width >= 80.0);
            assert!(parts[3].bounds().x + parts[3].bounds().width <= width + 0.1);
            assert_eq!(bar.bounds().y, badge.bounds().y);
            assert_eq!(bar.bounds().height, badge.bounds().height);
            let label = bar
                .children()
                .next()
                .unwrap()
                .children()
                .nth(1)
                .unwrap()
                .children()
                .next()
                .unwrap()
                .bounds();
            assert!(
                (label.x + label.width - (bar.bounds().x + bar.bounds().width - 8.0)).abs() < 0.1
            );
        }
    }
    #[test]
    fn queue_resize_preserves_header_height_and_renders_responsive_rows() {
        use iced::advanced::{Layout, layout, widget::Tree};
        let mut app = App::new("home".into(), "other".into(), vec![]);
        app.queue_expanded = true;
        let mut job = transfers::Job::new(
            1,
            transfers::Request {
                kind: transfers::Kind::Copy,
                sources: vec!["selected folder".into()],
                destination: Some("destination".into()),
            },
        );
        job.progress = transfers::Progress {
            total_bytes: 50 * 1_048_576,
            done_bytes: 10 * 1_048_576,
            bytes_per_second: 1_048_576.0,
            total_items: 20,
            done_items: 5,
            eta: Some(40),
            status: "Copying a selected folder and its contents".into(),
            ..Default::default()
        };
        app.jobs.push_back(job);
        app.running_job = Some(1);
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 13.0.into());
        for height in [150.0, 300.0] {
            let _ = app.update(Message::ResizeTransfers(height));
            let mut widget = app.transfer_queue_sized(1000.0, 900.0);
            let mut tree = Tree::new(&widget);
            let node = widget.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(1000.0, 900.0)),
            );
            let parts = Layout::new(&node)
                .children()
                .nth(1)
                .unwrap()
                .children()
                .next()
                .unwrap()
                .children()
                .collect::<Vec<_>>();
            assert_eq!(parts[0].bounds().height, 40.0);
            assert_eq!(parts[1].bounds().height, height);
        }
        app.queue_expanded = false;
        let _ = app.update(Message::ResizeTransfers(f32::NAN));
        assert_eq!(app.queue_height, 300.0);
        for width in [480u32, 640, 900, 1400] {
            for expanded in [false, true] {
                app.queue_expanded = expanded;
                app.queue_height = 150.0;
                let pixels = crate::final_ui_checks::render_view(
                    app.transfer_queue(),
                    &app,
                    iced::Size::new(width, 280),
                    None,
                );
                if std::env::var_os("FILE_BEHOLDER_CAPTURE").is_some() {
                    tiny_skia::Pixmap::from_vec(
                        pixels,
                        tiny_skia::IntSize::from_wh(width, 280).unwrap(),
                    )
                    .unwrap()
                    .save_png(format!("target/queue-{width}-{expanded}.png"))
                    .unwrap();
                }
            }
        }
    }
    #[test]
    fn completion_waits_for_whole_batch_aggregates_results_and_ignores_cancelled_batches() {
        let mut jobs = std::collections::VecDeque::new();
        for id in [1, 2] {
            jobs.push_back(transfers::Job::new(
                id,
                transfers::Request {
                    kind: transfers::Kind::Copy,
                    sources: vec!["source".into()],
                    destination: Some("target".into()),
                },
            ));
        }
        jobs[0].report = Some(report(3, 1, false));
        assert!(completion_summary(&jobs, 1).is_none());
        jobs[1].report = Some(report(4, 0, false));
        assert_eq!(
            completion_summary(&jobs, 1),
            Some("Transfer queue complete · 7 successful, 1 failed".into())
        );
        assert_eq!(
            completion_summary(&jobs, 2),
            Some("Transfer queue complete · 4 successful, 0 failed".into())
        );
        jobs[1].report = Some(report(4, 0, true));
        assert!(completion_summary(&jobs, 1).is_none());
    }
}

mod confirmation_view;
mod transfer_view;
#[cfg(test)]
use transfer_view::*;
