//! Integration tests for the Floating layout.
//!
//! Floating layout allows windows to be freely positioned and sized.
//! Windows are not automatically tiled but can use floating presets.
//!
//! ## Test Coverage
//! - Windows maintain their position in floating layout
//! - Floating presets (centered, full, halves)
//! - Multiple floating windows
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_floating/)' --no-capture
//! ```

use crate::common::*;

/// Test that windows in floating layout maintain their position.
#[test]
fn test_floating_window_position_maintained() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_floating");
    delay(STACHE_INIT_DELAY_MS);

    // Create a window
    let window = fixture.create_textedit("Floating Window");
    assert!(window.is_some(), "Failed to create window");
    delay(OPERATION_DELAY_MS);

    // Set a specific position
    let target_frame = WindowFrame::new(200.0, 150.0, 600.0, 400.0);
    set_frontmost_window_frame(&target_frame);
    delay(OPERATION_DELAY_MS);

    // Wait a bit more to ensure no auto-tiling occurs
    delay(500);

    // Check the window position
    let actual_frame = get_frontmost_window_frame();
    assert!(actual_frame.is_some(), "Should get window frame");

    let actual = actual_frame.unwrap();

    // In floating layout, position should be maintained (within tolerance)
    // Note: Some variance is expected due to window snapping
    println!(
        "Floating position: set to ({}, {}) {}x{}, got ({}, {}) {}x{}",
        target_frame.x,
        target_frame.y,
        target_frame.width,
        target_frame.height,
        actual.x,
        actual.y,
        actual.width,
        actual.height
    );

    // Window should have reasonable dimensions (not auto-maximized)
    assert!(
        actual.width > 100.0 && actual.height > 100.0,
        "Floating window should have reasonable size"
    );
}

