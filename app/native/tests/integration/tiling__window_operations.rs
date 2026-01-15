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
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__window_operations/)' --no-capture
//! ```

use crate::common::*;

/// Test swapping with next window.
#[test]
fn test_swap_next() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create two windows
    let _w1 = fixture.create_textedit("Swap-A");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Swap-B");
    delay(OPERATION_DELAY_MS * 2);

    // Get initial positions
    let frames_before = get_app_window_frames("TextEdit");
    assert!(frames_before.len() >= 2, "Should have at least 2 windows");

    let title_before = get_frontmost_window_title();
    let frame_before = get_frontmost_window_frame();

    // Swap with next
    fixture.stache_command(&["tiling", "window", "--swap", "next"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    let title_after = get_frontmost_window_title();

    assert!(frame_before.is_some(), "Should have frame before swap");
    assert!(frame_after.is_some(), "Should have frame after swap");

    println!(
        "Swap next: '{}' at ({:.0}, {:.0}) -> '{}' at ({:.0}, {:.0})",
        title_before.as_deref().unwrap_or("?"),
        frame_before.as_ref().map(|f| f.x).unwrap_or(0.0),
        frame_before.as_ref().map(|f| f.y).unwrap_or(0.0),
        title_after.as_deref().unwrap_or("?"),
        frame_after.as_ref().map(|f| f.x).unwrap_or(0.0),
        frame_after.as_ref().map(|f| f.y).unwrap_or(0.0),
    );
}

/// Test swapping with previous window.
#[test]
fn test_swap_previous() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create windows
    let _w1 = fixture.create_textedit("SwapPrev-A");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("SwapPrev-B");
    delay(OPERATION_DELAY_MS * 2);

    let frame_before = get_frontmost_window_frame();

    // Swap with previous
    fixture.stache_command(&["tiling", "window", "--swap", "previous"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();

    assert!(frame_before.is_some(), "Should have frame before swap");
    assert!(frame_after.is_some(), "Should have frame after swap");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let position_changed = (a.x - b.x).abs() > 10.0 || (a.y - b.y).abs() > 10.0;
        println!(
            "Swap previous - position changed: {} (delta: {:.0}, {:.0})",
            position_changed,
            a.x - b.x,
            a.y - b.y
        );
    }
}

/// Test swap left.
#[test]
fn test_swap_left() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create two windows (side by side in dwindle)
    let _w1 = fixture.create_textedit("Left");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Right");
    delay(OPERATION_DELAY_MS * 2);

    // Right window should be focused (last created)
    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before swap");

    // Swap left
    fixture.stache_command(&["tiling", "window", "--swap", "left"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after swap");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        println!("Swap left: x changed from {:.0} to {:.0}", b.x, a.x);
    }
}

/// Test swap right.
#[test]
fn test_swap_right() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create windows
    let _w1 = fixture.create_textedit("SwapR-L");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("SwapR-R");
    delay(OPERATION_DELAY_MS * 2);

    // Focus left window
    fixture.stache_command(&["tiling", "window", "--focus", "left"]);
    delay(OPERATION_DELAY_MS);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before swap");

    // Swap right
    fixture.stache_command(&["tiling", "window", "--swap", "right"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after swap");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        println!("Swap right: x changed from {:.0} to {:.0}", b.x, a.x);
    }
}

/// Test resize width increase.
#[test]
fn test_resize_width_increase() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create two windows to have something to resize against
    let _w1 = fixture.create_textedit("Resize-L");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Resize-R");
    delay(OPERATION_DELAY_MS * 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Increase width (positive amount)
    fixture.stache_command(&["tiling", "window", "--resize", "width", "100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let width_change = a.width - b.width;
        println!(
            "Resize width increase: {} -> {} (change: {:.1})",
            b.width, a.width, width_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 100.0 && a.height > 100.0,
            "Window should maintain reasonable size after resize"
        );
    }
}

/// Test resize width decrease.
#[test]
fn test_resize_width_decrease() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create windows
    let _w1 = fixture.create_textedit("Shrink-L");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Shrink-R");
    delay(OPERATION_DELAY_MS * 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Decrease width (negative amount)
    fixture.stache_command(&["tiling", "window", "--resize", "width", "-100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let width_change = a.width - b.width;
        println!(
            "Resize width decrease: {} -> {} (change: {:.1})",
            b.width, a.width, width_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 50.0 && a.height > 50.0,
            "Window should maintain minimum size after resize"
        );
    }
}

/// Test resize height increase.
#[test]
fn test_resize_height_increase() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create three windows to have vertical stacking
    let _w1 = fixture.create_textedit("Tall-1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Tall-2");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("Tall-3");
    delay(OPERATION_DELAY_MS * 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Increase height (positive amount)
    fixture.stache_command(&["tiling", "window", "--resize", "height", "100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let height_change = a.height - b.height;
        println!(
            "Resize height increase: {} -> {} (change: {:.1})",
            b.height, a.height, height_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 100.0 && a.height > 100.0,
            "Window should maintain reasonable size after resize"
        );
    }
}

/// Test resize height decrease.
#[test]
fn test_resize_height_decrease() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create windows
    let _w1 = fixture.create_textedit("Short-1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Short-2");
    delay(OPERATION_DELAY_MS * 2);

    let frame_before = get_frontmost_window_frame();
    assert!(frame_before.is_some(), "Should have frame before resize");

    // Decrease height (negative amount)
    fixture.stache_command(&["tiling", "window", "--resize", "height", "-100"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame_after = get_frontmost_window_frame();
    assert!(frame_after.is_some(), "Should have frame after resize");

    if let (Some(b), Some(a)) = (frame_before, frame_after) {
        let height_change = a.height - b.height;
        println!(
            "Resize height decrease: {} -> {} (change: {:.1})",
            b.height, a.height, height_change
        );
        // Window should still have reasonable size
        assert!(
            a.width > 50.0 && a.height > 50.0,
            "Window should maintain minimum size after resize"
        );
    }
}

/// Test swap with single window (no-op, shouldn't crash).
#[test]
fn test_swap_single_window() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create only one window
    let _window = fixture.create_textedit("Alone");
    delay(OPERATION_DELAY_MS);

    let frame_before = get_frontmost_window_frame();

    // Try all swap directions
    for direction in &["next", "previous", "up", "down", "left", "right"] {
        fixture.stache_command(&["tiling", "window", "--swap", direction]);
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
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create one window (fills entire space)
    let _window = fixture.create_textedit("Full");
    delay(OPERATION_DELAY_MS * 2);

    let frame_before = get_frontmost_window_frame();

    // Try resize operations
    fixture.stache_command(&["tiling", "window", "--resize", "width", "100"]);
    delay(OPERATION_DELAY_MS);
    fixture.stache_command(&["tiling", "window", "--resize", "height", "-100"]);
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
            a.width > 100.0 && a.height > 100.0,
            "Window should maintain reasonable size"
        );
    }
}

/// Test rapid resize operations.
#[test]
fn test_rapid_resize() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_basic");
    delay(STACHE_INIT_DELAY_MS);

    // Create windows
    let _w1 = fixture.create_textedit("Rapid-1");
    let _w2 = fixture.create_textedit("Rapid-2");
    delay(OPERATION_DELAY_MS * 2);

    // Rapid resize operations
    let start = std::time::Instant::now();
    for _ in 0..5 {
        fixture.stache_command(&["tiling", "window", "--resize", "width", "50"]);
        delay(50);
    }
    for _ in 0..5 {
        fixture.stache_command(&["tiling", "window", "--resize", "width", "-50"]);
        delay(50);
    }
    let elapsed = start.elapsed();

    println!("10 rapid resize operations completed in {:?}", elapsed);

    // Verify window still exists and has reasonable frame
    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Window should still exist after rapid resize");

    let frame = frame.unwrap();
    assert!(
        frame.width > 100.0 && frame.height > 100.0,
        "Window should maintain reasonable size"
    );
}
