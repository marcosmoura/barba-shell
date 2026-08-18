//! Effect subscriber for reacting to state changes.
//!
//! The subscriber watches for changes in tiling state and computes effects
//! that need to be applied to the system. It connects the reactive state
//! model to the effect executor.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     TilingState                                  │
//! │  (Observable collections: screens, workspaces, windows, focus)  │
//! └─────────────────────────┬───────────────────────────────────────┘
//!                           │ subscribe()
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   EffectSubscriber                               │
//! │  - Tracks previous layout positions                             │
//! │  - Tracks previous focus state                                  │
//! │  - Computes deltas when state changes                           │
//! │  - Generates effects for executor                               │
//! └─────────────────────────┬───────────────────────────────────────┘
//!                           │ Vec<TilingEffect>
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   EffectExecutor                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! The subscriber is typically spawned as a background task that runs
//! for the lifetime of the tiling system:
//!
//! ```rust,ignore
//! let subscriber = EffectSubscriber::new(actor_handle, executor);
//! tauri::async_runtime::spawn(subscriber.run());
//! ```

use std::collections::HashMap;

use tokio::sync::mpsc;
use uuid::Uuid;

use super::executor::{EffectExecutor, effects_from_layout_change};
use super::{LayoutChange, TilingEffect, begin_animation, cancel_animation};
use crate::modules::tiling::actor::{QueryResult, StateActorHandle, StateQuery};
use crate::modules::tiling::identity::WindowTarget;
use crate::modules::tiling::state::{LayoutType, Rect};

// ============================================================================
// Subscriber State
// ============================================================================

/// Tracks the previous state for computing deltas.
#[derive(Debug, Default)]
struct SubscriberState {
    /// Previous layout positions per workspace.
    layout_positions: HashMap<Uuid, Vec<(WindowTarget, Rect)>>,

    /// Previous focused window (exact target).
    focused_window: Option<WindowTarget>,

    /// Previous focused workspace.
    focused_workspace_id: Option<Uuid>,

    /// Previous visible workspaces (`workspace_id` -> `screen_id`).
    visible_workspaces: HashMap<Uuid, u32>,

    /// Workspace layouts (for determining monocle/floating state).
    workspace_layouts: HashMap<Uuid, LayoutType>,

    /// Floating window IDs.
    floating_windows: std::collections::HashSet<u32>,
}

impl SubscriberState {
    /// Creates a new empty state.
    fn new() -> Self { Self::default() }

    /// Updates layout positions and returns the change if any.
    ///
    /// For user-triggered changes, we always return a change because the actual
    /// window positions might differ from our tracked positions (e.g., after a drag).
    fn update_layout(
        &mut self,
        workspace_id: Uuid,
        new_positions: Vec<(WindowTarget, Rect)>,
        user_triggered: bool,
    ) -> Option<LayoutChange> {
        let old_positions = self.layout_positions.get(&workspace_id).cloned().unwrap_or_default();

        // For user-triggered changes (like after a drag), always apply the layout
        // because the actual window positions might differ from our tracked positions.
        // For programmatic changes, only apply if our tracking shows a change.
        if !user_triggered && old_positions == new_positions {
            return None;
        }

        let change = LayoutChange::new(
            workspace_id,
            old_positions,
            new_positions.clone(),
            user_triggered,
        );

        self.layout_positions.insert(workspace_id, new_positions);

        Some(change)
    }

    /// Updates focus and returns whether anything changed.
    fn update_focus(
        &mut self,
        focused_window: Option<WindowTarget>,
        focused_workspace_id: Option<Uuid>,
    ) -> bool {
        let changed = self.focused_window != focused_window
            || self.focused_workspace_id != focused_workspace_id;
        self.focused_window = focused_window;
        self.focused_workspace_id = focused_workspace_id;
        changed
    }

    /// Checks if a workspace is in monocle layout.
    #[allow(dead_code)] // May be useful for future features
    fn is_monocle(&self, workspace_id: Uuid) -> bool {
        self.workspace_layouts
            .get(&workspace_id)
            .is_some_and(|layout| *layout == LayoutType::Monocle)
    }

    /// Checks if a window is floating.
    #[allow(dead_code)] // May be useful for future features
    fn is_floating(&self, window_id: u32) -> bool { self.floating_windows.contains(&window_id) }

