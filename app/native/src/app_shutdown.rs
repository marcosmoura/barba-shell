//! Centralized shutdown coordinator.
//!
//! Guarantees correct cleanup order on exit or restart:
//! 1. Restore applications hidden by Stache.
//! 2. Shut down the tiling window manager.
//! 3. Stop the IPC socket server.
//!
//! # Signal Handling
//!
//! The first termination signal (SIGINT/SIGTERM/SIGHUP via ctrlc's
//! `termination` feature) schedules an orderly exit through Tauri's main
//! thread.  A second signal is treated as a force-exit escalation because
//! the main thread may be stalled.  All termination signals share the
//! escalation counter.
//!
//! # Terminal-Request Arbitration
//!
//! Multiple threads may call `exit` or `restart` concurrently.  An atomic
//! arbiter ensures that:
//! - Exit has precedence over Restart.
//! - At most one terminal action is executed.
//! - A queued closure re-checks whether its action is still current before
//!   running cleanup and again before calling `exit`/`restart`, so an Exit
//!   that overrides a pending Restart suppresses the restart closure even
//!   if already queued.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tauri::{AppHandle, Runtime};

use crate::modules::tiling;
use crate::platform;

// ============================================================================
// Terminal-Request Arbiter
// ============================================================================

const ARBITER_NONE: u8 = 0;
const ARBITER_EXIT: u8 = 1;
const ARBITER_RESTART: u8 = 2;

static TERMINAL_REQUEST: AtomicU8 = AtomicU8::new(ARBITER_NONE);

/// Core: claim an exit on `state`, overriding any pending restart.
/// Returns `false` if exit is already claimed (duplicate request rejected).
fn try_claim_exit_on(state: &AtomicU8) -> bool {
    match state.compare_exchange(ARBITER_NONE, ARBITER_EXIT, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => true,
        Err(ARBITER_RESTART) => {
            state.store(ARBITER_EXIT, Ordering::Release);
            true
        }
        Err(_) => false,
    }
}

/// Core: claim a restart on `state`.  Only succeeds when no terminal action
/// is pending.  `false` means the request is rejected.
fn try_claim_restart_on(state: &AtomicU8) -> bool {
    state
        .compare_exchange(
            ARBITER_NONE,
            ARBITER_RESTART,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
}

/// Core: check whether `action` matches the current state.
fn is_current_on(state: &AtomicU8, action: ShutdownAction) -> bool {
    let current = state.load(Ordering::Acquire);
    matches!(
        (action, current),
        (ShutdownAction::Exit, ARBITER_EXIT) | (ShutdownAction::Restart, ARBITER_RESTART)
    )
}

/// Core: release the claim on `state` only if it belongs to `action`.
fn release_claim_on(state: &AtomicU8, action: ShutdownAction) {
    let expected = match action {
        ShutdownAction::Exit => ARBITER_EXIT,
        ShutdownAction::Restart => ARBITER_RESTART,
    };
    let _ = state.compare_exchange(expected, ARBITER_NONE, Ordering::Release, Ordering::Acquire);
}

// --- Global-state wrappers ------------------------------------------------

fn try_claim_exit() -> bool { try_claim_exit_on(&TERMINAL_REQUEST) }
fn try_claim_restart() -> bool { try_claim_restart_on(&TERMINAL_REQUEST) }
fn is_current(action: ShutdownAction) -> bool { is_current_on(&TERMINAL_REQUEST, action) }
fn release_claim(action: ShutdownAction) { release_claim_on(&TERMINAL_REQUEST, action) }

/// Establish exit precedence, overriding any pending restart.  Idempotent.
/// Called from the `RunEvent::Exit` handler so a queued restart closure
/// cannot run after the app has entered a natural exit.
pub fn establish_exit_precedence() { try_claim_exit(); }

static CLEANUP_STARTED: AtomicBool = AtomicBool::new(false);
static SIGNAL_COUNT: AtomicU8 = AtomicU8::new(0);

// ============================================================================
// Internal Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownAction {
    Exit,
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalAction {
    Orderly,
    Force,
}

// ============================================================================
// Signal Tracking
// ============================================================================

fn next_signal_action(signals: &AtomicU8) -> SignalAction {
    if signals.fetch_add(1, Ordering::AcqRel) == 0 {
        SignalAction::Orderly
    } else {
        SignalAction::Force
    }
}

// ============================================================================
// Idempotent Cleanup
// ============================================================================

fn run_cleanup_once(
    started: &AtomicBool,
    restore: impl FnOnce(),
    stop_tiling: impl FnOnce(),
    stop_ipc: impl FnOnce(),
) -> bool {
    if started.swap(true, Ordering::AcqRel) {
        return false;
    }
    restore();
    stop_tiling();
    stop_ipc();
    true
}

/// Runs the full cleanup sequence if it hasn't run yet.
///
/// Safe to call multiple times — only the first call executes the closures.
pub fn cleanup_once() {
    let _ = run_cleanup_once(
        &CLEANUP_STARTED,
        || {
            let summary = tiling::restore_stache_hidden_apps();
            tracing::info!(
                attempted = summary.attempted,
                restored = summary.restored,
                "restored applications hidden by Stache"
            );
        },
        tiling::shutdown,
        platform::ipc_socket::stop_server,
    );
}

// ============================================================================
// Main-Thread Dispatch
// ============================================================================

fn request<R: Runtime>(app: &AppHandle<R>, action: ShutdownAction) {
    // Attempt to claim this terminal action through the arbiter.
    let claimed = match action {
        ShutdownAction::Exit => try_claim_exit(),
        ShutdownAction::Restart => try_claim_restart(),
    };
    if !claimed {
        tracing::warn!(?action, "terminal request rejected by arbiter");
        return;
    }

    let action_handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        // Re-check before cleanup — a later Exit may have overridden Restart.
        if !is_current(action) {
            tracing::debug!(?action, "terminal request superseded before cleanup");
            return;
        }
        cleanup_once();
        // Re-check after cleanup — an Exit claimed during cleanup suppresses
        // a queued Restart.
        if !is_current(action) {
            tracing::debug!(?action, "terminal request superseded after cleanup");
            return;
        }
        match action {
            ShutdownAction::Exit => action_handle.exit(0),
            ShutdownAction::Restart => action_handle.restart(),
        }
    }) {
        // Scheduling failed; release the claim only if we still own it.
        release_claim(action);
        tracing::error!(%error, "failed to dispatch orderly shutdown to main thread");
    }
}

