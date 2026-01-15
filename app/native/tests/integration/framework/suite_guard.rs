//! Suite-level precondition checks using #[ctor].
//!
//! This module ensures that Dictionary and TextEdit are not running
//! before the test suite starts, preventing interference with tests.

use std::sync::atomic::{AtomicBool, Ordering};

use super::native;

/// Flag to track if preconditions have been checked.
static CHECKED: AtomicBool = AtomicBool::new(false);

/// Flag to track if preconditions passed.
static PASSED: AtomicBool = AtomicBool::new(false);

/// Test apps that must not be running before tests start.
const TEST_APPS: &[&str] = &["Dictionary", "TextEdit"];

/// Checks suite preconditions (called by #[ctor] in main.rs).
///
/// This function:
/// 1. Checks if Dictionary or TextEdit are running
/// 2. If running, prints a warning and panics to stop the test suite
/// 3. Only runs once per test suite execution
pub fn check_suite_preconditions() {
    // Only check once
    if CHECKED.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut running_apps = Vec::new();

    for app_name in TEST_APPS {
        if native::is_app_running(app_name) {
            running_apps.push(*app_name);
        }
    }

    if !running_apps.is_empty() {
        eprintln!();
        eprintln!("═══════════════════════════════════════════════════════════════════");
        eprintln!("  INTEGRATION TEST SUITE PRECONDITION FAILED");
        eprintln!("═══════════════════════════════════════════════════════════════════");
        eprintln!();
        eprintln!("  The following test applications are currently running:");
        for app in &running_apps {
            eprintln!("    • {}", app);
        }
        eprintln!();
        eprintln!("  Please close these applications before running integration tests.");
        eprintln!("  These apps are used by the tests and must start in a clean state.");
        eprintln!();
        eprintln!("  Quick fix:");
        eprintln!("    pkill -x Dictionary; pkill -x TextEdit");
        eprintln!();
        eprintln!("═══════════════════════════════════════════════════════════════════");
        eprintln!();

        // Mark as failed
        PASSED.store(false, Ordering::SeqCst);

        panic!(
            "Test apps already running: {}. Close them and retry.",
            running_apps.join(", ")
        );
    }

    // Mark as passed
    PASSED.store(true, Ordering::SeqCst);

    eprintln!("✓ Suite preconditions passed: no test apps running");
}

/// Returns true if preconditions have been checked and passed.
pub fn preconditions_passed() -> bool {
    CHECKED.load(Ordering::SeqCst) && PASSED.load(Ordering::SeqCst)
}