    /// Updates workspace layout tracking.
    fn update_workspace_layout(&mut self, workspace_id: Uuid, layout: LayoutType) {
        self.workspace_layouts.insert(workspace_id, layout);
    }

    /// Updates floating window tracking.
    fn set_window_floating(&mut self, window_id: u32, floating: bool) {
        if floating {
            self.floating_windows.insert(window_id);
        } else {
            self.floating_windows.remove(&window_id);
        }
    }

    /// Prunes cached state for a closed window.
    ///
    /// Removes the window's floating state (stale flags would otherwise be
    /// inherited by a reused window ID) and clears a stale focused-window
    /// target so no border refresh targets the destroyed window.
    fn remove_window(&mut self, window_id: u32) {
        self.floating_windows.remove(&window_id);
        if self.focused_window.is_some_and(|target| target.window_id == window_id) {
            self.focused_window = None;
        }
    }

    /// Returns whether the focused window matches `target`.
    ///
    /// A notification about a window that is not focused yet (a focus-change
    /// notification may still be queued) must not refresh the active border.
    fn focuses_window(&self, target: &WindowTarget) -> bool {
        self.focused_window.as_ref() == Some(target)
    }

    /// Returns whether the focused workspace matches `workspace_id`.
    ///
    /// A layout notification for a workspace that is not focused yet (a
    /// focus-change notification may still be queued) must not refresh the
    /// active border.
    fn focuses_workspace(&self, workspace_id: Uuid) -> bool {
        self.focused_workspace_id == Some(workspace_id)
    }
}

// ============================================================================
// Effect Subscriber
// ============================================================================

/// Subscriber notification types.
#[derive(Debug)]
pub enum SubscriberNotification {
    /// Layout needs to be recomputed for a workspace.
    LayoutChanged {
        workspace_id: Uuid,
        user_triggered: bool,
    },

    /// Focus state changed, optionally including the focused workspace layout.
    FocusChanged {
        workspace_layout: Option<(Uuid, LayoutType)>,
    },

    /// Workspace visibility changed.
    VisibilityChanged { workspace_id: Uuid, visible: bool },

    /// Window floating state changed.
    FloatingChanged {
        target: WindowTarget,
        floating: bool,
    },

    /// Workspace layout type changed.
    WorkspaceLayoutChanged {
        workspace_id: Uuid,
        layout: LayoutType,
    },

    /// Window was destroyed (prune cached per-window state).
    WindowDestroyed { window_id: u32 },

    /// Shutdown the subscriber.
    Shutdown,
}

/// Subscribes to state changes and computes effects.
///
/// The subscriber maintains a cache of previous state to compute deltas
/// when state changes. It then generates effects that the executor applies.
pub struct EffectSubscriber {
    /// Handle to the state actor for queries.
    actor_handle: StateActorHandle,

    /// Executor for applying effects.
    executor: EffectExecutor,

    /// Receiver for notifications.
    notification_rx: mpsc::Receiver<SubscriberNotification>,

    /// Previous state for computing deltas.
    state: SubscriberState,

    /// Marked complete when the event loop exits (after `Shutdown` or channel close).
    stopped: crate::modules::tiling::init::CompletionLatch,
}

/// Handle for sending notifications to the subscriber.
#[derive(Clone)]
pub struct EffectSubscriberHandle {
    notification_tx: mpsc::Sender<SubscriberNotification>,
}

impl EffectSubscriberHandle {
    /// Notifies the subscriber that a layout changed.
    pub fn notify_layout_changed(&self, workspace_id: Uuid, user_triggered: bool) {
        if let Err(e) = self
            .notification_tx
            .try_send(SubscriberNotification::LayoutChanged { workspace_id, user_triggered })
        {
            tracing::warn!(
                "tiling: dropped LayoutChanged notification for workspace {workspace_id}: {e}"
            );
        }
    }

    /// Notifies the subscriber that focus changed.
    pub fn notify_focus_changed(&self) { self.notify_focus_changed_inner(None); }

    /// Notifies the subscriber about a focus change and its workspace layout.
    ///
    /// Carrying both values in one message ensures a full queue cannot leave a
    /// focus update using an out-of-date border layout cache.
    pub fn notify_focus_changed_with_layout(&self, workspace_id: Uuid, layout: LayoutType) {
        self.notify_focus_changed_inner(Some((workspace_id, layout)));
    }

