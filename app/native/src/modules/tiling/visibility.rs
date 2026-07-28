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
    HideAppOutcome, UnhideAppOutcome, app_is_hidden, hide_app_with_outcome, unhide_app,
    unhide_app_with_outcome,
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
                sequence = seq,
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
                sequence = seq, pid, operation = "hide", outcome = "HiddenByStache",
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
                sequence = seq, pid, operation = "hide", outcome = ?outcome,
                owner_before = ?owner_before, owner_after = ?state.pids.get(&pid).and_then(|e| e.owner),
                "no ownership change",
            );
        }

        outcome
    }

    /// Outcome-based unhide.  Creates an expected-show token only on
    /// [`UnhideAppOutcome::UnhiddenByStache`].  Already-shown preserves
    /// any existing ownership — the caller did not cause a hide→show
    /// transition so state should not change.
    fn unhide_with_outcome(
        &self,
        pid: i32,
        unhide: impl FnOnce(i32) -> UnhideAppOutcome,
    ) -> UnhideAppOutcome {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);

        // ── Shutdown boundary ─────────────────────────────────────────
        if state.shutting_down {
            tracing::debug!(
                sequence = seq,
                pid,
                operation = "unhide",
                "shutdown boundary active, rejected",
            );
            return UnhideAppOutcome::Failed;
        }

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
                    sequence = seq, pid, operation = "unhide", outcome = "UnhiddenByStache",
                    owner_before = ?owner_before, owner_after = ?Option::<u64>::None,
                    expected_show = generation,
                    "enqueued expected-show token, cleared ownership",
                );
            }
            UnhideAppOutcome::AlreadyShown => {
                // Owner is preserved — we did not cause a hide→show
                // transition so ownership state must not change.
                tracing::debug!(
                    sequence = seq, pid, operation = "unhide", outcome = "AlreadyShown",
                    owner_before = ?owner_before, owner_after = ?owner_before,
                    "app was already visible, ownership unchanged",
                );
            }
            UnhideAppOutcome::Failed => {
                tracing::debug!(
                    sequence = seq, pid, operation = "unhide", outcome = "Failed",
                    owner_before = ?owner_before, owner_after = ?state.pids.get(&pid).and_then(|e| e.owner),
                    "unhide call failed, ownership unchanged",
                );
            }
        }

        outcome
    }

    /// Classifies an incoming `AppShown` event.
    ///
    /// The `query` closure is invoked **inside** the tracker mutex so that
    /// the OS hidden-state read is atomic with the classification decision.
    /// This prevents a TOCTOU race where a concurrent `hide_with` records
    /// ownership between the OS query and lock acquisition.
    ///
    /// Policy by OS state (from the `query` result):
    ///
    /// * `Some(false)` — OS confirms visible.  Clear the ENTIRE PID entry
    ///   (owner and ALL stale expected-show tokens), dispatch.  Classification
    ///   is `SelfShown` if any token existed, `ExternalShown` otherwise.
    ///
    /// * `Some(true)` with an owner — OS reports the app as still hidden.
    ///   Classify `StaleNoDispatch`, retain the current owner regardless of
    ///   whether an expected-show token exists.  At most one FIFO token is
    ///   consumed.  This explicitly handles a duplicate `AppShown` arriving
    ///   after the token was already consumed by a prior classification.
    ///
    /// * `Some(true)` with no owner — Do not cause the actor to mark windows
    ///   visible while the OS says the application is hidden.  Classify
    ///   `StaleNoDispatch`.  At most one expected-show token is consumed and
    ///   zombie state is pruned (contradictory-notification policy).
    ///
    /// * `None` — OS state unknown.  Legacy generation/FIFO fallback: token
    ///   with a newer owner ⇒ `StaleNoDispatch`/retain; matched token without
    ///   a newer owner ⇒ `SelfShown`/apply; no token ⇒ `ExternalShown`/clear.
    ///
    /// The caller must forward `SelfShown` / `ExternalShown` to the actor
    /// and suppress `StaleNoDispatch`.
    // The lock must cover the entire decision tree so that in-flight hide
    // or unhide callbacks cannot observe an inconsistent mid-classification
    // state.  The drop-tightening false positive is therefore suppressed.
    #[allow(clippy::significant_drop_tightening)]
    fn classify_shown_with(
        &self,
        pid: i32,
        query: impl FnOnce(i32) -> Option<bool>,
    ) -> ShownClassification {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);
        let is_hidden = query(pid);

        // ── OS confirms the app is visible ─────────────────────────
        // Actual visible state wins over all bookkeeping.
        if is_hidden == Some(false) {
            let owner_before = state.pids.get(&pid).and_then(|e| e.owner);
            let classification = if let Some(entry) = state.pids.get_mut(&pid) {
                let had_any_token = !entry.expected_shows.is_empty();
                entry.owner = None;
                entry.expected_shows.clear(); // clear ALL stale tokens
                if entry.is_zombie() {
                    state.pids.remove(&pid);
                }
                if had_any_token {
                    ShownClassification::SelfShown
                } else {
                    ShownClassification::ExternalShown
                }
            } else {
                ShownClassification::ExternalShown
            };
            tracing::debug!(
                sequence = seq, pid, operation = "classify", outcome = ?classification,
                owner_before = ?owner_before, owner_after = ?Option::<u64>::None,
                "app visible per OS, cleared ownership",
            );
            return classification;
        }

        // ── OS reports the app as still hidden ────────────────────
        if is_hidden == Some(true) {
            let owner_before = state.pids.get(&pid).and_then(|e| e.owner);

            let Some(entry) = state.pids.get_mut(&pid) else {
                tracing::debug!(
                    sequence = seq, pid, operation = "classify", outcome = "StaleNoDispatch",
                    owner_before = ?Option::<u64>::None, owner_after = ?Option::<u64>::None,
                    "no entry while OS reports hidden",
                );
                return ShownClassification::StaleNoDispatch;
            };

            // Consume at most one FIFO token regardless.
            let _ = entry.expected_shows.pop_front();

            if owner_before.is_some() {
                // Owner exists → retain it, suppress dispatch.
                tracing::debug!(
                    sequence = seq, pid, operation = "classify", outcome = "StaleNoDispatch",
                    owner_before = ?owner_before, owner_after = ?owner_before,
                    "OS reports hidden with owner, retaining ownership",
                );
            } else {
                // No owner → consume token (already consumed above),
                // prune zombie, suppress dispatch.
                if entry.is_zombie() {
                    state.pids.remove(&pid);
                }
                tracing::debug!(
                    sequence = seq, pid, operation = "classify", outcome = "StaleNoDispatch",
                    owner_before = ?owner_before, owner_after = ?Option::<u64>::None,
                    "OS reports hidden without owner, contradictory-notification policy",
                );
            }
            return ShownClassification::StaleNoDispatch;
        }

        // ── OS state unknown — legacy generation/FIFO fallback ────
        let Some(entry) = state.pids.get_mut(&pid) else {
            tracing::debug!(
                sequence = seq, pid, operation = "classify", outcome = "ExternalShown",
                owner_before = ?Option::<u64>::None, owner_after = ?Option::<u64>::None,
                "no entry found",
            );
            return ShownClassification::ExternalShown;
        };

        let owner_before = entry.owner;

        if let Some(token) = entry.expected_shows.pop_front() {
            // Token exists.  Stale means a **newer** Stache hide
            // generation was recorded after this token's unhide.
            if let Some(owner_gen) = entry.owner
                && owner_gen > token
            {
                tracing::debug!(
                    sequence = seq, pid, operation = "classify", outcome = "StaleNoDispatch",
                    owner_before = ?owner_before, owner_after = ?entry.owner,
                    "stale self-show: owner > token",
                );
                ShownClassification::StaleNoDispatch
            } else {
                entry.owner = None;
                if entry.is_zombie() {
                    state.pids.remove(&pid);
                }
                tracing::debug!(
                    sequence = seq, pid, operation = "classify", outcome = "SelfShown",
                    owner_before = ?owner_before, owner_after = ?Option::<u64>::None,
                    "consumed expected-show token",
                );
                ShownClassification::SelfShown
            }
        } else {
            entry.owner = None;
            if entry.is_zombie() {
                state.pids.remove(&pid);
            }
            tracing::debug!(
                sequence = seq, pid, operation = "classify", outcome = "ExternalShown",
                owner_before = ?owner_before, owner_after = ?Option::<u64>::None,
                "no expected token",
            );
            ShownClassification::ExternalShown
        }
    }

    /// Classifies with an explicit OS-state value.
    ///
    /// Delegates to [`Self::classify_shown_with`] by injecting the value
    /// as a constant closure.  Useful for tests that cannot reach macOS
    /// APIs — the real `classify_stache_hidden_app` goes through
    /// [`Self::classify_shown_with`] directly so the OS query is inside
    /// the mutex.
    fn classify_shown_with_state(&self, pid: i32, is_hidden: Option<bool>) -> ShownClassification {
        self.classify_shown_with(pid, |_| is_hidden)
    }

    /// Unconditionally removes the entire PID entry.  Intended for process
    /// termination _before_ the actor handler runs so that a delayed actor
    /// message cannot accidentally re-animate stale entries later.
    #[allow(clippy::significant_drop_tightening)]
    fn forget_terminated(&self, pid: i32) {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);
        let owner_before = state.pids.get(&pid).and_then(|e| e.owner);
        let had_entry = state.pids.remove(&pid).is_some();
        let outcome = if had_entry { "cleared" } else { "absent" };
        tracing::debug!(
            sequence = seq, pid, operation = "terminate", outcome,
            owner_before = ?owner_before, owner_after = ?Option::<u64>::None,
            "cleared PID entry on termination",
        );
    }

    /// Begins the shutdown sequence: sets the `shutting_down` flag and
    /// drains only PIDs that have a current **owner** (not just pending
    /// expected-show tokens).  Returns a sorted, deduplicated list.
    ///
    /// After this call the tracker is sealed — successive `hide_with` calls
    /// return `Failed` without touching `AppKit`.
    #[allow(clippy::significant_drop_tightening)]
    fn begin_shutdown_and_drain(&self) -> Vec<i32> {
        let mut state = self.state.lock();
        let seq = Self::next_seq(&mut state);
        state.shutting_down = true;

        // Collect only PIDs that Stache has currently hidden.
        let mut owner_pids: Vec<i32> = state
            .pids
            .iter()
            .filter(|(_, e)| e.owner.is_some())
            .map(|(&pid, _)| pid)
            .collect();
        owner_pids.sort_unstable();
        owner_pids.dedup();

        // Emit per-owned-PID drain transitions while mutex is held.
        for &pid in &owner_pids {
            let owner = state.pids.get(&pid).and_then(|e| e.owner);
            tracing::debug!(
                sequence = seq, pid, operation = "shutdown-drain", outcome = "drained",
                owner_before = ?owner, owner_after = ?Option::<u64>::None,
                "draining owner for restoration",
            );
        }

        // Count pending-token-only entries (no per-entry restoration log).
        let token_only_count = state
            .pids
            .iter()
            .filter(|(_, e)| e.owner.is_none() && !e.expected_shows.is_empty())
            .count();

        state.pids.clear();

        tracing::debug!(
            sequence = seq,
            operation = "shutdown",
            owner_count = owner_pids.len(),
            token_only_count,
            "drained owners for restoration",
        );

        owner_pids
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

/// Classifies an incoming `AppShown` event by querying the OS hidden
/// state **inside** the tracker mutex.  See [`ShownClassification`] for
/// interpretation.
///
/// The OS `app_is_hidden` query is passed as a closure to
/// [`HiddenAppTracker::classify_shown_with`] so that it is invoked after
/// the tracker lock is acquired.  This prevents a TOCTOU race where a
/// concurrent `hide_with` records ownership between the OS query and
/// lock acquisition.
#[must_use]
pub fn classify_stache_hidden_app(pid: i32) -> ShownClassification {
    tracker().classify_shown_with(pid, app_is_hidden)
}

/// Classifies an incoming `AppShown` event with an explicit OS-state
/// hint.  Useful for tests that cannot reach macOS APIs.
///
/// # Parameters
///
/// * `pid` - Process ID of the application that was shown.
/// * `is_hidden` - `Some(true)` if OS reports the app as hidden,
///   `Some(false)` if visible, `None` if unknown (uses legacy logic).
#[must_use]
pub fn classify_stache_hidden_app_with_state(
    pid: i32,
    is_hidden: Option<bool>,
) -> ShownClassification {
    tracker().classify_shown_with_state(pid, is_hidden)
}

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
        let classification = tracker.classify_shown_with_state(10, None);
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
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::ExternalShown
        );
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
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::StaleNoDispatch
        );
        assert_eq!(tracker.snapshot_owner(10), Some(5));
        assert_eq!(tracker.snapshot_tokens(10), vec![4]);

        // Second delayed AppShown (g4): owner (5) > token (4) → stale
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::StaleNoDispatch
        );
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

        // AlreadyShown → no token, owner PRESERVED (Fix 1)
        assert!(tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::AlreadyShown).succeeded());
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert_eq!(tracker.snapshot_owner(10), Some(1));
        assert!(!tracker.snapshot().is_empty()); // owner preserved

        // Re-hide (gen 2) then Failed unhide
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert!(!tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::Failed).succeeded());
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert_eq!(tracker.snapshot_owner(10), Some(2)); // unchanged from hide (gen 2)
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
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::SelfShown
        );
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
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::ExternalShown
        );
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
        assert_eq!(
            tracker.classify_shown_with_state(999, None),
            ShownClassification::ExternalShown
        );
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
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::SelfShown
        );
    }

    // -----------------------------------------------------------------------
    // Fix 1: AlreadyShown in unhide_with_outcome preserves owner
    // -----------------------------------------------------------------------

    #[test]
    fn already_shown_preserves_owner_and_does_not_create_token() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // AlreadyShown must NOT clear owner and must NOT create token
        assert!(tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::AlreadyShown).succeeded());
        assert_eq!(tracker.snapshot_owner(10), Some(1));
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    // -----------------------------------------------------------------------
    // Fix 5: shutdown boundary in unhide_with_outcome
    // -----------------------------------------------------------------------

    #[test]
    fn unhide_with_outcome_respects_shutdown_boundary() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // Shutdown seals the tracker and drains the map.
        let _ = restore_with(&tracker, |_| true);

        // Unhide after shutdown must return Failed without calling OS.
        // The entry was drained by begin_shutdown_and_drain, but the
        // shutting_down flag prevents the mock from being called.
        let outcome = tracker.unhide_with_outcome(10, |_| panic!("must not reach OS callback"));
        assert_eq!(outcome, UnhideAppOutcome::Failed);
        // Entry was drained during shutdown, so no owner exists.
        assert!(tracker.snapshot_owner(10).is_none());
    }

    // -----------------------------------------------------------------------
    // Fix 3 + Fix 6: OS-state-aware classify_shown_with_state
    // -----------------------------------------------------------------------

    #[test]
    fn classify_shown_with_state_external_when_os_shown() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // OS says visible → clear ownership, dispatch
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(false)),
            ShownClassification::ExternalShown
        );
        assert_eq!(tracker.snapshot_owner(10), None);
    }

    #[test]
    fn classify_shown_with_state_stale_when_os_hidden() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        // owner = g3, token = [2]

        // OS says hidden → stale detection retains owner
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(true)),
            ShownClassification::StaleNoDispatch
        );
        assert_eq!(tracker.snapshot_owner(10), Some(3));
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    #[test]
    fn classify_shown_with_state_self_shown_when_os_shown() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        // owner = None, token = [2]

        // OS says visible → consume token, SelfShown
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(false)),
            ShownClassification::SelfShown
        );
        assert!(tracker.snapshot_owner(10).is_none());
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    #[test]
    fn classify_shown_with_state_none_matches_old_behavior_external() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        // owner = g1

        // OS unknown → old behavior (ExternalShown)
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::ExternalShown
        );
        assert_eq!(tracker.snapshot_owner(10), None);
    }

    #[test]
    fn classify_shown_with_state_none_matches_old_behavior_stale() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        // owner = g3, token = [2]

        // OS unknown, stale scenario → StaleNoDispatch (old behavior)
        assert_eq!(
            tracker.classify_shown_with_state(10, None),
            ShownClassification::StaleNoDispatch
        );
        assert_eq!(tracker.snapshot_owner(10), Some(3));
    }

    // -----------------------------------------------------------------------
    // Termination test: entry with both owner AND pending token
    // -----------------------------------------------------------------------

    #[test]
    fn termination_clears_entry_with_owner_and_token() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache);
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        // owner = g3, token = [2]
        assert_eq!(tracker.snapshot_owner(10), Some(3));
        assert_eq!(tracker.snapshot_tokens(10), vec![2]);

        // forget_terminated must clear everything
        tracker.forget_terminated(10);
        assert!(tracker.snapshot_owner(10).is_none());
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    // -----------------------------------------------------------------------
    // In-flight unhide holds shutdown barrier
    // -----------------------------------------------------------------------

    /// Proves that `unhide_with_outcome` holds the tracker mutex for the
    /// duration of the OS callback so that a concurrent shutdown drain
    /// cannot observe intermediate state.
    #[test]
    fn in_flight_unhide_holds_shutdown_lock_until_ownership_is_updated() {
        let tracker = Arc::new(HiddenAppTracker::default());

        // Arrange: hide first so there is ownership to release.
        tracker.hide_with(12, |_| HideAppOutcome::HiddenByStache);

        let unhide_entered = Arc::new(Barrier::new(2));
        let release_unhide = Arc::new(Barrier::new(2));

        let t = Arc::clone(&tracker);
        let ue = Arc::clone(&unhide_entered);
        let ru = Arc::clone(&release_unhide);
        let unhide_thread = thread::spawn(move || {
            t.unhide_with_outcome(12, |_| {
                ue.wait();
                ru.wait();
                UnhideAppOutcome::UnhiddenByStache
            })
        });
        unhide_entered.wait();

        // Shutdown drain must not be able to acquire the tracker lock
        // while an unhide callback is in flight.
        assert!(
            tracker.state.try_lock().is_none(),
            "shutdown must not acquire tracker while an unhide is in flight"
        );

        release_unhide.wait();
        assert!(unhide_thread.join().expect("unhide should not panic").succeeded());
    }

    // -----------------------------------------------------------------------
    // OS-state-aware classify — extended edge cases
    // -----------------------------------------------------------------------

    /// When the OS confirms the app is visible, ALL stale expected-show
    /// tokens (not just the front one) must be cleared along with the owner.
    #[test]
    fn os_visible_clears_all_stale_tokens() {
        let tracker = HiddenAppTracker::default();

        // Build up multiple stale tokens: hide → unhide → hide → unhide → hide
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache); // g1 owner
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache); // g2 token
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache); // g3 owner
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache); // g4 token
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache); // g5 owner

        assert_eq!(tracker.snapshot_owner(10), Some(5));
        assert_eq!(tracker.snapshot_tokens(10), vec![2, 4]); // TWO stale tokens

        // User genuinely shows the app → OS visible.
        // Classification is SelfShown because stale tokens existed
        // (Stache had unhidden at some point).
        let classification = tracker.classify_shown_with_state(10, Some(false));
        assert_eq!(classification, ShownClassification::SelfShown);

        // ALL state must be cleared (owner + ALL tokens).
        assert_eq!(tracker.snapshot_owner(10), None);
        assert!(tracker.snapshot_tokens(10).is_empty());
    }

    /// Duplicate `AppShown` arriving after the expected-show token has
    /// already been consumed, while the OS still reports the app as hidden,
    /// must retain the current owner (not clear it).
    #[test]
    fn duplicate_appshown_while_hidden_retains_owner() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache); // g1 owner
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache); // g2 token
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache); // g3 owner

        // First AppShown with OS hidden: consumes token, retains owner.
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(true)),
            ShownClassification::StaleNoDispatch
        );
        assert_eq!(tracker.snapshot_owner(10), Some(3));
        assert!(tracker.snapshot_tokens(10).is_empty());

        // Second AppShown with OS hidden: no token, OS still says hidden,
        // must retain owner (not clear it as ExternalShown).
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(true)),
            ShownClassification::StaleNoDispatch
        );
        assert_eq!(tracker.snapshot_owner(10), Some(3));
    }

    /// `AppShown` arriving while the OS reports the app as hidden and the
    /// tracker has no current owner, but has a pending expected-show token.
    /// Must classify `StaleNoDispatch` (do not tell the actor to mark
    /// windows visible), consume the token, and prune zombie state.
    #[test]
    fn hidden_with_token_no_owner_classifies_stale() {
        let tracker = HiddenAppTracker::default();

        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache); // g1 owner
        tracker.unhide_with_outcome(10, |_| UnhideAppOutcome::UnhiddenByStache); // g2 token, no owner

        // OS says hidden, no owner, token exists → StaleNoDispatch.
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(true)),
            ShownClassification::StaleNoDispatch
        );

        // Token is consumed, entry should be pruned (zombie).
        assert!(tracker.snapshot_tokens(10).is_empty());
        assert!(tracker.snapshot().is_empty());
    }

    // -----------------------------------------------------------------------
    // Fix: classify_shown_with query runs inside tracker mutex
    // -----------------------------------------------------------------------

    /// Proves that `classify_shown_with` holds the tracker mutex during the
    /// visibility query, preventing a concurrent `hide_with` from recording
    /// ownership between the OS query and classification (TOCTOU race).
    #[test]
    fn classify_shown_with_holds_mutex_during_query() {
        let tracker = Arc::new(HiddenAppTracker::default());

        let query_entered = Arc::new(Barrier::new(2));
        let release_query = Arc::new(Barrier::new(2));

        let t = Arc::clone(&tracker);
        let qe = Arc::clone(&query_entered);
        let rq = Arc::clone(&release_query);

        // Thread: classify_shown_with — query runs inside the lock.
        let classify_thread = thread::spawn(move || {
            t.classify_shown_with(42, |_| {
                qe.wait(); // Signal: we hold the lock inside the query
                rq.wait(); // Wait for main thread to verify
                None // query result doesn't matter for this test
            })
        });

        query_entered.wait(); // Classify thread has the lock

        // The tracker mutex MUST be held during the query closure.
        assert!(
            tracker.state.try_lock().is_none(),
            "classify_shown_with must hold the tracker mutex during the query"
        );

        // A concurrent hide_with issued now will block until classify
        // releases the lock — it cannot "sneak in" between query and
        // classification.
        release_query.wait();
        classify_thread.join().expect("classify thread should not panic");
    }

    /// An `AppShown` that runs and clears ownership (OS visible) before a
    /// later `hide_with` records ownership must NOT erase that later
    /// ownership.  This is guaranteed by linearization through the mutex:
    /// the classify releases the lock before the hide acquires it.
    #[test]
    fn appshown_before_hide_does_not_erase_later_ownership() {
        let tracker = HiddenAppTracker::default();

        // AppShown: OS says visible → clears any stale state.
        tracker.classify_shown_with_state(10, Some(false));
        assert!(tracker.snapshot_owner(10).is_none());

        // Later: Stache hides the app.
        tracker.hide_with(10, |_| HideAppOutcome::HiddenByStache);
        assert_eq!(tracker.snapshot_owner(10), Some(1));

        // The earlier classify does NOT erase the hide's ownership.
        // A subsequent classify while OS still says hidden retains it.
        assert_eq!(
            tracker.classify_shown_with_state(10, Some(true)),
            ShownClassification::StaleNoDispatch
        );
        assert_eq!(tracker.snapshot_owner(10), Some(1));
    }
}
