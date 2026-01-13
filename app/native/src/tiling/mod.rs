// Allow unused imports/code in this module - many public exports are for external CLI/IPC use
#![allow(dead_code)]
#![allow(unused_imports)]

//! Tiling Window Manager for Stache.
//!
//! This module provides a tiling window manager with virtual workspace support,
//! multiple layout modes, and keyboard-centric window management.
//!
//! # Features
//!
//! - Virtual workspaces with configurable rules for window assignment
//! - Multiple layout modes: Dwindle, Monocle, Split, Master, Floating
//! - Multi-monitor support with per-screen workspace assignment
//! - Window animations for smooth transitions
//! - Gaps between windows and screen edges
//! - Floating window presets for quick positioning
//!
//! # Window Management Approach
//!
//! Unlike some tiling managers that move windows to screen corners, this
//! implementation uses macOS native window hiding (similar to Cmd+H) for
//! workspace switching. This provides cleaner transitions and better
//! integration with macOS focus handling.
//!
//! # Usage
//!
//! The tiling manager is controlled via CLI commands:
//!
//! ```bash
//! # Query commands
//! stache tiling query screens
//! stache tiling query workspaces
//! stache tiling query windows
//!
//! # Workspace commands
//! stache tiling workspace --focus <name>
//! stache tiling workspace --layout <layout>
//!
//! # Window commands
//! stache tiling window --focus <direction>
//! stache tiling window --swap <direction>
//! ```

pub mod animation;
pub mod app_monitor;
pub mod borders;
pub mod drag_state;
pub mod layout;
pub mod manager;
pub mod mouse_monitor;
pub mod observer;
pub mod rules;
pub mod screen;
pub mod screen_monitor;
pub mod state;
pub mod window;
pub mod workspace;

// Re-export commonly used types
pub use animation::{
    AnimationConfig, AnimationSystem, WindowTransition, begin_animation, cancel_animation,
    get_interrupted_position,
};
pub use layout::{calculate_preset_frame, find_preset, list_preset_names};
pub use manager::{TilingManager, WorkspaceSwitchInfo, get_manager, init_manager};
pub use observer::{WindowEvent, WindowEventType};
pub use rules::{
    WorkspaceMatch, any_rule_matches, count_matching_rules, find_matching_workspace, matches_window,
};
pub use screen::{get_all_screens, get_main_screen, get_screen_by_name, get_screen_count};
pub use state::{Point, Rect, Screen, TilingState, TrackedWindow, Workspace};
use tauri::{AppHandle, Runtime};
pub use window::{
    AppInfo, CGWindowInfo, WindowInfo, focus_window, get_all_windows,
    get_all_windows_including_hidden, get_cg_window_list, get_cg_window_list_all,
    get_focused_window, get_running_apps, get_screen_for_window, hide_app, hide_window,
    move_window_to_screen, set_window_frame, show_window, unhide_app,
};
pub use workspace::{
    FocusHistory, WindowAssignment, WorkspaceSwitchResult, assign_window_to_workspace,
    find_workspace_for_window, get_visible_workspace_for_screen, get_workspaces_for_screen,
    hide_workspace_windows, should_ignore_window, show_workspace_windows, workspace_exists,
};

use crate::config::get_config;
use crate::is_accessibility_granted;

// ============================================================================
// Initialization
// ============================================================================

/// Initializes the tiling window manager.
///
/// This function checks if tiling is enabled in the configuration,
/// verifies accessibility permissions, and sets up window tracking
/// and event observers.
///
/// # Arguments
///
/// * `app_handle` - Optional Tauri app handle for emitting events.
///
/// # Behavior
///
/// - If `tiling.enabled` is `false` in config, this function returns immediately
/// - If accessibility permissions are not granted, returns early (warning already logged in lib.rs)
/// - On successful initialization, begins tracking windows and managing workspaces
pub fn init_with_handle<R: Runtime>(app_handle: Option<AppHandle<R>>) {
    let config = get_config();

    // Tiling is disabled by default
    if !config.tiling.is_enabled() {
        return;
    }

    // Use cached accessibility permission check from lib.rs
    if !is_accessibility_granted() {
        return;
    }

    // Initialize the tiling manager
    if !init_manager(app_handle) {
        return;
    }

    eprintln!("stache: tiling: manager initialized");

    // Initialize the border system (must be before tracking windows)
    if borders::init() && config.tiling.borders.is_enabled() {
        eprintln!("stache: tiling: borders initialized");
    }

    // Track existing windows
    track_existing_windows();

    // Initialize the mouse monitor for drag detection
    if mouse_monitor::init() {
        // Set up the callback for when mouse is released
        mouse_monitor::set_mouse_up_callback(on_mouse_up);
    }

    // Initialize the observer for window events
    if observer::init(handle_window_event) {
        eprintln!(
            "stache: tiling: observer initialized, watching {} apps",
            observer::observer_count()
        );
    }

    // Initialize the screen monitor for hotplug detection
    if screen_monitor::init(handle_screen_change) {
        eprintln!("stache: tiling: screen monitor initialized");
    }

    // Initialize the app launch monitor for tracking new apps
    if app_monitor::init(handle_app_launch) {
        eprintln!("stache: tiling: app launch monitor initialized");
    }

    // Apply startup behavior: switch to workspace containing focused window
    apply_startup_behavior();

    eprintln!("stache: tiling: initialization complete");
}

/// Initializes the tiling window manager without an app handle.
///
/// This is a convenience function for cases where we don't need to emit events.
pub fn init() { init_with_handle::<tauri::Wry>(None); }

// ============================================================================
// Window Tracking
// ============================================================================

/// Tracks all existing windows on startup.
///
/// Determines the initial focused workspace based on the currently focused window,
/// then uses that as the fallback for windows that don't match any rules.
fn track_existing_windows() {
    let Some(manager) = get_manager() else {
        return;
    };

    // Determine the initial focused workspace BEFORE tracking windows
    // This ensures non-matching windows go to the correct workspace
    let initial_focused_workspace = determine_initial_focused_workspace();

    let mut mgr = manager.write();
    if !mgr.is_enabled() {
        return;
    }

    // Set the focused workspace before tracking so the fallback is correct
    if let Some(ref ws_name) = initial_focused_workspace {
        mgr.set_focused_workspace_name(ws_name);
    }

    mgr.track_existing_windows();
    let window_count = mgr.get_windows().len();
    let workspace_count = mgr.get_workspaces().len();
    drop(mgr);

    eprintln!("stache: tiling: tracked {window_count} windows across {workspace_count} workspaces");

    // Move windows to their assigned screens
    move_windows_to_assigned_screens();
}

