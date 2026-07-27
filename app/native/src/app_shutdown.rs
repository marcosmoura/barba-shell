//! Centralized shutdown coordinator.
//!
//! Guarantees correct cleanup order on exit or restart:
//! 1. Restore applications hidden by Stache.
//! 2. Shut down the tiling window manager.
//! 3. Stop the IPC socket server.
//!
//! # Signal Handling
//!
//! The first SIGINT/SIGTERM schedules an orderly exit through Tauri's main
//! thread.  A second signal is treated as a force-exit escalation because
//! the main thread may be stalled.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tauri::{AppHandle, Runtime};

use crate::modules::tiling;
use crate::platform;

static CLEANUP_STARTED: AtomicBool = AtomicBool::new(false);
static SIGNAL_COUNT: AtomicU8 = AtomicU8::new(0);

// ============================================================================
// Internal Types
// ============================================================================

#[allow(dead_code)]
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
    let action_handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        cleanup_once();
        match action {
            ShutdownAction::Exit => action_handle.exit(0),
            ShutdownAction::Restart => action_handle.restart(),
        }
    }) {
        tracing::error!(%error, "failed to dispatch orderly shutdown to main thread");
    }
}

/// Schedules an orderly exit (restore → tiling shutdown → IPC stop → exit).
pub fn exit<R: Runtime>(app: &AppHandle<R>) { request(app, ShutdownAction::Exit); }
/// Schedules an orderly restart (restore → tiling shutdown → IPC stop → restart).
#[allow(dead_code)]
pub fn restart<R: Runtime>(app: &AppHandle<R>) {
    request(app, ShutdownAction::Restart);
}
// ============================================================================
// Signal Handler
// ============================================================================

/// Installs one global SIGINT/SIGTERM handler.
///
/// The first signal schedules an orderly exit.  If a second signal arrives
/// before the main thread completes cleanup, the process is force-exited.
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

    use super::{SignalAction, next_signal_action, run_cleanup_once};

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
}
