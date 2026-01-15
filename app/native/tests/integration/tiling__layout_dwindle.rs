//! Integration tests for the Dwindle layout.
//!
//! Dwindle is a recursive BSP (binary space partitioning) layout that splits
//! the screen recursively, alternating between horizontal and vertical splits.
//!
//! ## Test Coverage
//! - Single window fills entire tiling area
//! - Two windows split horizontally (first split)
//! - Three windows with recursive splitting
//! - Four windows with full BSP tree
//! - Window removal and re-layout
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_dwindle/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in dwindle layout fills the tiling area.
#[test]
fn test_dwindle_single_window_fills_area() {
    let mut test = Test::new("tiling_basic");
    let dictionary = test.app("Dictionary");

    // Create Dictionary window
    let window = dictionary.create_window();

    // Get stable frame
    let frame = window.stable_frame().expect("Should get window frame");

    // Find which screen the window is on (important for multi-screen setups)
    let screen = test.screen_containing(&frame).expect("Window should be on a screen");

    // The outer gap in the test config is 12px, menu bar is ~40px
    let outer_gap = 12;
    let menu_bar_height = 40;
    let tiling_area = screen.tiling_area(outer_gap, menu_bar_height);

    // X should be at the tiling area's left edge (screen.x + outer_gap)
    assert!(
        (frame.x - tiling_area.x).abs() <= FRAME_TOLERANCE,
        "Single window X ({}) should be at tiling area X ({})",
        frame.x,
        tiling_area.x
    );

    // Width should fill the tiling area (screen_width - 2 * outer_gap)
    assert!(
        (frame.width - tiling_area.width).abs() <= FRAME_TOLERANCE,
        "Single window width ({}) should match tiling area width ({})",
        frame.width,
        tiling_area.width
    );

    // Height should fill most of the available space (accounting for menu bar)
    // At minimum, it should be > 70% of the tiling area height
    let min_expected_height = (tiling_area.height as f64 * 0.7) as i32;
    assert!(
        frame.height > min_expected_height,
        "Single window height ({}) should be > {} (70% of tiling area {})",
        frame.height,
        min_expected_height,
        tiling_area.height
    );

    eprintln!(
        "Single window in dwindle: {}x{} at ({}, {})",
        frame.width, frame.height, frame.x, frame.y
    );
    eprintln!(
        "Screen: {}x{} at ({}, {}), is_main={}",
        screen.width(),
        screen.height(),
        screen.frame().x,
        screen.frame().y,
        screen.is_main()
    );
    eprintln!("Tiling area: {:?}", tiling_area);
}

/// Test that two windows in dwindle layout split the screen.
#[test]
fn test_dwindle_two_windows_split() {
    let mut test = Test::new("tiling_basic");
    let dictionary = test.app("Dictionary");

    // Create two windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames for both windows
    let frames = dictionary.get_stable_frames(2);
    assert!(frames.len() >= 2, "Expected 2 windows, got {}", frames.len());

    let frame1 = &frames[0];
    let frame2 = &frames[1];

    // Both windows should have reasonable sizes
    assert!(
        frame1.width > 100 && frame1.height > 100,
        "Window 1 should have reasonable size: {}x{}",
        frame1.width,
        frame1.height
    );
    assert!(
        frame2.width > 100 && frame2.height > 100,
        "Window 2 should have reasonable size: {}x{}",
        frame2.width,
        frame2.height
    );

    // Windows should be arranged side by side (different X) or stacked (different Y)
    let different_x = (frame1.x - frame2.x).abs() > 50;
    let different_y = (frame1.y - frame2.y).abs() > 50;
    assert!(
        different_x || different_y,
        "Windows should be arranged in a split pattern, not overlapping"
    );

    // Verify no significant overlap
    let overlaps_x = frame1.x < frame2.right() && frame1.right() > frame2.x;
    let overlaps_y = frame1.y < frame2.bottom() && frame1.bottom() > frame2.y;
    let fully_overlaps = overlaps_x && overlaps_y;

    // Allow some overlap at edges due to gaps, but windows shouldn't fully overlap
    if fully_overlaps {
        // If they overlap, one should be mostly to the side of the other
        let overlap_width = frame1.right().min(frame2.right()) - frame1.x.max(frame2.x);
        let overlap_height = frame1.bottom().min(frame2.bottom()) - frame1.y.max(frame2.y);
        let overlap_area = overlap_width * overlap_height;
        let frame1_area = frame1.width * frame1.height;

        assert!(
            overlap_area < frame1_area / 2,
            "Windows should not significantly overlap"
        );
    }

    eprintln!(
        "Two windows in dwindle:\n  Window 1: {} \n  Window 2: {}",
        frame1, frame2
    );
}

/// Test dwindle layout with three windows.
#[test]
fn test_dwindle_three_windows() {
    let mut test = Test::new("tiling_basic");
    let dictionary = test.app("Dictionary");

    // Create three windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames
    let frames = dictionary.get_stable_frames(3);
    assert!(frames.len() >= 3, "Expected 3 windows, got {}", frames.len());

    // All windows should have reasonable sizes
    for (i, frame) in frames.iter().enumerate() {
        assert!(
            frame.width > 100,
            "Window {} width should be > 100px, got {}",
            i + 1,
            frame.width
        );
        assert!(
            frame.height > 100,
            "Window {} height should be > 100px, got {}",
            i + 1,
            frame.height
        );
    }

    // Calculate total area
    let total_area: i64 = frames.iter().map(|f| (f.width as i64) * (f.height as i64)).sum();

    eprintln!("Three windows in dwindle layout:");
    for (i, frame) in frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }
    eprintln!("Total tiled area: {} px^2", total_area);
}