/// Determines the initial focused workspace based on the currently focused window.
///
/// If the focused window matches a workspace rule, returns that workspace.
/// Otherwise, returns the first workspace on the main screen.
fn determine_initial_focused_workspace() -> Option<String> {
    let focused = get_focused_window()?;
    let workspace_configs = workspace::get_workspace_configs();

    // Check if the focused window matches any workspace rule
    let workspaces = workspace_configs.iter().map(|ws| (ws.name.as_str(), ws.rules.as_slice()));
    if let Some(match_result) = rules::find_matching_workspace(&focused, workspaces) {
        return Some(match_result.workspace_name);
    }

    // No rule matched - return the first workspace on the main screen
    let config = crate::config::get_config();
    config.tiling.workspaces.first().map(|ws| ws.name.clone())
}

/// Moves windows to their assigned workspace's screen.
///
/// For each tracked window, checks if it's on the correct screen (the screen
/// where its workspace is assigned). If not, moves it to the correct screen.
fn move_windows_to_assigned_screens() {
    let Some(manager) = get_manager() else {
        return;
    };

    let mgr = manager.read();
    if !mgr.is_enabled() {
        return;
    }

    let screens = mgr.get_screens().to_vec();

    // Collect window info for moving (to avoid holding lock during moves)
    let mut windows_to_move: Vec<(u32, String, String, Screen, Option<Screen>)> = Vec::new();

    for window in mgr.get_windows() {
        // Get the workspace this window belongs to
        let Some(workspace) = mgr.get_workspace(&window.workspace_name) else {
            continue;
        };

        // Get the target screen for this workspace
        let Some(target_screen) = screens.iter().find(|s| s.id == workspace.screen_id) else {
            continue;
        };

        // Determine which screen the window is currently on
        let current_screen = get_screen_for_window(&window.frame, &screens);

        // Check if window needs to be moved
        let needs_move = current_screen.is_none_or(|cs| cs.id != target_screen.id);

        if needs_move {
            windows_to_move.push((
                window.id,
                window.app_name.clone(),
                window.workspace_name.clone(),
                target_screen.clone(),
                current_screen.cloned(),
            ));
        }
    }

    drop(mgr); // Release lock before doing moves

    if windows_to_move.is_empty() {
        return;
    }

    // Move windows to their target screens and collect successful moves
    let mut moved_windows: Vec<(u32, Rect)> = Vec::new();

    for (window_id, _app_name, _workspace_name, target_screen, current_screen) in windows_to_move {
        if move_window_to_screen(window_id, &target_screen, current_screen.as_ref()) {
            // Get the window's new frame after moving
            if let Some(new_frame) = get_window_frame_by_id(window_id) {
                moved_windows.push((window_id, new_frame));
            }
        }
    }

    // Update tracked window frames in the manager
    if !moved_windows.is_empty() {
        let mut mgr = manager.write();
        for (window_id, new_frame) in &moved_windows {
            mgr.update_window_frame(*window_id, *new_frame);
        }
    }
}

/// Gets a window's current frame by ID.
fn get_window_frame_by_id(window_id: u32) -> Option<Rect> {
    let windows = get_all_windows();
    windows.iter().find(|w| w.id == window_id).map(|w| w.frame)
}

/// Applies startup behavior: set initial workspace visibility based on focused window.
///
/// This should be called AFTER windows are tracked. It will:
/// - Detect the currently focused window
/// - Set the workspace containing that window as visible/focused on its screen
/// - For all other screens, set the first workspace (in config order) as visible
#[allow(clippy::significant_drop_tightening)] // Lock guard scope is intentional
fn apply_startup_behavior() {
    let Some(manager) = get_manager() else {
        return;
    };

    // Get the currently focused window
    let focused = get_focused_window();

    // Get the focused window ID if it's tracked
    let focused_window_id = {
        let mgr = manager.read();
        if !mgr.is_enabled() {
            return;
        }

        focused.as_ref().and_then(|fw| mgr.get_window(fw.id).map(|_| fw.id))
    };

    // Set initial workspace visibility
    let mut mgr = manager.write();
    mgr.set_initial_workspace_visibility(focused_window_id);

    // Now hide windows from non-visible workspaces and show windows from visible ones
    apply_initial_window_visibility(&mgr);

    // Apply layouts to all visible workspaces (uses instant positioning since not yet initialized)
    apply_initial_layouts(&mut mgr);

    // Mark the manager as initialized - from now on, animations will be used (if enabled)
    mgr.mark_initialized();
}

/// Applies initial window visibility based on workspace visibility.
///
/// For visible workspaces: Shows (unhides) all windows and their borders.
/// For non-visible workspaces: Hides all windows and their borders.
fn apply_initial_window_visibility(mgr: &TilingManager) {
    use workspace::{hide_workspace_windows, show_workspace_windows};

    for ws in mgr.get_workspaces() {
        let windows: Vec<_> = mgr.get_windows_for_workspace(&ws.name);

        if ws.is_visible {
            TilingManager::show_borders_for_workspace(&ws.name);
            if !windows.is_empty() {
                show_workspace_windows(&windows);
            }
        } else {
            TilingManager::hide_borders_for_workspace(&ws.name);
            if !windows.is_empty() {
                hide_workspace_windows(&windows);
            }
        }
    }
}

/// Applies layouts to all visible workspaces on startup.
fn apply_initial_layouts(mgr: &mut TilingManager) {
    let visible_workspaces: Vec<String> = mgr
        .get_workspaces()
        .iter()
        .filter(|ws| ws.is_visible)
        .map(|ws| ws.name.clone())
        .collect();

    for ws_name in visible_workspaces {
        mgr.apply_layout_forced(&ws_name);
    }
}

// ============================================================================
// Event Handling
// ============================================================================

/// Handles window events from the observer.
///
/// This is the callback function registered with the observer system.
fn handle_window_event(event: WindowEvent) {
    match event.event_type {
        WindowEventType::Created => handle_window_created(event.pid),
        WindowEventType::Destroyed => handle_window_destroyed(event.pid),
        WindowEventType::Focused => handle_window_focused(event.pid),
        WindowEventType::AppActivated => handle_app_activated(event.pid),
        WindowEventType::AppHidden => handle_app_hidden(event.pid),
        WindowEventType::AppShown => handle_app_shown(event.pid),
        WindowEventType::Moved => handle_window_moved(event.pid),
        WindowEventType::Resized => handle_window_resized(event.pid),
        // Events we track but don't need special handling for
        WindowEventType::Minimized
        | WindowEventType::Unminimized
        | WindowEventType::TitleChanged
        | WindowEventType::Unfocused
        | WindowEventType::AppDeactivated => {}
    }
}

/// Handles a window being moved.
///
/// If the mouse is down and no drag operation is in progress, this starts
/// tracking a move operation. During a drag, borders are updated to follow
/// the window but layout changes are deferred until mouse up.
fn handle_window_moved(pid: i32) {
    // If mouse is not down, this is a programmatic move (from us) - ignore
    if !mouse_monitor::is_mouse_down() {
        return;
    }

    // If we're already tracking an operation, just update borders to follow
    if drag_state::is_operation_in_progress() {
        update_borders_for_pid(pid);
        return;
    }

    // Start tracking this as a move operation
    start_drag_operation(pid, drag_state::DragOperation::Move);
}

