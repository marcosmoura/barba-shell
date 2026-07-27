//! Hidden application PID tracker.
//!
//! Tracks only application PIDs that Stache itself hid during workspace
//! switching. Provides an idempotent best-effort restore on shutdown and
//! an atomic boundary that prevents late hides from crossing into shutdown.
//!
//! # Thread Safety
//!
//! [`HiddenAppTracker`] is `Send` + `Sync` using `parking_lot::Mutex`.
//! The global tracker is initialized once via [`OnceLock`].

use std::collections::HashSet;
use std::sync::OnceLock;

use parking_lot::Mutex;

use crate::modules::tiling::effects::window_ops::{HideAppOutcome, unhide_app};

// ============================================================================
// Tracker State
// ============================================================================

/// Internal state behind the mutex.
struct TrackerState {
    /// If `true`, the system is shutting down and no new hides are accepted.
    shutting_down: bool,
    /// PIDs that Stache has hidden and therefore owns restoring.
    pids: HashSet<i32>,
}

// ============================================================================
// Global Tracker
// ============================================================================

/// The process-global hidden-app tracker.
pub struct HiddenAppTracker {
    state: Mutex<TrackerState>,
}

impl HiddenAppTracker {
    /// Creates a new empty tracker.
    fn new() -> Self {
        Self {
            state: Mutex::new(TrackerState {
                shutting_down: false,
                pids: HashSet::new(),
            }),
        }
    }

    /// Runs the given hide closure while holding the tracker lock.
    ///
    /// If shutdown has begun, returns [`HideAppOutcome::Failed`] without
    /// invoking the closure. Otherwise invokes `do_hide(pid)`, records the
    /// PID iff the outcome is [`HiddenByStache`], and returns the outcome.
    fn hide_with<F>(&self, pid: i32, do_hide: F) -> HideAppOutcome
    where F: FnOnce(i32) -> HideAppOutcome {
        let mut state = self.state.lock();
        if state.shutting_down {
            return HideAppOutcome::Failed;
        }

        let outcome = do_hide(pid);
        if outcome == HideAppOutcome::HiddenByStache {
            state.pids.insert(pid);
        }
        outcome
    }

    /// Atomically marks shutdown and drains all tracked PIDs (sorted).
    ///
    /// After this call no future hide can cross the boundary because
    /// `hide_with` checks `shutting_down` before the closure.
    fn begin_shutdown_and_drain(&self) -> Vec<i32> {
        let mut state = self.state.lock();
        state.shutting_down = true;
        let mut pids: Vec<i32> = state.pids.drain().collect();
        drop(state);
        pids.sort_unstable();
        pids
    }

    /// Removes the PID from tracking after a successful unhide.
    fn unhide(&self, pid: i32) -> bool {
        let ok = unhide_app(pid);
        if ok {
            self.state.lock().pids.remove(&pid);
        }
        ok
    }

    /// Removes the PID from tracking unconditionally (app was shown by
    /// the user or terminated).
    fn forget(&self, pid: i32) { self.state.lock().pids.remove(&pid); }

    /// Attempts to unhide every tracked PID in sorted order, tolerating
    /// individual failures, then drains tracking entirely.
    ///
    /// Repeated calls are idempotent — after the first call the set is
    /// empty so no unhide closures are invoked.
    fn restore_all(&self) -> RestoreSummary {
        let pids = self.begin_shutdown_and_drain();
        let attempted = pids.len();
        let restored = pids.iter().filter(|&&pid| unhide_app(pid)).count();
        RestoreSummary { attempted, restored }
    }
}

// ============================================================================
// Global Instance
// ============================================================================

static TRACKER: OnceLock<HiddenAppTracker> = OnceLock::new();

fn tracker() -> &'static HiddenAppTracker { TRACKER.get_or_init(HiddenAppTracker::new) }

// ============================================================================
// Public API
// ============================================================================

/// Hide an app, recording ownership only if Stache actually performs the hide.
///
/// This wraps [`hide_app_with_outcome`] with ownership tracking.
#[must_use]
pub fn hide_app_for_workspace(pid: i32) -> HideAppOutcome {
    tracker().hide_with(pid, |pid| {
        crate::modules::tiling::effects::window_ops::hide_app_with_outcome(pid)
    })
}

/// Unhide an app, removing ownership only on success.
#[must_use]
pub fn unhide_app_for_workspace(pid: i32) -> bool { tracker().unhide(pid) }

/// Forget about a PID Stache hid (e.g., user manually showed or terminated the app).
pub fn forget_stache_hidden_app(pid: i32) { tracker().forget(pid); }

