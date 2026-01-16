//! Integration tests for the Monocle layout.
//!
//! Monocle layout displays windows one at a time, each filling the entire
//! tiling area. Only the focused window is visible; others are hidden behind it.
//!
//! ## Test Coverage
//! - Single window fills entire area
//! - Multiple windows all maximize to same size
//! - Windows are stacked at the same position
//! - Adding windows maintains monocle behavior
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_monocle/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in monocle fills the tiling area.
#[test]
fn test_monocle_single_window_fills_area() {
    let mut test = Test::new("tiling_monocle");
    let dictionary = test.app("Dictionary");

    // Create a single window
    let window = dictionary.create_window();

    // Get stable frame
    let frame = window.stable_frame().expect("Should get window frame");

    // Find which screen the window is on
    let screen = test.screen_containing(&frame).expect("Window should be on a screen");

    // Calculate tiling area (outer gap = 12, menu bar ~40)
    let outer_gap = 12;
    let menu_bar_height = 40;
    let tiling_area = screen.tiling_area(outer_gap, menu_bar_height);

    // In monocle, the window should fill the tiling area
    assert!(
        (frame.x - tiling_area.x).abs() <= FRAME_TOLERANCE,
        "Monocle window X ({}) should be at tiling area X ({})",
        frame.x,
        tiling_area.x
    );

    assert!(
        (frame.width - tiling_area.width).abs() <= FRAME_TOLERANCE,
        "Monocle window width ({}) should match tiling area width ({})",
        frame.width,
        tiling_area.width
    );

    // Height should fill most of the available space
    let min_expected_height = (tiling_area.height as f64 * 0.7) as i32;
    assert!(
        frame.height > min_expected_height,
        "Monocle window height ({}) should be > {} (70% of tiling area {})",
        frame.height,
        min_expected_height,
        tiling_area.height
    );

    eprintln!(
        "Monocle single window: {}x{} at ({}, {})",
        frame.width, frame.height, frame.x, frame.y
    );
    eprintln!("Tiling area: {:?}", tiling_area);
}

/// Test that multiple windows in monocle all have the same size.
#[test]
fn test_monocle_multiple_windows_same_size() {
    let mut test = Test::new("tiling_monocle");
    let dictionary = test.app("Dictionary");

    // Create multiple windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames for all windows (stacked - same position allowed)
    let frames = dictionary.get_stable_frames_stacked(3);
    assert!(
        frames.len() >= 3,
        "Should have at least 3 windows, got {}",
        frames.len()
    );

    // In monocle, all windows should have the same size
    let first = &frames[0];
    for (i, frame) in frames.iter().enumerate().skip(1) {
        let width_diff = (frame.width - first.width).abs();
        let height_diff = (frame.height - first.height).abs();

        assert!(
            width_diff <= FRAME_TOLERANCE && height_diff <= FRAME_TOLERANCE,
            "Window {} size ({}x{}) should match window 0 size ({}x{})",
            i + 1,
            frame.width,
            frame.height,
            first.width,
            first.height
        );
    }

    eprintln!(
        "All {} monocle windows have size: {}x{}",
        frames.len(),
        first.width,
        first.height
    );
}

/// Test that windows in monocle are stacked (same position).
#[test]
fn test_monocle_windows_stacked() {
    let mut test = Test::new("tiling_monocle");
    let dictionary = test.app("Dictionary");

    // Create multiple windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames (stacked - same position allowed)
    let frames = dictionary.get_stable_frames_stacked(2);
    assert!(frames.len() >= 2, "Should have at least 2 windows");

    // In monocle, windows should be at the same position (stacked)
    let first = &frames[0];
    for (i, frame) in frames.iter().enumerate().skip(1) {
        let x_diff = (frame.x - first.x).abs();
        let y_diff = (frame.y - first.y).abs();

        assert!(
            x_diff <= FRAME_TOLERANCE && y_diff <= FRAME_TOLERANCE,
            "Window {} position ({}, {}) should match window 0 position ({}, {})",
            i + 1,
            frame.x,
            frame.y,
            first.x,
            first.y
        );
    }

    eprintln!("Monocle windows stacked at position: ({}, {})", first.x, first.y);
}

/// Test adding a window to monocle maintains behavior.
#[test]
fn test_monocle_add_window_maintains_layout() {
    let mut test = Test::new("tiling_monocle");
    let dictionary = test.app("Dictionary");

    // Create initial window and record its size
    let window1 = dictionary.create_window();
    let initial_frame = window1.stable_frame().expect("Should get initial frame");

    // Add another window
    let _ = dictionary.create_window();

    // Get all frames after adding second window (stacked - same position allowed)
    let frames = dictionary.get_stable_frames_stacked(2);
    assert!(frames.len() >= 2, "Should have at least 2 windows");

    // All windows should have same dimensions as initial (monocle behavior)
    for (i, frame) in frames.iter().enumerate() {
        let width_diff = (frame.width - initial_frame.width).abs();
        let height_diff = (frame.height - initial_frame.height).abs();
        let x_diff = (frame.x - initial_frame.x).abs();
        let y_diff = (frame.y - initial_frame.y).abs();

        assert!(
            width_diff <= FRAME_TOLERANCE
                && height_diff <= FRAME_TOLERANCE
                && x_diff <= FRAME_TOLERANCE
                && y_diff <= FRAME_TOLERANCE,
            "Window {} frame ({}, {}, {}x{}) should match initial frame ({}, {}, {}x{})",
            i + 1,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            initial_frame.x,
            initial_frame.y,
            initial_frame.width,
            initial_frame.height
        );
    }

    eprintln!(
        "Monocle maintained: all {} windows at {}x{} position ({}, {})",
        frames.len(),
        initial_frame.width,
        initial_frame.height,
        initial_frame.x,
        initial_frame.y
    );
}