    fn notify_focus_changed_inner(&self, workspace_layout: Option<(Uuid, LayoutType)>) {
        if let Err(e) = self
            .notification_tx
            .try_send(SubscriberNotification::FocusChanged { workspace_layout })
        {
            tracing::warn!("tiling: dropped FocusChanged notification: {e}");
        }
    }

    /// Notifies the subscriber that workspace visibility changed.
    pub fn notify_visibility_changed(&self, workspace_id: Uuid, visible: bool) {
        if let Err(e) = self
            .notification_tx
            .try_send(SubscriberNotification::VisibilityChanged { workspace_id, visible })
        {
            tracing::warn!(
                "tiling: dropped VisibilityChanged notification for workspace {workspace_id}: {e}"
            );
        }
    }

    /// Notifies the subscriber that a window's floating state changed.
    pub fn notify_floating_changed(&self, target: WindowTarget, floating: bool) {
        if let Err(e) = self
            .notification_tx
            .try_send(SubscriberNotification::FloatingChanged { target, floating })
        {
            tracing::warn!(
                "tiling: dropped FloatingChanged notification for window {}: {e}",
                target.window_id
            );
        }
    }

    /// Notifies the subscriber that a workspace's layout changed.
    pub fn notify_workspace_layout_changed(&self, workspace_id: Uuid, layout: LayoutType) {
        if let Err(e) = self
            .notification_tx
            .try_send(SubscriberNotification::WorkspaceLayoutChanged { workspace_id, layout })
        {
            tracing::warn!(
                "tiling: dropped WorkspaceLayoutChanged notification for workspace {workspace_id}: {e}"
            );
        }
    }

    /// Notifies the subscriber that a window was destroyed.
    ///
    /// The subscriber prunes cached per-window state (e.g. floating state)
    /// so a reused window ID does not inherit stale flags.
    pub fn notify_window_destroyed(&self, window_id: u32) {
        if let Err(e) = self
            .notification_tx
            .try_send(SubscriberNotification::WindowDestroyed { window_id })
        {
            tracing::warn!(
                "tiling: dropped WindowDestroyed notification for window {window_id}: {e}"
            );
        }
    }

    /// Shuts down the subscriber.
    pub fn shutdown(&self) {
        if let Err(e) = self.notification_tx.try_send(SubscriberNotification::Shutdown) {
            tracing::warn!("tiling: dropped Shutdown notification: {e}");
        }
    }
}

impl EffectSubscriber {
    /// Creates a new effect subscriber.
    ///
    /// # Arguments
    ///
    /// * `actor_handle` - Handle to the state actor for queries.
    /// * `executor` - Executor for applying effects.
    ///
    /// # Returns
    ///
    /// A tuple of (subscriber, handle, completion latch). The subscriber should
    /// be spawned as a background task; the handle used to send notifications;
    /// the latch completes only after the subscriber's loop exits.
    ///
    /// `pub(crate)` because the triple exposes the crate-private
    /// `crate::modules::tiling::init::CompletionLatch`.
    #[must_use]
    pub(crate) fn new(
        actor_handle: StateActorHandle,
        executor: EffectExecutor,
    ) -> (
        Self,
        EffectSubscriberHandle,
        crate::modules::tiling::init::CompletionLatch,
    ) {
        let (notification_tx, notification_rx) = mpsc::channel(256);
        let stopped = crate::modules::tiling::init::CompletionLatch::new();

        let subscriber = Self {
            actor_handle,
            executor,
            notification_rx,
            state: SubscriberState::new(),
            stopped: stopped.clone(),
        };

        let handle = EffectSubscriberHandle { notification_tx };

        (subscriber, handle, stopped)
    }

    /// Runs the subscriber event loop.
    ///
    /// This should be spawned as a background task:
    ///
    /// ```rust,ignore
    /// tauri::async_runtime::spawn(subscriber.run());
    /// ```
    pub async fn run(mut self) {
        tracing::debug!("Effect subscriber started");

        // Initialize with current state before processing notifications
        self.initialize().await;

        // Apply initial border colors based on focused workspace layout
        self.apply_initial_border_colors();

        while let Some(notification) = self.notification_rx.recv().await {
            match notification {
                SubscriberNotification::Shutdown => {
                    tracing::debug!("Effect subscriber received shutdown");
                    break;
                }
                notification => {
                    self.handle_notification(notification).await;
                }
            }
        }

        tracing::debug!("Effect subscriber stopped");
        self.stopped.mark_complete();
    }

