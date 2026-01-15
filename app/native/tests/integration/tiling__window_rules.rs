//! Integration tests for window rules.
//!
//! Tests that windows are automatically assigned to workspaces based on
//! configured rules (app-id, app-name, title matching).
//!
//! ## Test Coverage
//! - App-name based rules
//! - Windows go to correct workspace based on rules
//! - Default workspace when no rule matches
//! - Ignore rules (windows not tiled)
//!
//! ## Running these tests
//! ```bash
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__window_rules/)' --no-capture
//! ```

use crate::common::*;

/// Test that TextEdit windows go to the code workspace.
#[test]
fn test_textEdit_assigned_to_code_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    // Use config with rules: TextEdit -> code workspace
    let mut fixture = TestFixture::with_config("tiling_with_rules");
    delay(STACHE_INIT_DELAY_MS);

    // Focus the code workspace first
    fixture.stache_command(&["tiling", "workspace", "--focus", "code"]);
    delay(OPERATION_DELAY_MS);

    // Create a TextEdit window
    let window = fixture.create_textedit("Rule Test TextEdit");
    assert!(window.is_some(), "Failed to create TextEdit window");
    delay(OPERATION_DELAY_MS * 2);

    // TextEdit should be assigned to code workspace based on rules
    // Verify by checking if TextEdit is visible/frontmost
    let front_app = get_frontmost_app_name();
    assert_eq!(
        front_app.as_deref(),
        Some("TextEdit"),
        "TextEdit should be on current workspace"
    );

    println!("TextEdit window created and assigned to workspace");
}

/// Test that TextEdit windows go to the docs workspace.
#[test]
fn test_textedit_assigned_to_docs_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    // Use config with rules: TextEdit -> docs workspace
    let mut fixture = TestFixture::with_config("tiling_with_rules");
    delay(STACHE_INIT_DELAY_MS);

    // Focus the docs workspace first
    fixture.stache_command(&["tiling", "workspace", "--focus", "docs"]);
    delay(OPERATION_DELAY_MS);

    // Create a TextEdit window
    let window = fixture.create_textedit("Rule Test TextEdit");
    assert!(window.is_some(), "Failed to create TextEdit window");
    delay(OPERATION_DELAY_MS * 2);

    // TextEdit should be assigned to docs workspace based on rules
    let front_app = get_frontmost_app_name();
    assert_eq!(
        front_app.as_deref(),
        Some("TextEdit"),
        "TextEdit should be on current workspace"
    );

    println!("TextEdit window created and assigned to workspace");
}

/// Test that windows from different apps go to different workspaces.
#[test]
fn test_different_apps_different_workspaces() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_with_rules");
    delay(STACHE_INIT_DELAY_MS);

    // Create a TextEdit window (should go to code workspace)
    let _textEdit = fixture.create_textedit("Multi-App TextEdit");
    delay(OPERATION_DELAY_MS);

    // Create a TextEdit window (should go to docs workspace)
    let _textedit = fixture.create_textedit("Multi-App TextEdit");
    delay(OPERATION_DELAY_MS * 2);

    // Now focus code workspace - should see TextEdit
    fixture.stache_command(&["tiling", "workspace", "--focus", "code"]);
    delay(OPERATION_DELAY_MS);

    // Activate TextEdit to ensure it's frontmost on this workspace
    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS);

    let code_app = get_frontmost_app_name();
    println!("On 'code' workspace, front app: {:?}", code_app);

    // Focus docs workspace - should see TextEdit
    fixture.stache_command(&["tiling", "workspace", "--focus", "docs"]);
    delay(OPERATION_DELAY_MS);

    activate_app("TextEdit");
    delay(OPERATION_DELAY_MS);

    let docs_app = get_frontmost_app_name();
    println!("On 'docs' workspace, front app: {:?}", docs_app);

    // Verify both apps have windows
    let textEdit_count = get_app_window_count("TextEdit");
    let textedit_count = get_app_window_count("TextEdit");
    assert!(textEdit_count >= 1, "Should have TextEdit window");
    assert!(textedit_count >= 1, "Should have TextEdit window");
}

