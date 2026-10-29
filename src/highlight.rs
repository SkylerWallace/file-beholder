//! File-operation highlight requests stay app-owned; paint and timing are shared.
use crate::motion::Policy;
pub use sk_iced::animation::highlight::*;
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
const PENDING_LIFETIME: Duration = Duration::from_secs(10);
const LIMIT: usize = 64;
struct Entry {
    requested: Instant,
    pulse: Option<Pulse>,
}
#[derive(Default)]
pub struct Store {
    entries: BTreeMap<String, Entry>,
}
impl Store {
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    fn prune(&mut self, now: Instant) {
        self.entries.retain(|_, e| {
            e.pulse.map_or(
                now.saturating_duration_since(e.requested) < PENDING_LIFETIME,
                |p| now.saturating_duration_since(p.started()) < DURATION,
            )
        });
    }
    pub fn request(&mut self, path: &Path, policy: Policy) {
        if !policy.enabled() {
            self.clear();
            return;
        }
        let now = Instant::now();
        self.prune(now);
        let key = crate::path_utils::key(path);
        if self.entries.len() >= LIMIT && !self.entries.contains_key(&key) {
            // Prefer the newest explicit action over stale/offscreen requests.
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.requested)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            key,
            Entry {
                requested: now,
                pulse: None,
            },
        );
    }
    pub fn flash(&mut self, path: &Path, policy: Policy) {
        self.request(path, policy);
        if let Some(e) = self.entries.get_mut(&crate::path_utils::key(path)) {
            e.pulse = Pulse::new(policy);
        }
    }
    pub fn resolve<'a>(&mut self, paths: impl Iterator<Item = &'a Path>, policy: Policy) {
        if !policy.enabled() {
            self.clear();
            return;
        }
        let now = Instant::now();
        self.prune(now);
        if !self.entries.values().any(|e| e.pulse.is_none()) {
            return;
        }
        for path in paths {
            if let Some(e) = self.entries.get_mut(&crate::path_utils::key(path)) {
                if e.pulse.is_none() {
                    e.pulse = Pulse::new(policy);
                }
            }
        }
    }
    pub fn pulse(&self, path: &Path) -> Option<Pulse> {
        if self.entries.is_empty() {
            return None;
        }
        self.entries
            .get(&crate::path_utils::key(path))
            .and_then(|e| e.pulse)
            .filter(|p| Instant::now().saturating_duration_since(p.started()) < DURATION)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const ON: Policy = Policy {
        system_enabled: true,
    };
    const PE: Policy = sk_iced::motion::Policy::DISABLED;
    #[test]
    fn scans_only_resolve_explicit_paths_once_and_feedback_is_bounded() {
        let mut store = Store::default();
        store.resolve([Path::new("ordinary")].into_iter(), ON);
        assert!(store.pulse(Path::new("ordinary")).is_none());
        store.request(Path::new("created"), ON);
        assert!(store.pulse(Path::new("created")).is_none());
        store.resolve(
            [Path::new("created"), Path::new("ordinary")].into_iter(),
            ON,
        );
        let pulse = store.pulse(Path::new("created")).unwrap();
        store.resolve([Path::new("created")].into_iter(), ON);
        assert_eq!(store.pulse(Path::new("created")), Some(pulse));
        assert!(store.pulse(Path::new("ordinary")).is_none());
        assert_eq!(pulse.amount(pulse.started() + DURATION, ON), 0.0);
        store.prune(pulse.started() + DURATION);
        store.resolve([Path::new("created")].into_iter(), ON);
        assert!(store.pulse(Path::new("created")).is_none());
        for i in 0..1000 {
            store.request(Path::new(&format!("item-{i}")), ON);
        }
        assert_eq!(store.entries.len(), LIMIT);
        store.flash(Path::new("newest-drop"), ON);
        assert!(store.pulse(Path::new("newest-drop")).is_some());
        assert_eq!(store.entries.len(), LIMIT);
        store.prune(Instant::now() + PENDING_LIFETIME);
        assert!(store.entries.is_empty());
        store.flash(Path::new("drop"), PE);
        assert!(store.entries.is_empty());
    }
    #[test]
    fn only_successful_transfer_roots_are_requested() {
        let mut app = crate::App::new("source".into(), "destination".into(), vec![]);
        app.motion = ON;
        let id = crate::next_identity();
        app.jobs.push_back(crate::transfers::Job::new(
            id,
            crate::transfers::Request {
                kind: crate::transfers::Kind::Copy,
                sources: vec!["source/a".into(), "source/b".into()],
                destination: Some("destination".into()),
            },
        ));
        let report = crate::transfers::Report {
            issues: vec![],
            cancelled: true,
            failures: vec!["b failed".into()],
            failure_count: 1,
            succeeded: 1,
            completed_sources: vec![("source/a".into(), Some("destination/a - Copy".into()))],
            undo_warning: None,
            history: None,
        };
        let _ = app.update(crate::Message::Transfer(
            id,
            crate::transfers::Event::Finished(report),
        ));
        app.highlights.resolve(
            [
                Path::new("destination/a - Copy"),
                Path::new("destination/b"),
                Path::new("destination/a"),
            ]
            .into_iter(),
            ON,
        );
        assert!(
            app.highlights
                .pulse(Path::new("destination/a - Copy"))
                .is_some()
        );
        assert!(app.highlights.pulse(Path::new("destination/b")).is_none());
        assert!(app.highlights.pulse(Path::new("destination/a")).is_none());
        let pulse = app.highlights.pulse(Path::new("destination/a - Copy"));
        let _ = app.update(crate::Message::Transfer(
            id,
            crate::transfers::Event::Progress(Default::default()),
        ));
        assert_eq!(
            app.highlights.pulse(Path::new("destination/a - Copy")),
            pulse
        );
    }
    #[test]
    fn successful_edits_request_highlights_but_failed_edits_and_plain_scans_do_not() {
        for new_folder in [false, true] {
            let mut app = crate::App::new("parent".into(), "other".into(), vec![]);
            app.motion = ON;
            let id = crate::next_identity();
            app.rename = Some(crate::rename::Editor {
                id,
                source: "parent/old".into(),
                name: "new".into(),
                error: None,
                busy: true,
                new_folder,
            });
            let _ = app.update(crate::Message::RenameFinished(id, Err("failed".into())));
            assert!(app.highlights.entries.is_empty());
            let _ = app.update(crate::Message::RenameFinished(id, Ok(None)));
            assert_eq!(app.highlights.entries.len(), 1);
            app.highlights
                .resolve([Path::new("parent/new")].into_iter(), ON);
            assert!(app.highlights.pulse(Path::new("parent/new")).is_some());
        }
    }
    #[test]
    fn accepted_drop_flashes_destination_immediately_and_invalid_drop_is_quiet() {
        let mut app = crate::App::new("source".into(), "destination".into(), vec![]);
        app.motion = ON;
        app.drag_files = Some(crate::interactions::DraggedFiles {
            pane: 0,
            index: 0,
            paths: vec!["source/a".into()],
        });
        app.drop_files(0, 0, 9);
        assert!(app.highlights.entries.is_empty());
        app.drag_files = Some(crate::interactions::DraggedFiles {
            pane: 0,
            index: 0,
            paths: vec!["source/a".into()],
        });
        app.drop_files(0, 0, 1);
        assert_eq!(app.active, 1);
        assert!(app.highlights.pulse(Path::new("destination")).is_some());
        assert!(app.drop_confirmation.is_some());
        app.highlights.clear();
        app.drop_confirmation = None;
        app.drag_files = Some(crate::interactions::DraggedFiles {
            pane: 0,
            index: 0,
            paths: vec!["source/a".into()],
        });
        let _ = app.update_inner(crate::Message::FileDropInto(
            0,
            0,
            1,
            "destination/subfolder".into(),
        ));
        assert!(
            app.highlights
                .pulse(Path::new("destination/subfolder"))
                .is_some()
        );
        assert!(app.highlights.pulse(Path::new("destination")).is_none());
    }
}