    /// Handles a single notification.
    async fn handle_notification(&mut self, notification: SubscriberNotification) {
        tracing::debug!("tiling: subscriber received notification: {notification:?}");

        // For layout changes that may trigger animations, signal cancellation
        // of any ongoing animation so the new one can take priority, then
        // immediately decrement to indicate we're now the active command.
        // This pattern ensures:
        // 1. Any running animation sees WAITING_COMMANDS > 0 and cancels
        // 2. Our animation sees WAITING_COMMANDS == 0 and runs normally
        let is_layout_change = matches!(notification, SubscriberNotification::LayoutChanged { .. });
        if is_layout_change {
            cancel_animation();
            begin_animation();
        }

        let effects = match notification {
            SubscriberNotification::LayoutChanged { workspace_id, user_triggered } => {
                tracing::debug!(
                    "tiling: subscriber handling LayoutChanged for workspace {workspace_id}"
                );
                self.handle_layout_changed(workspace_id, user_triggered).await
            }

            SubscriberNotification::FocusChanged { workspace_layout } => {
                if let Some((workspace_id, layout)) = workspace_layout {
                    self.state.update_workspace_layout(workspace_id, layout);
                }
                self.handle_focus_changed().await
            }

            SubscriberNotification::VisibilityChanged { workspace_id, visible } => {
                self.handle_visibility_changed(workspace_id, visible).await
            }

            SubscriberNotification::FloatingChanged { target, floating } => {
                self.handle_floating_changed(target, floating);
                // The focused window's border depends on its floating state.
                // Skip the refresh when `target` is not the focused window
                // yet: the focus-change notification that makes it current
                // may still be queued, and refreshing now would use a stale
                // focused-window target.
                if self.state.focuses_window(&target) {
                    self.refresh_active_border(self.state.focused_window)
                } else {
                    Vec::new()
                }
            }

            SubscriberNotification::WorkspaceLayoutChanged { workspace_id, layout } => {
                self.handle_workspace_layout_changed(workspace_id, layout);
                // The focused window's border depends on its workspace's
                // layout. Skip the refresh when the changed workspace is not
                // the focused one: the focus change that makes it current may
                // still be queued, and refreshing now would use stale focus
                // state.
                if self.state.focuses_workspace(workspace_id) {
                    self.refresh_active_border(self.state.focused_window)
                } else {
                    Vec::new()
                }
            }

            SubscriberNotification::WindowDestroyed { window_id } => {
                self.state.remove_window(window_id);
                Vec::new()
            }

            SubscriberNotification::Shutdown => Vec::new(),
        };

        tracing::debug!("tiling: subscriber generated {} effects", effects.len());
        if !effects.is_empty() {
            let count = self.executor.execute_batch(effects);
            tracing::debug!("tiling: subscriber executed {count} effects");
        }
    }

    /// Handles a layout change notification.
    async fn handle_layout_changed(
        &mut self,
        workspace_id: Uuid,
        user_triggered: bool,
    ) -> Vec<TilingEffect> {
        tracing::debug!(
            "tiling: handle_layout_changed for workspace {workspace_id}, user_triggered={user_triggered}"
        );

        // Query the current layout targets for this workspace
        let layout_result = self
            .actor_handle
            .query(StateQuery::GetWindowLayoutTargets { workspace_id })
            .await;

        let Ok(QueryResult::TargetLayout(new_positions)) = layout_result else {
            tracing::warn!("tiling: failed to query layout for workspace {workspace_id}");
            return Vec::new();
        };

        tracing::debug!(
            "tiling: queried layout for workspace {workspace_id}: {} windows",
            new_positions.len()
        );
        for (target, frame) in &new_positions {
            tracing::trace!("tiling:   window {} -> frame {frame:?}", target.window_id);
        }

        // Store expected frames for minimum size detection before applying layout
        // This allows us to detect when windows fail to resize to their calculated positions
        if !new_positions.is_empty()
            && let Err(e) = self.actor_handle.set_expected_frames(new_positions.clone())
        {
            tracing::warn!("tiling: failed to set expected frames: {e}");
        }

        // Update state and get the change
        let Some(change) = self.state.update_layout(workspace_id, new_positions, user_triggered)
        else {
            tracing::debug!(
                "tiling: no actual layout change detected for workspace {workspace_id}"
            );
            return Vec::new(); // No actual change
        };

        tracing::debug!(
            "tiling: layout change detected - old: {} windows, new: {} windows",
            change.old_positions.len(),
            change.new_positions.len()
        );

        // Convert change to effects
        effects_from_layout_change(&change)
    }

