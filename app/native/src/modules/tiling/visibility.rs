//! Ownership tracking for applications hidden during workspace switching.

use std::collections::HashSet;
use std::sync::OnceLock;

use parking_lot::Mutex;

use super::effects::window_ops::{HideAppOutcome, hide_app_with_outcome, unhide_app};

/// Summary of a best-effort restoration attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreSummary {
    /// Number of tracked applications for which restoration was attempted.
    pub attempted: usize,
    /// Number of applications successfully unhidden.
    pub restored: usize,
}

#[derive(Debug, Default)]
struct TrackerState {
    shutting_down: bool,
    pids: HashSet<i32>,
}

#[derive(Debug, Default)]
struct HiddenAppTracker {
    state: Mutex<TrackerState>,
}

impl HiddenAppTracker {
    fn hide_with(&self, pid: i32, hide: impl FnOnce(i32) -> HideAppOutcome) -> HideAppOutcome {
        let mut state = self.state.lock();
        if state.shutting_down {
            return HideAppOutcome::Failed;
        }

        let outcome = hide(pid);
        if outcome == HideAppOutcome::HiddenByStache {
            state.pids.insert(pid);
        }
        outcome
    }

    fn unhide_with(&self, pid: i32, unhide: impl FnOnce(i32) -> bool) -> bool {
        let unhidden = unhide(pid);
        if unhidden {
            self.state.lock().pids.remove(&pid);
        }
        unhidden
    }

    fn forget(&self, pid: i32) { self.state.lock().pids.remove(&pid); }

    fn begin_shutdown_and_drain(&self) -> Vec<i32> {
        let mut state = self.state.lock();
        state.shutting_down = true;
        let mut pids: Vec<_> = state.pids.drain().collect();
        drop(state);
        pids.sort_unstable();
        pids
    }

    #[cfg(test)]
    fn snapshot(&self) -> Vec<i32> {
        let mut pids: Vec<_> = self.state.lock().pids.iter().copied().collect();
        pids.sort_unstable();
        pids
    }
}

static STACHE_HIDDEN_APPS: OnceLock<HiddenAppTracker> = OnceLock::new();

fn tracker() -> &'static HiddenAppTracker {
    STACHE_HIDDEN_APPS.get_or_init(HiddenAppTracker::default)
}

/// Hides an application for workspace switching and records Stache ownership.
#[must_use]
pub fn hide_app_for_workspace(pid: i32) -> HideAppOutcome {
    tracker().hide_with(pid, hide_app_with_outcome)
}

/// Unhides an application and relinquishes ownership after success.
#[must_use]
pub fn unhide_app_for_workspace(pid: i32) -> bool { tracker().unhide_with(pid, unhide_app) }

/// Relinquishes ownership after the application is shown or terminates.
pub fn forget_stache_hidden_app(pid: i32) { tracker().forget(pid); }

fn restore_with(
    hidden_apps: &HiddenAppTracker,
    mut unhide: impl FnMut(i32) -> bool,
) -> RestoreSummary {
    let pids = hidden_apps.begin_shutdown_and_drain();
    let attempted = pids.len();
    let restored = pids.into_iter().filter(|&pid| unhide(pid)).count();
    RestoreSummary { attempted, restored }
}

/// Best-effort restores every application hidden by Stache.
#[must_use]
pub fn restore_stache_hidden_apps() -> RestoreSummary { restore_with(tracker(), unhide_app) }

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;

    #[test]
    fn records_only_apps_newly_hidden_by_stache() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::AlreadyHidden);
        tracker.hide_with(11, |_| HideAppOutcome::Failed);
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);

        assert_eq!(tracker.snapshot(), vec![12]);
    }

    #[test]
    fn successful_workspace_unhide_forgets_owned_hide() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);

        assert!(tracker.unhide_with(12, |_| true));
        assert!(tracker.snapshot().is_empty());
    }

    #[test]
    fn shown_then_independently_hidden_app_is_not_owned() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        tracker.forget(12);

        tracker.hide_with(12, |_| HideAppOutcome::AlreadyHidden);

        assert!(tracker.snapshot().is_empty());
    }

    #[test]
    fn restore_continues_after_failure_and_drains_tracking() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(13, |_| HideAppOutcome::HiddenByStache);
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        let attempted = RefCell::new(Vec::new());

        let summary = restore_with(&tracker, |pid| {
            attempted.borrow_mut().push(pid);
            pid == 13
        });

        assert_eq!(attempted.into_inner(), vec![12, 13]);
        assert_eq!(summary, RestoreSummary { attempted: 2, restored: 1 });
        assert!(tracker.snapshot().is_empty());
    }

    #[test]
    fn repeated_restore_is_a_no_op() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        let _ = restore_with(&tracker, |_| true);

        assert_eq!(
            restore_with(&tracker, |_| panic!("empty tracker must not call unhide")),
            RestoreSummary { attempted: 0, restored: 0 }
        );
    }

    #[test]
    fn shutdown_boundary_rejects_late_hide_without_calling_os() {
        let tracker = HiddenAppTracker::default();
        let _ = restore_with(&tracker, |_| true);

        let outcome = tracker.hide_with(12, |_| panic!("late hide must not reach AppKit"));

        assert_eq!(outcome, HideAppOutcome::Failed);
        assert!(tracker.snapshot().is_empty());
    }

    #[test]
    fn in_flight_hide_holds_shutdown_lock_until_ownership_is_recorded() {
        let tracker = Arc::new(HiddenAppTracker::default());
        let hide_entered = Arc::new(Barrier::new(2));
        let release_hide = Arc::new(Barrier::new(2));

        let hide_tracker = Arc::clone(&tracker);
        let hide_entered_worker = Arc::clone(&hide_entered);
        let release_hide_worker = Arc::clone(&release_hide);
        let hide_thread = thread::spawn(move || {
            hide_tracker.hide_with(12, |_| {
                hide_entered_worker.wait();
                release_hide_worker.wait();
                HideAppOutcome::HiddenByStache
            })
        });
        hide_entered.wait();

        assert!(
            tracker.state.try_lock().is_none(),
            "shutdown must not acquire the tracker while a hide is in flight"
        );

        release_hide.wait();

        assert_eq!(
            hide_thread.join().expect("hide thread should not panic"),
            HideAppOutcome::HiddenByStache
        );
        assert_eq!(restore_with(&tracker, |_| true), RestoreSummary {
            attempted: 1,
            restored: 1
        });
    }
}