/// Handles a window being resized.
///
/// If the mouse is down and no resize operation is in progress, this starts
/// tracking a resize operation. During a resize, borders are updated to follow
/// the window but layout changes are deferred until mouse up.
fn handle_window_resized(pid: i32) {
    // If mouse is not down, this is a programmatic resize (from us) - ignore
    if !mouse_monitor::is_mouse_down() {
        return;
    }

    // If we're already tracking an operation, just update borders to follow
    if drag_state::is_operation_in_progress() {
        update_borders_for_pid(pid);
        return;
    }

    // Start tracking this as a resize operation
    start_drag_operation(pid, drag_state::DragOperation::Resize);
}

/// Updates border frames for all tracked windows from a given PID.
///
/// NOTE: With `JankyBorders` integration, border positioning is handled entirely
/// by `JankyBorders` itself via its own window event subscriptions. This function
/// is kept as a no-op for API compatibility but does nothing.
#[allow(unused_variables)]
const fn update_borders_for_pid(pid: i32) {
    // JankyBorders handles its own border positioning via window server events.
    // No action needed from Stache during drag operations.
}

/// Starts tracking a drag/resize operation for a window from the given PID.
fn start_drag_operation(pid: i32, operation: drag_state::DragOperation) {
    let Some(manager) = get_manager() else {
        return;
    };

    let mgr = manager.read();
    if !mgr.is_enabled() {
        return;
    }

    // Find a tracked window from this PID to determine the workspace
    let tracked_windows: Vec<_> = mgr.get_windows().iter().filter(|w| w.pid == pid).collect();

    if tracked_windows.is_empty() {
        return;
    }

    // Get the workspace name from the first tracked window
    let workspace_name = tracked_windows[0].workspace_name.clone();

    // Get ALL windows in this workspace (not just from this PID)
    let workspace_windows: Vec<_> = mgr.get_windows_for_workspace(&workspace_name);

    // Create snapshots of all windows in the workspace
    let window_snapshots: Vec<drag_state::WindowSnapshot> = workspace_windows
        .iter()
        .map(|w| drag_state::WindowSnapshot {
            window_id: w.id,
            original_frame: w.frame,
            is_floating: w.is_floating,
        })
        .collect();

    drop(mgr);

    // Record the operation with all workspace windows
    drag_state::start_operation(
        operation,
        pid,
        &workspace_name,
        window_snapshots,
        mouse_monitor::drag_sequence(),
    );
}

/// Called when the mouse button is released after a drag/resize operation.
///
/// This is registered as a callback with the mouse monitor.
fn on_mouse_up() {
    // Finish any ongoing operation
    let Some(info) = drag_state::finish_operation() else {
        return;
    };

    // Process the completed operation
    match info.operation {
        drag_state::DragOperation::Move => handle_move_finished(&info),
        drag_state::DragOperation::Resize => handle_resize_finished(&info),
    }
}

/// Handles the completion of a move operation.
///
/// For tiled windows:
/// - If dropped on another tiled window, swap them
/// - Otherwise, reapply the layout to snap back to position
///
/// For floating windows: leave them where they are (this is their new position).
fn handle_move_finished(info: &drag_state::DragInfo) {
    // Get current window frames before updating tracked frames
    let current_windows = get_all_windows();

    // Update tracked frames for all windows
    update_all_tracked_frames(&info.workspace_name);

    if !info.has_tiled_windows() {
        // All floating windows - nothing to snap back
        return;
    }

    // Check if a window was dragged onto another window for swapping
    if let Some((dragged_id, target_id)) =
        find_drag_swap_target(&info.window_snapshots, &current_windows)
    {
        let Some(manager) = get_manager() else {
            return;
        };

        cancel_animation();
        let mut mgr = manager.write();
        begin_animation();

        if mgr.is_enabled() {
            mgr.swap_windows_by_id(dragged_id, target_id);
        }
        drop(mgr);
        return;
    }

    // No swap target - just snap back to layout position
    let Some(manager) = get_manager() else {
        return;
    };

    // Cancel any running animation before acquiring lock
    cancel_animation();

    let mut mgr = manager.write();
    begin_animation(); // Signal we're no longer waiting
    if mgr.is_enabled() {
        mgr.apply_layout_forced(&info.workspace_name);
    }
}

/// Finds if a dragged window should be swapped with another window.
///
/// Returns `Some((dragged_id, target_id))` if a swap should occur:
/// - `dragged_id` is the window that was moved significantly
/// - `target_id` is the window whose original bounds contain the dragged window's new center
///
/// Returns `None` if no swap should occur (window wasn't dropped on another window).
fn find_drag_swap_target(
    snapshots: &[drag_state::WindowSnapshot],
    current_windows: &[WindowInfo],
) -> Option<(u32, u32)> {
    // Minimum distance a window must move to be considered "dragged" (in pixels)
    const MIN_DRAG_DISTANCE: f64 = 50.0;

    // Find which window was dragged (moved significantly from original position)
    let mut dragged: Option<(u32, Rect)> = None;
    let mut max_distance = 0.0f64;

    for snapshot in snapshots {
        // Skip floating windows - they don't participate in swap
        if snapshot.is_floating {
            continue;
        }

        // Find current frame for this window
        let Some(current) = current_windows.iter().find(|w| w.id == snapshot.window_id) else {
            continue;
        };

        // Calculate how far the window moved (center-to-center distance)
        let orig_center_x = snapshot.original_frame.x + snapshot.original_frame.width / 2.0;
        let orig_center_y = snapshot.original_frame.y + snapshot.original_frame.height / 2.0;
        let curr_center_x = current.frame.x + current.frame.width / 2.0;
        let curr_center_y = current.frame.y + current.frame.height / 2.0;

        let dx = curr_center_x - orig_center_x;
        let dy = curr_center_y - orig_center_y;
        let distance = dx.hypot(dy);

        if distance > max_distance && distance > MIN_DRAG_DISTANCE {
            max_distance = distance;
            dragged = Some((snapshot.window_id, current.frame));
        }
    }

    let (dragged_id, dragged_frame) = dragged?;

    // Calculate the center of the dragged window's new position
    let dragged_center_x = dragged_frame.x + dragged_frame.width / 2.0;
    let dragged_center_y = dragged_frame.y + dragged_frame.height / 2.0;

    // Find which other window's original bounds contain this center point
    for snapshot in snapshots {
        // Skip the dragged window itself and floating windows
        if snapshot.window_id == dragged_id || snapshot.is_floating {
            continue;
        }

        let orig = &snapshot.original_frame;

        // Check if the dragged window's center is inside this window's original bounds
        if dragged_center_x >= orig.x
            && dragged_center_x <= orig.x + orig.width
            && dragged_center_y >= orig.y
            && dragged_center_y <= orig.y + orig.height
        {
            return Some((dragged_id, snapshot.window_id));
        }
    }

    // Dragged window wasn't dropped on any other window
    None
}