    /// Handles a focus change notification.
    async fn handle_focus_changed(&mut self) -> Vec<TilingEffect> {
        // Query the current focus targets
        let focus_result = self.actor_handle.query(StateQuery::GetFocusTargets).await;

        let Ok(QueryResult::TargetFocus {
            focused_window,
            focused_workspace_id,
        }) = focus_result
        else {
            tracing::warn!("Failed to query focus targets");
            return Vec::new();
        };

        // Update state and detect change
        if !self.state.update_focus(focused_window, focused_workspace_id) {
            return Vec::new(); // No actual change
        }

        // Generate the active-border refresh effect (the executor validates
        // the exact target and invokes the border helper).
        self.refresh_active_border(focused_window)
    }

    /// Handles a visibility change notification.
    async fn handle_visibility_changed(
        &mut self,
        workspace_id: Uuid,
        visible: bool,
    ) -> Vec<TilingEffect> {
        tracing::debug!(
            "tiling: handle_visibility_changed workspace={workspace_id}, visible={visible}"
        );

        let mut effects = Vec::new();

        if visible {
            // Workspace became visible - need to apply layout and show borders
            // Query current layout targets
            let layout_result = self
                .actor_handle
                .query(StateQuery::GetWindowLayoutTargets { workspace_id })
                .await;

            if let Ok(QueryResult::TargetLayout(positions)) = layout_result {
                // --- DIAGNOSTIC: compare layout vs all workspace windows ---
                // GetWindowsForWorkspace returns ALL windows (including floating/excluded),
                // unlike GetWindowLayoutTargets which only returns layoutable positions.
                if tracing::enabled!(tracing::Level::DEBUG) {
                    let all_result = self
                        .actor_handle
                        .query(StateQuery::GetWindowsForWorkspace { workspace_id })
                        .await;
                    match all_result {
                        Ok(QueryResult::Windows(all_windows)) => {
                            for w in &all_windows {
                                let in_positions =
                                    positions.iter().any(|(t, _)| t.window_id == w.id);
                                if !in_positions {
                                    tracing::debug!(
                                        "tiling: visibility diag window absent from layout: \
                                         workspace={workspace_id}, window={window_id}, \
                                         floating={is_floating}, layoutable={is_layoutable}, \
                                         in_positions={in_positions}, minimized={is_minimized}, \
                                         hidden={is_hidden}, fullscreen={is_fullscreen}, \
                                         tab_group={tab_group_id:?}, active_tab={is_active_tab}",
                                        workspace_id = workspace_id,
                                        window_id = w.id,
                                        is_floating = w.is_floating,
                                        is_layoutable = w.is_layoutable(),
                                        in_positions = in_positions,
                                        is_minimized = w.is_minimized,
                                        is_hidden = w.is_hidden,
                                        is_fullscreen = w.is_fullscreen,
                                        tab_group_id = w.tab_group_id,
                                        is_active_tab = w.is_active_tab,
                                    );
                                }
                            }
                            tracing::debug!(
                                "tiling: visibility diag summary: workspace={workspace_id}, \
                                 layout_count={}, all_windows_count={}",
                                positions.len(),
                                all_windows.len(),
                            );
                        }
                        other => {
                            tracing::debug!(
                                "tiling: visibility diag unexpected result: \
                                 workspace={workspace_id}, result={other:?}"
                            );
                        }
                    }
                }
                // --- END DIAGNOSTIC ---

                // Apply layout to all windows (no animation since workspace just appeared)
                for (target, frame) in &positions {
                    effects.push(TilingEffect::SetWindowFrame {
                        target: *target,
                        frame: *frame,
                        animate: false,
                    });
                }

                // Show borders for all windows in this workspace
                let targets: Vec<WindowTarget> = positions.iter().map(|(t, _)| *t).collect();
                if !targets.is_empty() {
                    effects.push(TilingEffect::ShowBorders { targets });
                }

                // Update cached positions
                let _ = self.state.update_layout(workspace_id, positions, false);
            }
        } else {
            // Workspace became hidden - hide borders for its windows
            if let Some(positions) = self.state.layout_positions.get(&workspace_id) {
                let targets: Vec<WindowTarget> = positions.iter().map(|(t, _)| *t).collect();
                if !targets.is_empty() {
                    effects.push(TilingEffect::HideBorders { targets });
                }
            }
        }

        // Update visibility tracking
        if visible {
            // Query workspace to get screen_id and layout
            let ws_result =
                self.actor_handle.query(StateQuery::GetWorkspace { id: workspace_id }).await;

            if let Ok(QueryResult::Workspace(Some(ws))) = ws_result {
                self.state.visible_workspaces.insert(workspace_id, ws.screen_id);
                // Track workspace layout for monocle/floating detection
                self.state.update_workspace_layout(workspace_id, ws.layout);
            }
        } else {
            self.state.visible_workspaces.remove(&workspace_id);
        }

        effects
    }

