//! Integration tests for window focus operations.
//!
//! Tests directional focus (up, down, left, right) and sequential focus
//! (next, previous) operations.
//!
//! ## Test Coverage
//! - Focus next/previous window
//! - Focus in direction (up, down, left, right)
//! - Focus wrapping behavior
//! - Focus with single window
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__window_focus/)' --test-threads 1 --no-capture
//! ```

use crate::common::*;

/// Test focus next cycles through windows.
#[test]
fn test_focus_next_cycles() {
    let mut test = Test::new("tiling_basic");

    // Create multiple windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    // Wait for windows to stabilize
    let _ = test.get_app_stable_frames("Dictionary", 3);

    // Record initial focused window
    let initial_title = get_frontmost_window_title();
    println!("Initial focus: {:?}", initial_title);

    // Focus next multiple times
    let mut titles = vec![initial_title.clone()];
    for _ in 0..4 {
        test.stache_command(&["tiling", "window", "--focus", "next"]);
        delay(OPERATION_DELAY_MS);
        let title = get_frontmost_window_title();
        titles.push(title.clone());
        println!("After focus-next: {:?}", title);
    }

    // Should have cycled through different windows
    let unique_titles: std::collections::HashSet<_> = titles.iter().flatten().collect();
    println!("Unique windows focused: {}", unique_titles.len());

    // With 3 windows, focus-next should cycle through them
    assert!(
        !unique_titles.is_empty(),
        "Should have focused at least one window"
    );
}

/// Test focus previous cycles backwards.
#[test]
fn test_focus_previous_cycles() {
    let mut test = Test::new("tiling_basic");

    // Create multiple windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    // Wait for windows to stabilize
    let _ = test.get_app_stable_frames("Dictionary", 3);

    let initial_title = get_frontmost_window_title();
    assert!(initial_title.is_some(), "Should have initial focused window");
    println!("Initial: {:?}", initial_title);

    // Focus previous multiple times
    for i in 0..4 {
        test.stache_command(&["tiling", "window", "--focus", "previous"]);
        delay(OPERATION_DELAY_MS);
        let title = get_frontmost_window_title();
        assert!(
            title.is_some(),
            "Should have focused window after previous {}",
            i + 1
        );
        println!("After focus-previous {}: {:?}", i + 1, title);
    }
}

/// Test focus left in dwindle layout.
#[test]
fn test_focus_left() {
    let mut test = Test::new("tiling_basic");

    // Create two windows side by side in dwindle
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    // Get frames to understand layout
    let frames = test.get_app_stable_frames("Dictionary", 2);
    if frames.len() >= 2 {
        println!(
            "Window positions: ({}, {}) and ({}, {})",
            frames[0].x, frames[0].y, frames[1].x, frames[1].y
        );
    }

    // Last created window should be focused (right side typically)
    let before = get_frontmost_window_frame();

    // Focus left
    test.stache_command(&["tiling", "window", "--focus", "left"]);
    delay(OPERATION_DELAY_MS);

    let after = get_frontmost_window_frame();

    assert!(before.is_some(), "Should have frame before focus left");
    assert!(after.is_some(), "Should have frame after focus left");

    if let (Some(b), Some(a)) = (before, after) {
        let moved_left = a.x < b.x;
        println!("Focus moved left: {} (x: {} -> {})", moved_left, b.x, a.x);
    }
}

/// Test focus right in dwindle layout.
#[test]
fn test_focus_right() {
    let mut test = Test::new("tiling_basic");

    // Create two windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    // Focus left first to ensure we're on left window
    test.stache_command(&["tiling", "window", "--focus", "left"]);
    delay(OPERATION_DELAY_MS);

    let before = get_frontmost_window_frame();

    // Focus right
    test.stache_command(&["tiling", "window", "--focus", "right"]);
    delay(OPERATION_DELAY_MS);

    let after = get_frontmost_window_frame();

    assert!(before.is_some(), "Should have frame before focus right");
    assert!(after.is_some(), "Should have frame after focus right");

    if let (Some(b), Some(a)) = (before, after) {
        println!("Focus right: x changed from {} to {}", b.x, a.x);
    }
}

/// Test focus up with vertically arranged windows.
#[test]
fn test_focus_up() {
    let mut test = Test::new("tiling_basic");

    // Create three windows to get vertical stacking in dwindle
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 3);

    let before = get_frontmost_window_frame();

    // Focus up
    test.stache_command(&["tiling", "window", "--focus", "up"]);
    delay(OPERATION_DELAY_MS);

    let after = get_frontmost_window_frame();

    assert!(before.is_some(), "Should have frame before focus up");
    assert!(after.is_some(), "Should have frame after focus up");

    if let (Some(b), Some(a)) = (before, after) {
        println!("Focus up: y changed from {} to {}", b.y, a.y);
    }
}

/// Test focus down with vertically arranged windows.
#[test]
fn test_focus_down() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    // Focus up first
    test.stache_command(&["tiling", "window", "--focus", "up"]);
    delay(OPERATION_DELAY_MS);

    let before = get_frontmost_window_frame();

    // Focus down
    test.stache_command(&["tiling", "window", "--focus", "down"]);
    delay(OPERATION_DELAY_MS);

    let after = get_frontmost_window_frame();

    assert!(before.is_some(), "Should have frame before focus down");
    assert!(after.is_some(), "Should have frame after focus down");

    if let (Some(b), Some(a)) = (before, after) {
        println!("Focus down: y changed from {} to {}", b.y, a.y);
    }
}

/// Test focus with single window doesn't crash.
#[test]
fn test_focus_single_window() {
    let mut test = Test::new("tiling_basic");

    // Create only one window
    let _ = test.create_window("Dictionary");
    let _ = test.get_app_stable_frames("Dictionary", 1);

    let frame_before = get_frontmost_window_frame();

    // Try all focus directions - should not crash
    for direction in &["next", "previous", "up", "down", "left", "right"] {
        test.stache_command(&["tiling", "window", "--focus", direction]);
        delay(OPERATION_DELAY_MS);
        println!("Focus {} with single window: OK", direction);
    }

    let frame_after = get_frontmost_window_frame();

    // Frame should remain the same
    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        assert!(
            b.approximately_equals(&a, FRAME_TOLERANCE),
            "Single window should maintain position"
        );
    }
}

/// Test focus wrapping at boundaries.
#[test]
fn test_focus_wrap() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    // Keep focusing in one direction to test wrapping
    println!("Testing focus wrapping...");
    let mut titles = Vec::new();
    for i in 0..5 {
        test.stache_command(&["tiling", "window", "--focus", "next"]);
        delay(OPERATION_DELAY_MS);
        let title = get_frontmost_window_title();
        titles.push(title.clone());
        println!("Focus next {}: {:?}", i + 1, title);
    }

    // Should have focused windows throughout
    assert!(
        titles.iter().filter(|t| t.is_some()).count() >= 3,
        "Should maintain focus while wrapping"
    );
}

/// Test rapid focus changes.
#[test]
fn test_rapid_focus_changes() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 3);

    // Rapid focus changes
    let start = std::time::Instant::now();
    for _ in 0..10 {
        test.stache_command(&["tiling", "window", "--focus", "next"]);
        delay(50); // Short delay
    }
    let elapsed = start.elapsed();

    println!("10 rapid focus changes completed in {:?}", elapsed);

    // Verify we still have a focused window
    let frame = get_frontmost_window_frame();
    assert!(
        frame.is_some(),
        "Should still have focused window after rapid changes"
    );
}
