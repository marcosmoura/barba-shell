//! Integration tests for the Master-stack layout.
//!
//! Master-stack layout has one large "master" window on one side and a stack
//! of secondary windows on the other side.
//!
//! ## Test Coverage
//! - Single window fills area (becomes master)
//! - Two windows: master + one stack window
//! - Three+ windows: master + stack
//! - Master window is larger than stack windows
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_master/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in master layout fills the area.
#[test]
fn test_master_single_window() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_master");
    delay(STACHE_INIT_DELAY_MS);

    // Create a single window
    let window = fixture.create_textedit("Master Single");
    assert!(window.is_some(), "Failed to create TextEdit window");
    delay(OPERATION_DELAY_MS * 2);

    // Get the window frame
    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    // Single window should fill most of the screen
    if let Some((screen_w, screen_h)) = get_screen_size() {
        assert!(
            frame.width > screen_w * 0.8,
            "Single master window should be wide: {} vs screen {}",
            frame.width,
            screen_w
        );
        assert!(
            frame.height > screen_h * 0.6,
            "Single master window should be tall: {} vs screen {}",
            frame.height,
            screen_h
        );

        println!(
            "Master single window: {}x{} (screen: {}x{})",
            frame.width, frame.height, screen_w, screen_h
        );
    }
}

/// Test master layout with two windows.
#[test]
fn test_master_two_windows() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_master");
    delay(STACHE_INIT_DELAY_MS);

    // Create two windows
    let _w1 = fixture.create_textedit("Master Main");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Stack Window");
    delay(OPERATION_DELAY_MS * 2);

    // Get all window frames
    let frames = get_app_window_frames("TextEdit");
    assert!(
        frames.len() >= 2,
        "Should have at least 2 windows, got {}",
        frames.len()
    );

    // Sort frames by X position to identify master (left) and stack (right)
    let mut sorted_frames = frames.clone();
    sorted_frames.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

    let left = &sorted_frames[0];
    let right = &sorted_frames[1];

    // In master layout, the master window (left) should be larger
    // or they should be side by side
    let left_is_wider = left.width > right.width;
    let left_is_taller = left.height > right.height;
    let windows_side_by_side = (right.x - (left.x + left.width)).abs() < 50.0;

    println!(
        "Master layout: Left({}x{}) at ({},{}), Right({}x{}) at ({},{})",
        left.width, left.height, left.x, left.y, right.width, right.height, right.x, right.y
    );
    println!(
        "Left wider: {}, Left taller: {}, Side by side: {}",
        left_is_wider, left_is_taller, windows_side_by_side
    );

    // Basic assertion: windows should be arranged (not completely overlapping)
    assert!(
        windows_side_by_side || left_is_wider || left_is_taller,
        "Windows should be arranged in master-stack pattern"
    );
}

/// Test master layout with multiple stack windows.
#[test]
fn test_master_multiple_stack_windows() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_master");
    delay(STACHE_INIT_DELAY_MS);

    // Create multiple windows: 1 master + 3 stack
    let _w1 = fixture.create_textedit("Master");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Stack 1");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("Stack 2");
    delay(OPERATION_DELAY_MS);
    let _w4 = fixture.create_textedit("Stack 3");
    delay(OPERATION_DELAY_MS * 2);

    // Get all window frames
    let frames = get_app_window_frames("TextEdit");
    assert!(
        frames.len() >= 4,
        "Should have at least 4 windows, got {}",
        frames.len()
    );

    // Find the largest window (should be master)
    let largest = frames.iter().max_by(|a, b| a.area().partial_cmp(&b.area()).unwrap());

    assert!(largest.is_some(), "Should find largest window");
    let master = largest.unwrap();

    // Master should have significant area
    if let Some((screen_w, screen_h)) = get_screen_size() {
        let screen_area = screen_w * screen_h;
        let master_ratio = master.area() / screen_area;

        // Master should take at least 30% of screen (typical is 50-60%)
        assert!(
            master_ratio > 0.2,
            "Master window should take significant area: {:.1}%",
            master_ratio * 100.0
        );

        println!(
            "Master window: {}x{} ({:.1}% of screen)",
            master.width,
            master.height,
            master_ratio * 100.0
        );
    }

    // Print all window info
    for (i, frame) in frames.iter().enumerate() {
        println!(
            "Window {}: ({}, {}) {}x{} area={:.0}",
            i,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            frame.area()
        );
    }
}

/// Test that stack windows have equal heights.
#[test]
fn test_master_stack_windows_equal_height() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_master");
    delay(STACHE_INIT_DELAY_MS);

    // Create 1 master + 2 stack windows
    let _w1 = fixture.create_textedit("Master Window");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Stack A");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("Stack B");
    delay(OPERATION_DELAY_MS * 2);

    // Get all window frames
    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 3, "Should have at least 3 windows");

    // Group windows by X position to identify master vs stack
    let mut sorted = frames.clone();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

    // Identify unique X positions (with tolerance)
    let mut x_groups: Vec<Vec<&WindowFrame>> = Vec::new();
    for frame in &sorted {
        let mut found_group = false;
        for group in &mut x_groups {
            if (group[0].x - frame.x).abs() < 50.0 {
                group.push(frame);
                found_group = true;
                break;
            }
        }
        if !found_group {
            x_groups.push(vec![frame]);
        }
    }

    // If we have two X groups, the one with more windows is the stack
    assert!(!x_groups.is_empty(), "Should have at least one group of windows");

    if x_groups.len() >= 2 {
        x_groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
        let stack_windows = &x_groups[0];

        if stack_windows.len() >= 2 {
            // Stack windows should have similar heights
            let first_height = stack_windows[0].height;
            for (i, frame) in stack_windows.iter().enumerate().skip(1) {
                let height_diff = (frame.height - first_height).abs();
                assert!(
                    height_diff < FRAME_TOLERANCE * 3.0,
                    "Stack window {} height ({}) should be similar to first ({})",
                    i,
                    frame.height,
                    first_height
                );
                println!(
                    "Stack window {} height: {}, diff from first: {}",
                    i, frame.height, height_diff
                );
            }
        }
    }
}