/// Handles the completion of a resize operation.
///
/// For tiled windows: find which window was resized and calculate new ratios.
/// For floating windows: just update the tracked frames.
fn handle_resize_finished(info: &drag_state::DragInfo) {
    // First, get the current window frames
    let current_windows = get_all_windows();

    // Find which window was resized by comparing current frames to snapshots
    let resized_window = find_resized_window(&info.window_snapshots, &current_windows);

    // Update all tracked frames
    update_all_tracked_frames(&info.workspace_name);

    if !info.has_tiled_windows() {
        return;
    }

    let Some(manager) = get_manager() else {
        return;
    };

    cancel_animation();

    let mut mgr = manager.write();
    begin_animation();
    if !mgr.is_enabled() {
        return;
    }

    // Calculate and apply new ratios, passing the resized window info
    if let Some((window_id, old_frame, new_frame)) = resized_window {
        mgr.calculate_and_apply_ratios_for_window(
            &info.workspace_name,
            window_id,
            old_frame,
            new_frame,
        );
    } else {
        mgr.apply_layout_forced(&info.workspace_name);
    }
}

/// Finds which window was resized by comparing snapshots to current frames.
///
/// Returns the window ID, old frame, and new frame if found.
fn find_resized_window(
    snapshots: &[drag_state::WindowSnapshot],
    current_windows: &[WindowInfo],
) -> Option<(u32, Rect, Rect)> {
    let mut max_diff = 0.0f64;
    let mut resized: Option<(u32, Rect, Rect)> = None;

    for snapshot in snapshots {
        // Skip floating windows
        if snapshot.is_floating {
            continue;
        }

        // Find the current frame for this window
        let Some(current) = current_windows.iter().find(|w| w.id == snapshot.window_id) else {
            continue;
        };

        // Calculate how much the frame changed (focus on size changes for resize)
        let width_diff = (current.frame.width - snapshot.original_frame.width).abs();
        let height_diff = (current.frame.height - snapshot.original_frame.height).abs();
        let size_diff = width_diff + height_diff;

        if size_diff > max_diff {
            max_diff = size_diff;
            resized = Some((snapshot.window_id, snapshot.original_frame, current.frame));
        }
    }

    // Only return if there was a significant change (more than 5 pixels)
    if max_diff > 5.0 { resized } else { None }
}

/// Updates tracked frames for all windows in a workspace from their current on-screen positions.
fn update_all_tracked_frames(workspace_name: &str) {
    let Some(manager) = get_manager() else {
        return;
    };

    // Get the current frames from the window list
    let current_windows = get_all_windows();

    let mut mgr = manager.write();

    // Get window IDs for this workspace
    let workspace_window_ids: Vec<u32> =
        mgr.get_windows_for_workspace(workspace_name).iter().map(|w| w.id).collect();

    // Update each window's frame
    for window_id in workspace_window_ids {
        if let Some(current) = current_windows.iter().find(|w| w.id == window_id) {
            mgr.update_window_frame(window_id, current.frame);
        }
    }
}

/// Maximum time to wait for windows to be ready (in milliseconds).
const WINDOW_READY_TIMEOUT_MS: u64 = 25;

/// How often to poll for window readiness (in milliseconds).
const WINDOW_READY_POLL_INTERVAL_MS: u64 = 5;

/// Handles a new window being created.
///
/// This function polls for window readiness instead of using a fixed delay:
/// 1. Poll until windows have valid AX properties (position/size) or timeout
/// 2. Then track and apply layout
///
/// This avoids race conditions where we try to position a window before
/// its AX element is fully ready, while also being faster for apps that
/// initialize windows quickly.
fn handle_window_created(pid: i32) {
    // Spawn a thread to handle this asynchronously
    std::thread::spawn(move || {
        // Get the count of currently tracked windows for this PID
        let tracked_count = get_manager().map_or(0, |m| {
            m.read().get_windows().iter().filter(|w| w.pid == pid).count()
        });

        // Poll for windows, waiting until we see more than what's tracked
        // (since we received a window created event, we expect a new window)
        let app_windows = wait_for_new_windows(pid, tracked_count);

        let current_ids: std::collections::HashSet<u32> =
            app_windows.iter().map(|w| w.id).collect();

        let Some(manager) = get_manager() else {
            return;
        };

        // Cancel any running animation before acquiring lock
        cancel_animation();

        let mut mgr = manager.write();
        begin_animation(); // Signal we're no longer waiting
        if !mgr.is_enabled() {
            return;
        }

        // Get visible workspace names
        let visible_workspaces: std::collections::HashSet<String> = mgr
            .get_workspaces()
            .iter()
            .filter(|ws| ws.is_visible)
            .map(|ws| ws.name.clone())
            .collect();

        // Find stale windows (tracked but no longer in deduplicated list)
        // These are old tab IDs that need to be swapped or removed
        let stale_windows: Vec<(u32, Rect)> = mgr
            .get_windows()
            .iter()
            .filter(|w| {
                w.pid == pid
                    && !current_ids.contains(&w.id)
                    && visible_workspaces.contains(&w.workspace_name)
            })
            .map(|w| (w.id, w.frame))
            .collect();

        // Find new windows (in deduplicated list but not tracked)
        let new_windows: Vec<&WindowInfo> =
            app_windows.iter().filter(|w| mgr.get_window(w.id).is_none()).collect();

        // DETECT TAB SWAPS FIRST: Match stale windows with new windows by frame
        // If frames match, it's just a tab ID change - swap in place WITHOUT triggering layout
        let mut swapped_stale_ids: std::collections::HashSet<u32> =
            std::collections::HashSet::new();
        let mut swapped_new_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();

        for (stale_id, stale_frame) in &stale_windows {
            for new_window in &new_windows {
                if !swapped_new_ids.contains(&new_window.id)
                    && frames_approximately_equal(stale_frame, &new_window.frame)
                {
                    // This is a tab swap - just update the ID in place (no layout change)
                    if mgr.swap_window_id(*stale_id, new_window.id) {
                        swapped_stale_ids.insert(*stale_id);
                        swapped_new_ids.insert(new_window.id);
                    }
                    break;
                }
            }
        }

        // Untrack stale windows that weren't swapped (real window closures)
        let mut workspaces_changed: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for (stale_id, _) in &stale_windows {
            if !swapped_stale_ids.contains(stale_id) {
                if let Some(w) = mgr.get_window(*stale_id) {
                    workspaces_changed.insert(w.workspace_name.clone());
                }
                mgr.untrack_window_no_layout(*stale_id);
            }
        }

        // Track new windows that weren't swapped (real new windows)
        for new_window in &new_windows {
            if !swapped_new_ids.contains(&new_window.id)
                && let Some(workspace_name) = mgr.track_window_no_layout(new_window)
            {
                workspaces_changed.insert(workspace_name);
            }
        }

        // Only apply layout if there were real window changes (not just tab swaps)
        if !workspaces_changed.is_empty() {
            for workspace_name in &workspaces_changed {
                mgr.apply_layout_forced(workspace_name);
            }

            // Get layout info before dropping the lock
            let layout_info = get_focused_workspace_layout(&mgr);

            // Drop manager before updating border colors to avoid holding lock
            drop(mgr);

            // Update border colors after layout is applied
            if let Some((is_monocle, is_floating)) = layout_info {
                borders::janky::update_colors_for_state(is_monocle, is_floating);
            }
        }
    });
}

