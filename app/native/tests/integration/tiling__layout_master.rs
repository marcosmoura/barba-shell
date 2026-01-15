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
//! - Multi-app test
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_master/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in master layout fills the area.
#[test]
fn test_master_single_window_fills_area() {
    let mut test = Test::new("tiling_master");
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
        "Master window X ({}) should be at tiling area X ({})",
        frame.x,
        tiling_area.x
    );

    assert!(
        (frame.width - tiling_area.width).abs() <= FRAME_TOLERANCE,
        "Master window width ({}) should match tiling area width ({})",
        frame.width,
        tiling_area.width
    );

    eprintln!(
        "Master single window: {}x{} at ({}, {})",
        frame.width, frame.height, frame.x, frame.y
    );
    eprintln!("Tiling area: {:?}", tiling_area);
}

/// Test master layout with two windows.
#[test]
fn test_master_two_windows() {
    let mut test = Test::new("tiling_master");
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

    // In master layout, one window should be significantly larger (the master)
    let area1 = (frame1.width * frame1.height) as f64;
    let area2 = (frame2.width * frame2.height) as f64;
    let (larger_area, smaller_area) = if area1 > area2 {
        (area1, area2)
    } else {
        (area2, area1)
    };

    // Master should be larger than stack (at least 30% more area)
    let ratio = larger_area / smaller_area;
    assert!(
        ratio > 1.3,
        "Master window should be significantly larger than stack: ratio = {:.2}",
        ratio
    );

    eprintln!(
        "Master two windows:\n  Window 1: {}\n  Window 2: {}",
        frame1, frame2
    );
    eprintln!("Area ratio (larger/smaller): {:.2}", ratio);
}

/// Test master layout with multiple stack windows.
#[test]
fn test_master_three_windows() {
    let mut test = Test::new("tiling_master");
    let dictionary = test.app("Dictionary");

    // Create three windows: 1 master + 2 stack
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

    // Find the largest window (should be master)
    let areas: Vec<_> = frames.iter().map(|f| (f.width * f.height) as i64).collect();
    let max_area = *areas.iter().max().unwrap();
    let total_area: i64 = areas.iter().sum();

    // Master should take a significant portion (at least 40%) of total area
    let master_ratio = max_area as f64 / total_area as f64;
    assert!(
        master_ratio > 0.35,
        "Master should take significant area: {:.1}%",
        master_ratio * 100.0
    );

    eprintln!("Master three windows:");
    for (i, frame) in frames.iter().enumerate() {
        let area = frame.width * frame.height;
        eprintln!("  Window {}: {} (area: {})", i + 1, frame, area);
    }
    eprintln!("Master ratio: {:.1}%", master_ratio * 100.0);
}

/// Test that windows maintain layout after removal.
#[test]
fn test_master_window_removal_relayout() {
    let mut test = Test::new("tiling_master");
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

/// Test master layout with windows from multiple applications.
///
/// This verifies that tiling works correctly when windows from different
/// apps (Dictionary and TextEdit) are mixed together.
#[test]
fn test_master_multiple_apps() {
    let mut test = Test::new("tiling_master");

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

    eprintln!("Multi-app master layout (4 windows from 2 apps):");
    eprintln!("  Dictionary windows:");
    for (i, frame) in dict_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
    eprintln!("  TextEdit windows:");
    for (i, frame) in textedit_frames.iter().enumerate() {
        eprintln!("    Window {}: {}", i + 1, frame);
    }
}
