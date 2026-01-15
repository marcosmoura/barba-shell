//! Integration tests for window operations (swap, resize).
//!
//! Tests window manipulation operations including swapping positions
//! with adjacent windows and resizing window dimensions.
//!
//! ## Test Coverage
//! - Swap with next/previous window
//! - Swap in direction (up, down, left, right)
//! - Resize width (increase/decrease)
//! - Resize height (increase/decrease)
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__window_operations/)' --test-threads 1 --no-capture
//! ```

use crate::common::*;

/// Test swapping with next window.
#[test]
fn test_swap_next() {
    let mut test = Test::new("tiling_basic");

    // Create two windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    // Wait for stable frames
    let frames_before = test.get_app_stable_frames("Dictionary", 2);
    assert!(frames_before.len() >= 2, "Should have at least 2 windows");

    let title_before = get_frontmost_window_title();
    let frame_before = get_frontmost_window_frame();

    // Swap with next
    test.stache_command(&["tiling", "window", "--swap", "next"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    let title_after = get_frontmost_window_title();

    assert!(frame_before.is_some(), "Should have frame before swap");
    assert!(frame_after.is_some(), "Should have frame after swap");

    println!(
        "Swap next: '{}' at ({}, {}) -> '{}' at ({}, {})",
        title_before.as_deref().unwrap_or("?"),
        frame_before.as_ref().map(|f| f.x).unwrap_or(0),
        frame_before.as_ref().map(|f| f.y).unwrap_or(0),
        title_after.as_deref().unwrap_or("?"),
        frame_after.as_ref().map(|f| f.x).unwrap_or(0),
        frame_after.as_ref().map(|f| f.y).unwrap_or(0),
    );
}

/// Test swapping with previous window.
#[test]
fn test_swap_previous() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    let frame_before = get_frontmost_window_frame();

    // Swap with previous
    test.stache_command(&["tiling", "window", "--swap", "previous"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();

    assert!(frame_before.is_some(), "Should have frame before swap");
    assert!(frame_after.is_some(), "Should have frame after swap");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let position_changed = (a.x - b.x).abs() > 10 || (a.y - b.y).abs() > 10;
        println!(
            "Swap previous - position changed: {} (delta: {}, {})",
            position_changed,
            a.x - b.x,
            a.y - b.y
        );
    }
}

/// Test swap left.
#[test]
fn test_swap_left() {
    let mut test = Test::new("tiling_basic");

    // Create two windows (side by side in dwindle)
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    // Right window should be focused (last created)
    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before swap");

    // Swap left
    test.stache_command(&["tiling", "window", "--swap", "left"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after swap");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        println!("Swap left: x changed from {} to {}", b.x, a.x);
    }
}

/// Test swap right.
#[test]
fn test_swap_right() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    // Focus left window
    test.stache_command(&["tiling", "window", "--focus", "left"]);
    delay(OPERATION_DELAY_MS);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before swap");

    // Swap right
    test.stache_command(&["tiling", "window", "--swap", "right"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after swap");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        println!("Swap right: x changed from {} to {}", b.x, a.x);
    }
}

/// Test resize width increase.
#[test]
fn test_resize_width_increase() {
    let mut test = Test::new("tiling_basic");

    // Create two windows to have something to resize against
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Increase width (positive amount)
    test.stache_command(&["tiling", "window", "--resize", "width", "100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let width_change = a.width - b.width;
        println!(
            "Resize width increase: {} -> {} (change: {})",
            b.width, a.width, width_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 100 && a.height > 100,
            "Window should maintain reasonable size after resize"
        );
    }
}

/// Test resize width decrease.
#[test]
fn test_resize_width_decrease() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Decrease width (negative amount)
    test.stache_command(&["tiling", "window", "--resize", "width", "-100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let width_change = a.width - b.width;
        println!(
            "Resize width decrease: {} -> {} (change: {})",
            b.width, a.width, width_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 50 && a.height > 50,
            "Window should maintain minimum size after resize"
        );
    }
}

/// Test resize height increase.
#[test]
fn test_resize_height_increase() {
    let mut test = Test::new("tiling_basic");

    // Create three windows to have vertical stacking
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 3);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Increase height (positive amount)
    test.stache_command(&["tiling", "window", "--resize", "height", "100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let height_change = a.height - b.height;
        println!(
            "Resize height increase: {} -> {} (change: {})",
            b.height, a.height, height_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 100 && a.height > 100,
            "Window should maintain reasonable size after resize"
        );
    }
}

/// Test resize height decrease.
#[test]
fn test_resize_height_decrease() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Decrease height (negative amount)
    test.stache_command(&["tiling", "window", "--resize", "height", "-100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let height_change = a.height - b.height;
        println!(
            "Resize height decrease: {} -> {} (change: {})",
            b.height, a.height, height_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 50 && a.height > 50,
            "Window should maintain minimum size after resize"
        );
    }
}

/// Test swap with single window (no-op, shouldn't crash).
#[test]
fn test_swap_single_window() {
    let mut test = Test::new("tiling_basic");

    // Create only one window
    let _ = test.create_window("Dictionary");
    let _ = test.get_app_stable_frames("Dictionary", 1);

    let frame_before = get_frontmost_window_frame();

    // Try all swap directions
    for direction in &["next", "previous", "up", "down", "left", "right"] {
        test.stache_command(&["tiling", "window", "--swap", direction]);
        delay(OPERATION_DELAY_MS);
        println!("Swap {} with single window: OK", direction);
    }

    let frame_after = get_frontmost_window_frame();

    // Position should be unchanged
    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        assert!(
            b.approximately_equals(&a, FRAME_TOLERANCE),
            "Single window position should not change on swap"
        );
    }
}

/// Test resize with single window.
#[test]
fn test_resize_single_window() {
    let mut test = Test::new("tiling_basic");

    // Create one window (fills entire space)
    let _ = test.create_window("Dictionary");
    let _ = test.get_app_stable_frames("Dictionary", 1);

    let frame_before = get_frontmost_window_frame();

    // Try resize operations
    test.stache_command(&["tiling", "window", "--resize", "width", "100"]);
    delay(OPERATION_DELAY_MS);
    test.stache_command(&["tiling", "window", "--resize", "height", "-100"]);
    delay(OPERATION_DELAY_MS);

    let frame_after = get_frontmost_window_frame();

    assert!(frame_before.is_some(), "Should have frame before resize");
    assert!(frame_after.is_some(), "Should have frame after resize");

    // With single window filling space, resize may have no effect
    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        println!(
            "Single window resize: {}x{} -> {}x{}",
            b.width, b.height, a.width, a.height
        );
        // Window should still have reasonable size
        assert!(
            a.width > 100 && a.height > 100,
            "Window should maintain reasonable size"
        );
    }
}

/// Test rapid resize operations.
#[test]
fn test_rapid_resize() {
    let mut test = Test::new("tiling_basic");

    // Create windows
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");

    let _ = test.get_app_stable_frames("Dictionary", 2);

    // Rapid resize operations
    let start = std::time::Instant::now();
    for _ in 0..5 {
        test.stache_command(&["tiling", "window", "--resize", "width", "50"]);
        delay(50);
    }
    for _ in 0..5 {
        test.stache_command(&["tiling", "window", "--resize", "width", "-50"]);
        delay(50);
    }
    let elapsed = start.elapsed();

    println!("10 rapid resize operations completed in {:?}", elapsed);

    // Verify window still exists and has reasonable frame
    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Window should still exist after rapid resize");

    let frame = frame.unwrap();
    assert!(
        frame.width > 100 && frame.height > 100,
        "Window should maintain reasonable size"
    );
}