/// Checks if two frames are approximately equal (within a small tolerance).
/// This accounts for minor floating-point differences in frame coordinates.
fn frames_approximately_equal(a: &Rect, b: &Rect) -> bool {
    const TOLERANCE: f64 = 2.0;
    (a.x - b.x).abs() < TOLERANCE
        && (a.y - b.y).abs() < TOLERANCE
        && (a.width - b.width).abs() < TOLERANCE
        && (a.height - b.height).abs() < TOLERANCE
}

/// Waits for new windows to appear for a given PID.
///
/// Since we received a window created event, we expect to see more windows than
/// what's currently tracked. This function polls until we see new windows or timeout.
fn wait_for_new_windows(pid: i32, currently_tracked: usize) -> Vec<WindowInfo> {
    use std::time::{Duration, Instant};

    let start = Instant::now();
    let max_wait = Duration::from_millis(WINDOW_READY_TIMEOUT_MS);
    let poll_interval = Duration::from_millis(WINDOW_READY_POLL_INTERVAL_MS);

    // Small initial delay to let the window system register the new window
    std::thread::sleep(Duration::from_millis(20));

    loop {
        let windows = window::get_all_windows_including_hidden();
        let app_windows: Vec<WindowInfo> = windows.into_iter().filter(|w| w.pid == pid).collect();

        // If we found more windows than tracked, or if there are new IDs, we're done
        if app_windows.len() > currently_tracked {
            return app_windows;
        }

        // Even if count is same, check if any IDs are different (tab swap case)
        // This handles the case where a tab is closed and a new one opened simultaneously
        if !app_windows.is_empty() && start.elapsed() >= Duration::from_millis(50) {
            // After 50ms, return whatever we have - the IDs might have changed
            return app_windows;
        }

        // Check timeout
        if start.elapsed() >= max_wait {
            return app_windows;
        }

        std::thread::sleep(poll_interval);
    }
}

/// Handles a window being destroyed.
///
/// Also handles tab closure: when the "representative" tab (highest ID) is closed,
/// the remaining tab becomes the new representative and needs to be tracked.
fn handle_window_destroyed(pid: i32) {
    let Some(manager) = get_manager() else {
        return;
    };

    // Get current window list - windows that no longer exist won't be in this list
    // This list is already deduplicated for tabs
    // IMPORTANT: This only returns ON-SCREEN windows
    let current_windows = get_all_windows();
    let app_windows: Vec<_> = current_windows.iter().filter(|w| w.pid == pid).collect();
    let current_ids: std::collections::HashSet<u32> = app_windows.iter().map(|w| w.id).collect();

    // Cancel any running animation before acquiring lock
    cancel_animation();

    let mut mgr = manager.write();
    begin_animation(); // Signal we're no longer waiting
    if !mgr.is_enabled() {
        return;
    }

    // Get visible workspace names - only untrack windows in visible workspaces
    // Windows in hidden workspaces won't appear in CGWindowList
    let visible_workspaces: std::collections::HashSet<String> = mgr
        .get_workspaces()
        .iter()
        .filter(|ws| ws.is_visible)
        .map(|ws| ws.name.clone())
        .collect();

    // Find destroyed windows (tracked but no longer exist)
    let destroyed_windows: Vec<(u32, Rect)> = mgr
        .get_windows()
        .iter()
        .filter(|w| {
            w.pid == pid
                && !current_ids.contains(&w.id)
                && visible_workspaces.contains(&w.workspace_name)
        })
        .map(|w| (w.id, w.frame))
        .collect();

    // Find new windows (exist but not tracked) - these are tabs that became representatives
    let new_windows: Vec<&WindowInfo> =
        app_windows.iter().filter(|w| mgr.get_window(w.id).is_none()).copied().collect();

    // DETECT TAB SWAPS FIRST: Match destroyed windows with new windows by frame
    // If frames match, it's just a tab ID change - swap in place WITHOUT triggering layout
    let mut swapped_destroyed_ids: std::collections::HashSet<u32> =
        std::collections::HashSet::new();
    let mut swapped_new_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();

    for (destroyed_id, destroyed_frame) in &destroyed_windows {
        for new_window in &new_windows {
            if !swapped_new_ids.contains(&new_window.id)
                && frames_approximately_equal(destroyed_frame, &new_window.frame)
            {
                // This is a tab swap - just update the ID in place (no layout change)
                if mgr.swap_window_id(*destroyed_id, new_window.id) {
                    swapped_destroyed_ids.insert(*destroyed_id);
                    swapped_new_ids.insert(new_window.id);
                }
                break;
            }
        }
    }

    // Untrack destroyed windows that weren't swapped (real window closures)
    let mut workspaces_changed: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for (destroyed_id, _) in &destroyed_windows {
        if !swapped_destroyed_ids.contains(destroyed_id) {
            if let Some(w) = mgr.get_window(*destroyed_id) {
                workspaces_changed.insert(w.workspace_name.clone());
            }
            mgr.untrack_window_no_layout(*destroyed_id);
        }
    }

    // Track new windows that weren't swapped (shouldn't normally happen in destroy handler)
    for new_window in new_windows {
        if !swapped_new_ids.contains(&new_window.id)
            && let Some(workspace_name) = mgr.track_window_no_layout(new_window)
        {
            workspaces_changed.insert(workspace_name);
        }
    }

    // Apply layout for workspaces that had real window changes (not just tab swaps)
    for workspace_name in &workspaces_changed {
        mgr.apply_layout_forced(workspace_name);
    }
}