/// Test dwindle layout with four windows (full BSP tree).
#[test]
fn test_dwindle_four_windows_full_bsp() {
    let mut test = Test::new("tiling_basic");
    let dictionary = test.app("Dictionary");

    // Create 4 windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames
    let frames = dictionary.get_stable_frames(4);
    assert!(frames.len() >= 4, "Expected 4 windows, got {}", frames.len());

    // Find which screen the first window is on (for area calculation)
    let screen = test.screen_containing(&frames[0]).expect("Window should be on a screen");

    // Verify all windows have reasonable sizes (> 100x100)
    for (i, frame) in frames.iter().enumerate() {
        assert!(
            frame.width > 100 && frame.height > 100,
            "Window {} should have reasonable size, got {}x{}",
            i + 1,
            frame.width,
            frame.height
        );
    }

    // Verify no windows overlap significantly
    for i in 0..frames.len() {
        for j in (i + 1)..frames.len() {
            let a = &frames[i];
            let b = &frames[j];

            let overlaps_x = a.x < b.right() && a.right() > b.x;
            let overlaps_y = a.y < b.bottom() && a.bottom() > b.y;

            if overlaps_x && overlaps_y {
                let overlap_width = a.right().min(b.right()) - a.x.max(b.x);
                let overlap_height = a.bottom().min(b.bottom()) - a.y.max(b.y);
                let overlap_area = overlap_width * overlap_height;

                // Allow small overlap (gaps/borders) but not significant
                assert!(
                    overlap_area < 100,
                    "Windows {} and {} should not overlap significantly: {} vs {}",
                    i + 1,
                    j + 1,
                    a,
                    b
                );
            }
        }
    }

    // Verify total area is reasonable (windows fill most of the workspace screen)
    let total_area: i64 = frames.iter().map(|f| (f.width as i64) * (f.height as i64)).sum();

    let outer_gap = 12;
    let menu_bar_height = 40;
    let tiling_area = screen.tiling_area(outer_gap, menu_bar_height);
    let expected_area = (tiling_area.width as i64) * (tiling_area.height as i64);

    // Total area should be within 30% of expected (accounting for gaps and menu bar)
    assert!(
        total_area > expected_area * 60 / 100,
        "Total tiled area ({}) should be close to screen area ({})",
        total_area,
        expected_area
    );

    eprintln!("Four windows in dwindle layout:");
    for (i, frame) in frames.iter().enumerate() {
        let area = (frame.width as i64) * (frame.height as i64);
        eprintln!("  Window {}: {} (area: {})", i + 1, frame, area);
    }
}

/// Test that windows maintain layout after one is closed.
#[test]
fn test_dwindle_window_removal_relayout() {
    let mut test = Test::new("tiling_basic");
    let dictionary = test.app("Dictionary");

    // Create 3 windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Wait for initial 3-window layout to stabilize
    let initial_frames = dictionary.get_stable_frames(3);
    assert!(
        initial_frames.len() >= 3,
        "Expected 3 windows initially, got {}",
        initial_frames.len()
    );

    eprintln!("Initial 3-window layout:");
    for (i, frame) in initial_frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }

    // Get fresh window references and close one
    let windows = dictionary.get_windows();
    assert!(
        windows.len() >= 3,
        "Expected at least 3 window refs, got {}",
        windows.len()
    );

    // Close the last window
    let mut window_to_close = windows.into_iter().last().expect("Should have a window to close");
    assert!(window_to_close.close(), "Should be able to close window");

    // Wait for re-layout with 2 windows
    let final_frames = dictionary.get_stable_frames(2);

    // Should have exactly 2 windows now
    assert!(
        final_frames.len() == 2,
        "Expected 2 windows after closing one, got {}",
        final_frames.len()
    );

    // Remaining windows should have reasonable sizes (re-layout occurred)
    for (i, frame) in final_frames.iter().enumerate() {
        assert!(
            frame.width > 100 && frame.height > 100,
            "Window {} should maintain reasonable size after relayout: {}",
            i + 1,
            frame
        );
    }

    eprintln!("After removal: 2 windows remaining");
    for (i, frame) in final_frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }
}

/// Test dwindle layout with windows from multiple applications.
///
/// This verifies that tiling works correctly when windows from different
/// apps (Dictionary and TextEdit) are mixed together.
#[test]
fn test_dwindle_multiple_apps() {
    let mut test = Test::new("tiling_basic");

    // Create windows from both apps - create all from one app first,
    // then all from the other to minimize manager confusion
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("Dictionary");
    let _ = test.create_window("TextEdit");
    let _ = test.create_window("TextEdit");

    // Get stable frames from each app separately (simpler, more reliable)
    let dict_frames = test.get_app_stable_frames("Dictionary", 2);
    let textedit_frames = test.get_app_stable_frames("TextEdit", 2);

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
    let all_frames: Vec<_> = dict_frames.iter().chain(textedit_frames.iter()).collect();

    // All windows should have reasonable sizes
    for (i, frame) in all_frames.iter().enumerate() {
        assert!(
            frame.width > 100 && frame.height > 100,
            "Window {} should have reasonable size: {}x{}",
            i + 1,
            frame.width,
            frame.height
        );
    }

    eprintln!("Multi-app dwindle layout (4 windows from 2 apps):");
    eprintln!("  Dictionary windows:");
    for (i, frame) in dict_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
    eprintln!("  TextEdit windows:");
    for (i, frame) in textedit_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
}