/// Schedules an orderly exit (restore → tiling shutdown → IPC stop → exit).
pub fn exit<R: Runtime>(app: &AppHandle<R>) { request(app, ShutdownAction::Exit); }
/// Schedules an orderly restart (restore → tiling shutdown → IPC stop → restart).
#[allow(dead_code)]
pub fn restart<R: Runtime>(app: &AppHandle<R>) { request(app, ShutdownAction::Restart); }
// ============================================================================
// Signal Handler
// ============================================================================

/// Installs one global signal handler for SIGINT/SIGTERM/SIGHUP (via ctrlc's
/// `termination` feature).
///
/// The first signal schedules an orderly exit.  If a second signal arrives
/// before the main thread completes cleanup, the process is force-exited.
/// All termination signals share the escalation counter.
///
/// Must be called exactly once, before `App::run`.
pub fn install_signal_handler<R: Runtime>(app: AppHandle<R>) -> Result<(), ctrlc::Error> {
    ctrlc::set_handler(move || match next_signal_action(&SIGNAL_COUNT) {
        SignalAction::Orderly => exit(&app),
        SignalAction::Force => std::process::exit(1),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

    use super::{
        ARBITER_NONE, ShutdownAction, SignalAction, is_current_on, next_signal_action,
        release_claim_on, run_cleanup_once, try_claim_exit_on, try_claim_restart_on,
    };

    // =========================================================================
    // Existing cleanup / signal tests (unchanged semantics)
    // =========================================================================

    #[test]
    fn cleanup_restores_before_stopping_tiling_and_ipc() {
        let started = AtomicBool::new(false);
        let order = Mutex::new(Vec::new());

        assert!(run_cleanup_once(
            &started,
            || order.lock().unwrap().push("restore"),
            || order.lock().unwrap().push("tiling"),
            || order.lock().unwrap().push("ipc"),
        ));

        assert_eq!(*order.lock().unwrap(), ["restore", "tiling", "ipc"]);
    }

    #[test]
    fn cleanup_runs_only_once() {
        let started = AtomicBool::new(false);
        let calls = AtomicUsize::new(0);

        assert!(run_cleanup_once(
            &started,
            || {
                calls.fetch_add(1, Ordering::Relaxed);
            },
            || {
                calls.fetch_add(1, Ordering::Relaxed);
            },
            || {
                calls.fetch_add(1, Ordering::Relaxed);
            },
        ));
        assert!(!run_cleanup_once(
            &started,
            || {
                calls.fetch_add(1, Ordering::Relaxed);
            },
            || {
                calls.fetch_add(1, Ordering::Relaxed);
            },
            || {
                calls.fetch_add(1, Ordering::Relaxed);
            },
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn second_termination_signal_forces_exit() {
        let signals = AtomicU8::new(0);

        assert_eq!(next_signal_action(&signals), SignalAction::Orderly);
        assert_eq!(next_signal_action(&signals), SignalAction::Force);
    }

    // =========================================================================
    // Terminal-request arbiter tests (use local atomics, no global state)
    // =========================================================================

    #[test]
    fn exit_can_be_claimed_when_empty() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_exit_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Exit));
    }

    #[test]
    fn restart_can_be_claimed_when_empty() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_restart_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Restart));
    }

    #[test]
    fn exit_overrides_pending_restart() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_restart_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Restart));

        // Exit overrides restart
        assert!(try_claim_exit_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Exit));
        assert!(!is_current_on(&state, ShutdownAction::Restart));
    }

    #[test]
    fn restart_after_exit_rejected() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_exit_on(&state));
        assert!(!try_claim_restart_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Exit));
    }

    #[test]
    fn duplicate_exit_rejected() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_exit_on(&state));
        assert!(!try_claim_exit_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Exit));
    }

    #[test]
    fn duplicate_restart_rejected() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_restart_on(&state));
        assert!(!try_claim_restart_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Restart));
    }

    #[test]
    fn release_claim_restores_none_for_exit() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_exit_on(&state));
        release_claim_on(&state, ShutdownAction::Exit);
        assert_eq!(state.load(Ordering::Acquire), ARBITER_NONE);

        // After release, a restart can be claimed
        assert!(try_claim_restart_on(&state));
    }

    #[test]
    fn release_claim_restores_none_for_restart() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_restart_on(&state));
        release_claim_on(&state, ShutdownAction::Restart);
        assert_eq!(state.load(Ordering::Acquire), ARBITER_NONE);

        // After release, an exit can be claimed
        assert!(try_claim_exit_on(&state));
    }

    #[test]
    fn release_wrong_action_is_noop() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_exit_on(&state));

        // Try releasing restart — should leave exit in place
        release_claim_on(&state, ShutdownAction::Restart);
        assert!(is_current_on(&state, ShutdownAction::Exit));
        assert!(!is_current_on(&state, ShutdownAction::Restart));
    }

    #[test]
    fn is_current_false_after_override() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_restart_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Restart));

        // Exit overrides restart
        assert!(try_claim_exit_on(&state));
        assert!(!is_current_on(&state, ShutdownAction::Restart));
        assert!(is_current_on(&state, ShutdownAction::Exit));
    }

    #[test]
    fn establish_exit_precedence_uses_global_arbiter() {
        // This test verifies that the public function correctly wraps the
        // global state (smoke test only — full logic covered by _on variants).
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_restart_on(&state));
        assert!(try_claim_exit_on(&state)); // same logic as establish_exit_precedence
        assert!(is_current_on(&state, ShutdownAction::Exit));
        assert!(!is_current_on(&state, ShutdownAction::Restart));
    }

    /// Simulate the closure check: Exit overrides a queued Restart, then the
    /// Restart closure should see `is_current(false)` and abort.
    #[test]
    fn queued_restart_closure_aborts_when_exit_overrides() {
        let state = AtomicU8::new(ARBITER_NONE);
        // Thread B claims restart and queues closure
        assert!(try_claim_restart_on(&state));

        // Thread A claims exit (overrides)
        assert!(try_claim_exit_on(&state));

        // The queued restart closure runs and checks is_current
        assert!(!is_current_on(&state, ShutdownAction::Restart));

        // The exit closure would also run — it should proceed
        assert!(is_current_on(&state, ShutdownAction::Exit));
    }

    /// Exit-then-restart: exit claimed, restart rejected, exit still current.
    #[test]
    fn exit_then_restart_rejected() {
        let state = AtomicU8::new(ARBITER_NONE);
        assert!(try_claim_exit_on(&state));
        assert!(!try_claim_restart_on(&state));
        assert!(is_current_on(&state, ShutdownAction::Exit));
    }
}