/// Handles a window gaining focus.
///
/// Implements focus-follows-workspace: when a window is focused that belongs
/// to a different workspace on the same screen, switch to that workspace.
///
/// Also handles native tab switching: when the focused window ID isn't tracked
/// but matches a tracked window's frame (same app, same position), it's a tab
/// swap - we update the ID inline and proceed with workspace switching.
fn handle_window_focused(pid: i32) {
    let Some(manager) = get_manager() else {
        return;
    };

    // Get the currently focused window
    let Some(focused) = get_focused_window() else {
        return;
    };

    // Only handle if it's from the app that triggered the event
    if focused.pid != pid {
        return;
    }

    cancel_animation();

    let mut mgr = manager.write();
    begin_animation();
    if !mgr.is_enabled() {
        return;
    }

    // Find the tracked window - if not found, check for tab swap or new window
    let tracked = if let Some(t) = mgr.get_window(focused.id).cloned() {
        t
    } else {
        // Window not tracked - check if this is a tab swap (same app, same frame)
        // This handles native macOS tabs where the window ID changes when switching tabs
        if let Some(_swapped_workspace) = try_handle_tab_swap_inline(&mut mgr, &focused) {
            // Tab swap detected and handled - get the updated tracked window
            if let Some(t) = mgr.get_window(focused.id).cloned() {
                t
            } else {
                // Shouldn't happen, but handle gracefully
                mgr.clear_all_focus_borders();
                return;
            }
        } else {
            // Not a tab swap - try to track this as a new window
            // This handles new app launches and windows created from apps we're already tracking
            if let Some(tracked) = try_track_new_focused_window(&mut mgr, &focused) {
                tracked
            } else {
                // Window couldn't be tracked (likely ignored by rules)
                mgr.clear_all_focus_borders();
                return;
            }
        }
    };

    // Check if we should skip this focus event (it might be a stale event
    // from macOS after we programmatically focused a different window)
    if mgr.should_skip_focus_event(focused.id) {
        return;
    }

    let window_workspace = tracked.workspace_name;

    // Check if this workspace is already visible on its screen
    let target_ws = mgr.get_workspace(&window_workspace);

    if let Some(target) = target_ws {
        if target.is_visible {
            // Workspace is already visible, just update the focused window
            mgr.set_focused_window(&window_workspace, focused.id);
            return;
        }

        // Check if we should skip this workspace switch (debounce recent switches)
        if mgr.should_skip_workspace_switch(&window_workspace) {
            return;
        }

        // Workspace is not visible - switch to it
        mgr.switch_workspace(&window_workspace);
    }
}

/// Tries to handle a tab swap inline during focus handling.
///
/// When a focus event arrives for an untracked window, this checks if it's actually
/// a tab swap (same app, same frame as a tracked window). If so, it swaps the
/// window ID in place.
///
/// This is necessary because native macOS tabs generate new window IDs when
/// switching tabs, but the asynchronous tab detection in `handle_window_created`
/// may not have run yet.
///
/// # Returns
///
/// `Some(workspace_name)` if a tab swap was detected and handled, `None` otherwise.
fn try_handle_tab_swap_inline(
    mgr: &mut TilingManager,
    focused: &window::WindowInfo,
) -> Option<String> {
    // Find a tracked window from the same app with a matching frame
    let matching_window = mgr
        .get_windows()
        .iter()
        .find(|w| w.pid == focused.pid && frames_approximately_equal(&w.frame, &focused.frame))
        .map(|w| (w.id, w.workspace_name.clone()));

    if let Some((old_id, workspace_name)) = matching_window {
        // This is a tab swap - update the window ID inline
        if mgr.swap_window_id(old_id, focused.id) {
            return Some(workspace_name);
        }
    }

    None
}

/// Tries to track a new focused window that isn't currently tracked.
///
/// This handles:
/// 1. Windows from newly launched apps (after startup)
/// 2. Windows created from apps we're already tracking
///
/// If the window matches an ignore rule, returns `None`.
/// If the window is tracked successfully, returns the tracked window info.
///
/// Also ensures an observer is registered for the app if not already present.
fn try_track_new_focused_window(
    mgr: &mut TilingManager,
    focused: &window::WindowInfo,
) -> Option<state::TrackedWindow> {
    // Check if this window should be ignored
    let ignore_rules = workspace::get_ignore_rules();
    if workspace::should_ignore_window(focused, &ignore_rules) {
        return None;
    }

    // Ensure we have an observer for this app
    // This is important for apps launched after startup
    let _ = observer::add_observer_by_pid(focused.pid, Some(&focused.app_name));

    // Determine which workspace to assign this window to
    let workspace_configs = workspace::get_workspace_configs();
    let focused_ws = mgr
        .get_focused_workspace()
        .map_or_else(|| "main".to_string(), |ws| ws.name.clone());

    let assignment =
        workspace::assign_window_to_workspace(focused, &workspace_configs, &focused_ws);

    // Track the window
    if let Some(workspace_name) = mgr.track_window_no_layout(focused) {
        // Get the tracked window info
        if let Some(tracked) = mgr.get_window(focused.id).cloned() {
            // If the window was assigned to a workspace with a rule, switch to that workspace
            if assignment.matched_rule {
                // The window matched a rule - switch to its assigned workspace
                mgr.switch_workspace(&workspace_name);
            }

            // Apply layout to the workspace
            mgr.apply_layout_forced(&workspace_name);

            // Update border colors for the new window
            if let Some((is_monocle, is_floating)) = get_focused_workspace_layout(mgr) {
                borders::janky::update_colors_for_state(is_monocle, is_floating);
            }

            return Some(tracked);
        }
    }

    None
}

/// Handles an application being activated.
///
/// When an app is activated (brought to front), we need to:
/// 1. Track any new windows that might have been created while hidden
/// 2. Handle focus-follows-workspace: switch to the workspace containing the focused window
fn handle_app_activated(pid: i32) {
    // First, track any new windows from this app
    handle_window_created(pid);

    // Then handle focus - this implements focus-follows-workspace when switching apps
    handle_window_focused(pid);
}

/// Handles an application being hidden.
const fn handle_app_hidden(_pid: i32) {
    // Currently no special handling needed
    // Windows remain tracked but hidden
}

/// Handles an application being shown (unhidden).
fn handle_app_shown(pid: i32) {
    // Check for any new windows that might have appeared
    handle_window_created(pid);

    // Handle focus - this implements focus-follows-workspace when unhiding an app
    handle_window_focused(pid);
}

// ============================================================================
// Screen Change Handler
// ============================================================================

/// Handles screen configuration changes (screens connected/disconnected).
///
/// This is called by the screen monitor when displays are added or removed.
fn handle_screen_change() {
    let Some(manager) = get_manager() else {
        eprintln!("stache: tiling: screen change: manager not available");
        return;
    };

    // Prevent recursive callbacks during our screen refresh
    screen_monitor::set_processing(true);

    // Cancel any running animation before acquiring lock
    cancel_animation();

    let (added, removed) = {
        let mut mgr = manager.write();
        begin_animation(); // Signal we're no longer waiting
        if !mgr.is_enabled() {
            screen_monitor::set_processing(false);
            return;
        }
        mgr.handle_screen_change()
    };

    eprintln!(
        "stache: tiling: screen change handled: {added} screens added, {removed} screens removed"
    );

    screen_monitor::set_processing(false);
}

// ============================================================================
// App Launch Handler
// ============================================================================

