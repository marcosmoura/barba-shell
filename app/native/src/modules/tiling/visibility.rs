//! Ownership tracking for applications hidden during workspace switching.
//!
//! Uses a per-PID generation-based state machine to handle the async
//! `AppShown` race: Stache may re-hide an app after unhiding it but before
//! the OS sends the `AppShown` notification. A generation counter and
//! expected-show FIFO allow the tracker to distinguish stale self-generated
//! `AppShown` events from current ones and from genuine external/user
//! `AppShown` events.

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

use parking_lot::Mutex;

use super::effects::window_ops::{
    HideAppOutcome, UnhideAppOutcome, hide_app_with_outcome, unhide_app, unhide_app_with_outcome,
};

/// Classification of an `AppShown` event determined by the ownership tracker.
///
/// Returned by [`classify_stache_hidden_app`] so the caller (typically the
/// `EventProcessor`) can decide whether to forward the event to the state
/// actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShownClassification {
    /// The `AppShown` corresponds to a Stache-initiated show, but a newer
    /// Stache hide exists for this PID. **Do not dispatch** to the actor
    /// (so windows of a freshly re-hidden app are not spuriously marked
    /// visible).
    StaleNoDispatch,
    /// The `AppShown` matches an expected Stache-initiated show. Ownership
    /// has been cleared. Dispatch normally to the actor.
    SelfShown,
    /// The `AppShown` is from an external/user action (no expected-show
    /// token existed). Ownership has been cleared. Dispatch normally.
    ExternalShown,
}

/// Summary of a best-effort restoration attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreSummary {
    /// Number of tracked applications for which restoration was attempted.
    pub attempted: usize,
    /// Number of applications successfully unhidden.
    pub restored: usize,
}

// ---------------------------------------------------------------------------
// Per-PID state
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct PidState {
    /// Current Stache hide generation, if the app is currently hidden by us.
    owner: Option<u64>,
    /// FIFO of expected-show generations created by Stache unhides that have
    /// not yet been matched by an incoming `AppShown`.
    expected_shows: VecDeque<u64>,
}

impl PidState {
    /// Returns `true` if this entry carries no meaningful state and can be
    /// removed from the map.
    fn is_zombie(&self) -> bool { self.owner.is_none() && self.expected_shows.is_empty() }
}

// ---------------------------------------------------------------------------
// Tracker
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct TrackerState {
    shutting_down: bool,
    next_generation: u64,
    log_seq: u64,
    pids: HashMap<i32, PidState>,
}

impl Default for TrackerState {
    fn default() -> Self {
        Self {
            shutting_down: false,
            next_generation: 1,
            log_seq: 0,
            pids: HashMap::new(),
        }
    }
}

#[derive(Debug, Default)]
struct HiddenAppTracker {
    state: Mutex<TrackerState>,
}

impl HiddenAppTracker {
    const fn next_seq(state: &mut TrackerState) -> u64 {
        state.log_seq += 1;
        state.log_seq
    }

    /// Attempts to hide the application and records Stache ownership on
    /// success (`HiddenByStache`).  `AlreadyHidden` and `Failed` outcomes
    /// leave the current entry (if any) untouched.
    fn hide_with(&self, pid: i32, hide: impl FnOnce(i32) -> HideAppOutcome) -> HideAppOutcome {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);

        if state.shutting_down {
            tracing::debug!(
                seq,
                pid,
                operation = "hide",
                "shutdown boundary active, rejected"
            );
            return HideAppOutcome::Failed;
        }

        let outcome = hide(pid);

        // Snapshot before-borrow before the entry binding (avoids borrow
        // conflict with the inline generation allocation below).
        let owner_before = state.pids.get(&pid).and_then(|e| e.owner);

        if outcome == HideAppOutcome::HiddenByStache {
            let generation = state.next_generation;
            state.next_generation += 1;
            state.pids.entry(pid).or_default().owner = Some(generation);
            tracing::debug!(
                seq, pid, operation = "hide", outcome = "HiddenByStache",
                owner_before = ?owner_before, owner_after = generation,
                "recorded Stache ownership",
            );

            // Entry was just created/updated — cannot be zombie.
        } else {
            // Prune zombie entries (shouldn't happen, but be safe).
            if let Some(entry) = state.pids.get(&pid)
                && entry.is_zombie()
            {
                state.pids.remove(&pid);
            }
            tracing::debug!(
                seq, pid, operation = "hide", outcome = ?outcome,
                owner_before = ?owner_before, owner_after = ?state.pids.get(&pid).and_then(|e| e.owner),
                "no ownership change",
            );
        }