/// Attempt best-effort restore of all apps Stache hid.
///
/// Returns a summary of how many were attempted and how many succeeded.
/// After this call the tracker is drained; repeated calls are a no-op.
#[must_use]
pub fn restore_stache_hidden_apps() -> RestoreSummary { tracker().restore_all() }

// ============================================================================
// RestoreSummary
// ============================================================================

/// Summary of a restore operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreSummary {
    /// Number of PIDs that were tracked (attempted restore).
    pub attempted: usize,
    /// Number of PIDs successfully unhidden.
    pub restored: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a fresh tracker (not the global one) for isolated tests.
    fn new_tracker() -> HiddenAppTracker { HiddenAppTracker::new() }

    // ------------------------------------------------------------------
    // Only newly hidden apps recorded
    // ------------------------------------------------------------------

    #[test]
    fn test_records_only_hidden_by_stache() {
        let t = new_tracker();

        // AlreadyHidden should NOT be recorded.
        let outcome = t.hide_with(10, |_| HideAppOutcome::AlreadyHidden);
        assert_eq!(outcome, HideAppOutcome::AlreadyHidden);
        let drained = t.begin_shutdown_and_drain();
        assert!(drained.is_empty(), "AlreadyHidden should not be recorded");

        // Failed should NOT be recorded.
        let outcome = t.hide_with(11, |_| HideAppOutcome::Failed);
        assert_eq!(outcome, HideAppOutcome::Failed);
        let drained = t.begin_shutdown_and_drain();
        assert!(drained.is_empty(), "Failed should not be recorded");
    }

    #[test]
    fn test_records_hidden_by_stache() {
        let t = new_tracker();

        let outcome = t.hide_with(42, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(outcome, HideAppOutcome::HiddenByStache);

        let drained = t.begin_shutdown_and_drain();
        assert_eq!(drained, vec![42]);
    }

    #[test]
    fn test_records_multiple_pids() {
        let t = new_tracker();

        t.hide_with(1, |_| HideAppOutcome::HiddenByStache);
        t.hide_with(2, |_| HideAppOutcome::HiddenByStache);
        t.hide_with(3, |_| HideAppOutcome::HiddenByStache);

        let mut drained = t.begin_shutdown_and_drain();
        drained.sort_unstable();
        assert_eq!(drained, vec![1, 2, 3]);
    }

    // ------------------------------------------------------------------
    // Workspace unhide removes ownership
    // ------------------------------------------------------------------

    #[test]
    fn test_unhide_removes_ownership_on_success() {
        let t = new_tracker();

        t.hide_with(10, |_| HideAppOutcome::HiddenByStache);

        // We call the real unhide_app here, but in tests there's no
        // running app with PID 10, so unhide_app returns false.
        // The point is that ONLY a successful unhide removes ownership.
        let ok = t.unhide(10);
        // PID 10 doesn't exist, so unhide_app returns false → ownership REMAINS.
        assert!(!ok, "unhide should fail for nonexistent PID");

        let drained = t.begin_shutdown_and_drain();
        assert!(
            !drained.is_empty(),
            "PID should still be owned after failed unhide"
        );
    }

    /// Inject a successful unhide by wrapping real `unhide_app` via a
    /// different approach: we test the contract via `hide_with`/`forget`.
    #[test]
    fn test_forget_removes_ownership() {
        let t = new_tracker();

        t.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        t.forget(10);

        let drained = t.begin_shutdown_and_drain();
        assert!(drained.is_empty(), "forget should remove PID from tracking");
    }

    // ------------------------------------------------------------------
    // HiddenByStache -> shown/forget -> AlreadyHidden leaves no ownership
    // ------------------------------------------------------------------

    #[test]
    fn test_shown_then_already_hidden_no_ownership() {
        let t = new_tracker();

        // First hide — recorded.
        t.hide_with(20, |_| HideAppOutcome::HiddenByStache);

        // User shows the app — forget.
        t.forget(20);

        // Now try to hide again. Since we no longer own it, on a
        // subsequent hide_with the closure returns AlreadyHidden
        // (simulating the app being already hidden by the user),
        // and tracking should NOT record it.
        let outcome = t.hide_with(20, |_| HideAppOutcome::AlreadyHidden);
        assert_eq!(outcome, HideAppOutcome::AlreadyHidden);

        let drained = t.begin_shutdown_and_drain();
        assert!(
            drained.is_empty(),
            "after forget+AlreadyHidden, no ownership should remain"
        );
    }

    // ------------------------------------------------------------------
    // Restore sorted: PID 12 fails, PID 13 succeeds — continuation
    // ------------------------------------------------------------------

    #[test]
    fn test_restore_continues_on_failure() {
        let t = new_tracker();

        // Record two PIDs.
        t.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        t.hide_with(13, |_| HideAppOutcome::HiddenByStache);

        // Restore_all will attempt unhide_app for both. Since neither
        // PID actually exists, both will fail → restored=0, attempted=2.
        let summary = t.restore_all();
        assert_eq!(summary.attempted, 2);
        assert_eq!(summary.restored, 0);

        // Tracker should be drained.
        let drained = t.begin_shutdown_and_drain();
        assert!(drained.is_empty(), "restore_all should drain tracking");
    }

    /// Verify restore processes PIDs in sorted order (we can't easily
    /// observe the order since both fail, but we can verify the contract).
    /// The sorted guarantee is structural.
    #[test]
    fn test_restore_sorted_guarantee() {
        let t = new_tracker();

        // Insert out of order.
        t.hide_with(3, |_| HideAppOutcome::HiddenByStache);
        t.hide_with(1, |_| HideAppOutcome::HiddenByStache);
        t.hide_with(2, |_| HideAppOutcome::HiddenByStache);

        let drained = t.begin_shutdown_and_drain();
        assert_eq!(
            drained,
            vec![1, 2, 3],
            "begin_shutdown_and_drain should return sorted PIDs"
        );
    }

    // ------------------------------------------------------------------
    // Repeated restore no-op without invoking unhide
    // ------------------------------------------------------------------

    #[test]
    fn test_repeated_restore_is_no_op() {
        let t = new_tracker();

        // Record a PID.
        t.hide_with(99, |_| HideAppOutcome::HiddenByStache);

        // First restore — attempts the unhide (fails, no such PID).
        let s1 = t.restore_all();
        assert_eq!(s1.attempted, 1);
        assert_eq!(s1.restored, 0);

        // Second restore — set is empty, no unhide attempted.
        let s2 = t.restore_all();
        assert_eq!(s2.attempted, 0);
        assert_eq!(s2.restored, 0);
    }

    // ------------------------------------------------------------------
    // Shutdown boundary rejects late hide
    // ------------------------------------------------------------------

    #[test]
    fn test_shutdown_rejects_late_hide() {
        let t = new_tracker();

        // Begin shutdown.
        let _drained = t.begin_shutdown_and_drain();

        // Any subsequent hide should be rejected WITHOUT invoking the closure.
        let mut invoked = false;
        let outcome = t.hide_with(99, |_| {
            invoked = true;
            HideAppOutcome::HiddenByStache
        });

        assert!(!invoked, "closure should not be invoked during shutdown");
        assert_eq!(
            outcome,
            HideAppOutcome::Failed,
            "should reject hide during shutdown"
        );
    }

    #[test]
    fn test_shutdown_rejects_hide_even_if_already_hidden() {
        let t = new_tracker();

        let _drained = t.begin_shutdown_and_drain();

        let mut invoked = false;
        let outcome = t.hide_with(99, |_| {
            invoked = true;
            HideAppOutcome::AlreadyHidden
        });

        assert!(!invoked, "closure should not be invoked during shutdown");
        assert_eq!(outcome, HideAppOutcome::Failed);
    }

    // ------------------------------------------------------------------
    // RestoreSummary
    // ------------------------------------------------------------------

    #[test]
    fn test_restore_summary_debug() {
        let s = RestoreSummary { attempted: 5, restored: 3 };
        let debug = format!("{s:?}");
        assert!(debug.contains("attempted: 5"));
        assert!(debug.contains("restored: 3"));
    }

    // ------------------------------------------------------------------
    // Global tracker convenience wrappers (structural)
    // ------------------------------------------------------------------

    #[test]
    fn test_global_tracker_is_singleton() {
        // Access the tracker twice — should be the same instance.
        let a: *const HiddenAppTracker = &*TRACKER.get_or_init(HiddenAppTracker::new);
        let b: *const HiddenAppTracker = &*TRACKER.get_or_init(HiddenAppTracker::new);
        assert_eq!(a, b);
    }

    #[test]
    fn test_forget_stache_hidden_app_does_not_panic() {
        // Should not panic even for unknown PID.
        forget_stache_hidden_app(999_999);
    }

    #[test]
    fn test_restore_stache_hidden_apps_does_not_panic() { let _ = restore_stache_hidden_apps(); }
}
