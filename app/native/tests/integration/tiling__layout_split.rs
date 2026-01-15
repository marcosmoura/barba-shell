//! Integration tests for the Split layout.
//!
//! Split layout divides the screen either horizontally or vertically,
//! with each subsequent window taking half of the remaining space.
//!
//! ## Test Coverage
//! - Single window fills area
//! - Two windows split evenly
//! - Multiple windows with cascading splits
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_split/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in split layout fills the area.
#[test]
fn test_split_single_window() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_split");
    delay(STACHE_INIT_DELAY_MS);

    // Create a TextEdit window
    let window = fixture.create_textedit("Split Single");
    assert!(window.is_some(), "Failed to create TextEdit window");
    delay(OPERATION_DELAY_MS * 2);

    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    if let Some((screen_w, screen_h)) = get_screen_size() {
        assert!(
            frame.width > screen_w * 0.8,
            "Single split window should fill width: {} vs {}",
            frame.width,
            screen_w
        );
        println!(
            "Split single window: {}x{} (screen: {}x{})",
            frame.width, frame.height, screen_w, screen_h
        );
    }
}

/// Test split layout with two windows (50/50 split).
#[test]
fn test_split_two_windows_even() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_split");
    delay(STACHE_INIT_DELAY_MS);

    // Create two windows
    let _w1 = fixture.create_textedit("Split Left");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Split Right");
    delay(OPERATION_DELAY_MS * 2);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 2, "Should have at least 2 windows");

    // Windows should have similar widths (horizontal split) or heights (vertical split)
    let f1 = &frames[0];
    let f2 = &frames[1];

    let width_similar = (f1.width - f2.width).abs() < FRAME_TOLERANCE * 2.0;
    let height_similar = (f1.height - f2.height).abs() < FRAME_TOLERANCE * 2.0;

    println!(
        "Split windows: {}x{} and {}x{}",
        f1.width, f1.height, f2.width, f2.height
    );

    // At least one dimension should be similar (indicating a split)
    assert!(
        width_similar || height_similar,
        "Split windows should have at least one similar dimension"
    );

    // Both windows should have similar areas (50/50 split)
    let area_ratio = f1.area() / f2.area();
    assert!(
        area_ratio > 0.7 && area_ratio < 1.3,
        "Split should be roughly 50/50: ratio = {:.2}",
        area_ratio
    );
}

/// Test split layout with three windows.
#[test]
fn test_split_three_windows() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_split");
    delay(STACHE_INIT_DELAY_MS);

    // Create three windows
    let _w1 = fixture.create_textedit("Split 1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Split 2");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("Split 3");
    delay(OPERATION_DELAY_MS * 2);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 3, "Should have at least 3 windows");

    // All windows should have reasonable sizes
    for (i, frame) in frames.iter().take(3).enumerate() {
        assert!(
            frame.width > 100.0 && frame.height > 100.0,
            "Window {} should have reasonable size",
            i
        );
        println!(
            "Split window {}: ({:.0}, {:.0}) {}x{}",
            i, frame.x, frame.y, frame.width, frame.height
        );
    }

    // Calculate coverage
    let total_area: f64 = frames.iter().take(3).map(|f| f.area()).sum();
    if let Some((screen_w, screen_h)) = get_screen_size() {
        let coverage = total_area / (screen_w * screen_h);
        println!("Split layout coverage: {:.1}%", coverage * 100.0);
    }
}

/// Test split layout maintains proportions after window removal.
#[test]
fn test_split_window_removal() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_split");
    delay(STACHE_INIT_DELAY_MS);

    // Create three windows
    let _w1 = fixture.create_textedit("Persist 1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Persist 2");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("ToRemove");
    delay(OPERATION_DELAY_MS * 2);

    let initial_count = get_app_window_count("TextEdit");
    assert!(initial_count >= 3, "Should have at least 3 windows");

    // Close one window
    let _ = run_applescript(
        r#"
        tell application "TextEdit"
            close front window saving no
        end tell
        "#,
    );
    delay(OPERATION_DELAY_MS * 2);

    let new_frames = get_app_window_frames("TextEdit");
    assert!(
        new_frames.len() >= 2,
        "Should have at least 2 windows after removal"
    );

    // Remaining windows should have reasonable sizes
    for (i, frame) in new_frames.iter().enumerate() {
        assert!(
            frame.width > 100.0 && frame.height > 100.0,
            "Window {} should maintain reasonable size after relayout",
            i
        );
    }

    println!("After removal: {} windows remaining", new_frames.len());
}