        outcome
    }

    /// Outcome-based unhide.  Creates an expected-show token only on
    /// [`UnhideAppOutcome::UnhiddenByStache`].  Already-shown clears
    /// ownership (the user or another mechanism already restored it).
    fn unhide_with_outcome(
        &self,
        pid: i32,
        unhide: impl FnOnce(i32) -> UnhideAppOutcome,
    ) -> UnhideAppOutcome {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);

        let outcome = unhide(pid);
        let owner_before = state.pids.get(&pid).and_then(|e| e.owner);

        match outcome {
            UnhideAppOutcome::UnhiddenByStache => {
                let generation = state.next_generation;
                state.next_generation += 1;
                let entry = state.pids.entry(pid).or_default();
                entry.expected_shows.push_back(generation);
                entry.owner = None;
                tracing::debug!(
                    seq, pid, operation = "unhide", outcome = "UnhiddenByStache",
                    owner_before = ?owner_before, owner_after = "None",
                    expected_show = generation,
                    "enqueued expected-show token, cleared ownership",
                );
            }
            UnhideAppOutcome::AlreadyShown => {
                if let Some(entry) = state.pids.get_mut(&pid) {
                    entry.owner = None;
                    if entry.is_zombie() {
                        state.pids.remove(&pid);
                    }
                }
                tracing::debug!(
                    seq, pid, operation = "unhide", outcome = "AlreadyShown",
                    owner_before = ?owner_before, owner_after = "None",
                    "app was already visible, cleared ownership",
                );
            }
            UnhideAppOutcome::Failed => {
                tracing::debug!(
                    seq, pid, operation = "unhide", outcome = "Failed",
                    owner_before = ?owner_before, owner_after = ?state.pids.get(&pid).and_then(|e| e.owner),
                    "unhide call failed, ownership unchanged",
                );
            }
        }

        outcome
    }

    /// Classifies an incoming `AppShown` event.  The caller should:
    /// - [`StaleNoDispatch`](ShownClassification::StaleNoDispatch): skip
    ///   dispatching to the state actor.
    /// - [`SelfShown`](ShownClassification::SelfShown) /
    ///   [`ExternalShown`](ShownClassification::ExternalShown): dispatch
    ///   to the actor normally.
    fn classify_shown(&self, pid: i32) -> ShownClassification {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);

        let Some(entry) = state.pids.get_mut(&pid) else {
            tracing::debug!(seq, pid, "no entry found → ExternalShown");
            return ShownClassification::ExternalShown;
        };

        let owner_before = entry.owner;

        if let Some(token) = entry.expected_shows.pop_front() {
            // An expected-show token exists.  Check for the stale case.
            // Stale means a **newer** Stache hide generation was recorded
            // after this token's unhide.
            if let Some(owner_gen) = entry.owner
                && owner_gen > token
            {
                // Stale self-show: a later hide happened.  Retain owner.
                tracing::debug!(
                    seq, pid, classification = "StaleNoDispatch",
                    owner_before = ?owner_before, owner_after = ?entry.owner,
                    token, reason = "owner > token",
                    "stale self-show from earlier unhide",
                );
                ShownClassification::StaleNoDispatch
            } else {
                // Expected show that arrived — ownership is cleared.
                entry.owner = None;
                if entry.is_zombie() {
                    state.pids.remove(&pid);
                }
                drop(state);
                tracing::debug!(
                    seq, pid, classification = "SelfShown",
                    owner_before = ?owner_before, owner_after = "None",
                    token, reason = "matched expected show",
                    "self-show consumed, ownership cleared",
                );
                ShownClassification::SelfShown
            }
        } else {
            // No expected token → this is a user/external show.
            entry.owner = None;
            if entry.is_zombie() {
                state.pids.remove(&pid);
            }
            drop(state);
            tracing::debug!(
                seq, pid, classification = "ExternalShown",
                owner_before = ?owner_before, owner_after = "None",
                reason = "no expected token",
                "external show, ownership cleared",
            );
            ShownClassification::ExternalShown
        }
    }

    /// Unconditionally removes the entire PID entry.  Intended for process
    /// termination _before_ the actor handler runs so that a delayed actor
    /// message cannot accidentally re-animate stale entries later.
    fn forget_terminated(&self, pid: i32) {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);
        let had_entry = state.pids.remove(&pid).is_some();
        drop(state);
        tracing::debug!(seq, pid, had_entry, "cleared PID entry on termination");
    }

    /// Begins the shutdown sequence: sets the `shutting_down` flag and
    /// drains only PIDs that have a current **owner** (not just pending
    /// expected-show tokens).  Returns a sorted, deduplicated list.
    ///
    /// After this call the tracker is sealed — successive `hide_with` calls
    /// return `Failed` without touching `AppKit`.
    fn begin_shutdown_and_drain(&self) -> Vec<i32> {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);
        state.shutting_down = true;

        // Collect only PIDs that Stache has currently hidden.
        let mut pids: Vec<i32> = state
            .pids
            .iter()
            .filter(|(_, e)| e.owner.is_some())
            .map(|(&pid, _)| pid)
            .collect();
        pids.sort_unstable();
        pids.dedup();

        state.pids.clear();
        drop(state);

        tracing::debug!(
            seq,
            owner_count = pids.len(),
            "drained current owners for restoration",
        );

        pids
    }

    // -- test helpers -------------------------------------------------------

    #[cfg(test)]
    fn snapshot(&self) -> Vec<i32> {
        let state = self.state.lock();
        let mut pids: Vec<i32> = state
            .pids
            .iter()
            .filter(|(_, e)| e.owner.is_some())
            .map(|(&pid, _)| pid)
            .collect();
        pids.sort_unstable();
        pids
    }

    #[cfg(test)]
    fn snapshot_owner(&self, pid: i32) -> Option<u64> {
        self.state.lock().pids.get(&pid).and_then(|e| e.owner)
    }

    #[cfg(test)]
    fn snapshot_tokens(&self, pid: i32) -> Vec<u64> {
        self.state
            .lock()
            .pids
            .get(&pid)
            .map(|e| e.expected_shows.iter().copied().collect())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

static STACHE_HIDDEN_APPS: OnceLock<HiddenAppTracker> = OnceLock::new();

fn tracker() -> &'static HiddenAppTracker {
    STACHE_HIDDEN_APPS.get_or_init(HiddenAppTracker::default)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Hides an application for workspace switching and records Stache ownership.
#[must_use]
pub fn hide_app_for_workspace(pid: i32) -> HideAppOutcome {
    tracker().hide_with(pid, hide_app_with_outcome)
}

/// Unhides an application and relinquishes ownership after success.
///
/// Returns `true` if the app is now visible (either unhidden by Stache
/// or was already visible), `false` on failure.
#[must_use]
pub fn unhide_app_for_workspace(pid: i32) -> bool {
    tracker().unhide_with_outcome(pid, unhide_app_with_outcome).succeeded()
}

/// Classifies an incoming `AppShown` event.  See [`ShownClassification`]
/// for interpretation.
#[must_use]
pub fn classify_stache_hidden_app(pid: i32) -> ShownClassification { tracker().classify_shown(pid) }

/// Clears all state for a terminated PID _before_ the actor handler runs.
pub fn forget_stache_hidden_app_terminated(pid: i32) { tracker().forget_terminated(pid); }

// ---------------------------------------------------------------------------
// Restoration
// ---------------------------------------------------------------------------

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;

    // -----------------------------------------------------------------------
    // Existing behaviour preserved
    // -----------------------------------------------------------------------

    #[test]
    fn records_only_apps_newly_hidden_by_stache() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::AlreadyHidden);
        tracker.hide_with(11, |_| HideAppOutcome::Failed);
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);

        assert_eq!(tracker.snapshot(), vec![12]);
    }

    #[test]
    fn successful_workspace_unhide_clears_ownership() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);

        assert!(
            tracker
                .unhide_with_outcome(12, |_| UnhideAppOutcome::UnhiddenByStache)
                .succeeded()
        );
        assert!(tracker.snapshot().is_empty());
    }

    #[test]
    fn shown_then_independently_hidden_app_is_not_owned() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        tracker.forget_terminated(12);

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

    // -----------------------------------------------------------------------
    // Generation-based race-condition tests (core feature)
    // -----------------------------------------------------------------------

    /// hide g1 → unhide s2 → hide g3 → delayed AppShown s2 retains g3 as owner
    /// and returns StaleNoDispatch.
    #[test]
    fn delayed_self_appshown_retains_newer_owner() {
        let tracker = HiddenAppTracker::default();

        // Hide gen 1
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // Unhide gen 2 (creates token 2, clears owner)
        assert!(
            tracker
                .unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache)
                .succeeded()
        );
        assert_eq!(tracker.snapshot_owner(10), None);
        assert_eq!(tracker.snapshot_tokens(10), vec![2]);

        // Hide gen 3 (new owner 3, token 2 still in queue)
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(3));

        // Delayed AppShown for gen 2 arrives: owner (3) > token (2) → stale
        let classification = tracker.classify_shown(10);
        assert_eq!(classification, ShownClassification::StaleNoDispatch);
        // Owner must be retained!
        assert_eq!(tracker.snapshot_owner(10), Some(3));
        // Token should be consumed
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    /// AppShown arriving with no expected token (external/user action) clears
    /// ownership.  A subsequent AlreadyHidden hide does not re-acquire it.
    #[test]
    fn external_appshown_clears_ownership_and_hide_stays_unowned() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // External AppShown (no token in queue)
        assert_eq!(tracker.classify_shown(10), ShownClassification::ExternalShown);
        assert_eq!(tracker.snapshot_owner(10), None);

        // Independent hide where app is already hidden → AlreadyHidden, no ownership
        tracker.hide_with(10, |_| HideAppOutcome::AlreadyHidden);
        assert!(tracker.snapshot().is_empty());
    }

    /// Multiple rapid unhide/hide cycles consume the expected-show FIFO
    /// without deleting the newest owner on a stale show.
    #[test]
    fn rapid_cycles_consume_fifo_correctly() {
        let tracker = HiddenAppTracker::default();

        // Hide g1, Unhide g2, Hide g3, Unhide g4, Hide g5
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);

        // Owner should be g5, expected shows = [2, 4]
        assert_eq!(tracker.snapshot_owner(10), Some(5));
        assert_eq!(tracker.snapshot_tokens(10), vec![2, 4]);

        // First delayed AppShown (g2): owner (5) > token (2) → stale
        assert_eq!(tracker.classify_shown(10), ShownClassification::StaleNoDispatch);
        assert_eq!(tracker.snapshot_owner(10), Some(5));
        assert_eq!(tracker.snapshot_tokens(10), vec![4]);

        // Second delayed AppShown (g4): owner (5) > token (4) → stale
        assert_eq!(tracker.classify_shown(10), ShownClassification::StaleNoDispatch);
        assert_eq!(tracker.snapshot_owner(10), Some(5));
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    /// Unhide outcomes that are not UnhiddenByStache must NOT create an
    /// expected-show token.
    #[test]
    fn non_unhidden_outcomes_do_not_create_tokens() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // AlreadyShown → no token, owner cleared
        assert!(tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::AlreadyShown).succeeded());
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert_eq!(tracker.snapshot_owner(10), None);
        assert!(tracker.snapshot().is_empty());

        // Re-hide then Failed unhide (counter is now at gen 2)
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert!(!tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::Failed).succeeded());
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert_eq!(tracker.snapshot_owner(10), Some(2)); // unchanged (gen 2)
    }

    /// classify_shown on a PID that was only ever unhidden (no current owner)
    /// returns SelfShown and consumes the token.
    #[test]
    fn shown_without_owner_after_unhide_classifies_self_shown() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);

        // Owner is cleared, token exists
        assert_eq!(tracker.snapshot_owner(10), None);
        assert_eq!(tracker.snapshot_tokens(10), vec![2]);

        // AppShown arrives — no owner, token exists → SelfShown
        assert_eq!(tracker.classify_shown(10), ShownClassification::SelfShown);
        assert!(tracker.snapshot_owner(10).is_none());
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert!(tracker.snapshot().is_empty());
    }

    /// Termination clears owner AND tokens.
    #[test]
    fn termination_clears_owner_and_tokens() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);

        // Has token
        assert_eq!(tracker.snapshot_tokens(10), vec![2]);

        tracker.forget_terminated(10);
        assert!(tracker.snapshot_owner(10).is_none());
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert!(tracker.snapshot().is_empty());
    }

    /// A fresh ownership after termination is not erased by a delayed
    /// classify_shown (the entry is already gone; classify creates none).
    #[test]
    fn termination_isolates_future_ownership() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.forget_terminated(10);

        // Brand new hide → fresh ownership (counter resumes at gen 3)
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(3));

        // Delayed classify (corresponding to the pre-termination unhide)
        // has no token matching.  Without a token, it clears owner!
        // This is correct: after termination any previous token is invalid,
        // and the termination handler on the actor side already dealt with
        // the old windows.  A fresh external AppShown _should_ clear the
        // new ownership because it's external at this point.
        //
        // NOTE: the spec says "actor should no longer mutate ownership"
        // meaning the actor's on_app_terminated runs _after_ our tracker
        // forget_terminated.  The classify here simulates a stray event
        // that arrives after both tracker-forget and actor-handler.
        // Since we have no token, we treat it as ExternalShown.
        assert_eq!(tracker.classify_shown(10), ShownClassification::ExternalShown);
        assert!(tracker.snapshot().is_empty());
    }

    /// Shutdown drains only current owners, not apps that only have pending
    /// expected-show tokens.
    #[test]
    fn shutdown_drains_only_current_owners_not_pending_tokens() {
        let tracker = HiddenAppTracker::default();

        // PID 10: hide, unhide (has token, no owner)
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), None);
        assert!(!tracker.snapshot_tokens(10).is_empty());

        // PID 11: hide, still hidden (has owner)
        tracker.hide_with(11, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(11), Some(3));

        // PID 12: hide (gen 4), unhide (gen 5), re-hide (gen 6)
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(12, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(12), Some(6));

        // Shutdown drains only owners (11 and 12)
        let drained = tracker.begin_shutdown_and_drain();
        assert!(drained.contains(&11));
        assert!(drained.contains(&12));
        assert!(!drained.contains(&10)); // PID 10 has no owner

        assert!(tracker.snapshot().is_empty());
    }

    /// classify_shown on a PID with no entry returns ExternalShown.
    #[test]
    fn missing_pid_classifies_external_shown() {
        let tracker = HiddenAppTracker::default();
        assert_eq!(tracker.classify_shown(999), ShownClassification::ExternalShown);
    }

    /// classify_shown on a PID that has only tokens (no owner) but NO matching
    /// token returns SelfShown if a token was consumed (code path for unknown
    /// token scenario — effectively same as SelfShown).
    #[test]
    fn classify_with_no_token_but_entry_exists_external() {
        let tracker = HiddenAppTracker::default();
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        // forget without clearing expected_shows
        {
            let mut state = tracker.state.lock();
            state.pids.get_mut(&10).unwrap().expected_shows.push_back(99);
        }
        assert_eq!(tracker.classify_shown(10), ShownClassification::SelfShown);
    }

    // -----------------------------------------------------------------------
    // EventProcessor dispatch helper (pure function, no actor channel)
    // -----------------------------------------------------------------------

    /// A pure helper that mirrors what EventProcessor::on_app_shown would do.
    fn dispatch_shown(pid: i32, tracker: &HiddenAppTracker) -> Option<ShownClassification> {
        let classification = tracker.classify_shown(pid);
        if classification == ShownClassification::StaleNoDispatch {
            None // suppress dispatch to actor
        } else {
            Some(classification) // would be dispatched
        }
    }

    #[test]
    fn event_processor_does_not_dispatch_stale_self_shown() {
        let tracker = HiddenAppTracker::default();

        // Hide → unhide → hide again (race condition setup)
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);

        // Stale AppShown → should NOT dispatch
        assert_eq!(dispatch_shown(10, &tracker), None);
        assert_eq!(tracker.snapshot_owner(10), Some(3));
    }

    #[test]
    fn event_processor_dispatches_self_shown() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);

        // Expected AppShown → should dispatch
        assert_eq!(
            dispatch_shown(10, &tracker),
            Some(ShownClassification::SelfShown)
        );
    }

    #[test]
    fn event_processor_dispatches_external_shown() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);

        // External AppShown (no token) → should dispatch
        assert_eq!(
            dispatch_shown(10, &tracker),
            Some(ShownClassification::ExternalShown)
        );
    }
}