    /// Handles a floating state change.
    fn handle_floating_changed(&mut self, target: WindowTarget, floating: bool) {
        self.state.set_window_floating(target.window_id, floating);
    }

    /// Handles a workspace layout type change.
    fn handle_workspace_layout_changed(&mut self, workspace_id: Uuid, layout: LayoutType) {
        self.state.update_workspace_layout(workspace_id, layout);
    }

    /// Initializes the subscriber with current state.
    ///
    /// Call this after creating the subscriber to sync with current state
    /// before starting the event loop.
    pub async fn initialize(&mut self) {
        // Query all visible workspaces and their layouts
        let ws_result = self.actor_handle.query(StateQuery::GetVisibleWorkspaces).await;

        if let Ok(QueryResult::Workspaces(workspaces)) = ws_result {
            for ws in &workspaces {
                self.state.visible_workspaces.insert(ws.id, ws.screen_id);
                self.state.update_workspace_layout(ws.id, ws.layout);

                // Query and cache layout for this workspace
                let layout_result = self
                    .actor_handle
                    .query(StateQuery::GetWindowLayoutTargets { workspace_id: ws.id })
                    .await;

                if let Ok(QueryResult::TargetLayout(positions)) = layout_result {
                    self.state.layout_positions.insert(ws.id, positions);
                }
            }
        }

        // Query current focus targets
        let focus_result = self.actor_handle.query(StateQuery::GetFocusTargets).await;

        if let Ok(QueryResult::TargetFocus {
            focused_window,
            focused_workspace_id,
        }) = focus_result
        {
            self.state.focused_window = focused_window;
            self.state.focused_workspace_id = focused_workspace_id;
        }

        // Query all windows to track floating state
        let windows_result = self.actor_handle.query(StateQuery::GetAllWindows).await;

        if let Ok(QueryResult::Windows(windows)) = windows_result {
            for window in windows {
                if window.is_floating {
                    self.state.floating_windows.insert(window.id);
                }
            }
        }

        tracing::debug!(
            "Effect subscriber initialized with {} visible workspaces",
            self.state.visible_workspaces.len()
        );
    }

    /// Applies initial border colors based on the focused workspace's layout.
    ///
    /// This should be called after `initialize()` to set up `JankyBorders`
    /// with the correct active color for the current state.
    fn apply_initial_border_colors(&self) {
        let effects = self.refresh_active_border(self.state.focused_window);
        if !effects.is_empty() {
            let _ = self.executor.execute_batch(effects);
        }
    }

