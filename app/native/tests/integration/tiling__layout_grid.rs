//! Integration tests for the Grid layout.
//!
//! Grid layout arranges windows in a grid pattern, attempting to use
//! rows and columns that best fit the number of windows.
//!
//! ## Test Coverage
//! - Single window fills area
//! - Two windows: 1 row, 2 columns
//! - Four windows: 2x2 grid
//! - Windows have similar sizes in grid
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_grid/)' --no-capture
//! ```

use crate::common::*;

/// Test that a single window in grid layout fills the area.
#[test]
fn test_grid_single_window() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_grid");
    delay(STACHE_INIT_DELAY_MS);

    // Create a single window
    let window = fixture.create_textedit("Grid Single");
    assert!(window.is_some(), "Failed to create TextEdit window");
    delay(OPERATION_DELAY_MS);

    // Re-activate TextEdit to force a focus event and ensure the window ID swap occurs
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    if let Some((screen_w, screen_h)) = get_screen_size() {
        assert!(
            frame.width > screen_w * 0.8,
            "Single grid window should fill width"
        );
        println!(
            "Grid single window: {}x{} (screen: {}x{})",
            frame.width, frame.height, screen_w, screen_h
        );
    }
}

/// Test grid layout with two windows (1x2 arrangement).
#[test]
fn test_grid_two_windows() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_grid");
    delay(STACHE_INIT_DELAY_MS);

    // Create two windows
    let _w1 = fixture.create_textedit("Grid Left");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Grid Right");
    delay(OPERATION_DELAY_MS * 2);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 2, "Should have at least 2 windows");

    // Both windows should have similar sizes
    let width_diff = (frames[0].width - frames[1].width).abs();
    let height_diff = (frames[0].height - frames[1].height).abs();

    println!(
        "Grid 2 windows: {}x{} and {}x{}",
        frames[0].width, frames[0].height, frames[1].width, frames[1].height
    );
    println!(
        "Differences - width: {:.1}, height: {:.1}",
        width_diff, height_diff
    );

    // Windows should have similar dimensions (within tolerance)
    assert!(
        width_diff < FRAME_TOLERANCE * 2.0 && height_diff < FRAME_TOLERANCE * 2.0,
        "Grid windows should have similar sizes"
    );
}

/// Test grid layout with four windows (2x2 arrangement).
#[test]
fn test_grid_four_windows_2x2() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_grid");
    delay(STACHE_INIT_DELAY_MS);

    // Create four windows
    for i in 1..=4 {
        let _w = fixture.create_textedit(&format!("Grid {}", i));
        delay(OPERATION_DELAY_MS);
    }
    delay(OPERATION_DELAY_MS);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 4, "Should have at least 4 windows");

    // All four windows should have similar sizes
    let areas: Vec<f64> = frames.iter().take(4).map(|f| f.area()).collect();
    let avg_area = areas.iter().sum::<f64>() / 4.0;

    for (i, area) in areas.iter().enumerate() {
        let variance = (area - avg_area).abs() / avg_area;
        println!(
            "Grid window {} area: {:.0} (variance: {:.1}%)",
            i,
            area,
            variance * 100.0
        );

        // Each window should be within 30% of average area
        assert!(
            variance < 0.3,
            "Grid window {} area ({:.0}) should be close to average ({:.0})",
            i,
            area,
            avg_area
        );
    }

    // Check that windows form a grid (2 rows, 2 columns)
    // Group by Y position
    let mut sorted_by_y = frames.clone();
    sorted_by_y.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());

    // First two should be in top row, last two in bottom row
    if frames.len() >= 4 {
        let top_y = sorted_by_y[0].y;
        let bottom_y = sorted_by_y[2].y;

        println!("Grid rows: top at y={:.0}, bottom at y={:.0}", top_y, bottom_y);

        // Rows should be different
        assert!(
            (bottom_y - top_y).abs() > 100.0,
            "Grid should have distinct rows"
        );
    }
}

/// Test grid layout with six windows (2x3 or 3x2 arrangement).
#[test]
fn test_grid_six_windows() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_grid");
    delay(STACHE_INIT_DELAY_MS);

    // Create six windows
    for i in 1..=6 {
        let _w = fixture.create_textedit(&format!("Grid {}", i));
        delay(OPERATION_DELAY_MS);
    }
    delay(OPERATION_DELAY_MS);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 6, "Should have at least 6 windows");

    // All windows should have reasonable sizes
    for (i, frame) in frames.iter().take(6).enumerate() {
        assert!(
            frame.width > 100.0 && frame.height > 100.0,
            "Grid window {} should have reasonable size: {}x{}",
            i,
            frame.width,
            frame.height
        );
        println!(
            "Grid window {}: ({:.0}, {:.0}) {}x{}",
            i, frame.x, frame.y, frame.width, frame.height
        );
    }

    // Calculate total tiled area
    let total_area: f64 = frames.iter().take(6).map(|f| f.area()).sum();
    if let Some((screen_w, screen_h)) = get_screen_size() {
        let screen_area = screen_w * screen_h;
        let coverage = total_area / screen_area;
        println!("Grid coverage: {:.1}% of screen", coverage * 100.0);
    }
}

/// Test that grid windows don't overlap.
#[test]
fn test_grid_windows_no_overlap() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_grid");
    delay(STACHE_INIT_DELAY_MS);

    // Create four windows
    for i in 1..=4 {
        let _w = fixture.create_textedit(&format!("NoOverlap {}", i));
        delay(OPERATION_DELAY_MS);
    }
    delay(OPERATION_DELAY_MS);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 4, "Should have at least 4 windows");

    // Check for overlaps between each pair
    for i in 0..frames.len().min(4) {
        for j in (i + 1)..frames.len().min(4) {
            let f1 = &frames[i];
            let f2 = &frames[j];

            // Check if rectangles overlap (accounting for gaps)
            let gap = 16.0; // Inner gap + tolerance
            let overlap_x = f1.x + f1.width > f2.x + gap && f2.x + f2.width > f1.x + gap;
            let overlap_y = f1.y + f1.height > f2.y + gap && f2.y + f2.height > f1.y + gap;

            if overlap_x && overlap_y {
                // Calculate overlap area
                let x1 = f1.x.max(f2.x);
                let y1 = f1.y.max(f2.y);
                let x2 = (f1.x + f1.width).min(f2.x + f2.width);
                let y2 = (f1.y + f1.height).min(f2.y + f2.height);
                let overlap_area = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);

                // Small overlaps are okay (gaps/borders)
                assert!(
                    overlap_area < 500.0,
                    "Windows {} and {} have significant overlap: {:.0} px^2",
                    i,
                    j,
                    overlap_area
                );
            }
        }
    }

    println!("Grid windows have no significant overlaps");
}
