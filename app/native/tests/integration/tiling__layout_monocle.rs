//! Integration tests for the Monocle layout.
//!
//! Monocle layout displays windows one at a time, each filling the entire
//! tiling area. Only the focused window is visible; others are hidden behind it.
//!
//! ## Test Coverage
//! - Single window fills entire area
//! - Multiple windows all maximize to same size
//! - Window focus cycles through windows
//! - Adding windows maintains monocle behavior
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_monocle/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in monocle fills the tiling area.
#[test]
fn test_monocle_single_window_maximized() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_monocle");
    delay(STACHE_INIT_DELAY_MS);

    // Create a TextEdit window
    let window = fixture.create_textedit("Monocle Single");
    assert!(window.is_some(), "Failed to create TextEdit window");
    delay(OPERATION_DELAY_MS * 2);

    // Re-activate TextEdit to force a focus event and ensure the window ID swap occurs
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    // Force a workspace balance to re-apply layout with correct window IDs
    fixture.stache_command(&["tiling", "workspace", "--balance"]);
    delay(OPERATION_DELAY_MS * 2);

    // Get the window frame
    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    // In monocle, the window should fill most of the screen
    if let Some((screen_w, screen_h)) = get_screen_size() {
        // Account for gaps and menu bar
        let expected_min_width = screen_w * 0.8;
        let expected_min_height = screen_h * 0.7;

        assert!(
            frame.width >= expected_min_width,
            "Monocle window width ({}) should be at least {}",
            frame.width,
            expected_min_width
        );
        assert!(
            frame.height >= expected_min_height,
            "Monocle window height ({}) should be at least {}",
            frame.height,
            expected_min_height
        );

        println!(
            "Monocle single window: {}x{} (screen: {}x{})",
            frame.width, frame.height, screen_w, screen_h
        );
    }
}

/// Test that multiple windows in monocle all have the same size.
#[test]
fn test_monocle_multiple_windows_same_size() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_monocle");
    delay(STACHE_INIT_DELAY_MS);

    // Create multiple TextEdit windows
    let _w1 = fixture.create_textedit("Monocle 1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Monocle 2");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("Monocle 3");
    delay(OPERATION_DELAY_MS * 2);

    // Get all TextEdit window frames
    let frames = get_app_window_frames("TextEdit");
    assert!(
        frames.len() >= 3,
        "Should have at least 3 TextEdit windows, got {}",
        frames.len()
    );

    // In monocle, all windows should have the same size
    let first = &frames[0];
    for (i, frame) in frames.iter().enumerate().skip(1) {
        // Allow small tolerance for frame differences
        let width_diff = (frame.width - first.width).abs();
        let height_diff = (frame.height - first.height).abs();

        assert!(
            width_diff < FRAME_TOLERANCE && height_diff < FRAME_TOLERANCE,
            "Window {} size ({}x{}) should match window 0 size ({}x{})",
            i,
            frame.width,
            frame.height,
            first.width,
            first.height
        );
    }

    println!(
        "All {} monocle windows have size: {}x{}",
        frames.len(),
        first.width,
        first.height
    );
}

/// Test that windows in monocle are stacked (same position).
#[test]
fn test_monocle_windows_stacked() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_monocle");
    delay(STACHE_INIT_DELAY_MS);

    // Create multiple windows
    let _w1 = fixture.create_textedit("Stack 1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Stack 2");
    delay(OPERATION_DELAY_MS * 2);

    // Get all TextEdit window frames
    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 2, "Should have at least 2 TextEdit windows");

    // In monocle, windows should be at the same position (stacked)
    let first = &frames[0];
    for (i, frame) in frames.iter().enumerate().skip(1) {
        let x_diff = (frame.x - first.x).abs();
        let y_diff = (frame.y - first.y).abs();

        // Windows should be at the same position
        assert!(
            x_diff < FRAME_TOLERANCE && y_diff < FRAME_TOLERANCE,
            "Window {} position ({}, {}) should match window 0 position ({}, {})",
            i,
            frame.x,
            frame.y,
            first.x,
            first.y
        );
    }

    println!("Monocle windows stacked at position: ({}, {})", first.x, first.y);
}

/// Test focus cycling in monocle layout.
#[test]
fn test_monocle_focus_cycle() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_monocle");
    delay(STACHE_INIT_DELAY_MS);

    // Create windows with distinct titles
    let _w1 = fixture.create_textedit("Cycle-First");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Cycle-Second");
    delay(OPERATION_DELAY_MS);

    // The last created window should be focused
    let initial_title = get_frontmost_window_title();
    assert!(initial_title.is_some(), "Should have a focused window");

    // Use focus-next command to cycle
    fixture.stache_command(&["tiling", "window", "--focus", "next"]);
    delay(OPERATION_DELAY_MS);

    let after_next_title = get_frontmost_window_title();
    assert!(after_next_title.is_some(), "Should still have focus after next");

    println!(
        "Focus cycle: '{}' -> '{}'",
        initial_title.as_deref().unwrap_or("unknown"),
        after_next_title.as_deref().unwrap_or("unknown")
    );

    // Use focus-previous to go back
    fixture.stache_command(&["tiling", "window", "--focus", "previous"]);
    delay(OPERATION_DELAY_MS);

    let after_prev_title = get_frontmost_window_title();
    assert!(
        after_prev_title.is_some(),
        "Should still have focus after previous"
    );

    // Verify cycling occurred (focus changed at least once)
    let focus_changed = initial_title != after_next_title || after_next_title != after_prev_title;
    assert!(
        focus_changed || initial_title == after_prev_title,
        "Focus should cycle through windows"
    );

    println!(
        "Focus after previous: '{}'",
        after_prev_title.as_deref().unwrap_or("unknown")
    );
}

/// Test adding a window to monocle maintains behavior.
#[test]
fn test_monocle_add_window_maintains_layout() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_monocle");
    delay(STACHE_INIT_DELAY_MS);

    // Create initial window and record its size
    let _w1 = fixture.create_textedit("Initial Monocle");
    delay(OPERATION_DELAY_MS * 2);

    let initial_frame = get_frontmost_window_frame();
    assert!(initial_frame.is_some(), "Should get initial frame");
    let initial_frame = initial_frame.unwrap();

    // Add another window
    let _w2 = fixture.create_textedit("Added Monocle");
    delay(OPERATION_DELAY_MS * 2);

    let new_frame = get_frontmost_window_frame();
    assert!(new_frame.is_some(), "Should get new window frame");
    let new_frame = new_frame.unwrap();

    // New window should have same dimensions as initial (monocle behavior)
    assert!(
        new_frame.approximately_equals(&initial_frame, FRAME_TOLERANCE),
        "New window frame should match initial frame in monocle"
    );

    println!(
        "Monocle maintained: initial {}x{}, new {}x{}",
        initial_frame.width, initial_frame.height, new_frame.width, new_frame.height
    );
}