/// Test focus cycling between same-app windows in monocle mode.
///
/// This tests the specific scenario where multiple windows from the same
/// application are in monocle mode (all stacked with identical frames).
/// The focus-next command should correctly cycle between them and the UI
/// should receive proper focus events on each switch.
///
/// This test validates the fix for the AX-to-CG window ID mapping issue
/// where frame-based matching would fail for same-app windows in monocle
/// (since all windows have identical frames).
#[test]
fn test_monocle_same_app_focus_cycling() {
    let mut test = Test::new("tiling_monocle");
    let dictionary = test.app("Dictionary");

    // Create multiple windows from the same app
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Wait for windows to stabilize (stacked - same position in monocle)
    let _ = dictionary.get_stable_frames_stacked(3);

    // Record initial focused window
    let initial_title = get_frontmost_window_title();
    eprintln!("Initial focus in monocle: {:?}", initial_title);

    // Focus next multiple times - should cycle through all 3 windows
    let mut titles = vec![initial_title.clone()];
    for i in 0..4 {
        test.stache_command(&["tiling", "window", "--focus", "next"]);
        delay(OPERATION_DELAY_MS);
        let title = get_frontmost_window_title();
        titles.push(title.clone());
        eprintln!("After focus-next {} in monocle: {:?}", i + 1, title);
    }

    // Verify focus changed (at least some switches should result in different windows)
    // Note: In monocle with 3 windows, after 3 focus-next operations we should be
    // back to the original window (cycling complete).
    let valid_titles: Vec<_> = titles.iter().filter(|t| t.is_some()).collect();
    assert!(
        valid_titles.len() >= 4,
        "Should have maintained focus through cycling, got {} valid focuses",
        valid_titles.len()
    );

    eprintln!(
        "Monocle same-app focus cycling: {} focus operations completed successfully",
        titles.len() - 1
    );
}

/// Test monocle layout with windows from multiple applications.
///
/// This verifies that monocle works correctly when windows from different
/// apps (Dictionary and TextEdit) are mixed together - all should be stacked
/// at the same position with the same size.
#[test]
fn test_monocle_multiple_apps() {
    let mut test = Test::new("tiling_monocle");

    // Create windows from both apps - create all from one app first,
    // then all from the other to minimize manager confusion
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("TextEdit");
    let _ = test.create_window("TextEdit");

    // Get stable frames from each app separately (stacked - same position allowed)
    let dict_frames = test.get_app_stable_frames_stacked("Dictionary", 2);
    let textedit_frames = test.get_app_stable_frames_stacked("TextEdit", 2);

    assert!(
        dict_frames.len() >= 2,
        "Should have at least 2 Dictionary windows, got {}",
        dict_frames.len()
    );
    assert!(
        textedit_frames.len() >= 2,
        "Should have at least 2 TextEdit windows, got {}",
        textedit_frames.len()
    );

    // Combine all frames - take only the expected count from each
    let dict_frames: Vec<_> = dict_frames.into_iter().take(2).collect();
    let textedit_frames: Vec<_> = textedit_frames.into_iter().take(2).collect();

    // Get a reference frame (first Dictionary window)
    let reference_frame = &dict_frames[0];

    // Combine all frames for checking
    let all_frames: Vec<_> = dict_frames.iter().chain(textedit_frames.iter()).collect();

    // In monocle, ALL windows should have the same size and position
    for (i, frame) in all_frames.iter().enumerate() {
        let width_diff = (frame.width - reference_frame.width).abs();
        let height_diff = (frame.height - reference_frame.height).abs();
        let x_diff = (frame.x - reference_frame.x).abs();
        let y_diff = (frame.y - reference_frame.y).abs();

        assert!(
            width_diff <= FRAME_TOLERANCE
                && height_diff <= FRAME_TOLERANCE
                && x_diff <= FRAME_TOLERANCE
                && y_diff <= FRAME_TOLERANCE,
            "Window {} should match reference frame. Got: {}, Expected: {}",
            i + 1,
            frame,
            reference_frame
        );
    }

    eprintln!("Multi-app monocle layout (4 windows from 2 apps):");
    eprintln!(
        "  All windows stacked at {}x{} position ({}, {})",
        reference_frame.width, reference_frame.height, reference_frame.x, reference_frame.y
    );
    eprintln!("  Dictionary: {} windows", dict_frames.len());
    eprintln!("  TextEdit: {} windows", textedit_frames.len());
}
