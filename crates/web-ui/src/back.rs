//! Keeps one browser history entry on top while the app can go back, so the browser's back
//! button steps back inside the app before it leaves the page.

/// What to do after the browser moved in its history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveStep {
    /// The browser landed on the app's own entry, so nothing changes.
    Keep,
    /// The browser went back, so the app goes back one step.
    SendBack,
    /// The app has nothing to go back to, so go back in the browser once more.
    LeavePage,
}

/// Remembers whether the browser is on the one history entry that the app added.
#[derive(Debug, Default)]
pub(crate) struct BackHistory {
    on_own_entry: bool,
}

impl BackHistory {
    /// `on_own_entry` is true when the page was reloaded on the app's own entry.
    pub(crate) fn new(on_own_entry: bool) -> Self {
        Self { on_own_entry }
    }

    /// Call after each frame. True when the app's entry must be pushed now.
    pub(crate) fn after_frame(&mut self, can_go_back: bool) -> bool {
        let must_push = can_go_back && !self.on_own_entry;
        self.on_own_entry |= must_push;
        must_push
    }

    /// Call after the browser moved. `landed_on_own_entry` is true when the entry it landed on
    /// is the app's.
    pub(crate) fn after_move(&mut self, landed_on_own_entry: bool, can_go_back: bool) -> MoveStep {
        self.on_own_entry = landed_on_own_entry;
        match (landed_on_own_entry, can_go_back) {
            (true, _) => MoveStep::Keep,
            (false, true) => MoveStep::SendBack,
            (false, false) => MoveStep::LeavePage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_keeps_one_entry_while_the_app_can_go_back() {
        let mut history = BackHistory::default();
        assert!(
            !history.after_frame(false),
            "a fresh app has nothing to go back to"
        );
        assert!(
            history.after_frame(true),
            "the entry is pushed once the app can go back"
        );
        assert!(!history.after_frame(true), "one entry is enough");

        assert_eq!(
            history.after_move(false, true),
            MoveStep::SendBack,
            "back inside the app while it can go back"
        );
        assert!(
            history.after_frame(true),
            "the entry is pushed again while the app can still go back"
        );

        assert_eq!(
            history.after_move(false, false),
            MoveStep::LeavePage,
            "the person went back inside the app themselves, so the entry was stale"
        );

        assert_eq!(
            history.after_move(true, true),
            MoveStep::Keep,
            "forward onto the app's entry changes nothing"
        );
        assert!(
            !history.after_frame(true),
            "forward put the app's entry back on top, so no second one is pushed"
        );

        let mut reloaded = BackHistory::new(true);
        assert!(
            !reloaded.after_frame(true),
            "a reload on the app's entry already has it on top"
        );
    }
}