/// Handles a new application being launched.
///
/// This is called by the app launch monitor when `NSWorkspaceDidLaunchApplicationNotification`
/// is received. It:
/// 1. Adds an `AXObserver` for the new app
/// 2. Waits for the app's windows to be ready
/// 3. Tracks the windows according to workspace rules
/// 4. Switches to the appropriate workspace if rules match
#[allow(clippy::needless_pass_by_value)] // Signature required by callback type
fn handle_app_launch(pid: i32, _bundle_id: Option<String>, app_name: Option<String>) {
    // First, add an observer for this app so we receive window events
    let _ = observer::add_observer_by_pid(pid, app_name.as_deref());

    // Spawn a thread to wait for windows and track them
    std::thread::spawn(move || {
        // Wait for the app's windows to be ready
        // Use short timeout since the AXObserver will also catch window creation events
        let app_windows = window::wait_for_app_windows_ready(
            pid,
            WINDOW_READY_TIMEOUT_MS, // 150ms - same as handle_window_created
            WINDOW_READY_POLL_INTERVAL_MS, // 5ms poll
        );

        if app_windows.is_empty() {
            // App might not have windows yet, or windows aren't ready
            // The observer will catch them when they're created
            return;
        }

        let Some(manager) = get_manager() else {
            return;
        };

        // Cancel any running animation before acquiring lock
        cancel_animation();

        let mut mgr = manager.write();
        begin_animation();
        if !mgr.is_enabled() {
            return;
        }

        // Get ignore rules and workspace configs
        let ignore_rules = workspace::get_ignore_rules();
        let workspace_configs = workspace::get_workspace_configs();
        let focused_ws = mgr
            .get_focused_workspace()
            .map_or_else(|| "main".to_string(), |ws| ws.name.clone());

        // Track each window
        let mut workspaces_changed: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut matched_workspace: Option<String> = None;

        for window in &app_windows {
            // Skip if window should be ignored
            if workspace::should_ignore_window(window, &ignore_rules) {
                continue;
            }

            // Skip if already tracked
            if mgr.get_window(window.id).is_some() {
                continue;
            }

            // Determine which workspace to assign this window to
            let assignment =
                workspace::assign_window_to_workspace(window, &workspace_configs, &focused_ws);

            // Track the window
            if let Some(workspace_name) = mgr.track_window_no_layout(window) {
                workspaces_changed.insert(workspace_name.clone());

                // Remember if any window matched a rule
                if assignment.matched_rule && matched_workspace.is_none() {
                    matched_workspace = Some(workspace_name);
                }
            }
        }

        // If any windows matched a rule, switch to that workspace
        if let Some(ws_name) = matched_workspace {
            mgr.switch_workspace(&ws_name);
        }

        // Apply layouts to changed workspaces
        for workspace_name in &workspaces_changed {
            mgr.apply_layout_forced(workspace_name);
        }

        // Update border colors after layout is applied
        if !workspaces_changed.is_empty() {
            let layout_info = get_focused_workspace_layout(&mgr);

            // Drop manager before updating border colors to avoid holding lock
            drop(mgr);

            if let Some((is_monocle, is_floating)) = layout_info {
                borders::janky::update_colors_for_state(is_monocle, is_floating);
            }
        }
    });
}

/// Gets the focused workspace's layout info for border color updates.
///
/// Returns `Some((is_monocle, is_floating))` if there's a focused workspace.
fn get_focused_workspace_layout(mgr: &TilingManager) -> Option<(bool, bool)> {
    use crate::config::LayoutType;

    let focused_ws = mgr.get_focused_workspace()?;
    let is_monocle = focused_ws.layout == LayoutType::Monocle;
    let is_floating = focused_ws.layout == LayoutType::Floating;

    Some((is_monocle, is_floating))
}

// ============================================================================
// IPC Query Handler
// ============================================================================

use crate::utils::ipc_socket::{IpcQuery, IpcResponse};

/// Handles IPC queries for tiling state.
///
/// This is called by the IPC server when a query is received from the CLI.
pub fn handle_ipc_query(query: IpcQuery) -> IpcResponse {
    match query {
        IpcQuery::Ping => IpcResponse::success(serde_json::json!({"status": "ok"})),
        IpcQuery::Screens => query_screens(),
        IpcQuery::Workspaces { screen, focused_screen } => {
            query_workspaces(screen.as_deref(), focused_screen)
        }
        IpcQuery::Windows {
            screen,
            workspace,
            focused_screen,
            focused_workspace,
        } => query_windows(
            screen.as_deref(),
            workspace.as_deref(),
            focused_screen,
            focused_workspace,
        ),
    }
}

/// Queries all screens.
fn query_screens() -> IpcResponse {
    let screens = get_all_screens();
    IpcResponse::success(screens)
}

/// Queries workspaces with optional filters.
#[allow(clippy::significant_drop_tightening)] // Guard needs to be held for entire function
fn query_workspaces(screen_filter: Option<&str>, focused_screen: bool) -> IpcResponse {
    let Some(manager) = get_manager() else {
        return IpcResponse::error("Tiling not initialized");
    };

    let mgr = manager.read();
    if !mgr.is_enabled() {
        return IpcResponse::error("Tiling not enabled");
    }

    let mut workspaces: Vec<serde_json::Value> = Vec::new();

    // Determine which screen(s) to filter by
    let filter_screen_id: Option<u32> = if focused_screen {
        mgr.get_focused_screen().map(|s| s.id)
    } else if let Some(name) = screen_filter {
        match mgr.get_screen_by_name(name) {
            Some(s) => Some(s.id),
            None => return IpcResponse::error(format!("Screen '{name}' not found")),
        }
    } else {
        None
    };

    for ws in mgr.get_workspaces() {
        // Apply screen filter if specified
        if let Some(screen_id) = filter_screen_id
            && ws.screen_id != screen_id
        {
            continue;
        }

        let screen_name = mgr
            .get_screen(ws.screen_id)
            .map_or_else(|| "unknown".to_string(), |s| s.name.clone());

        workspaces.push(serde_json::json!({
            "name": ws.name,
            "screenId": ws.screen_id,
            "screenName": screen_name,
            "layout": format!("{:?}", ws.layout).to_lowercase(),
            "isVisible": ws.is_visible,
            "isFocused": ws.is_focused,
            "windowCount": ws.window_ids.len(),
            "windowIds": ws.window_ids,
        }));
    }

    IpcResponse::success(workspaces)
}

