//! Integration tests for the Floating layout.
//!
//! Floating layout allows windows to be freely positioned and sized.
//! Windows are not automatically tiled but maintain their user-set positions.
//!
//! ## Test Coverage
//! - Windows maintain their position in floating layout
//! - Multiple floating windows can coexist
//! - Multi-app test
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_floating/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in floating layout has reasonable size.
///
/// In floating layout, windows are not auto-tiled to fill the screen,
/// but they should still have reasonable dimensions.
#[test]
fn test_floating_single_window() {
    let mut test = Test::new("tiling_floating");
    let dictionary = test.app("Dictionary");

    // Create a single window
    let window = dictionary.create_window();

    // Get stable frame
    let frame = window.stable_frame().expect("Should get window frame");

    // Window should have reasonable dimensions
    assert!(
        frame.width > 100 && frame.height > 100,
        "Floating window should have reasonable size: {}x{}",
        frame.width,
        frame.height
    );

    eprintln!(
        "Floating single window: {}x{} at ({}, {})",
        frame.width, frame.height, frame.x, frame.y
    );
}

/// Test floating layout with two windows.
///
/// In floating layout, windows are not auto-tiled into a grid,
/// so they may overlap or have any position.
#[test]
fn test_floating_two_windows() {
    let mut test = Test::new("tiling_floating");
    let dictionary = test.app("Dictionary");

    // Create two windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames (in floating, frames can be anywhere)
    // Use stacked variant since floating windows may overlap
    let frames = dictionary.get_stable_frames_stacked(2);
    assert!(frames.len() >= 2, "Should have at least 2 windows");

    // Both windows should have reasonable sizes
    for (i, frame) in frames.iter().enumerate() {
        assert!(
            frame.width > 100 && frame.height > 100,
            "Window {} should have reasonable size: {}x{}",
            i + 1,
            frame.width,
            frame.height
        );
    }

    eprintln!(
        "Floating two windows:\n  Window 1: {}\n  Window 2: {}",
        frames[0], frames[1]
    );
}

/// Test floating layout with three windows.
#[test]
fn test_floating_three_windows() {
    let mut test = Test::new("tiling_floating");
    let dictionary = test.app("Dictionary");

    // Create three windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames (stacked since floating windows may overlap)
    let frames = dictionary.get_stable_frames_stacked(3);
    assert!(frames.len() >= 3, "Should have at least 3 windows");

    // All windows should have reasonable sizes
    for (i, frame) in frames.iter().enumerate() {
        assert!(
            frame.width > 100 && frame.height > 100,
            "Window {} should have reasonable size: {}x{}",
            i + 1,
            frame.width,
            frame.height
        );
    }

    eprintln!("Floating three windows:");
    for (i, frame) in frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }
}

/// Test that window count is maintained after closing a window.
#[test]
fn test_floating_window_removal() {
    let mut test = Test::new("tiling_floating");
    let dictionary = test.app("Dictionary");

    // Create three windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Wait for initial layout
    let initial_frames = dictionary.get_stable_frames_stacked(3);
    assert!(
        initial_frames.len() >= 3,
        "Should have at least 3 windows initially"
    );

    eprintln!("Initial 3-window floating:");
    for (i, frame) in initial_frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }

    // Get fresh window references and close one
    let windows = dictionary.get_windows();
    assert!(windows.len() >= 3, "Should have window refs");

    let mut window_to_close = windows.into_iter().last().expect("Should have window to close");
    assert!(window_to_close.close(), "Should be able to close window");

    // Wait for new state with 2 windows
    let final_frames = dictionary.get_stable_frames_stacked(2);
    assert!(
        final_frames.len() == 2,
        "Should have 2 windows after closing, got {}",
        final_frames.len()
    );

    eprintln!("After removal: 2 windows remaining");
    for (i, frame) in final_frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }
}

/// Test floating layout with windows from multiple applications.
///
/// This verifies that floating layout works correctly when windows from different
/// apps (Dictionary and TextEdit) are mixed together.
#[test]
fn test_floating_multiple_apps() {
    let mut test = Test::new("tiling_floating");

    // Create windows from both apps
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("TextEdit");
    let _ = test.create_window("TextEdit");

    // Get frames from each app (stacked since floating windows may overlap)
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

    // All windows should have reasonable sizes
    let all_frames: Vec<_> = dict_frames.iter().chain(textedit_frames.iter()).collect();
    for (i, frame) in all_frames.iter().enumerate() {
        assert!(
            frame.width > 100 && frame.height > 100,
            "Window {} should have reasonable size: {}x{}",
            i + 1,
            frame.width,
            frame.height
        );
    }

    eprintln!("Multi-app floating layout (4 windows from 2 apps):");
    eprintln!("  Dictionary windows:");
    for (i, frame) in dict_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
    eprintln!("  TextEdit windows:");
    for (i, frame) in textedit_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
}
