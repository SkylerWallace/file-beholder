//! Shared keyed tab/panel motion.
pub use sk_iced::animation::keyed::*;
#[cfg(test)]
mod tests {
    const ON: crate::motion::Policy = crate::motion::Policy {
        system_enabled: true,
    };
    #[test]
    fn moving_tabs_keeps_model_and_scan_state_and_closing_cancels_immediately() {
        use crate::{App, Message, Tab, browser::Event as ScanEvent};
        use std::sync::{Arc, atomic::Ordering};
        let mut app = App::new("home".into(), "other".into(), vec![]);
        app.motion = ON;
        app.panes[0].tabs.push(Tab::new("moving".into()));
        let tab = &mut app.panes[0].tabs[1];
        tab.generation = 771;
        tab.view_identity = 772;
        tab.busy = true;
        tab.search = "needle".into();
        tab.back.push("previous".into());
        tab.forward.push("next".into());
        tab.selected = Some(3);
        tab.selections.insert(3);
        let (identity, cancel) = (tab.identity, tab.cancel.clone());
        let _ = app.update(Message::MoveTab(0, identity, 1, 0));
        let tab = &app.panes[1].tabs[0];
        assert_eq!(tab.identity, identity);
        assert_eq!(tab.generation, 771);
        assert_eq!(tab.view_identity, 772);
        assert_eq!(tab.search, "needle");
        assert_eq!(tab.selected, Some(3));
        assert!(tab.selections.contains(&3));
        assert_eq!(tab.back, vec![std::path::PathBuf::from("previous")]);
        assert_eq!(tab.forward, vec![std::path::PathBuf::from("next")]);
        assert!(tab.busy && Arc::ptr_eq(&tab.cancel, &cancel) && !cancel.load(Ordering::Relaxed));
        let _ = app.update(Message::Scan(
            0,
            771,
            ScanEvent::Listed(Err("routed".into())),
        ));
        assert_eq!(app.panes[1].error.as_deref(), Some("routed"));
        let _ = app.update(Message::CloseTab(1, 0));
        assert!(cancel.load(Ordering::Relaxed));
        assert!(
            !app.panes[..app.pane_count]
                .iter()
                .flat_map(|p| &p.tabs)
                .any(|tab| tab.identity == identity)
        );
    }
}