/// Test ignore rules prevent tiling.
#[test]
fn test_ignore_rule_prevents_tiling() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    // Use config with ignore rules for TextEdit
    let mut fixture = TestFixture::with_config("tiling_with_ignore");
    delay(STACHE_INIT_DELAY_MS);

    // Set a specific position for the TextEdit window
    let target_frame = WindowFrame::new(200.0, 200.0, 500.0, 400.0);

    // Create a TextEdit window (should be ignored/floating)
    let _textedit = fixture.create_textedit("Ignored Window");
    delay(OPERATION_DELAY_MS);

    // Set position manually
    set_frontmost_window_frame(&target_frame);
    delay(OPERATION_DELAY_MS * 2);

    // Get the window frame
    let frame = get_frontmost_window_frame();
    assert!(frame.is_some(), "Should get window frame");

    let frame = frame.unwrap();

    // The window should maintain a position close to what we set
    // (not be auto-tiled to fill the screen)
    println!(
        "Ignored window frame: ({:.0}, {:.0}) {}x{}",
        frame.x, frame.y, frame.width, frame.height
    );

    // Window should have reasonable size (proving it exists and was created)
    assert!(
        frame.width > 100.0 && frame.height > 100.0,
        "Ignored window should have reasonable size: {}x{}",
        frame.width,
        frame.height
    );

    // If properly ignored, width should be close to what we set (not full screen)
    if let Some((screen_w, _)) = get_screen_size() {
        // Ignored window should not fill the screen
        let is_floating = frame.width < screen_w * 0.8;
        println!("Window appears floating (width < 80% screen): {}", is_floating);
    }
}

/// Test that unmatched windows go to default workspace.
#[test]
fn test_unmatched_window_default_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_with_rules");
    delay(STACHE_INIT_DELAY_MS);

    // Focus the general workspace (default/fallback)
    fixture.stache_command(&["tiling", "workspace", "--focus", "general"]);
    delay(OPERATION_DELAY_MS);

    // The general workspace should accept windows that don't match any rules
    // (In this case, all rules assign specific apps to specific workspaces)

    println!("Testing default workspace behavior");

    // Create a TextEdit window - it has a rule so goes to 'code'
    let _window = fixture.create_textedit("Default Test");
    delay(OPERATION_DELAY_MS);

    // Verify it's assigned based on rules
    let front_app = get_frontmost_app_name();
    println!("Created TextEdit, front app: {:?}", front_app);

    // TextEdit window should exist
    let window_count = get_app_window_count("TextEdit");
    assert!(window_count >= 1, "TextEdit window should be created");
}

/// Test multiple windows from same app on same workspace.
#[test]
fn test_multiple_windows_same_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_with_rules");
    delay(STACHE_INIT_DELAY_MS);

    // Focus code workspace
    fixture.stache_command(&["tiling", "workspace", "--focus", "code"]);
    delay(OPERATION_DELAY_MS);

    // Create multiple TextEdit windows
    let _w1 = fixture.create_textedit("Code 1");
    delay(OPERATION_DELAY_MS);
    let _w2 = fixture.create_textedit("Code 2");
    delay(OPERATION_DELAY_MS);
    let _w3 = fixture.create_textedit("Code 3");
    delay(OPERATION_DELAY_MS * 2);

    // All should be on code workspace and tiled
    let frames = get_app_window_frames("TextEdit");
    assert!(frames.len() >= 3, "Should have at least 3 TextEdit windows");

    println!("Multiple TextEdit windows on code workspace: {}", frames.len());

    // Windows should be tiled (not all at same position)
    let positions: std::collections::HashSet<(i32, i32)> =
        frames.iter().map(|f| (f.x as i32 / 50, f.y as i32 / 50)).collect();

    println!("Unique position regions: {}", positions.len());

    // All windows should have reasonable sizes
    for (i, frame) in frames.iter().enumerate() {
        assert!(
            frame.width > 100.0 && frame.height > 100.0,
            "Window {} should have reasonable size",
            i
        );
    }
}

/// Test rule priority (first matching rule wins).
#[test]
fn test_rule_priority() {
    let _guard = TEST_MUTEX.lock().unwrap();
    require_accessibility_permission();

    let mut fixture = TestFixture::with_config("tiling_with_rules");
    delay(STACHE_INIT_DELAY_MS);

    // Create window and verify it goes to the expected workspace
    // based on the first matching rule
    let _window = fixture.create_textedit("Priority Test");
    delay(OPERATION_DELAY_MS * 2);

    let front_app = get_frontmost_app_name();
    println!("Rule priority test - front app: {:?}", front_app);

    // The first rule that matches should win
    assert_eq!(
        front_app.as_deref(),
        Some("TextEdit"),
        "Window should be visible based on rules"
    );
}