    /// Builds a `RefreshActiveBorder` effect for the focused window target.
    ///
    /// The executor validates the exact target before invoking the border
    /// helper; the subscriber never calls `borders::on_focus_changed` directly.
    fn refresh_active_border(&self, focused_window: Option<WindowTarget>) -> Vec<TilingEffect> {
        let Some(target) = focused_window else {
            return Vec::new();
        };

        // These are maintained by the notifications that precede focus
        // changes (the actor notifies the focused workspace's layout before
        // the focus change, and window creations/toggles update the floating
        // set). Avoid two more actor queries here: they can sit behind a
        // geometry burst and make the visible focus response lag noticeably.
        let layout = self
            .state
            .focused_workspace_id
            .and_then(|id| self.state.workspace_layouts.get(&id).copied())
            .unwrap_or(LayoutType::Floating);
        let is_window_floating = self.state.floating_windows.contains(&target.window_id);

        vec![TilingEffect::RefreshActiveBorder {
            target,
            layout,
            is_window_floating,
        }]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tiling::identity::{AppIdentity, LaunchDateBits};

    fn t(window_id: u32) -> WindowTarget {
        WindowTarget {
            identity: AppIdentity {
                pid: 42,
                launch_date: LaunchDateBits::from_time_interval_since_reference_date(1.0).unwrap(),
            },
            window_id,
        }
    }

    #[test]
    fn test_subscriber_state_default() {
        let state = SubscriberState::new();
        assert!(state.layout_positions.is_empty());
        assert!(state.focused_window.is_none());
        assert!(state.visible_workspaces.is_empty());
    }

    #[test]
    fn test_subscriber_state_update_layout() {
        let mut state = SubscriberState::new();
        let ws_id = Uuid::now_v7();
        let positions = vec![(t(1), Rect::new(0.0, 0.0, 100.0, 100.0))];

        // First update should produce a change
        let change = state.update_layout(ws_id, positions.clone(), false);
        assert!(change.is_some());
        assert_eq!(change.unwrap().new_positions, positions);

        // Same update should not produce a change
        let change = state.update_layout(ws_id, positions, false);
        assert!(change.is_none());

        // Different update should produce a change
        let new_positions = vec![(t(1), Rect::new(50.0, 50.0, 100.0, 100.0))];
        let change = state.update_layout(ws_id, new_positions, false);
        assert!(change.is_some());
    }

    #[test]
    fn test_subscriber_state_update_focus() {
        let mut state = SubscriberState::new();
        let ws_id = Uuid::now_v7();

        // First update should produce a change
        assert!(state.update_focus(Some(t(1)), Some(ws_id)));

        // Same update should not produce a change
        assert!(!state.update_focus(Some(t(1)), Some(ws_id)));

        // Different update should produce a change
        assert!(state.update_focus(Some(t(2)), Some(ws_id)));
        assert_eq!(state.focused_window, Some(t(2)));
    }

    #[test]
    fn test_subscriber_state_monocle_tracking() {
        let mut state = SubscriberState::new();
        let ws_id = Uuid::now_v7();

        assert!(!state.is_monocle(ws_id));

        state.update_workspace_layout(ws_id, LayoutType::Monocle);
        assert!(state.is_monocle(ws_id));

        state.update_workspace_layout(ws_id, LayoutType::Dwindle);
        assert!(!state.is_monocle(ws_id));
    }

    #[test]
    fn test_subscriber_state_floating_tracking() {
        let mut state = SubscriberState::new();

        assert!(!state.is_floating(1));

        state.set_window_floating(1, true);
        assert!(state.is_floating(1));

        state.set_window_floating(1, false);
        assert!(!state.is_floating(1));
    }

    #[test]
    fn test_subscriber_state_remove_window_prunes_floating_and_focus() {
        let mut state = SubscriberState::new();
        state.set_window_floating(1, true);
        state.set_window_floating(2, true);
        state.focused_window = Some(t(1));

        state.remove_window(1);

        assert!(
            !state.is_floating(1),
            "closed window's floating flag must be pruned"
        );
        assert!(state.is_floating(2), "other windows must be unaffected");
        assert_eq!(
            state.focused_window, None,
            "a closed focused window must not remain the refresh target"
        );
    }

    #[test]
    fn test_subscriber_state_focus_guards_match_only_current_focus() {
        let mut state = SubscriberState::new();
        let ws_id = Uuid::now_v7();
        state.focused_window = Some(t(1));
        state.focused_workspace_id = Some(ws_id);

        assert!(state.focuses_window(&t(1)));
        assert!(!state.focuses_window(&t(2)));
        assert!(state.focuses_workspace(ws_id));
        assert!(!state.focuses_workspace(Uuid::now_v7()));
    }

    fn test_subscriber(ws_id: Uuid, target: WindowTarget) -> EffectSubscriber {
        let (tx, _rx) = mpsc::channel(16);
        let actor_handle = StateActorHandle::new(tx);
        let (mut subscriber, _handle, _latch) =
            EffectSubscriber::new(actor_handle, EffectExecutor::new());
        subscriber.state.focused_workspace_id = Some(ws_id);
        subscriber.state.workspace_layouts.insert(ws_id, LayoutType::Dwindle);
        subscriber.state.focused_window = Some(target);
        subscriber
    }

    #[test]
    fn refresh_active_border_uses_cached_workspace_layout_on_direct_focus() {
        // Regression: after a direct AX focus (which emits no visibility
        // notification), the actor notifies the focused workspace's layout
        // BEFORE the focus change. The border refresh must pick up that
        // cached layout instead of defaulting to Floating.
        let ws_id = Uuid::now_v7();
        let target = t(7);
        let subscriber = test_subscriber(ws_id, target);

        let effects = subscriber.refresh_active_border(Some(target));
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            TilingEffect::RefreshActiveBorder {
                target: effect_target,
                layout,
                is_window_floating,
            } => {
                assert_eq!(*effect_target, target);
                assert_eq!(*layout, LayoutType::Dwindle);
                assert!(!is_window_floating);
            }
            _ => panic!("expected RefreshActiveBorder effect"),
        }
    }