/// Queries windows with optional filters.
#[allow(clippy::significant_drop_tightening)] // Guard needs to be held for entire function
fn query_windows(
    screen_filter: Option<&str>,
    workspace_filter: Option<&str>,
    focused_screen: bool,
    focused_workspace: bool,
) -> IpcResponse {
    let Some(manager) = get_manager() else {
        return IpcResponse::error("Tiling not initialized");
    };

    let mgr = manager.read();
    if !mgr.is_enabled() {
        return IpcResponse::error("Tiling not enabled");
    }

    // Get the currently focused window ID
    let focused_window_id = get_focused_window().map(|w| w.id);

    // Determine filter criteria
    let filter_screen_id: Option<u32> = if focused_screen {
        mgr.get_focused_screen().map(|s| s.id)
    } else if let Some(name) = screen_filter {
        match mgr.get_screen_by_name(name) {
            Some(s) => Some(s.id),
            None => return IpcResponse::error(format!("Screen '{name}' not found")),
        }
    } else {
        None
    };

    let filter_workspace: Option<String> = if focused_workspace {
        mgr.state().focused_workspace.clone()
    } else {
        workspace_filter.map(String::from)
    };

    // Validate workspace filter if specified
    if let Some(ref ws_name) = filter_workspace
        && mgr.get_workspace(ws_name).is_none()
    {
        return IpcResponse::error(format!("Workspace '{ws_name}' not found"));
    }

    // Build list of workspaces to include (for screen filtering)
    let workspace_names_for_screen: Option<Vec<String>> = filter_screen_id.map(|screen_id| {
        mgr.get_workspaces()
            .iter()
            .filter(|ws| ws.screen_id == screen_id)
            .map(|ws| ws.name.clone())
            .collect()
    });

    let mut windows: Vec<serde_json::Value> = Vec::new();

    for w in mgr.get_windows() {
        // Apply workspace filter
        if let Some(ref ws_name) = filter_workspace
            && !w.workspace_name.eq_ignore_ascii_case(ws_name)
        {
            continue;
        }

        // Apply screen filter (via workspace)
        if let Some(ref ws_names) = workspace_names_for_screen
            && !ws_names.iter().any(|name| name.eq_ignore_ascii_case(&w.workspace_name))
        {
            continue;
        }

        let is_focused = focused_window_id == Some(w.id);

        windows.push(serde_json::json!({
            "id": w.id,
            "pid": w.pid,
            "appId": w.app_id,
            "appName": w.app_name,
            "title": w.title,
            "workspace": w.workspace_name,
            "isFocused": is_focused,
            "frame": {
                "x": w.frame.x,
                "y": w.frame.y,
                "width": w.frame.width,
                "height": w.frame.height,
            },
        }));
    }

    IpcResponse::success(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_compiles() {
        // Basic sanity check that the module structure is valid
        assert!(true);
    }

    #[test]
    fn test_exports() {
        // Verify that re-exports work
        let _point = Point::new(0.0, 0.0);
        let _rect = Rect::new(0.0, 0.0, 100.0, 100.0);
    }

    #[test]
    fn test_screen_functions_available() {
        let count = get_screen_count();
        assert!(count >= 1);

        let screens = get_all_screens();
        assert!(!screens.is_empty());

        let main = get_main_screen();
        assert!(main.is_some());
    }

    // ========================================================================
    // Drag-and-Drop Tests
    // ========================================================================

    fn make_snapshot(
        id: u32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        floating: bool,
    ) -> drag_state::WindowSnapshot {
        drag_state::WindowSnapshot {
            window_id: id,
            original_frame: Rect::new(x, y, w, h),
            is_floating: floating,
        }
    }

    fn make_window_info(id: u32, x: f64, y: f64, w: f64, h: f64) -> WindowInfo {
        WindowInfo::new_for_test(id, 1, Rect::new(x, y, w, h))
    }

    #[test]
    fn test_find_drag_swap_target_window_dropped_on_another() {
        // Two windows side by side, window 1 is dragged onto window 2's position
        let snapshots = vec![
            make_snapshot(1, 0.0, 0.0, 500.0, 600.0, false), // Left window
            make_snapshot(2, 500.0, 0.0, 500.0, 600.0, false), // Right window
        ];

        // Window 1 moved to center of window 2's original position
        let current = vec![
            make_window_info(1, 600.0, 200.0, 500.0, 600.0), // Dragged to right
            make_window_info(2, 500.0, 0.0, 500.0, 600.0),   // Stayed in place
        ];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_some());
        let (dragged, target) = result.unwrap();
        assert_eq!(dragged, 1);
        assert_eq!(target, 2);
    }

    #[test]
    fn test_find_drag_swap_target_no_swap_when_not_on_window() {
        // Window dragged but not dropped on another window
        let snapshots = vec![
            make_snapshot(1, 0.0, 0.0, 500.0, 600.0, false),
            make_snapshot(2, 500.0, 0.0, 500.0, 600.0, false),
        ];

        // Window 1 moved but not into window 2's bounds
        let current = vec![
            make_window_info(1, 100.0, 100.0, 500.0, 600.0), // Moved slightly
            make_window_info(2, 500.0, 0.0, 500.0, 600.0),
        ];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_drag_swap_target_ignores_small_moves() {
        // Window moved less than MIN_DRAG_DISTANCE (50px)
        let snapshots = vec![
            make_snapshot(1, 0.0, 0.0, 500.0, 600.0, false),
            make_snapshot(2, 500.0, 0.0, 500.0, 600.0, false),
        ];

        // Window 1 moved only 30px
        let current = vec![
            make_window_info(1, 30.0, 0.0, 500.0, 600.0),
            make_window_info(2, 500.0, 0.0, 500.0, 600.0),
        ];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_drag_swap_target_ignores_floating_windows() {
        // Floating window dragged onto tiled window - should not swap
        let snapshots = vec![
            make_snapshot(1, 0.0, 0.0, 500.0, 600.0, true), // Floating
            make_snapshot(2, 500.0, 0.0, 500.0, 600.0, false), // Tiled
        ];

        let current = vec![
            make_window_info(1, 600.0, 200.0, 500.0, 600.0), // Dragged to window 2
            make_window_info(2, 500.0, 0.0, 500.0, 600.0),
        ];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_drag_swap_target_does_not_swap_with_floating() {
        // Tiled window dragged onto floating window - should not swap
        let snapshots = vec![
            make_snapshot(1, 0.0, 0.0, 500.0, 600.0, false), // Tiled
            make_snapshot(2, 500.0, 0.0, 500.0, 600.0, true), // Floating
        ];

        let current = vec![
            make_window_info(1, 600.0, 200.0, 500.0, 600.0), // Dragged to window 2
            make_window_info(2, 500.0, 0.0, 500.0, 600.0),
        ];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_drag_swap_target_with_three_windows() {
        // Window 1 dragged onto window 3
        let snapshots = vec![
            make_snapshot(1, 0.0, 0.0, 333.0, 600.0, false),
            make_snapshot(2, 333.0, 0.0, 333.0, 600.0, false),
            make_snapshot(3, 666.0, 0.0, 334.0, 600.0, false),
        ];

        // Window 1's center is now inside window 3's original bounds
        let current = vec![
            make_window_info(1, 700.0, 100.0, 333.0, 600.0), // Dragged to window 3
            make_window_info(2, 333.0, 0.0, 333.0, 600.0),
            make_window_info(3, 666.0, 0.0, 334.0, 600.0),
        ];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_some());
        let (dragged, target) = result.unwrap();
        assert_eq!(dragged, 1);
        assert_eq!(target, 3);
    }

    #[test]
    fn test_find_drag_swap_target_empty_snapshots() {
        let snapshots: Vec<drag_state::WindowSnapshot> = vec![];
        let current: Vec<WindowInfo> = vec![];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_drag_swap_target_single_window() {
        // Single window can't be swapped with anything
        let snapshots = vec![make_snapshot(1, 0.0, 0.0, 500.0, 600.0, false)];
        let current = vec![make_window_info(1, 200.0, 200.0, 500.0, 600.0)];

        let result = find_drag_swap_target(&snapshots, &current);
        assert!(result.is_none());
    }
}