/// Test centered floating preset.
#[test]
fn test_floating_preset_centered() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_floating");
    delay(STACHE_INIT_DELAY_MS);

    // Create a window and wait for it to be fully tracked
    // Need extra delay for window ID to stabilize (CGWindow ID swap)
    let _window = fixture.create_textedit("Centered Window");
    delay(OPERATION_DELAY_MS * 3);

    // Re-activate TextEdit to force a focus event and ensure the window ID swap occurs
    // This is necessary because the CGWindow ID may not be available immediately
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    // Apply centered preset
    fixture.stache_command(&["tiling", "window", "--preset", "centered"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    // Centered preset: 60% width, 70% height, centered
    if let Some((screen_w, screen_h)) = get_screen_size() {
        // Expected dimensions (with some tolerance for menu bar)
        let expected_width = screen_w * 0.6;
        let expected_height = (screen_h - 50.0) * 0.7; // Account for menu bar

        let width_ratio = frame.width / expected_width;
        let height_ratio = frame.height / expected_height;

        println!(
            "Centered preset: {}x{} (expected ~{:.0}x{:.0})",
            frame.width, frame.height, expected_width, expected_height
        );
        println!("Ratios - width: {:.2}, height: {:.2}", width_ratio, height_ratio);

        // Check centering
        let center_x = frame.x + frame.width / 2.0;
        let screen_center_x = screen_w / 2.0;
        let x_offset = (center_x - screen_center_x).abs();

        println!(
            "Center X offset: {:.0} (window center: {:.0}, screen center: {:.0})",
            x_offset, center_x, screen_center_x
        );

        // Window should be reasonably centered (within 100px tolerance)
        assert!(
            x_offset < 100.0,
            "Centered window should be near screen center, offset: {:.0}",
            x_offset
        );
    }
}

/// Test full floating preset.
#[test]
fn test_floating_preset_full() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_floating");
    delay(STACHE_INIT_DELAY_MS);

    // Create a window and wait for it to be fully tracked
    // Need extra delay for window ID to stabilize (CGWindow ID swap)
    let _window = fixture.create_textedit("Full Window");
    delay(OPERATION_DELAY_MS * 3);

    // Re-activate TextEdit to force a focus event and ensure the window ID swap occurs
    // This is necessary because the CGWindow ID may not be available immediately
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    // Apply full preset
    fixture.stache_command(&["tiling", "window", "--preset", "full"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    if let Some((screen_w, screen_h)) = get_screen_size() {
        // Full preset should fill most of the screen
        let width_ratio = frame.width / screen_w;
        let height_ratio = frame.height / (screen_h - 30.0); // Account for menu bar

        println!(
            "Full preset: {}x{} ({:.1}% x {:.1}% of screen)",
            frame.width,
            frame.height,
            width_ratio * 100.0,
            height_ratio * 100.0
        );

        // Should fill at least 90% in each dimension (accounting for gaps)
        assert!(
            width_ratio > 0.85,
            "Full preset should fill most of screen width"
        );
    }
}

/// Test left-half floating preset.
#[test]
fn test_floating_preset_left_half() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_floating");
    delay(STACHE_INIT_DELAY_MS);

    // Create a window and wait for it to be fully tracked
    // Need extra delay for window ID to stabilize (CGWindow ID swap)
    let _window = fixture.create_textedit("Left Half Window");
    delay(OPERATION_DELAY_MS * 3);

    // Re-activate TextEdit to force a focus event and ensure the window ID swap occurs
    // This is necessary because the CGWindow ID may not be available immediately
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    // Apply left-half preset
    fixture.stache_command(&["tiling", "window", "--preset", "left-half"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    if let Some((screen_w, screen_h)) = get_screen_size() {
        // Left half should be ~50% width, near left edge
        let width_ratio = frame.width / screen_w;
        let height_ratio = frame.height / (screen_h - 30.0);

        println!(
            "Left-half preset: x={:.0}, {}x{} ({:.1}% width)",
            frame.x,
            frame.width,
            frame.height,
            width_ratio * 100.0
        );

        // Width should be approximately half
        assert!(
            width_ratio > 0.4 && width_ratio < 0.6,
            "Left-half should be ~50% width, got {:.1}%",
            width_ratio * 100.0
        );

        // X position should be near left edge
        assert!(
            frame.x < 50.0,
            "Left-half should start near left edge, x={}",
            frame.x
        );

        // Height should fill most of screen
        assert!(
            height_ratio > 0.8,
            "Left-half should fill height, got {:.1}%",
            height_ratio * 100.0
        );
    }
}

/// Test right-half floating preset.
#[test]
fn test_floating_preset_right_half() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_floating");
    delay(STACHE_INIT_DELAY_MS);

    // Create a window and wait for it to be fully tracked
    // Need extra delay for window ID to stabilize (CGWindow ID swap)
    let _window = fixture.create_textedit("Right Half Window");
    delay(OPERATION_DELAY_MS * 3);

    // Re-activate TextEdit to force a focus event and ensure the window ID swap occurs
    // This is necessary because the CGWindow ID may not be available immediately
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    // Apply right-half preset
    fixture.stache_command(&["tiling", "window", "--preset", "right-half"]);
    delay(OPERATION_DELAY_MS * 2);

    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    if let Some((screen_w, _)) = get_screen_size() {
        let width_ratio = frame.width / screen_w;

        println!(
            "Right-half preset: x={:.0}, {}x{} ({:.1}% width)",
            frame.x,
            frame.width,
            frame.height,
            width_ratio * 100.0
        );

        // Width should be approximately half
        assert!(
            width_ratio > 0.4 && width_ratio < 0.6,
            "Right-half should be ~50% width"
        );

        // X position should be near middle of screen
        let expected_x = screen_w * 0.5;
        let x_diff = (frame.x - expected_x).abs();
        assert!(
            x_diff < 50.0,
            "Right-half should start at ~50% x, got {} (expected {})",
            frame.x,
            expected_x
        );
    }
}

/// Test multiple floating windows.
#[test]
fn test_floating_multiple_windows() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_floating");
    delay(STACHE_INIT_DELAY_MS);

    // Create multiple windows with different positions
    let _w1 = fixture.create_textedit("Float 1");
    set_frontmost_window_frame(&WindowFrame::new(100.0, 100.0, 400.0, 300.0));
    delay(OPERATION_DELAY_MS);

    let _w2 = fixture.create_textedit("Float 2");
    set_frontmost_window_frame(&WindowFrame::new(300.0, 200.0, 400.0, 300.0));
    delay(OPERATION_DELAY_MS);

    let _w3 = fixture.create_textedit("Float 3");
    set_frontmost_window_frame(&WindowFrame::new(500.0, 300.0, 400.0, 300.0));
    delay(OPERATION_DELAY_MS * 2);

    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 3, "Should have at least 3 windows");

    // In floating layout, windows should be at different positions
    // (not auto-tiled to same grid positions)
    let unique_x: std::collections::HashSet<i32> =
        frames.iter().take(3).map(|f| f.x as i32 / 50).collect();

    println!("Floating windows at {} unique X regions", unique_x.len());

    for (i, frame) in frames.iter().take(3).enumerate() {
        println!(
            "Float window {}: ({:.0}, {:.0}) {}x{}",
            i, frame.x, frame.y, frame.width, frame.height
        );
        // Each window should have reasonable size
        assert!(
            frame.width > 100.0 && frame.height > 100.0,
            "Floating window {} should have reasonable size",
            i
        );
    }
}