    #[test]
    fn refresh_active_border_detects_floating_window_from_cache() {
        let ws_id = Uuid::now_v7();
        let target = t(7);
        let mut subscriber = test_subscriber(ws_id, target);
        subscriber.state.floating_windows.insert(target.window_id);

        let effects = subscriber.refresh_active_border(Some(target));
        match &effects[0] {
            TilingEffect::RefreshActiveBorder { is_window_floating, .. } => {
                assert!(is_window_floating);
            }
            _ => panic!("expected RefreshActiveBorder effect"),
        }
    }

    #[test]
    fn refresh_active_border_falls_back_to_floating_when_layout_unknown() {
        // Fail-safe: an unknown focused workspace layout defaults to
        // Floating. The actor now notifies the workspace layout before any
        // focus change, so this only protects against startup races.
        let target = t(7);
        let (tx, _rx) = mpsc::channel(16);
        let actor_handle = StateActorHandle::new(tx);
        let (mut subscriber, _handle, _latch) =
            EffectSubscriber::new(actor_handle, EffectExecutor::new());
        subscriber.state.focused_workspace_id = Some(Uuid::now_v7());
        subscriber.state.focused_window = Some(target);

        let effects = subscriber.refresh_active_border(Some(target));
        match &effects[0] {
            TilingEffect::RefreshActiveBorder { layout, .. } => {
                assert_eq!(*layout, LayoutType::Floating);
            }
            _ => panic!("expected RefreshActiveBorder effect"),
        }
    }

    #[test]
    fn refresh_active_border_no_effects_without_focus() {
        let ws_id = Uuid::now_v7();
        let subscriber = test_subscriber(ws_id, t(7));
        assert!(subscriber.refresh_active_border(None).is_empty());
    }

    #[test]
    fn test_subscriber_handle_send() {
        let (tx, mut rx) = mpsc::channel(10);
        let handle = EffectSubscriberHandle { notification_tx: tx };

        let ws_id = Uuid::now_v7();
        handle.notify_layout_changed(ws_id, true);

        let notification = rx.try_recv().unwrap();
        match notification {
            SubscriberNotification::LayoutChanged { workspace_id, user_triggered } => {
                assert_eq!(workspace_id, ws_id);
                assert!(user_triggered);
            }
            _ => panic!("Wrong notification type"),
        }
    }

    #[test]
    fn test_subscriber_handle_window_destroyed_notification() {
        let (tx, mut rx) = mpsc::channel(10);
        let handle = EffectSubscriberHandle { notification_tx: tx };

        handle.notify_window_destroyed(42);

        let notification = rx.try_recv().unwrap();
        match notification {
            SubscriberNotification::WindowDestroyed { window_id } => {
                assert_eq!(window_id, 42);
            }
            _ => panic!("Wrong notification type"),
        }
    }
}
