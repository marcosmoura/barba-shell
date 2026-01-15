//! Integration tests for the Split layout.
//!
//! Split layout divides the screen either horizontally or vertically,
//! with each subsequent window taking half of the remaining space.
//!
//! ## Test Coverage
//! - Single window fills area
//! - Two windows split evenly
//! - Multiple windows with cascading splits
//! - Window removal triggers relayout
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_split/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in split layout fills the tiling area.
#[test]
fn test_split_single_window_fills_area() {
    let mut test = Test::new("tiling_split");
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

    // Single window should fill the tiling area
    assert!(
        (frame.x - tiling_area.x).abs() <= FRAME_TOLERANCE,
        "Split window X ({}) should be at tiling area X ({})",
        frame.x,
        tiling_area.x
    );

    assert!(
        (frame.width - tiling_area.width).abs() <= FRAME_TOLERANCE,
        "Split window width ({}) should match tiling area width ({})",
        frame.width,
        tiling_area.width
    );

    eprintln!(
        "Split single window: {}x{} at ({}, {})",
        frame.width, frame.height, frame.x, frame.y
    );
    eprintln!("Tiling area: {:?}", tiling_area);
}

/// Test split layout with two windows (50/50 split).
#[test]
fn test_split_two_windows_even() {
    let mut test = Test::new("tiling_split");
    let dictionary = test.app("Dictionary");

    // Create two windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames
    let frames = dictionary.get_stable_frames(2);
    assert!(frames.len() >= 2, "Should have at least 2 windows");

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

    // Windows should have similar sizes (within tolerance)
    // In split, either widths or heights should be similar
    let width_similar = (frame1.width - frame2.width).abs() <= FRAME_TOLERANCE * 2;
    let height_similar = (frame1.height - frame2.height).abs() <= FRAME_TOLERANCE * 2;

    assert!(
        width_similar || height_similar,
        "Split windows should have at least one similar dimension: {} vs {}",
        frame1,
        frame2
    );

    // Both windows should have similar areas (50/50 split)
    let area1 = (frame1.width * frame1.height) as f64;
    let area2 = (frame2.width * frame2.height) as f64;
    let area_ratio = area1 / area2;

    assert!(
        area_ratio > 0.7 && area_ratio < 1.3,
        "Split should be roughly 50/50: ratio = {:.2}",
        area_ratio
    );

    eprintln!(
        "Split two windows:\n  Window 1: {}\n  Window 2: {}",
        frame1, frame2
    );
    eprintln!("Area ratio: {:.2}", area_ratio);
}

/// Test split layout with three windows.
#[test]
fn test_split_three_windows() {
    let mut test = Test::new("tiling_split");
    let dictionary = test.app("Dictionary");

    // Create three windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Get stable frames
    let frames = dictionary.get_stable_frames(3);
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

    // Windows should not significantly overlap
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

                assert!(
                    overlap_area < 100,
                    "Windows {} and {} should not overlap significantly",
                    i + 1,
                    j + 1
                );
            }
        }
    }

    eprintln!("Split three windows:");
    for (i, frame) in frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }
}

/// Test split layout maintains proportions after window removal.
#[test]
fn test_split_window_removal_relayout() {
    let mut test = Test::new("tiling_split");
    let dictionary = test.app("Dictionary");

    // Create three windows
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();
    let _ = dictionary.create_window();

    // Wait for initial layout
    let initial_frames = dictionary.get_stable_frames(3);
    assert!(
        initial_frames.len() >= 3,
        "Should have at least 3 windows initially"
    );

    eprintln!("Initial 3-window layout:");
    for (i, frame) in initial_frames.iter().enumerate() {
        eprintln!("  Window {}: {}", i + 1, frame);
    }

    // Get fresh window references and close one
    let windows = dictionary.get_windows();
    assert!(windows.len() >= 3, "Should have window refs");

    let mut window_to_close = windows.into_iter().last().expect("Should have window to close");
    assert!(window_to_close.close(), "Should be able to close window");

    // Wait for relayout with 2 windows
    let final_frames = dictionary.get_stable_frames(2);
    assert!(
        final_frames.len() == 2,
        "Should have 2 windows after closing, got {}",
        final_frames.len()
    );

    // Remaining windows should have reasonable sizes
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

/// Test split layout with windows from multiple applications.
///
/// This verifies that tiling works correctly when windows from different
/// apps (Dictionary and TextEdit) are mixed together.
#[test]
fn test_split_multiple_apps() {
    let mut test = Test::new("tiling_split");

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

    // All windows in split layout should have similar widths (within 50% tolerance)
    // This verifies they're all part of the same split layout
    let first_width = all_frames[0].width;
    for (i, frame) in all_frames.iter().enumerate().skip(1) {
        let ratio = frame.width as f64 / first_width as f64;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Window {} width ({}) should be similar to window 1 width ({})",
            i + 1,
            frame.width,
            first_width
        );
    }

    eprintln!("Multi-app split layout (4 windows from 2 apps):");
    eprintln!("  Dictionary windows:");
    for (i, frame) in dict_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
    eprintln!("  TextEdit windows:");
    for (i, frame) in textedit_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
}
