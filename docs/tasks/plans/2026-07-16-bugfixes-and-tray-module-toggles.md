# Bug Fixes + Tray Module Toggles — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three bugs (menubar latency, Ghostty window loss, floating-window loss), add a tray menu with runtime pause/resume toggles for six modules, and restore applications hidden by Stache before supported shutdown paths complete.

**Architecture:** Six phases. Phase 1 fixes menubar visibility detection and latency. Phase 2 adds debug-only tracing for the two intermittent bugs (no fixes). Phase 3 fixes those bugs from Phase 2 evidence (scope left open). Phase 4 introduces a `LifecycleModule` trait and makes each of the 6 modules retain its OS handle so it can pause/resume. Phase 5 builds the tray submenu from the registry. Phase 6 assigns each application a stable `AppIdentity` (PID + launch date), tracks ownership through an actor-single-writer `VisibilityRegistry`, and restores only Stache-hidden identities before tiling teardown using exact-instance validation. Phases 1 and 4/5 are independent of 2/3; 3 depends on 2's evidence; 5 depends on 4; 6 must restore windows before tiling shuts down.

**Tech Stack:** Rust (Tauri 2.x), `objc` v0.2.7 (already a dependency — no `objc2` needed), `tracing` for debug spans, Tauri `CheckMenuItem` / `TrayIcon` (already in deps), and `ctrlc` 3.4 with its `termination` feature for SIGINT/SIGTERM delivery on a safe handler thread.

**Spec:** `docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md`

---

## File Structure

| File                                                    | Responsibility                                                                       |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `app/native/src/modules/bar/menubar.rs`                 | Phase 1: add `NSMenu.menuBarVisible()` query; swap poll source; reduce interval      |
| `app/native/src/modules/bar/watcher.rs`                 | Phase 1: reduce fallback poll interval constant                                      |
| `app/native/src/modules/tiling/window.rs`               | Phase 2: tracing in subrole/size filter                                              |
| `app/native/src/modules/tiling/tabs.rs`                 | Phase 2: tracing in `is_new_window_a_tab`                                            |
| `app/native/src/modules/tiling/init.rs`                 | Phase 2: tracing in observer ordering; Phase 4: `reset()` for tiling                 |
| `app/native/src/modules/tiling/effects/subscriber.rs`   | Phase 2: tracing in `handle_visibility_changed` + `is_layoutable` path               |
| `app/native/src/modules/services/lifecycle.rs`          | Phase 4: new `LifecycleModule` trait + `ModuleStatus` enum                           |
| `app/native/src/modules/mod.rs`                         | Phase 4: expose the new `services` submodule (add `pub mod services;`)               |
| `app/native/src/modules/wallpaper/manager.rs`           | Phase 4: retain manager handle; `pause`/`resume` via timer flag                      |
| `app/native/src/modules/cmd_q/mod.rs`                   | Phase 4: retain `CGEventTap` handle; `pause`/`resume` via `CGEventTapEnable`         |
| `app/native/src/modules/notunes/mod.rs`                 | Phase 4: retain observer; `pause`/`resume` via `removeObserver`/`addObserver`        |
| `app/native/src/modules/audio/watcher.rs`               | Phase 4: retain listener addresses+callback; add `AudioObjectRemovePropertyListener` |
| `app/native/src/modules/menu_anywhere/event_monitor.rs` | Phase 4: retain `CGEventTap` handle; `pause`/`resume` via `CGEventTapEnable`         |
| `app/native/src/modules/tiling/init.rs`                 | Phase 4: implement `LifecycleModule` for tiling (shutdown + reset + re-init)         |
| `app/native/src/modules/tray/mod.rs`                    | Phase 5: retain `TrayIcon`; build `CheckMenuItem` per module; wire `on_menu_event`   |
| `app/native/src/modules/lifecycle_registry.rs`          | Phase 4/5: registry of `Box<dyn LifecycleModule>` held via `app.manage()`            |
| `app/native/src/modules/tiling/identity.rs`             | Phase 6: `AppIdentity` + `LaunchDateBits` capture/validation                         |
| `app/native/src/modules/tiling/visibility.rs`           | Phase 6: `VisibilityRegistry` (sealed `BTreeSet<AppIdentity>`), actor-single-writer  |
| `app/native/src/modules/tiling/effects/window_ops.rs`   | Phase 6: distinguish hidden-now, already-hidden, and failed hide outcomes            |
| `app/native/src/app_shutdown.rs`                        | Phase 6: shared restore → tiling shutdown → IPC shutdown orchestration               |
| `app/native/src/lib.rs`                                 | Phase 6: install signal bridge and run cleanup on Tauri exit                         |
| `app/native/src/config/watcher.rs`                      | Phase 6: route config-triggered restart through orderly cleanup                      |
| `app/native/src/modules/bar/ipc_listener.rs`            | Phase 6: route CLI reload through orderly cleanup                                    |

> Note: the existing dead trait file is `app/native/src/services/traits.rs` (NOT under `modules/`). Per spec, we do NOT reuse it. The new trait lives in `app/native/src/modules/services/lifecycle.rs` (create the `modules/services/` dir).

---

## Phase 1 — Menubar Latency Fix

> **Status: Tasks 1-2 are merged (`d03e811`..`7db3e82`) but failed manual
> verification.** `NSMenu.menuBarVisible()` reports application menu-bar
> policy, not transient auto-hide reveal state; it stayed constant during
> mouse reveal/hide and its success suppressed the `CGWindowList` fallback, so
> Stache never reacted. Task 3 is pending a corrected detector design. Do not
> re-implement `query_menu_bar_visible_via_nsmenu`; replace it after a design
> decision (owner: user).

### Task 1: Add `NSMenu.menuBarVisible()` query helper

**Files:**

- Modify: `app/native/src/modules/bar/menubar.rs`

- [ ] **Step 1: Add the query function**

Add near `query_menu_bar_visible` (after line 320). Use the existing `objc` v0.2.7 crate (already a dependency), not `objc2`.

```rust
/// Query the system menu bar visibility via the documented NSMenu.menuBarVisible()
/// API. This is the official, cheap way to detect auto-hide/show; it has no change
/// notification, so it is polled. Returns None if the ObjC call fails.
fn query_menu_bar_visible_via_nsmenu() -> Option<bool> {
    unsafe {
        let menu: *mut Object = msg_send![class!(NSMenu), classObject];
        let visible: bool = msg_send![menu, menuBarVisible];
        Some(visible)
    }
}
```

- [ ] **Step 2: Use it as the poll source in `refresh_menu_bar_visibility`**

In `refresh_menu_bar_visibility` (menubar.rs:95-115), replace the `query_menu_bar_visible()` call with the new helper, falling back to the old CGWindowList query only on failure:

```rust
fn refresh_menu_bar_visibility(
    app_handle: &AppHandle,
    window_label: &str,
    last_visible: &mut bool,
) {
    let visible = query_menu_bar_visible_via_nsmenu()
        .or_else(|| query_menu_bar_visible().ok())
        .unwrap_or(*last_visible);
    if visible != *last_visible {
        *last_visible = visible;
        MENU_BAR_VISIBLE.store(visible, Ordering::Release);
        if let Err(e) = emit_menubar_visibility_event(app_handle, window_label, visible) {
            tracing::warn!(error = %e, "failed to emit menubar visibility");
        }
    }
}
```

Also update `register_menu_bar_visibility_observer` (menubar.rs:66-93) initial-state read to use the helper:

```rust
let initial_state = query_menu_bar_visible_via_nsmenu()
    .or_else(|| query_menu_bar_visible().ok())
    .unwrap_or(false);
```

- [ ] **Step 3: Build to confirm it compiles**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles with no errors (warnings about unused `query_menu_bar_visible` are fine for now; it is still used as fallback).

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/bar/menubar.rs
git commit -m "fix(bar): query menubar visibility via NSMenu.menuBarVisible"
```

### Task 2: Reduce the fallback poll interval

**Files:**

- Modify: `app/native/src/modules/bar/menubar.rs:31`

- [ ] **Step 1: Change the poll interval constant**

In `menubar.rs`, change line 31:

```rust
const MENU_BAR_FALLBACK_POLL_INTERVAL: Duration = Duration::from_millis(100);
```

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/bar/menubar.rs
git commit -m "fix(bar): reduce menubar fallback poll to 100ms"
```

### Task 3: Verify Phase 1 latency

**Files:** none (manual verification)

- [ ] **Step 1: Run the app in debug and reproduce**

Run: `cd /Users/marcosmoura/Projects/stache && pnpm tauri:dev`
Move the mouse to the top screen edge to reveal the system menu bar, then away. Observe the Stache bar reaction.

- [ ] **Step 2: Confirm reaction is ≤200ms (blocked)**

This step failed with the merged detector: `NSMenu.menuBarVisible()` stayed
constant during mouse reveal/hide and suppressed the CG fallback, so Stache
never reacted (manual Phase 1/2 session). Verification cannot pass until a
corrected detector is designed and merged. Do not re-run this step against the
current NSMenu-first path; do not add temporary tracing around
`query_menu_bar_visible_via_nsmenu` (it is being removed by the redesign).

- [ ] **Step 3: Commit nothing** — verification only. Report result.

---

## Phase 2 — Diagnostic Tracing (No Fixes)

> **Status: Tasks 4-7 are merged and reviewed (`de63053`..`76c960a`).** Task 8
> was attempted in the combined manual session: neither the Ghostty nor the
> floating-window bug reproduced (Ghostty was managed as a standard window),
> and no logs were saved. Phase 3 remains blocked until the user supplies a
> repro log.

> All tracing uses `tracing::debug!`. Debug builds default to `RUST_LOG=warn,stache=debug`, so spans appear automatically. No new env var.

### Task 4: Trace window enumeration (Ghostty subrole/size)

**Files:**

- Modify: `app/native/src/modules/tiling/window.rs:227-244`

- [ ] **Step 1: Add debug spans around the subrole/size decision**

In the `should_manage` match (window.rs:227-244), log the decision per window:

```rust
let should_manage = match subrole.as_deref() {
    Some("AXSheet" | "AXDrawer" | "AXUnknown") => {
        tracing::debug!(
            window_owner = %owner,
            subrole = %subrole.as_deref().unwrap_or(""),
            "tiling: skipping window with non-standard subrole"
        );
        false
    }
    Some("AXDialog") => {
        let ok = frame.width >= MIN_DIALOG_WIDTH && frame.height >= MIN_DIALOG_HEIGHT;
        tracing::debug!(window_owner = %owner, subrole = "AXDialog", width = frame.width, height = frame.height, managed = ok, "tiling: dialog subrole size check");
        ok
    }
    _ => {
        let ok = frame.width >= MIN_STANDARD_WIDTH && frame.height >= MIN_STANDARD_HEIGHT;
        tracing::debug!(window_owner = %owner, subrole = %subrole.as_deref().unwrap_or("<none>"), width = frame.width, height = frame.height, managed = ok, "tiling: standard subrole size check");
        ok
    }
};
```

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/tiling/window.rs
git commit -m "debug(tiling): trace window enumeration subrole/size decisions"
```

### Task 5: Trace tab classification

**Files:**

- Modify: `app/native/src/modules/tiling/tabs.rs:376-408`

- [ ] **Step 1: Add debug spans in `is_new_window_a_tab`**

At the start and end of `is_new_window_a_tab` (tabs.rs:376):

```rust
pub fn is_new_window_a_tab(pid: i32, new_window_id: u32, workspace_window_ids: &[u32]) -> bool {
    tracing::debug!(pid, new_window_id, "tiling: evaluating is_new_window_a_tab");
    scan_and_register_tabs_for_app(pid);
    if is_tab(new_window_id) {
        tracing::debug!(new_window_id, "tiling: window already registered as tab");
        return true;
    }
    let registry = get_registry().read();
    let tabs_for_this_pid: HashSet<u32> = registry.tabs_for_pid(pid).into_iter().collect();
    for &wid in workspace_window_ids {
        if wid == new_window_id { continue; }
        if let Some(&tracked_pid) = registry.window_to_pid.get(&wid)
            && tracked_pid == pid
            && !tabs_for_this_pid.contains(&wid)
        {
            tracing::debug!(new_window_id, sibling_window = wid, "tiling: classified as tab (has non-tab sibling from same pid)");
            return true;
        }
    }
    tracing::debug!(new_window_id, "tiling: classified as NOT a tab");
    false
}
```

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/tiling/tabs.rs
git commit -m "debug(tiling): trace tab classification decisions"
```

### Task 6: Trace observer init ordering

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs` (around `init_internal` and `init`)

- [ ] **Step 1: Add debug spans around init ordering**

In `init()` (init.rs:122-170) after the `INITIALIZED` guard and before `init_internal`:

```rust
    tracing::debug!("tiling: init() entered; accessibility granted = {}", is_accessibility_granted());
```

In `init_internal` (init.rs:191-285), add a span before each major step so the order is observable:

```rust
    tracing::debug!("tiling: spawning StateActor");
    // ... after StateActor::spawn()
    tracing::debug!("tiling: starting EventProcessor");
    // ... after processor.start()
    tracing::debug!("tiling: starting EffectSubscriber");
    // ... after subscriber spawn
    tracing::debug!("tiling: installing app/screen/ax monitors");
    // ... after the three install_adapter calls
    tracing::debug!("tiling: initializing state (screens + windows)");
    // ... after initialize_state
```

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/tiling/init.rs
git commit -m "debug(tiling): trace init/observer ordering"
```

### Task 7: Trace floating-window visibility path

**Files:**

- Modify: `app/native/src/modules/tiling/effects/subscriber.rs:471-529`

- [ ] **Step 1: Add debug spans in `handle_visibility_changed`**

In `handle_visibility_changed` (subscriber.rs:471), at the top:

```rust
    tracing::debug!(workspace_id = %workspace_id, visible, "tiling: handle_visibility_changed");
```

In the `visible` branch, after `GetWindowLayout`, log which window ids are layoutable vs floating:

```rust
            for (window_id, frame) in &positions {
                effects.push(TilingEffect::SetWindowFrame { window_id: *window_id, frame: *frame, animate: false });
            }
            // Log floating exclusion for diagnostics
            for (window_id, _frame) in &positions {
                if let Ok(QueryResult::Window(Some(w))) = self.actor_handle.query(StateQuery::GetWindow { id: *window_id }).await {
                    if !w.is_layoutable() {
                        tracing::debug!(window_id = *window_id, floating = w.is_floating, "tiling: window excluded from layout (not layoutable)");
                    }
                }
            }
```

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles. (If `StateQuery::GetWindow` does not exist, use the actual query variant — check `app/native/src/modules/tiling/actor/messages.rs` and adjust; the goal is only to log floating exclusion, not to change behavior.)

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/tiling/effects/subscriber.rs
git commit -m "debug(tiling): trace floating-window visibility handling"
```

### Task 8: Manual Phase 2 reproduction (evidence collection)

**Files:** none

- [ ] **Step 1: Run app in debug**

Run: `cd /Users/marcosmoura/Projects/stache && pnpm tauri:dev`

- [ ] **Step 2: Reproduce Ghostty bug**

Open several Ghostty windows, move them, focus/blur, tile. Capture `tracing::debug!` output filtered to `tiling:`. Save the log.

- [ ] **Step 3: Reproduce floating-window bug**

Set a window floating, switch workspaces away and back, observe whether it becomes undetectable. Capture logs. Save the log.

- [ ] **Step 4: Hand logs back** — Phase 3 is written only after this evidence is reviewed. Do NOT proceed to Phase 3 until root cause is confirmed from these logs.

---

## Phase 3 — Fix Ghostty + Floating-Window Bugs

> **This phase is intentionally deferred.** Its tasks are written only after Phase 2 log evidence confirms the actual mechanism. Do not implement speculative fixes. When evidence is available, add tasks here following the same TDD structure (write failing test → implement minimal fix → verify → commit). The concrete change list will be derived from the logs, not from the hypotheses in the spec.

---

## Phase 4 — Uniform Module Lifecycle Contract

### Task 9: Define the `LifecycleModule` trait

**Files:**

- Create: `app/native/src/modules/services/lifecycle.rs`
- Modify: `app/native/src/modules/services/mod.rs` (create if missing, or add `pub mod lifecycle;`)
- Modify: `app/native/src/modules/mod.rs` (add `pub mod services;`; without it, `crate::modules::services` does not compile)

- [ ] **Step 1: Create the trait file**

```rust
use std::fmt::Debug;

/// Runtime lifecycle state for a Stache module, surfaced in the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleStatus {
    /// Disabled in persisted config. Shown in tray but locked (cannot be toggled at runtime).
    ConfiguredOff,
    /// Configured on and currently active.
    Running,
    /// Configured on but paused by the user via the tray. Resumes on restart/reload.
    Paused,
    /// Configured on but failed to start. `reason` explains why (e.g. missing permission).
    Unavailable(String),
}

/// Uniform contract so every module can be paused/resumed from the tray without
/// per-module special-casing. Implemented by each of the 6 toggleable modules.
pub trait LifecycleModule: Send + Sync {
    /// Human-readable module name (also used as the tray item label).
    fn name(&self) -> &'static str;
    /// Stable menu item id used to route `on_menu_event`.
    fn id(&self) -> &'static str;
    /// Start the module (called once at startup if configured on).
    fn start(&self) -> Result<(), String>;
    /// Pause the module, releasing/disabling its OS resources.
    fn pause(&self) -> Result<(), String>;
    /// Resume the module, re-acquiring OS resources.
    fn resume(&self) -> Result<(), String>;
    /// Current lifecycle status.
    fn status(&self) -> ModuleStatus;
}
```

- [ ] **Step 2: Expose the module**

If `app/native/src/modules/services/mod.rs` does not exist, create it:

```rust
pub mod lifecycle;
```

If it exists, add `pub mod lifecycle;`.

Then add `pub mod services;` to `app/native/src/modules/mod.rs` (next to the
existing `pub mod` declarations). Without this parent declaration, Rust will
not compile `crate::modules::services::lifecycle` in Task 10.

- [ ] **Step 3: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/services/lifecycle.rs app/native/src/modules/services/mod.rs app/native/src/modules/mod.rs
git commit -m "feat(lifecycle): add LifecycleModule trait and ModuleStatus"
```

### Task 10: Retain wallpapers handle + implement trait

**Files:**

- Modify: `app/native/src/modules/wallpaper/manager.rs`

- [ ] **Step 1: Make `WallpaperManager` implement `LifecycleModule`**

At the end of `manager.rs`, add:

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

impl LifecycleModule for WallpaperManager {
    fn name(&self) -> &'static str { "Wallpapers" }
    fn id(&self) -> &'static str { "wallpapers" }

    fn start(&self) -> Result<(), String> {
        self.start_timer();
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        self.stop_timer();
        Ok(())
    }

    fn resume(&self) -> Result<(), String> {
        self.start_timer();
        Ok(())
    }

    fn status(&self) -> ModuleStatus {
        if !self.config.enabled {
            return ModuleStatus::ConfiguredOff;
        }
        if self.timer_running.load(Ordering::SeqCst) {
            ModuleStatus::Running
        } else {
            ModuleStatus::Paused
        }
    }
}
```

> Note: `WallpaperManager` is already behind `Arc` (used as `Arc<Self>` in `start_timer`). The `OnceLock<WallpaperManager>` in `setup()` already retains the handle — no new field needed; the `LifecycleModule` impl is added to the existing struct.

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/wallpaper/manager.rs
git commit -m "feat(wallpapers): implement LifecycleModule"
```

### Task 11: Retain commandQuit tap handle + implement trait

**Files:**

- Modify: `app/native/src/modules/cmd_q/mod.rs`

- [ ] **Step 1: Store the tap handle**

In `cmd_q/mod.rs`, add a static to retain the tap handle (the `tap` local in `start_event_tap` is currently dropped). Add near the other statics (around line 104):

```rust
use core_foundation::mach_port::CFMachPort;
static EVENT_TAP: Mutex<Option<CFMachPort>> = Mutex::new(None);
```

In `start_event_tap` (cmd_q/mod.rs:200-228), after `CGEventTapEnable(tap, true);` and before the run loop, store the handle:

```rust
CGEventTapEnable(tap, true);
*EVENT_TAP.lock().unwrap() = Some(tap_port.clone());
CFRunLoop::run_current();
```

(Use the existing `tap_port: CFMachPort` variable already created at line 215.)

- [ ] **Step 2: Implement `LifecycleModule`**

At the end of `cmd_q/mod.rs`:

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

impl LifecycleModule for CmdQ {
    fn name(&self) -> &'static str { "Command Quit" }
    fn id(&self) -> &'static str { "commandQuit" }

    fn start(&self) -> Result<(), String> {
        if IS_RUNNING.load(Ordering::SeqCst) { return Ok(()); }
        init(self.app_handle.clone());
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        if let Some(tap) = EVENT_TAP.lock().unwrap().as_ref() {
            unsafe { CGEventTapEnable(tap.as_concrete_TypeRef(), false); }
            Ok(())
        } else {
            Err("event tap handle not available".to_string())
        }
    }

    fn resume(&self) -> Result<(), String> {
        if let Some(tap) = EVENT_TAP.lock().unwrap().as_ref() {
            unsafe { CGEventTapEnable(tap.as_concrete_TypeRef(), true); }
            Ok(())
        } else {
            Err("event tap handle not available".to_string())
        }
    }

    fn status(&self) -> ModuleStatus {
        if !config::get_config().command_quit.is_enabled() {
            return ModuleStatus::ConfiguredOff;
        }
        if IS_RUNNING.load(Ordering::SeqCst) {
            ModuleStatus::Running
        } else {
            ModuleStatus::Paused
        }
    }
}
```

> Adjust `CmdQ` struct field names (`app_handle`) and `config` access to match the actual file. The `init` function signature in cmd_q may take `&AppHandle` — adapt the `start()` call accordingly. If `CmdQ` is not a struct but module functions, wrap the statics in a small `CmdQLifecycle` struct that implements the trait instead.

- [ ] **Step 3: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles. Fix any `CFMachPort`/`CGEventTapEnable` type mismatches against the existing `start_event_tap` usage.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/cmd_q/mod.rs
git commit -m "feat(commandQuit): retain tap handle + implement LifecycleModule"
```

### Task 12: Retain notunes observer + implement trait

**Files:**

- Modify: `app/native/src/modules/notunes/mod.rs`

- [ ] **Step 1: Store the observer reference**

Add a static near the others (notunes/mod.rs:31):

```rust
use objc::runtime::Object;
static WORKSPACE_OBSERVER: Mutex<Option<*mut Object>> = Mutex::new(None);
```

In `setup_workspace_observer` (notunes/mod.rs:112-130), after `addObserver:`, store the observer:

```rust
    let observer = unsafe { create_observer_object() };
    // ... existing addObserver msg_send ...
    *WORKSPACE_OBSERVER.lock().unwrap() = Some(observer);
```

- [ ] **Step 2: Implement `LifecycleModule`**

At the end of `notunes/mod.rs`:

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

impl LifecycleModule for NoTunes {
    fn name(&self) -> &'static str { "NoTunes" }
    fn id(&self) -> &'static str { "notunes" }

    fn start(&self) -> Result<(), String> {
        if IS_RUNNING.load(Ordering::SeqCst) { return Ok(()); }
        init();
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        if let Some(observer) = WORKSPACE_OBSERVER.lock().unwrap().take() {
            unsafe {
                let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
                let center: *mut Object = msg_send![workspace, notificationCenter];
                let _: () = msg_send![center, removeObserver: observer];
            }
            Ok(())
        } else {
            Err("observer not available".to_string())
        }
    }

    fn resume(&self) -> Result<(), String> {
        setup_workspace_observer();
        Ok(())
    }

    fn status(&self) -> ModuleStatus {
        if !config::get_config().no_tunes.is_enabled() {
            return ModuleStatus::ConfiguredOff;
        }
        if IS_RUNNING.load(Ordering::SeqCst) {
            ModuleStatus::Running
        } else {
            ModuleStatus::Paused
        }
    }
}
```

> Adapt `NoTunes` struct / `init()` / `config` access to the actual file. If `notunes` is module functions, wrap in a `NoTunesLifecycle` struct.

- [ ] **Step 3: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/notunes/mod.rs
git commit -m "feat(notunes): retain observer + implement LifecycleModule"
```

### Task 13: Retain proxyAudio listeners + add removal + implement trait

**Files:**

- Modify: `app/native/src/modules/audio/watcher.rs`

- [ ] **Step 1: Store listener addresses + callback pointer**

Add a struct to retain the three `AudioObjectPropertyAddress` values and the `tx_ptr` client data used in `register_audio_listeners` (watcher.rs:177-222):

```rust
struct RetainedListeners {
    output: AudioObjectPropertyAddress,
    input: AudioObjectPropertyAddress,
    devices: AudioObjectPropertyAddress,
    client_data: *mut c_void,
}
static RETAINED_LISTENERS: Mutex<Option<RetainedListeners>> = Mutex::new(None);
```

In `register_audio_listeners`, after the three `AudioObjectAddPropertyListener` calls, store them:

```rust
*RETAINED_LISTENERS.lock().unwrap() = Some(RetainedListeners {
    output: output_property_address,
    input: input_property_address,
    devices: devices_property_address,
    client_data: tx_ptr as *mut c_void,
});
```

- [ ] **Step 2: Add `AudioObjectRemovePropertyListener` calls**

Add a `remove_audio_listeners()` function mirroring `register_audio_listeners` but calling `AudioObjectRemovePropertyListener` with the same addresses and callback (`audio_device_property_listener`):

```rust
unsafe fn remove_audio_listeners() {
    if let Some(retained) = RETAINED_LISTENERS.lock().unwrap().take() {
        AudioObjectRemovePropertyListener(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&retained.output),
            Some(audio_device_property_listener),
            retained.client_data,
        );
        AudioObjectRemovePropertyListener(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&retained.input),
            Some(audio_device_property_listener),
            retained.client_data,
        );
        AudioObjectRemovePropertyListener(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&retained.devices),
            Some(audio_device_property_listener),
            retained.client_data,
        );
    }
}
```

> The callback reference MUST match exactly (`Some(audio_device_property_listener)` and the same `client_data` pointer) or CoreAudio leaks/crashes. This is the highest-risk part of proxyAudio.

- [ ] **Step 3: Implement `LifecycleModule`**

At the end of `watcher.rs` (or in `audio/mod.rs` if the module struct lives there):

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

impl LifecycleModule for ProxyAudio {
    fn name(&self) -> &'static str { "Proxy Audio" }
    fn id(&self) -> &'static str { "proxyAudio" }

    fn start(&self) -> Result<(), String> {
        start();
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        unsafe { remove_audio_listeners(); }
        Ok(())
    }

    fn resume(&self) -> Result<(), String> {
        start();
        Ok(())
    }

    fn status(&self) -> ModuleStatus {
        if !config::get_config().proxy_audio.is_enabled() {
            return ModuleStatus::ConfiguredOff;
        }
        if RETAINED_LISTENERS.lock().unwrap().is_some() {
            ModuleStatus::Running
        } else {
            ModuleStatus::Paused
        }
    }
}
```

> Adapt `ProxyAudio` struct / `start()` / `config` access to the actual file. If `audio` is module functions, wrap in a `ProxyAudioLifecycle` struct.

- [ ] **Step 4: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add app/native/src/modules/audio/watcher.rs
git commit -m "feat(proxyAudio): retain listeners, add removal, implement LifecycleModule"
```

### Task 14: Retain menuAnywhere tap handle + implement trait

**Files:**

- Modify: `app/native/src/modules/menu_anywhere/event_monitor.rs`

- [ ] **Step 1: Store the tap handle**

Add a static near the top of `event_monitor.rs`:

```rust
use core_foundation::mach_port::CFMachPort;
static EVENT_TAP: Mutex<Option<CFMachPort>> = Mutex::new(None);
```

In `start` (event_monitor.rs:86-107), after `CGEventTapEnable(tap, true);`, store the handle:

```rust
CGEventTapEnable(tap, true);
*EVENT_TAP.lock().unwrap() = Some(tap_port.clone());
CFRunLoop::run_current();
```

(Use the existing `tap_port: CFMachPort` variable already created.)

- [ ] **Step 2: Implement `LifecycleModule`**

In `menu_anywhere/mod.rs` (or `event_monitor.rs`), add:

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

impl LifecycleModule for MenuAnywhere {
    fn name(&self) -> &'static str { "Menu Anywhere" }
    fn id(&self) -> &'static str { "menuAnywhere" }

    fn start(&self) -> Result<(), String> {
        if IS_RUNNING.load(Ordering::SeqCst) { return Ok(()); }
        init(self.app_handle.clone());
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        if let Some(tap) = EVENT_TAP.lock().unwrap().as_ref() {
            unsafe { CGEventTapEnable(tap.as_concrete_TypeRef(), false); }
            Ok(())
        } else {
            Err("event tap handle not available".to_string())
        }
    }

    fn resume(&self) -> Result<(), String> {
        if let Some(tap) = EVENT_TAP.lock().unwrap().as_ref() {
            unsafe { CGEventTapEnable(tap.as_concrete_TypeRef(), true); }
            Ok(())
        } else {
            Err("event tap handle not available".to_string())
        }
    }

    fn status(&self) -> ModuleStatus {
        if !config::get_config().menu_anywhere.is_enabled() {
            return ModuleStatus::ConfiguredOff;
        }
        if IS_RUNNING.load(Ordering::SeqCst) {
            ModuleStatus::Running
        } else {
            ModuleStatus::Paused
        }
    }
}
```

> Adapt `MenuAnywhere` struct / `init()` / `config` access to the actual file. If `menu_anywhere` is module functions, wrap in a `MenuAnywhereLifecycle` struct.

- [ ] **Step 3: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/menu_anywhere/event_monitor.rs app/native/src/modules/menu_anywhere/mod.rs
git commit -m "feat(menuAnywhere): retain tap handle + implement LifecycleModule"
```

### Task 15: Add tiling `reset()` + implement trait

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs` — serialized lifecycle state,
  clearable runtime ownership, startup rollback, ordered teardown
- Modify: `app/native/src/modules/tiling/actor/mod.rs` and `actor/handle.rs` —
  actor completion signal and shutdown acknowledgement
- Modify: `app/native/src/modules/tiling/events/processor.rs` — stop/gate,
  clear pending batches/routes, and quiescence acknowledgement
- Modify: `app/native/src/modules/tiling/effects/subscriber.rs` — subscriber
  completion signal and shutdown acknowledgement
- Modify: `app/native/src/modules/tiling/events/app_monitor.rs` — retain/remove
  NSWorkspace observer and generation-gate callbacks
- Modify: `app/native/src/modules/tiling/events/screen_monitor.rs` — unregister
  CG display callback and invalidate delayed workers
- Modify: `app/native/src/modules/tiling/events/ax_observer.rs` — deactivate then
  uninstall adapter
- Modify: `app/native/src/modules/tiling/events/observer.rs` — drain/release all
  standalone AX observers and reset its init guard
- Modify: `app/native/src/modules/tiling/events/mouse_monitor.rs` — make the
  process-lifetime tap explicitly active/inactive and gate callbacks
- Modify: `app/native/src/modules/tiling/events/drag_state.rs` — cancel stale drag
- Modify: `app/native/src/modules/tiling/effects/animation/state.rs` — complete
  transient animation reset
- Modify: `app/native/src/modules/tiling/borders.rs` — pause/resume the retained
  process-lifetime runner and clear stale commands
- Modify: `app/native/src/modules/tiling/tabs.rs` — clear tab state on teardown
- Modify: `app/native/src/modules/tiling/effects/window_cache.rs` — clear retained
  AX objects on teardown
- Modify: `app/native/src/modules/tiling/mod.rs` — lifecycle re-exports
- Modify: `app/native/src/lib.rs` — invoke tiling startup through the existing
  synchronous main-thread bridge rather than directly from the lazy async task

- [ ] **Step 1: Write failing lifecycle and completion tests**

Add deterministic tests around injected runtime resources, without invoking
live AX/AppKit APIs:

1. `init → pause → resume` creates actor/processor/subscriber generation 2;
   generation-1 handles are closed and cannot accept messages.
2. Teardown order is restore visibility ownership → main-thread adapter and
   standalone-observer gates/unregister plus mouse gating → processor
   quiescence → drag/animation/border pause → subscriber stop/ack → actor
   stop/ack → cache/tab/transient clearing. No completion wait precedes queued
   main-thread removal work.
3. A delayed screen/AX/app callback tagged generation 1 is ignored after
   generation 2 starts.
4. Processor stop clears pending geometry batches and exact routing maps before
   acknowledging quiescence.
5. Actor and subscriber completion receivers both resolve before runtime slots
   are dropped.
6. Failure after every fatal initialization stage invokes the same rollback
   path, leaves lifecycle `Stopped`, leaves `RuntimeSlot::Empty`, and permits a
   subsequent successful start. Optional mouse/border failures instead enter
   `Running` in a logged degraded mode.
7. A subscriber/actor/processor completion timeout retains the complete runtime
   in `RuntimeSlot::Quarantined`; start/resume is rejected, and a later
   pause/shutdown retries only the unfinished idempotent teardown stages.
8. Concurrent start/pause/retry requests serialize; only one transition owns
   the runtime, and no request can discard a quarantined generation.
9. Main-thread-required startup and teardown hooks are observed on the main
   thread, while no lifecycle/runtime/visibility lock is held across dispatch.

Run focused tests first and observe RED compilation for the missing runtime and
completion APIs.

- [ ] **Step 2: Replace single-shot globals with one clearable runtime**

In `init.rs`, replace `HANDLE`, `PROCESSOR`, `SUBSCRIBER_HANDLE`, and
`INITIALIZED` with:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleState { Stopped, Starting, Running, Stopping }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitStage {
    Actor,
    Processor,
    Subscriber,
    AppMonitor,
    ScreenMonitor,
    AxAdapter,
    StandaloneObservers,
    InitialState,
    MouseMonitor,
    Borders,
}

/// Repeatable completion state: unlike a consumed one-shot receiver, retries
/// can observe that a resource has already stopped.
#[derive(Clone)]
pub(crate) struct CompletionLatch(Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>);

/// Owns every resource created so far during startup. Optional fields permit
/// rollback/quarantine after failure at any fatal InitStage.
struct PartialRuntime {
    generation: u64,
    actor: Option<StateActorHandle>,
    actor_stopped: Option<CompletionLatch>,
    processor: Option<Arc<EventProcessor>>,
    subscriber: Option<EffectSubscriberHandle>,
    subscriber_stopped: Option<CompletionLatch>,
    app_monitor: Option<Arc<AppMonitorAdapter>>,
    screen_monitor: Option<Arc<ScreenMonitorAdapter>>,
    ax_adapter: Option<Arc<AXObserverAdapter>>,
    teardown: TeardownProgress,
}

struct TilingRuntime {
    generation: u64,
    actor: StateActorHandle,
    actor_stopped: CompletionLatch,
    processor: Arc<EventProcessor>,
    subscriber: EffectSubscriberHandle,
    subscriber_stopped: CompletionLatch,
    app_monitor: Arc<AppMonitorAdapter>,
    screen_monitor: Arc<ScreenMonitorAdapter>,
    ax_adapter: Arc<AXObserverAdapter>,
    teardown: TeardownProgress,
}

#[derive(Default)]
struct TeardownProgress {
    visibility_restored: bool,
    main_thread_sources_removed: bool,
    processor_stopped: bool,
    transient_services_paused: bool,
    subscriber_stopped: bool,
    actor_stopped: bool,
    caches_cleared: bool,
}

enum RuntimeSlot {
    Empty,
    Running(TilingRuntime),
    Quarantined(PartialRuntime),
}

static LIFECYCLE: Mutex<LifecycleState> = Mutex::new(LifecycleState::Stopped);
static RUNTIME: Mutex<RuntimeSlot> = Mutex::new(RuntimeSlot::Empty);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
const RUNTIME_STOP_TIMEOUT: Duration = Duration::from_secs(2);
```

Remove `std::sync::Mutex`/`OnceLock` from the old global-state imports; these
slots deliberately use non-poisoning `parking_lot::Mutex`, so every `.lock()`
expression in this task returns a guard directly.

`get_handle`, `get_processor`, and `get_subscriber_handle` return cloned owned
handles from `RUNTIME`; no API returns a `'static` reference into the mutex.
Update every callsite. `is_initialized()` is exactly
`*LIFECYCLE.lock() == LifecycleState::Running`.

Starting performs a compare/set under `LIFECYCLE`, builds all resources as
locals for one generation, and publishes `RuntimeSlot::Running` only after every
fatal stage succeeds. Define a small injected `RuntimeFactory`/hook table whose
methods are keyed by `InitStage`; production methods create real resources and
tests fail one named stage at a time. Fatal stages are actor, processor,
subscriber, app-monitor observer registration, screen-monitor callback
registration, AX adapter activation, standalone observer setup, and initial
screen/state enumeration. Mouse drag monitoring and borders are optional:
failure logs a degraded status but still permits `Running`.

`PartialRuntime` owns each created resource until publication; its rollback
calls the same idempotent teardown helpers in reverse-safe order. After every
fatal stage succeeds, `TilingRuntime::try_from(PartialRuntime)` requires all
mandatory fields and publishes `RuntimeSlot::Running`. A fatal startup error
must either finish rollback and publish `RuntimeSlot::Empty`/`Stopped`, or
retain that partially populated payload as
`RuntimeSlot::Quarantined(PartialRuntime)`/`Stopping` if a completion barrier
times out. A timed-out fully running runtime is converted losslessly into the
same optional-resource `PartialRuntime` representation. Never report `Stopped`
while a resource remains alive.

`CompletionLatch` exposes `mark_complete()` and repeatable
`wait_timeout(Duration) -> bool`; completion remains observable after a timeout
or successful wait. `TeardownProgress` is stored with the runtime and advances
only after each stage succeeds, making every retry explicit and idempotent.

All AppKit/AX stages (`AppMonitor`, standalone observer setup/removal, AX
activation/deactivation, and initial screen enumeration) run through the
existing synchronous main-thread helper, executing inline when already on the
main thread. `lib.rs` must bridge the current lazy async `tiling::init` call to
that helper. Copy/take owned inputs before dispatch and hold no lifecycle,
runtime, or visibility-registry lock while waiting for main-thread execution.

- [ ] **Step 3: Add completion and callback-quiescence contracts**

Change actor spawn and subscriber construction to return reusable
`CompletionLatch` values. Their task bodies mark the latch complete as the final
action after the event loop exits. Teardown sends subscriber shutdown first,
waits with a bounded timeout, then sends actor shutdown and waits. Completed
stages are recorded in `TilingRuntime`, so retry does not resend or repeat
destructive work. On timeout, return the lifecycle error and reinsert the owned
runtime as `RuntimeSlot::Quarantined`; do not publish a replacement runtime
while an old task may still mutate state. A later pause/shutdown retries the
remaining barriers from that slot.

`EventProcessor::stop_and_wait()` atomically rejects new events, stops all
timers, waits for each timer's completion acknowledgement, then clears pending
geometry/creation batches and routing maps. `start()` resets those structures
and activates only the new generation.

App/screen/AX adapters carry `generation` plus an active flag. Their callbacks
check both before forwarding. App monitor retains its observer token and
removes it from NSWorkspace notification center. Screen monitor removes its CG
display reconfiguration callback and invalidates delayed 200ms workers. AX
adapter calls `deactivate()` before `uninstall_adapter()`. Standalone observer
`shutdown()` drains/removes/releases every observer and resets its clearable
init state.

The delayed screen-monitor workers must never call AppKit or enumerate screens
on their worker thread. After the debounce/generation check, synchronously
dispatch the screen query/state handoff to the main thread and re-check the
generation there. Teardown performs every main-thread unregister/deactivate
operation before waiting on processor/subscriber/actor completion; no bounded
wait may depend on main-thread work that is still queued behind the waiter.

- [ ] **Step 4: Gate process-lifetime services and clear transient state**

Do not attempt to recreate low-level services that currently have permanent
run loops/OnceLocks. Make their lifetime explicit and their behavior reversible:

- Mouse monitor keeps its tap/thread for process lifetime, but `set_active(false)`
  disables event processing, clears the mouse-up callback, and calls
  `drag_state::cancel_operation()`. Resume sets the new callback, then activates.
- Border animation runner remains process lifetime. `borders::pause()` marks it
  inactive, drops/ignores queued animation commands, sends a zero-width/hidden
  border command, and clears the last-command cache. `resume()` activates it
  and calls `refresh()` after fresh tiling state exists.
- Cancel and await active window animations, then clear animation end time,
  interrupted positions, active/waiting counters. Retain display-link/sync
  singletons process-wide; they may not publish effects while lifecycle is not
  `Running`.
- Clear exact tab registry and AX window/application caches after actor exit.

- [ ] **Step 5: Implement ordered `pause`/`resume` and trait**

`pause_runtime()` serializes `Running → Stopping` while leaving the running
runtime published long enough for the public restoration helper to access its
current ownership store. It releases lifecycle/runtime mutexes, performs stage
1, then takes the runtime from `RuntimeSlot::Running` for the remaining stages.
For a retry it takes `RuntimeSlot::Quarantined` directly. It performs these
idempotent stages:

1. call `restore_stache_hidden_apps()` before actor teardown, using the current
   generation's ownership store; this is idempotent with terminal cleanup;
2. on the main thread, deactivate/gate and unregister AX, app, screen, and
   standalone observers/callbacks; complete all main-thread work before waits;
3. processor `stop_and_wait` and pending-map clear;
4. cancel drag/animations and pause borders/mouse callbacks;
5. subscriber shutdown and reusable completion-latch wait;
6. actor shutdown and reusable completion-latch wait;
7. drop all taken handles/Arcs;
8. clear tabs, AX caches, and transient animation/border state;
9. set `RuntimeSlot::Empty` and lifecycle `Stopped` only after every required
   completion barrier passes.

If any stage times out or fails, retain the entire runtime and its per-stage
completion flags in `RuntimeSlot::Quarantined`, keep lifecycle `Stopping`, and
return the error. `start`/`resume` reject this state. Repeated pause/shutdown is
the only allowed transition and retries unfinished teardown; completed stages
are no-ops. Add tests for timeout→quarantine→successful retry, repeated retry,
and concurrent start/pause attempts while quarantined.

`resume` calls the same fresh start path as `start`; it never resets a boolean
while retaining old resources. `shutdown()` delegates to `pause_runtime()` and
logs errors. Keep `APP_HANDLE` available across temporary pause/resume; clear it
only during final application shutdown after runtime teardown.

Implement `TilingLifecycle` (holding `AppHandle`) as `LifecycleModule`:

```rust
fn start(&self) -> Result<(), String> { start_runtime(self.app_handle.clone()) }
fn pause(&self) -> Result<(), String> { pause_runtime() }
fn resume(&self) -> Result<(), String> { start_runtime(self.app_handle.clone()) }
fn status(&self) -> ModuleStatus { /* config gate, then LifecycleState */ }
```

- [ ] **Step 6: Verify two full cycles**

```bash
cargo test -p stache --lib modules::tiling::init::tests
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo test -p stache --lib modules::tiling::effects::subscriber::tests
cargo test -p stache --lib modules::tiling::events
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
```

On macOS with Accessibility enabled, manually start, pause, and resume twice.
Confirm one actor/subscriber/processor generation is active, stale callbacks do
nothing, Stache-owned hidden applications are restored before each pause,
windows are freshly enumerated, and borders/drag handling resume once.

- [ ] **Step 7: Commit**

```bash
git add app/native/src/modules/tiling/init.rs \
  app/native/src/lib.rs \
  app/native/src/modules/tiling/mod.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/actor/handle.rs \
  app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/events/app_monitor.rs \
  app/native/src/modules/tiling/events/screen_monitor.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/events/observer.rs \
  app/native/src/modules/tiling/events/mouse_monitor.rs \
  app/native/src/modules/tiling/events/drag_state.rs \
  app/native/src/modules/tiling/effects/subscriber.rs \
  app/native/src/modules/tiling/effects/animation/state.rs \
  app/native/src/modules/tiling/effects/window_cache.rs \
  app/native/src/modules/tiling/borders.rs \
  app/native/src/modules/tiling/tabs.rs
git commit -m "feat(tiling): support complete pause and fresh resume"
```

### Task 16: Create the lifecycle registry + wire into `app.manage()`

**Files:**

- Create: `app/native/src/modules/lifecycle_registry.rs`
- Modify: `app/native/src/lib.rs` (where modules are initialized)

- [ ] **Step 1: Create the registry**

```rust
use std::sync::Mutex;
use crate::modules::services::lifecycle::LifecycleModule;

/// Holds all toggleable modules so the tray can iterate them uniformly.
pub struct LifecycleRegistry {
    pub modules: Mutex<Vec<Box<dyn LifecycleModule>>>,
}

impl LifecycleRegistry {
    pub fn new() -> Self {
        Self { modules: Mutex::new(Vec::new()) }
    }

    pub fn register(&self, module: Box<dyn LifecycleModule>) {
        self.modules.lock().unwrap().push(module);
    }

    /// Toggle the module matching `id`. Returns the new status string for logging.
    pub fn toggle(&self, id: &str) -> Option<String> {
        let mut modules = self.modules.lock().unwrap();
        let module = modules.iter().find(|m| m.id() == id)?;
        let was_running = matches!(module.status(), crate::modules::services::lifecycle::ModuleStatus::Running);
        let result = if was_running { module.pause() } else { module.resume() };
        match result {
            Ok(()) => Some(if was_running { "paused" } else { "running" }.to_string()),
            Err(e) => Some(format!("error: {e}")),
        }
    }
}
```

- [ ] **Step 2: Register modules at startup**

In `lib.rs`, after each module's `init()`, register its `LifecycleModule` impl in the registry (created once and stored via `app.manage()`). Example pattern:

```rust
let registry = LifecycleRegistry::new();
registry.register(Box::new(WallpaperLifecycle::new()));
registry.register(Box::new(CmdQLifecycle::new(app_handle.clone())));
registry.register(Box::new(NoTunesLifecycle::new()));
registry.register(Box::new(ProxyAudioLifecycle::new()));
registry.register(Box::new(MenuAnywhereLifecycle::new(app_handle.clone())));
registry.register(Box::new(TilingLifecycle::new(app_handle.clone())));
app.manage(registry);
```

> The exact wrapper struct names depend on how each module implemented the trait in Tasks 10-15. Use the actual types. If a module implemented the trait directly on its existing struct (e.g. `WallpaperManager`), register `Box::new(manager)` instead of a wrapper.

- [ ] **Step 3: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/lifecycle_registry.rs app/native/src/lib.rs
git commit -m "feat(lifecycle): add registry and wire modules into app state"
```

---

## Phase 5 — Tray UI

### Task 17: Retain TrayIcon + build module CheckMenuItems

**Files:**

- Modify: `app/native/src/modules/tray/mod.rs`

- [ ] **Step 1: Store the TrayIcon handle and build CheckMenuItems**

Rewrite `init` (tray/mod.rs:27-66) to retain the `TrayIcon` and add a submenu of `CheckMenuItem`s built from the registry:

```rust
use tauri::menu::{CheckMenuItem, Menu, MenuItem, Submenu};
use crate::modules::lifecycle_registry::LifecycleRegistry;
use crate::modules::services::lifecycle::ModuleStatus;

pub fn init(app: &App) {
    let handle = app.handle();
    let registry = app.state::<LifecycleRegistry>();

    let quit_item = MenuItem::with_id(handle, QUIT_ID, "Quit Stache", true, None::<&str>)
        .expect("failed to create quit menu item");

    #[cfg(not(debug_assertions))]
    let reload_item = MenuItem::with_id(handle, RELOAD_ID, "Reload Stache", true, None::<&str>)
        .expect("failed to create reload menu item");

    // Build one CheckMenuItem per module from the registry
    let mut module_items: Vec<CheckMenuItem<tauri::Wry>> = Vec::new();
    for module in registry.modules.lock().unwrap().iter() {
        let status = module.status();
        let (checked, enabled) = match &status {
            ModuleStatus::Running => (true, true),
            ModuleStatus::Paused => (false, true),
            ModuleStatus::ConfiguredOff => (false, false),
            ModuleStatus::Unavailable(reason) => (false, false),
        };
        let label = match &status {
            ModuleStatus::Unavailable(reason) => format!("{} ({})", module.name(), reason),
            _ => module.name().to_string(),
        };
        let item = CheckMenuItem::with_id(handle, module.id(), label, enabled, checked, None::<&str>)
            .expect("failed to create module check item");
        module_items.push(item);
    }

    let modules_submenu = Submenu::with_items(handle, "Modules", true, &module_items.iter().collect::<Vec<_>>())
        .expect("failed to create modules submenu");

    #[cfg(not(debug_assertions))]
    let menu = Menu::with_items(handle, &[&reload_item, &modules_submenu, &quit_item])
        .expect("failed to create system tray menu");

    #[cfg(debug_assertions)]
    let menu = Menu::with_items(handle, &[&modules_submenu, &quit_item])
        .expect("failed to create system tray menu");

    let tray = TrayIconBuilder::new()
        .icon(handle.default_window_icon().expect("missing default window icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let registry = app.state::<LifecycleRegistry>();
            match event.id.as_ref() {
                #[cfg(not(debug_assertions))]
                RELOAD_ID => { app.restart(); }
                QUIT_ID => { app.exit(0); }
                id => {
                    if let Some(result) = registry.toggle(id) {
                        tracing::debug!(module = id, result, "tray: toggled module");
                    }
                }
            }
        })
        .build(handle)
        .expect("failed to build system tray icon");

    // Retain the handle so future code can update items (e.g. after a toggle)
    app.manage(tray);

    tracing::debug!("system tray initialized");
}
```

> The `CheckMenuItem` handles are owned by the `Menu`/`Submenu`. To reflect state immediately after a toggle (rather than on next menu open), retain the handles in a `TrayMenuState` struct managed via `app.manage()` and call `set_checked`/`set_enabled`/`set_text` inside `on_menu_event` after `registry.toggle(id)` succeeds. The minimal version above relies on macOS re-querying the menu on open, which is acceptable for v1. Add `TrayMenuState` only if immediate reflection is required.

- [ ] **Step 2: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p stache 2>&1 | tail -20`
Expected: compiles. Fix any `CheckMenuItem`/`Submenu`/`TrayIcon` API mismatches against Tauri 2.x (the `with_id` builder signature may differ slightly — check `tauri::menu::CheckMenuItemBuilder`).

- [ ] **Step 3: Commit**

```bash
git add app/native/src/modules/tray/mod.rs
git commit -m "feat(tray): add module toggle submenu with CheckMenuItems"
```

### Task 18: Manual Phase 5 verification

**Files:** none

- [ ] **Step 1: Run app in release (tray fully active)**

Run: `cd /Users/marcosmoura/Projects/stache && pnpm tauri:build && open target/release/stache.app`
(Debug build works too, but "Reload Stache" won't appear — see spec note.)

- [ ] **Step 2: Toggle each module**

Open the tray menu, expand "Modules", toggle each of the 6. Confirm:

- Configured-off modules are grayed/locked (cannot toggle).
- Running → Paused shows the checkmark clear and the OS resource actually stops (e.g. `commandQuit` event tap disabled — verify via `Activity Monitor` or by testing Cmd+Q behavior).
- Paused → Running re-acquires the resource.
- Unavailable modules show a reason in the label and are locked.

- [ ] **Step 3: Restart and confirm reset**

Quit and relaunch. All modules return to their config-driven default state (toggles reset).

- [ ] **Step 4: Report** — verification only. No commit.

---

## Phase 6 — Restore Stache-Hidden Applications on Shutdown

> **Commits `f2e20c8..95fa2f5` are superseded** by Tasks 19A-19G below. Those
> commits implemented the generation/FIFO/PID-set approach that repeatedly
> failed (Manual Task21 showed attempted=0/restored=0 despite apps visibly
> hidden). The root cause was that `AppShown` callbacks and actor workspace
> operations mutated PID ownership in different ordering domains. The
> architecture below replaces them with `AppIdentity`-keyed ownership through
> an actor-single-writer `VisibilityRegistry`, exact-instance revalidation,
> and conservative unavoidable-history policy. The detailed `HideAppOutcome`/
> `UnhideAppOutcome` types from those commits are kept — they remain useful
> for the hide/unhide workspace operations.

### Task 19A: `AppIdentity` type with capture/validation seam

**Files:**

- Create: `app/native/src/modules/tiling/identity.rs`
- Modify: `app/native/src/modules/tiling/mod.rs` (add `pub mod identity;`)

- [ ] **Step 1: Write identity capture test that fails to compile**

Add `pub mod identity;` to `tiling/mod.rs`. Write the test module in
`identity.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_from_ns_running_app_valid() {
        // No ObjC runtime in test — verify the fail-closed path:
        // a null pointer must return None.
        let identity = unsafe { AppIdentity::from_ns_running_app(std::ptr::null_mut()) };
        assert!(identity.is_none());
    }

    #[test]
    fn identity_ord_deterministic() {
        let a = AppIdentity { pid: 10, launch_date: LaunchDateBits(1000) };
        let b = AppIdentity { pid: 10, launch_date: LaunchDateBits(2000) };
        let c = AppIdentity { pid: 20, launch_date: LaunchDateBits(1000) };
        assert!(a < b);
        assert!(a < c);
        assert!(b < c);
    }

    #[test]
    fn identity_equality_pid_and_launch_date() {
        let a = AppIdentity { pid: 10, launch_date: LaunchDateBits(42) };
        let b = AppIdentity { pid: 10, launch_date: LaunchDateBits(42) };
        let c = AppIdentity { pid: 10, launch_date: LaunchDateBits(43) };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn window_target_distinguishes_reused_numeric_window_id() {
        let a = AppIdentity { pid: 10, launch_date: LaunchDateBits(42) };
        let b = AppIdentity { pid: 10, launch_date: LaunchDateBits(43) };
        assert_ne!(
            WindowTarget { identity: a, window_id: 99 },
            WindowTarget { identity: b, window_id: 99 },
        );
    }

    #[test]
    fn launch_date_bits_rejects_non_positive() {
        // 0.0.to_bits() must be rejectable (fail-closed).
        // This tests the validation predicate, not the ObjC call.
        let bits = LaunchDateBits::from_time_interval_since_reference_date(0.0);
        assert!(bits.is_none(), "zero interval must fail");
        let bits = LaunchDateBits::from_time_interval_since_reference_date(-1.0);
        assert!(bits.is_none(), "negative interval must fail");
        let bits = LaunchDateBits::from_time_interval_since_reference_date(f64::NAN);
        assert!(bits.is_none(), "NaN must fail");
        let bits = LaunchDateBits::from_time_interval_since_reference_date(f64::INFINITY);
        assert!(bits.is_none(), "infinity must fail");
    }
}
```

- [ ] **Step 2: Run tests, verify RED**

```bash
cargo test -p stache --lib modules::tiling::identity::tests
```

Expected: compilation fails — `AppIdentity`, `LaunchDateBits` do not exist.

- [ ] **Step 3: Implement `AppIdentity` and `LaunchDateBits`**

In `identity.rs`, add:

```rust
use objc::{msg_send, sel, sel_impl};
use serde::{Deserialize, Serialize};

/// Stable application identity combining PID and launch date.
///
/// Prevents PID-reuse races: after an app terminates the kernel may reuse
/// its PID for a different process. Binding ownership to `(pid, launch_date)`
/// ensures we never restore a wrong process that inherited the same PID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AppIdentity {
    pub pid: i32,
    pub launch_date: LaunchDateBits,
}

/// Exact target for every delayed, cached, or external window operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WindowTarget {
    pub identity: AppIdentity,
    pub window_id: u32,
}

/// High-precision launch-date bits from
/// `NSRunningApplication.launchDate.timeIntervalSinceReferenceDate`.
///
/// Stored as `f64::to_bits` for `Send + Sync + Copy`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct LaunchDateBits(u64);

impl LaunchDateBits {
    /// Creates bits from a `timeIntervalSinceReferenceDate` value.
    /// Returns `None` if `t` is not finite and >0 (fail-closed on
    /// missing/null launch date).
    #[must_use]
    pub const fn from_time_interval_since_reference_date(t: f64) -> Option<Self> {
        if t.is_finite() && t > 0.0 {
            Some(Self(t.to_bits()))
        } else {
            None
        }
    }

    /// Returns the stored launch-date bit pattern for structured diagnostics.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }
}

impl AppIdentity {
    /// Captures identity from an `NSRunningApplication` ObjC object.
    ///
    /// Returns `None` if pid ≤ 0, launchDate is null, or the time interval
    /// is not finite and positive. This is fail-closed: callers must skip
    /// the app rather than proceeding with an invalid identity.
    ///
    /// # Safety
    ///
    /// `app` must be a valid non-null `*mut Object` pointing to an
    /// `NSRunningApplication` instance for the duration of this call.
    #[must_use]
    pub unsafe fn from_ns_running_app(app: *mut objc::runtime::Object) -> Option<Self> {
        // SAFETY: caller guarantees app is valid for the call duration.
        // We check null before any msg_send.
        unsafe {
            if app.is_null() {
                return None;
            }

            let pid: i32 = msg_send![app, processIdentifier];
            if pid <= 0 {
                return None;
            }

            let launch_date: *mut objc::runtime::Object = msg_send![app, launchDate];
            if launch_date.is_null() {
                return None;
            }

            let interval: f64 = msg_send![launch_date, timeIntervalSinceReferenceDate];
            let bits = LaunchDateBits::from_time_interval_since_reference_date(interval)?;

            Some(Self { pid, launch_date: bits })
        }
    }
}
```

- [ ] **Step 4: Run tests, verify GREEN**

```bash
cargo test -p stache --lib modules::tiling::identity::tests
cargo fmt --all -- --check
cargo check -p stache
```

Expected: all pass. `AppIdentity::from_ns_running_app(std::ptr::null_mut())` returns
`None` safely — the implementation checks `app.is_null()` before any `msg_send!`. The
null-pointer test validates the fail-closed safety path without an ObjC runtime.

- [ ] **Step 5: Commit**

```bash
git add app/native/src/modules/tiling/identity.rs app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): add AppIdentity with capture/validation"
```

---

### Task 19B: Propagate `AppIdentity` into Window, events, and StateMessage

**Files:**

- Modify: `app/native/src/modules/tiling/state/types.rs:308` — add `pub identity: Option<AppIdentity>` to `Window`
- Modify: `app/native/src/modules/tiling/actor/messages.rs:271` — add `pub identity: Option<AppIdentity>` to `WindowCreatedInfo`
- Modify: `app/native/src/modules/tiling/actor/messages.rs:54-68` — add identity to `AppLaunched`, `AppTerminated`, `AppHidden`, `AppShown`
- Modify: `app/native/src/modules/tiling/events/types.rs:139` — add `pub identity: AppIdentity` to `WindowEvent`
- Modify: `app/native/src/modules/tiling/events/observer.rs` — `ObserverState` keyed by observer address; `ObserverRecord` stores identity; callback copies identity; add `remove_observer_for_identity` for exact termination
- Modify: `app/native/src/modules/tiling/events/ax_observer.rs` — capture identity at observer registration; callback copies stored identity
- Modify: `app/native/src/modules/tiling/events/app_monitor.rs` — capture identity from `NSWorkspaceApplicationKey` object, not fresh PID lookup
- Modify: `app/native/src/modules/tiling/events/processor.rs` — exact event routing/batching and target-aware screen map
- Modify: `app/native/src/modules/tiling/events/drag_state.rs` — retain exact targets throughout mouse-down/up drag state
- Modify: `app/native/src/modules/tiling/tabs.rs` — key tab ownership/scans by exact `AppIdentity`, never PID
- Modify: `app/native/src/modules/tiling/actor/handlers/window.rs` — upgrade transitional identity and handle reused window IDs
- Modify: `app/native/src/modules/tiling/actor/handlers/app.rs` — exact application-cache invalidation after cache API migration
- Modify: `app/native/src/modules/tiling/actor/handle.rs` — exact
  `set_expected_frames(Vec<(WindowTarget, Rect)>)` and completion-compatible
  handle construction
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — exact actor messages, target queries, and mismatch rejection
- Modify: `app/native/src/modules/tiling/rules/mod.rs` — update complete `Window`
  literals with transitional identity
- Modify: `app/native/src/modules/tiling/state/tiling_state.rs` — update complete
  `Window` literals with transitional identity
- Modify: `app/native/src/modules/tiling/init.rs` — exact initial scan/focus/effect targets
- Modify: `app/native/src/modules/tiling/effects/mod.rs` — replace every target-bearing numeric ID with `WindowTarget`; exact layout/focus descriptors
- Modify: `app/native/src/modules/tiling/effects/subscriber.rs` — store/query exact targets before producing effects
- Modify: `app/native/src/modules/tiling/effects/executor.rs` — batch and execute exact targets only
- Modify: `app/native/src/modules/tiling/effects/window_cache.rs` — exact window/application cache keys and validation
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — exact-target window operations; no bare-ID fallback
- Modify: `app/native/src/modules/tiling/effects/animation/mod.rs` — exact-target animation execution
- Modify: `app/native/src/modules/tiling/effects/animation/state.rs` — exact-target interrupted-position keys
- Modify: `app/native/src/modules/tiling/effects/animation/transition.rs` — `WindowTransition` owns `WindowTarget`
- Modify: `app/native/src/modules/tiling/actor/handlers/preset.rs` — construct exact targets for delayed preset operations
- Modify: `app/native/src/modules/tiling/actor/handlers/focus.rs` and `workspace.rs` — pass exact targets to immediate focus operations
- Modify: `app/native/src/modules/tiling/actor/handlers/window_move.rs` — send exact targets in floating-state notifications
- Modify: `app/native/src/modules/bar/components/tiling.rs` — resolve stored `Window.identity`, construct `WindowTarget`, and focus exactly
- All call sites that construct these types

- [ ] **Step 1: Write identity-propagation tests that fail**

Add to `tests` in a suitable module (e.g. `events/types.rs` or a new integration
module). The key invariants:

```rust
// In events/types.rs tests
#[test]
fn window_event_carries_identity() {
    let ident = AppIdentity {
        pid: 42,
        launch_date: LaunchDateBits::from_time_interval_since_reference_date(100.0).unwrap(),
    };
    let event = WindowEvent::new(
        WindowEventType::Created,
        42,
        0,
        ident, // ~ compiler error until WindowEvent::new accepts AppIdentity
    );
    // Post-impl assertion:
    // assert_eq!(event.identity, ident);
    let _ = ident;
}

#[test]
fn state_message_app_shown_carries_identity() {
    let ident = AppIdentity {
        pid: 42,
        launch_date: LaunchDateBits::from_time_interval_since_reference_date(100.0).unwrap(),
    };
    // Does not compile until StateMessage::AppShown carries identity:
    // let msg = StateMessage::AppShown { identity: ident, pid: 42 };
    // assert_eq!(msg.identity(), ident);
    let _ = ident;
}
```

- [ ] **Step 2: Run, verify RED**

```bash
cargo test -p stache --lib modules::tiling::events::types::tests
```

Expected: compilation fails — `identity` field missing from `WindowEvent`.

- [ ] **Step 3: Add identity field to each type**

**`Window`** (`state/types.rs:308`) — add optional identity. Managed windows
(those that have gone through at least one actor-owned workspace action) MUST
have `Some` identity. `None` is valid only for transitional windows (initial
window-scan path before the first capture) and default/test constructors.
The actor never calls `hide`/`unhide`/`match`/`owns`/`terminate`/`restore`
against a window whose identity is `None`:

```rust
pub struct Window {
    pub id: u32,
    pub pid: i32,
    pub identity: Option<AppIdentity>,  // NEW — Some after first capture
    pub app_id: String,
    // Existing fields remain unchanged; only add `identity` above.
}
```

**`WindowCreatedInfo`** (`actor/messages.rs:271`) — add optional identity
(same `None`-only-transitional convention as `Window`):

```rust
pub struct WindowCreatedInfo {
    pub window_id: u32,
    pub pid: i32,
    pub identity: Option<AppIdentity>, // NEW
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub frame: Rect,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
    pub minimum_size: Option<(f64, f64)>,
    pub tab_group_id: Option<uuid::Uuid>,
    pub is_active_tab: bool,
}
```

**`WindowEvent`** (`events/types.rs:139`):

```rust
pub struct WindowEvent {
    pub event_type: WindowEventType,
    pub pid: i32,
    pub identity: AppIdentity,   // NEW
    pub element: usize,
}
```

Update `WindowEvent::new` to require `AppIdentity`.

**`StateMessage` lifecycle variants** (`actor/messages.rs:54-68`):

```rust
AppLaunched { identity: AppIdentity, pid: i32, bundle_id: String, name: String },
AppTerminated { identity: AppIdentity, pid: i32 },
AppHidden { identity: AppIdentity, pid: i32 },
AppShown { identity: AppIdentity, pid: i32 },
```

Keep the `pid` field for backward compat with handler internals; the actor uses
`identity` for ownership.

**Every AX-derived ID-targeted window message also carries exact identity.**
Update `StateMessage::{WindowDestroyed, WindowFocused, WindowUnfocused,
WindowMoved, WindowResized, WindowMinimized, WindowTitleChanged,
WindowFullscreenChanged}` to add `identity: AppIdentity`. Add identity to
`GeometryUpdate`, and key geometry batches by `(AppIdentity, window_id)` so a
late A update can never merge into B's update after window-ID reuse. The AX
adapter passes `WindowEvent.identity` through every EventProcessor method.
EventProcessor's destroy-detection cache changes from PID→window IDs to
`AppIdentity`→window IDs; its AX fallback becomes
`on_window_destroyed_for_identity(identity)` and never scans/reports by bare
PID.

Internal delayed messages must be exact too. Change
`SetExpectedFrames { frames }` and `StateActorHandle::set_expected_frames` from
`Vec<(u32, Rect)>` to `Vec<(WindowTarget, Rect)>`. The actor updates
`expected_frame` only when the stored window identity equals the target
identity. Add a regression where stale A expected frames arrive after same-ID
B creation and assert B's frame/expected frame remain unchanged; a matching B
target still updates it.

Propagate exact targets through user drag state and mouse-up completion:

- `WindowSnapshot` stores `target: WindowTarget` instead of `window_id`.
- `DragInfo` stores the initiating exact target/identity, not a bare PID.
- `get_current_frames_for_snapshots` resolves each snapshot through exact
  `window_ops::get_window_frame(target)` and returns target/frame pairs.
- `SwapWindows` carries `target_a`/`target_b`; `UserResizeCompleted` carries a
  `target`; actor handlers validate each stored identity immediately before
  swapping, ratio updates, or layout refresh. `UserMoveCompleted` remains
  workspace-scoped only when no target-specific mutation follows.
- Mouse-up skips missing/mismatched identities and never falls back to a
  numeric window ID.

Add deterministic stale-drag tests: capture drag state for A, replace it with B
at the same numeric ID, then finish move and resize. Neither `SwapWindows` nor
`UserResizeCompleted` may mutate B. A matching B drag still completes normally.

The EventProcessor routing map must use the same exact key. Replace
`window_screen_map: DashMap<u32, u32>` with
`DashMap<(AppIdentity, u32), u32>` and update every API/callsite atomically:

```rust
pub fn set_window_screen(&self, identity: AppIdentity, window_id: u32, screen_id: u32);
pub fn remove_window(&self, identity: AppIdentity, window_id: u32);
fn get_window_screen(&self, identity: AppIdentity, window_id: u32) -> u32;
```

Creation/move/resize routing passes the event identity. Destruction removes
only `(identity, window_id)`, never every route sharing the numeric window ID.
Add a deterministic same-ID A→B test that registers both routes, queues A and
B geometry, then processes delayed A destruction. Assert B's route and pending
geometry remain while A's exact route/update are removed.

At actor ingress, reject every ID-targeted event unless the stored window has
`Some(message.identity)`. Apply the same check independently to every batched
`GeometryUpdate`. A delayed destroy/move/resize/focus/title/minimize/fullscreen
event from A therefore cannot remove or mutate replacement B. For the initial
focus event in `init.rs`, capture the focused window's identity from
`window_infos` before moving the vector into `BatchWindowsCreated`; send
`WindowFocused` only with `Some(identity)`, otherwise fail closed and log.

`WindowDestroyed` has one exact special case before the tracked-window check:
tab windows are intentionally absent from `TilingState`, so call
`tabs::is_tab_for_identity(window_id, identity)`. If true, call
`tabs::unregister_tab_for_identity(window_id, identity)` and return without a
layout mutation. If false, continue to the normal tracked-window exact-identity
guard. Never call bare `is_tab(window_id)` or `unregister_tab(window_id)` from
actor ingress: a stale A tab destruction must not remove same-ID tab B.

Add tests that create same-ID A then B and send delayed A destroy, move,
resize, and one batched geometry update. Assert B and its frame survive. Also
test a matching B destroy still runs normal cleanup.

**Upgrade existing-window identity safely** in
`actor/handlers/window.rs::on_window_created_internal`. Before the current
"already tracked, updating" branch, compare the existing and incoming optional
identities:

```rust
let mut affected_workspaces = Vec::new();
let existing_identity = state.get_window(info.window_id).and_then(|w| w.identity);
match (existing_identity, info.identity) {
    (Some(existing), Some(incoming)) if existing != incoming => {
        // Window ID was reused by another application instance. Remove every
        // stale state/workspace/focus/cache reference before creating the new
        // instance; do not overwrite identity in place.
        if let Some(workspace_id) = on_window_destroyed(state, info.window_id) {
            affected_workspaces.push(workspace_id);
        }
        // Fall through to normal new-window creation below and add its
        // workspace to affected_workspaces as well.
    }
    (_, incoming) if state.get_window(info.window_id).is_some() => {
        state.update_window(info.window_id, |w| {
            if w.identity.is_none() {
                w.identity = incoming;
            }
            // Preserve an existing Some identity when incoming is None or equal.
            w.title.clone_from(&info.title);
            w.frame = info.frame;
            w.is_minimized = info.is_minimized;
            w.is_fullscreen = info.is_fullscreen;
        });
        return affected_workspaces;
    }
    _ => {}
}
```

Add focused tests for both transitions:

1. Existing `Window { identity: None }` plus `WindowCreatedInfo { identity:
Some(A) }` upgrades to `Some(A)` without duplicating the window.
2. Existing `Some(A)` plus incoming `Some(B)` at the same window ID removes
   A's stale workspace/focus/cache references and creates a new B window;
   identity A is never silently retained or overwritten in place. Change
   `on_window_created_internal` to return a de-duplicated `Vec<Uuid>` of every
   affected workspace rather than one `Option<Uuid>`; `on_window_created`
   sends the existing layout notification for each returned workspace, while
   the silent batch caller intentionally ignores the vector. Assert both A's
   old workspace and B's assigned workspace are returned/notified when they
   differ.

After the existing normal new-window insertion assigns `workspace_id`, append
that workspace and de-duplicate before returning:

```rust
affected_workspaces.push(workspace_id);
affected_workspaces.sort_unstable();
affected_workspaces.dedup();
affected_workspaces
```

Every non-silent caller iterates this returned vector and emits the existing
layout notification once per workspace. This makes the stale A workspace and
replacement B workspace observable without adding a new notification type.

**Propagate `WindowTarget` through every effect/cache/delayed-operation path in
this same atomic task.** `Window.identity` is optional only at transitional
ingress; no effect may be created for `None`. Add actor query variants that
produce exact targets in one actor turn:

```rust
StateQuery::GetWindowLayoutTargets { workspace_id: Uuid }
StateQuery::GetFocusTargets

QueryResult::TargetLayout(Vec<(WindowTarget, Rect)>)
QueryResult::TargetFocus {
    focused_window: Option<WindowTarget>,
    focused_workspace_id: Option<Uuid>,
}
```

`GetWindowLayoutTargets` maps each computed `(window_id, frame)` to the stored
`Window.identity`; entries with `None` or a missing window are omitted and
logged at trace level. `GetFocusTargets` resolves the focused ID and identity
from the same actor state snapshot; a missing identity yields `None`, never a
bare-ID fallback. Existing non-effect queries remain unchanged for frontend and
CLI consumers.

In `effects/mod.rs`, make every OS-targeting value exact:

```rust
SetWindowFrame { target: WindowTarget, frame: Rect, animate: bool }
SetWindowVisible { target: WindowTarget, visible: bool }
FocusWindow { target: WindowTarget }
RaiseWindow { target: WindowTarget }
RefreshActiveBorder {
    target: WindowTarget,
    layout: LayoutType,
    is_window_floating: bool,
}
HideBorders { targets: Vec<WindowTarget> }
ShowBorders { targets: Vec<WindowTarget> }

LayoutChange {
    old_positions: Vec<(WindowTarget, Rect)>,
    new_positions: Vec<(WindowTarget, Rect)>,
    // existing workspace/user fields unchanged
}
FocusChange {
    old_window: Option<WindowTarget>,
    new_window: Option<WindowTarget>,
    // existing workspace fields unchanged
}
```

`LayoutChange::added_windows`/`removed_windows`, executor layout maps, subscriber
`layout_positions`, and subscriber `floating_windows` use `WindowTarget` keys.
The subscriber consumes only `TargetLayout`/`TargetFocus`; it must not combine a
bare layout result with a separate identity lookup. All executor batch vectors
carry `WindowTarget`. Before border, focus, raise, visibility, SkyLight, or AX
work, the executor resolves/validates the exact target; on mismatch it logs and
skips the effect.

This is an atomic replacement, not an additive variant. Remove the existing
`TilingEffect::UpdateBorder` definition, every `UpdateBorder` constructor, every
executor match/count-only branch, every `effects_from_focus_change` use, and
their old tests in the staged effects files. Replace them with
`RefreshActiveBorder` and its exact-target tests in the same Task19B commit.
After Task19B, a repository search for `UpdateBorder` and
`effects_from_focus_change` must return no production matches.

Move active-border application fully behind that executor boundary. The
subscriber must no longer call `borders::on_focus_changed` directly. From one
actor snapshot it obtains the focused `WindowTarget`, workspace `LayoutType`,
and floating flag, then emits `RefreshActiveBorder` carrying all three. The
executor resolves and validates that exact target immediately before invoking
the existing `borders::on_focus_changed(layout, is_window_floating)` helper;
the target is the authorization boundary even though the global helper itself
accepts only layout/floating semantics. Do not collapse those semantics into
`BorderState`. `HideBorders`/`ShowBorders` also carry exact targets and are
counted/applied only after validation, preserving their current behavior.
Border effect accounting counts only validated/applied targets, not merely
received effects. Add an injected executor test where stale target A and
current target B share a numeric window ID: A produces no global border call,
while B calls the helper exactly once with its supplied layout/floating values.
No `borders.rs` API change is required.

Change `SubscriberNotification::FloatingChanged` and
`notify_floating_changed` to carry `WindowTarget`. In
`actor/handlers/window_move.rs`, after toggling floating state, re-read the
matching stored window, require `Some(identity)`, construct
`WindowTarget { identity, window_id }`, and notify. If the window or identity is
missing, log and skip the notification; never send a numeric-only floating
update.

Refactor `WindowElementCache` to exact keys:

```rust
windows: DashMap<WindowTarget, CachedWindowElement>
apps: DashMap<AppIdentity, CachedAppElement>
```

Every API accepts `WindowTarget` or `AppIdentity`: `resolve`,
`resolve_and_cache`, `batch_resolve`, `invalidate_window`, `invalidate_app`,
`find_invalid_windows`, `get_window_frame`, and `set_window_frame_fast`.
Resolution obtains one local `NSRunningApplication` for `target.identity.pid`,
captures `AppIdentity` from that same object, requires exact equality, then
enumerates only that app's AX windows for `target.window_id`. A cache hit is
valid only when both the AX window number and exact application identity still
match. Remove the global "first matching numeric window ID" fallback and the
PID-only application cache path.

Change all ID-targeted `window_ops` APIs used by tiling to accept
`WindowTarget` (`get_window_frame`, `set_window_frame`,
`set_window_frame_fast`, `focus_window`, `raise_window`, and batch frame
updates). Delayed main-thread closures capture the full target and revalidate
identity immediately before acting. The preset, focus, workspace, and init
callers construct a target only from a stored `Window` with `Some(identity)`;
otherwise they fail closed. Numeric IDs may be passed to SkyLight/border code
only after exact validation in that same synchronous operation.

Audit and convert/remove every internal numeric-only helper too:

- `resolve_window_element(WindowTarget)` resolves only within the exact app.
- `set_window_frame_verified(WindowTarget, ...)` revalidates before writing.
- `get_window_minimum_size(WindowTarget)` uses the exact resolved element.
- Delete `get_window_pid(window_id)` and every global PID-discovery fallback;
  the PID is `target.identity.pid` after exact revalidation.
- Any private helper that can reach AX, cache, SkyLight, focus, raise, move, or
  resize must accept `WindowTarget` or an already-validated local element whose
  lifetime cannot escape the synchronous call.

Update their existing tests to use exact targets. Add a source-level regression
test or focused API audit asserting no tiling `window_ops` resolver accepts only
`u32`; numeric IDs are allowed only in low-level SkyLight/border calls invoked
after exact validation.

`WindowTransition` stores `target: WindowTarget`; animation interrupted
positions use `DashMap<WindowTarget, Rect>`. Animation creation/execution and
`actor/handlers/preset.rs` never retain a bare numeric target.

Add deterministic tests for:

1. Same numeric ID under identities A/B creates distinct effects, subscriber
   state, cache entries, transitions, and interrupted positions.
2. A delayed A effect/cache invalidation cannot resolve, remove, move, focus,
   raise, animate, or alter borders for B.
3. Exact cache resolution uses one injected app value through identity and
   window lookup; mismatched identity fails before action.
4. Target-layout/focus actor queries omit transitional `None` identities.
5. Existing matching targets preserve current layout/focus/border behavior.

- [ ] **Step 4: Capture identity in AX observer registration**

The current `observer.rs` (type `parking_lot::Mutex<Option<ObserverState>>`,
currently storing `ObserverRef` values, with
(`*mut c_void` wrapping `AXObserverRef`), `add_observer_for_pid` at line ~207,
callback `observer_callback` at line ~329, and the old PID-oriented setup path
around line ~291). Refactor it so the primary key is the `AXObserverRef`
address (the observer pointer cast to `usize`), never PID. This prevents a
delayed termination of A from removing B after PID reuse.

**Add identity to ObserverState, keyed by observer address.** Replace
`ObserverRef` values with `ObserverRecord`, and add an exact reverse index:

```rust
use crate::modules::tiling::identity::AppIdentity;
use objc::{class, msg_send, sel, sel_impl};

struct ObserverRecord {
    observer: ObserverRef,
    identity: AppIdentity,
}

struct ObserverState {
    /// Primary index: AXObserverRef address → ObserverRecord.
    observers: HashMap<usize, ObserverRecord>,
    /// Exact reverse index: application identity → AXObserverRef address.
    identity_to_observer: HashMap<AppIdentity, usize>,
}
```

In `add_observer_for_pid`, replace the current function-wide `state_guard` with
two short critical sections. First capture `AppIdentity` from the same
`NSRunningApplication` object, lock only long enough to return when
`identity_to_observer` already contains that exact identity, and drop the guard.
Then call `AXObserverCreate`, `AXUIElementCreateApplication`, and register the
notifications without holding `OBSERVER_STATE`. Immediately before publishing
the record or adding its source to the run loop, resolve a fresh
`NSRunningApplication` for the PID and capture `current_identity` from that
same object. Require `current_identity == identity`. If the process exited or
the PID was reused during observer setup, release the newly created observer
and abort without publishing it. Only after that revalidation may setup
reacquire the mutex, repeat the identity/address duplicate check, and insert
both indices. Drop that guard before `CFRunLoopAddSource`. This guarantees
callbacks cannot run before the address-keyed record exists, a losing duplicate
is never added to the run loop, and an observer created for replacement process
B can never be indexed under terminated process A's identity.
There is no non-reentrant re-lock and no run-loop-retained duplicate source:

```rust
// Capture identity, then perform the first short duplicate check.
let identity = {
    let app = unsafe { msg_send![
        class!(NSRunningApplication),
        runningApplicationWithProcessIdentifier: pid
    ]};
    if app.is_null() {
        return Err(format!("no NSRunningApplication for pid {pid}"));
    }
    match unsafe { AppIdentity::from_ns_running_app(app) } {
        Some(id) => id,
        None => return Err(format!("no valid identity for pid {pid}")),
    }
};
{
    let state_guard = OBSERVER_STATE.lock();
    let state = state_guard.as_ref()
        .ok_or_else(|| "Observer state not initialized".to_string())?;
    if state.identity_to_observer.contains_key(&identity) {
        return Ok(());
    }
}

// Create the observer/application element and add AX notifications here.
// Do not hold OBSERVER_STATE and do not add the run-loop source yet.
let source = unsafe { AXObserverGetRunLoopSource(observer) };
if source.is_null() {
    unsafe { CFRelease(observer.cast()) };
    return Err(format!("observer has no run-loop source: pid {pid}"));
}

// Re-resolve immediately before publication to close the PID-reuse window.
let current_identity = {
    let app = unsafe { msg_send![
        class!(NSRunningApplication),
        runningApplicationWithProcessIdentifier: pid
    ]};
    if app.is_null() {
        unsafe { CFRelease(observer.cast()) };
        return Err(format!("application exited during observer setup: pid {pid}"));
    }
    unsafe { AppIdentity::from_ns_running_app(app) }
};
if current_identity != Some(identity) {
    unsafe { CFRelease(observer.cast()) };
    return Err(format!("application identity changed during observer setup: pid {pid}"));
}

let record = ObserverRecord { observer: ObserverRef(observer), identity };
let mut state_guard = OBSERVER_STATE.lock();
let Some(state) = state_guard.as_mut() else {
    drop(state_guard);
    unsafe { CFRelease(observer.cast()) };
    return Err("Observer state not initialized".to_string());
};
let address = observer as usize;
if state.observers.contains_key(&address)
    || state.identity_to_observer.contains_key(&identity)
{
    drop(state_guard);
    unsafe { CFRelease(observer.cast()) };
    return Ok(());
}
state.observers.insert(address, record);
state.identity_to_observer.insert(identity, address);
drop(state_guard);

// Setup and exact removal are both main-thread-only. There is no await/yield
// between publication and this void-returning call, so removal cannot
// interleave with installation on the same run loop.
unsafe {
    let run_loop = CFRunLoop::get_main();
    let mode = core_foundation::runloop::kCFRunLoopDefaultMode;
    CFRunLoopAddSource(
        run_loop.as_concrete_TypeRef().cast(),
        source,
        mode.cast(),
    );
}
```

Assert main-thread execution at the beginning of both setup and removal. A
null source is rejected before either index is published. Every failure before
publication releases the new observer. Once published, the source is added
immediately on the same main-thread turn; `CFRunLoopAddSource` returns `void`,
so there is no fallible post-publication branch to roll back.

**Callback derives identity from the observer address** (refcon continues to
carry the PID for logging, but is not a lookup key):

```rust
unsafe extern "C" fn observer_callback(
    _observer: AXObserverRef,
    element: AXUIElementRef,
    notification: *const c_void,
    refcon: *mut c_void,
) {
    let pid = refcon as i32;
    let identity = {
        let state_guard = OBSERVER_STATE.lock();
        let Some(ref state) = *state_guard else { return; };
        let Some(record) = state.observers.get(&(_observer as usize)) else { return; };
        record.identity  // Copy (AppIdentity is Copy)
    };
    // ... construct WindowEvent::new(event_type, pid, element as usize, identity)
}
```

This avoids holding the lock for more than a short lookup; the identity value
is copied. Add a pure map-removal seam that never calls Core Foundation, then
let the production wrapper release the returned record:

```rust
fn take_observer_record_for_identity(
    state: &mut ObserverState,
    identity: &AppIdentity,
) -> Option<ObserverRecord> {
    let address = state.identity_to_observer.remove(identity)?;
    state.observers.remove(&address)
}

pub fn remove_observer_for_identity(identity: &AppIdentity) {
    let record = {
        let mut state_guard = OBSERVER_STATE.lock();
        state_guard
            .as_mut()
            .and_then(|state| take_observer_record_for_identity(state, identity))
    };
    let Some(record) = record else { return; };

    let address = record.observer.0 as usize;
    unsafe {
        let source = AXObserverGetRunLoopSource(record.observer.0);
        if !source.is_null() {
            let run_loop = CFRunLoop::get_main();
            let mode = core_foundation::runloop::kCFRunLoopDefaultMode;
            CFRunLoopRemoveSource(
                run_loop.as_concrete_TypeRef().cast(),
                source,
                mode.cast(),
            );
        }
        CFRelease(record.observer.0.cast());
    }
    tracing::trace!(address, "ax_observer: removed observer for identity");
}
```

Declare/import `CFRunLoopRemoveSource` beside the existing Core Foundation FFI
and use the already imported `CFRunLoop`, `TCFType`, and default mode. Removal
must run on the main thread (the exact NSWorkspace termination ingress already
does), must drop `OBSERVER_STATE` before touching the run loop, must remove the
source from the main run loop/default mode, and only then `CFRelease` the
observer. The pure `take_observer_record_for_identity` test seam never calls
either run-loop or release APIs.

Delete or stop exporting `remove_observer_for_pid`; there must be no
production removal path that scans by PID. Termination MUST call
`remove_observer_for_identity` with the identity captured at main-thread
ingress. Exact removal first removes the reverse index, then the exact
address-keyed record, and never removes a record merely because its PID
matches.

For `ax_observer.rs::handle_window_created`, the `WindowEvent` is built by the
callback and already carries the identity before reaching the handler.

**Locking:** `OBSERVER_STATE` uses `parking_lot::Mutex` (use `.lock()`, never
`.unwrap()`). Setup drops the guard before all OS setup and before any second
lock. The callback holds it only for one address-keyed `HashMap::get`. There is
no registry lock (`VisibilityRegistry` mutex) involved here.

**Important thread note:** The AX callback runs on the main thread's CFRunLoop.
The `OBSERVER_STATE` mutex is held for the duration of a single address-keyed
`HashMap::get` (microseconds). Setup never re-locks while holding the guard.
The callback copies the stored identity and does not retain the lock while
constructing or forwarding the event.

Add an address-aware unit test with observer records A and B sharing a PID but
having different `AppIdentity` launch dates. Use fabricated non-dereferenced
observer addresses and call only `take_observer_record_for_identity` (never the
production CFRelease wrapper). Remove A by its captured identity and assert B's
address-keyed record and reverse-index entry remain. Also test that duplicate
checks distinguish identity/address pairs and never use PID as the primary key.

Update initialization to construct both empty maps:
`ObserverState { observers: HashMap::new(), identity_to_observer: HashMap::new() }`.

**Make the tab registry exact-instance aware in the same atomic task.** Tab
windows are intentionally excluded from `TilingState`, so they cannot inherit
identity later from tracked windows. In `tabs.rs`:

- Replace `window_to_pid: HashMap<u32, i32>` with
  `window_to_identity: HashMap<u32, AppIdentity>`.
- Change `register_tab`, `tabs_for_*`, `clear_for_*`, and `replace_tabs_for_*`
  to accept/use exact `AppIdentity`. Public APIs become
  `register_tab(window_id, identity)`, `tabs_for_identity(identity)`,
  `is_tab_for_identity(window_id, identity)`,
  `unregister_tab_for_identity(window_id, identity)`,
  `clear_tabs_for_identity(identity)`, and
  `replace_tabs_for_identity(identity, window_ids)`. Remove PID-wide mutation
  APIs from production use.
- Change `scan_and_register_tabs_for_app` and `is_new_window_a_tab` to accept an
  `AppIdentity`. Resolve the current `NSRunningApplication` from
  `identity.pid`, require the captured identity to match before scanning, and
  re-resolve/revalidate the identity immediately before publishing the scan via
  `replace_tabs_for_identity`. If either check fails, publish nothing.
- During new-window handling, call tab scanning/classification only when
  `WindowCreatedInfo.identity` is `Some(identity)`; `None` is fail-closed and
  cannot create PID-owned tab records.
- Compare siblings through `window_to_identity`, never PID.

Use a deterministic publish seam rather than testing live AX state:

```rust
fn publish_tabs_if_identity_matches(
    registry: &mut TabRegistry,
    expected: AppIdentity,
    observed_after_scan: Option<AppIdentity>,
    window_ids: impl IntoIterator<Item = u32>,
) -> bool {
    if observed_after_scan != Some(expected) {
        return false;
    }
    registry.replace_tabs_for_identity(expected, window_ids);
    true
}
```

Production re-resolves `observed_after_scan` from the current local
`NSRunningApplication` immediately before locking the registry and calls this
helper. Unit tests inject matching/mismatching identities and assert mismatch
returns false without changing either A or B records.

Add `TabRegistry` tests with identities A and B sharing PID 42 but different
launch dates. Register untracked tab IDs 101 for A and 202 for B, clear A by
identity, and assert B remains. Also verify replacing A's scan cannot replace
B's tabs and that a mismatched post-scan identity publishes no replacement.
Add a same-window-ID replacement test: register tab ID 303 for A, replace it
with owner B, then process exact unregister for A and assert B's record remains;
exact unregister for B removes it.

- [ ] **Step 5: Capture identity in NSWorkspace termination**

Current `extract_app_info` signature (line 245 of `app_monitor.rs`):

```rust
fn extract_app_info(notification: *mut Object) -> (i32, Option<String>, Option<String>)
```

It obtains `running_app` from `NSWorkspaceApplicationKey` (line 259), then
extracts PID (line 265), bundle ID, and app name from the same object.

Replace extraction to also capture identity FROM THE SAME `running_app` object
(before any PID rediscovery). Return `None` identity so the EventProcessor drops
events without a valid identity (fail-closed). No PID rediscovery — identity is
extracted from the same object that supplies PID/bundle/name:

```rust
/// Extracts app info from an `NSNotification`.
/// Returns (identity, pid, bundle_id, app_name).
/// identity is None if capture fails (fail-closed — caller drops the event).
fn extract_app_info(
    notification: *mut Object,
) -> (Option<AppIdentity>, i32, Option<String>, Option<String>) {
    if notification.is_null() {
        return (None, 0, None, None);
    }

    unsafe {
        let user_info: *mut Object = msg_send![notification, userInfo];
        if user_info.is_null() {
            return (None, 0, None, None);
        }

        let app_key = nsstring("NSWorkspaceApplicationKey");
        let running_app: *mut Object = msg_send![user_info, objectForKey: app_key];
        if running_app.is_null() {
            return (None, 0, None, None);
        }

        // Capture identity from this exact object BEFORE extracting PID.
        // Must use the same object — no separate PID rediscovery.
        let identity = AppIdentity::from_ns_running_app(running_app);

        let pid: i32 = msg_send![running_app, processIdentifier];
        if pid <= 0 {
            return (None, 0, None, None);
        }

        let bundle_id: Option<String> = get_bundle_id(running_app);
        let app_name: Option<String> = get_app_name(running_app);

        (identity, pid, bundle_id, app_name)
    }
}
```

Update `ExtractAppInfo` type alias if it exists. Update
`AppMonitorAdapter::on_app_launched` and `on_app_terminated` signatures to
accept `Option<AppIdentity>`. Preserve the current
`bundle_id.unwrap_or_default()` and `name.unwrap_or_default()` conversion in
`on_app_launched` before passing owned `String`s to `EventProcessor` and
`StateMessage`; do not change downstream signatures merely to preserve
`Option<String>`. The adapter drops events with `None` identity before
forwarding to `EventProcessor`:

```rust
fn on_app_launched(&self, identity: Option<AppIdentity>, pid: i32, bundle_id: Option<String>, name: Option<String>) {
    let Some(identity) = identity else {
        tracing::trace!(pid, "app_monitor: dropping launch event (no identity)");
        return;
    };
    let bundle_id = bundle_id.unwrap_or_default();
    let name = name.unwrap_or_default();
    self.processor.on_app_launched(identity, pid, bundle_id, name);
}

fn on_app_terminated(&self, identity: Option<AppIdentity>, pid: i32, bundle_id: Option<&str>, name: Option<&str>) {
    let Some(identity) = identity else {
        tracing::trace!(pid, "app_monitor: dropping terminate event (no identity)");
        return;
    };
    // Remove AX observer for this exact identity (main thread — safe)
    crate::modules::tiling::events::observer::remove_observer_for_identity(&identity);
    self.processor.on_app_terminated(identity, pid);
}
```

On `None` identity the event is dropped. No fallback to bare-PID forwarding —
fail-closed per spec.

- [ ] **Step 6: Fix all call sites — atomic type + constructor update**

`WindowEvent` adds a non-optional `identity: AppIdentity` field, so its
constructor `WindowEvent::new(event_type, pid, element, identity)` breaks at
every existing call site. Similarly `StateMessage` lifecycle variants gain an
`identity` field, breaking every match arm. These cannot be added in a
compile-safe intermediate state — they are fixed in ONE atomic step:

1. **Add field + update all constructors and match arms in one commit:**
   - `Window { identity: None, .. }` for `Window::default()` — compiles because
     `Option<AppIdentity>` field exists with `None` default.
   - `WindowCreatedInfo { identity: None, .. }` — same optional pattern.
   - `WindowEvent::new(...)` — add 4th `identity: AppIdentity` parameter;
     update EVERY call site (AX callback, ax_observer adapter, test code).
   - `StateMessage::AppShown { identity, pid }` / `AppHidden { identity, pid }`
     / `AppTerminated { identity, pid }` / `AppLaunched { identity, pid, .. }` —
     add identity field to each variant; update EVERY match arm.
   - Every AX-derived ID-targeted `StateMessage` variant listed in Step3 and
     `GeometryUpdate` — add identity; update EventProcessor methods, batch-map
     keys, actor matches, and tests atomically. Actor matches reject identity
     mismatch before invoking any handler.
   - `ax_observer.rs::observer_callback` — capture identity from `ObserverRecord`
     and pass to `WindowEvent::new` (step 4 above).
   - `app_monitor.rs::on_app_launched`/`on_app_terminated` — accept
     `Option<AppIdentity>`, drop on `None` (step 5 above).
   - Initial window scan in `init.rs` — capture identity per window's PID.
   - `tabs.rs` — update every tab registration/scan/classification call to exact
     identity APIs and add post-scan identity revalidation before publication.
   - `actor/handlers/window.rs` — upgrade `None` identity on an existing window
     and remove/recreate state when the same window ID arrives with a different
     exact identity.

   After all updates, run `cargo check` once to verify. No intermediate
   check between type addition and call-site fixes is possible for the
   non-optional `WindowEvent.identity` field and `StateMessage` variants.

2. **Capture in AX observer** (step 4 above) — store identity in `ObserverRecord`,
   callback copies identity into `WindowEvent::new`.

3. **Capture in app_monitor** (step 5 above) — `extract_app_info` returns
   `Option<AppIdentity>`; adapter drops `None` before forwarding.

- [ ] **Step 7: Build and run specific tests**

```bash
cargo check -p stache 2>&1 | tail -20
# Fix all type errors iteratively, then:
cargo test -p stache --lib modules::tiling::events::types::tests
cargo test -p stache --lib modules::tiling::actor::messages::tests
cargo test -p stache --lib modules::tiling::tabs::tests
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::effects::tests
cargo test -p stache --lib modules::tiling::effects::subscriber::tests
cargo test -p stache --lib modules::tiling::effects::executor::tests
cargo test -p stache --lib modules::tiling::effects::window_cache::tests
cargo test -p stache --lib modules::tiling::effects::animation
cargo fmt --all -- --check
cargo check -p stache
```

- [ ] **Step 8: Commit**

```bash
git add app/native/src/modules/tiling/state/types.rs \
  app/native/src/modules/tiling/actor/messages.rs \
  app/native/src/modules/tiling/events/types.rs \
  app/native/src/modules/tiling/events/observer.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/events/app_monitor.rs \
  app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/events/drag_state.rs \
  app/native/src/modules/tiling/tabs.rs \
  app/native/src/modules/tiling/actor/handlers/window.rs \
  app/native/src/modules/tiling/actor/handlers/app.rs \
  app/native/src/modules/tiling/actor/handlers/focus.rs \
  app/native/src/modules/tiling/actor/handlers/workspace.rs \
  app/native/src/modules/tiling/actor/handlers/preset.rs \
  app/native/src/modules/tiling/actor/handlers/window_move.rs \
  app/native/src/modules/tiling/actor/handle.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/rules/mod.rs \
  app/native/src/modules/tiling/state/tiling_state.rs \
  app/native/src/modules/tiling/init.rs \
  app/native/src/modules/tiling/effects/mod.rs \
  app/native/src/modules/tiling/effects/subscriber.rs \
  app/native/src/modules/tiling/effects/executor.rs \
  app/native/src/modules/tiling/effects/window_cache.rs \
  app/native/src/modules/tiling/effects/window_ops.rs \
  app/native/src/modules/tiling/effects/animation/mod.rs \
  app/native/src/modules/tiling/effects/animation/state.rs \
  app/native/src/modules/tiling/effects/animation/transition.rs \
  app/native/src/modules/bar/components/tiling.rs
git commit -m "feat(tiling): propagate AppIdentity through windows, events, tabs"
```

---

### Task 19C: `VisibilityRegistry` — shared, sealed `BTreeSet<AppIdentity>`

**Files:**

- Create (rewrite): `app/native/src/modules/tiling/visibility.rs`
- Modify: `app/native/src/modules/tiling/actor/handle.rs` — add `registry: Arc<VisibilityRegistry>`
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — create registry at spawn, share with handle
- Modify: `app/native/src/modules/tiling/mod.rs` — update re-exports

> **Safe staging:** Task 19C adds the registry alongside the existing FIFO/generation
> `HiddenAppTracker` code in `visibility.rs`. The old code still compiles and the
> old tests still pass. The two systems coexist until Task 19G deletes the obsolete
> code. No production code calls the new `VisibilityRegistry` until Task 19D.

- [ ] **Step 1: Write registry tests that define the API**

Write these before implementation — they must fail to compile:

```rust
// In visibility.rs tests — uses helpers because these are cross-module
// (visibility.rs cannot directly construct private LaunchDateBits field).
fn bits(v: u64) -> LaunchDateBits {
    LaunchDateBits::from_time_interval_since_reference_date(v as f64).unwrap()
}

#[derive(Clone, Copy)]
struct FakeApp {
    identity: AppIdentity,
    hidden: bool,
}
fn id10_1() -> AppIdentity { AppIdentity { pid: 10_i32, launch_date: bits(1) } }
fn id20_2() -> AppIdentity { AppIdentity { pid: 20_i32, launch_date: bits(2) } }
fn id30_3() -> AppIdentity { AppIdentity { pid: 30_i32, launch_date: bits(3) } }

#[test]
fn registry_accepts_and_drains_identities() {
    let reg = VisibilityRegistry::default();
    let id1 = id10_1();
    let id2 = id20_2();

    assert!(!reg.sealed());
    reg.insert(id1);
    reg.insert(id2);
    assert!(reg.contains(&id1));
    assert_eq!(reg.len(), 2);

    // seal_and_drain is the only closure API — test insert after seal
    let drained = reg.seal_and_drain();
    assert_eq!(drained, vec![id1, id2]); // sorted by Ord
    assert!(reg.is_empty());
    // Late insert is a no-op after seal_and_drain
    reg.insert(id30_3());
    assert!(reg.is_empty());
}

#[test]
fn registry_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let reg = Arc::new(VisibilityRegistry::default());
    // Offset by 1: bits(0) → from_time_interval_since_reference_date(0.0) → None → panic.
    let ids: Vec<_> = (1..=100_i32).map(|i| AppIdentity {
        pid: i,
        launch_date: bits(i as u64),
    }).collect();

    // Concurrent inserts from multiple threads
    let mut handles = Vec::new();
    for chunk in ids.chunks(25) {
        let r = Arc::clone(&reg);
        let c = chunk.to_vec();
        handles.push(thread::spawn(move || {
            for id in c { r.insert(id); }
        }));
    }
    for h in handles { h.join().unwrap(); }

    assert_eq!(reg.len(), 100);
    let drained = reg.seal_and_drain();
    assert_eq!(drained.len(), 100);
}

#[test]
fn registry_seal_rejects_actor_ops() {
    // Prove that post-seal actor hide/unhide are rejected without OS calls.
    let reg = VisibilityRegistry::default();
    reg.seal_and_drain();
    // The actor's closure APIs (hide_if_open / unhide_if_open) check sealed
    // and return Failed without invoking the OS callback.
    let outcome = reg.hide_if_open(id10_1(), |id| {
        panic!("must not call OS op after seal: {id:?}")
    });
    assert_eq!(outcome, HideAppOutcome::Failed);
    let outcome = reg.unhide_if_open(id10_1(), |id| {
        panic!("must not call OS op after seal: {id:?}")
    });
    assert_eq!(outcome, UnhideAppOutcome::Failed);
}
```

- [ ] **Step 2: Implement `VisibilityRegistry`**

```rust
use std::collections::BTreeSet;
use parking_lot::Mutex;

use super::identity::AppIdentity;
use crate::modules::tiling::effects::window_ops::{HideAppOutcome, UnhideAppOutcome};

/// Passive registry shared between `StateActor` and `StateActorHandle`.
///
/// The actor is the sole runtime writer. The handle's `seal_and_drain_visibility`
/// is the only external mutation API. Raw `RegistryState` fields are private —
/// actor code uses closure methods `hide_if_open`/`unhide_if_open`.
pub struct VisibilityRegistry {
    inner: Mutex<RegistryState>,
}

#[derive(Debug)]
struct RegistryState {
    sealed: bool,
    owned: BTreeSet<AppIdentity>,
}

impl Default for VisibilityRegistry {
    fn default() -> Self {
        Self { inner: Mutex::new(RegistryState { sealed: false, owned: BTreeSet::new() }) }
    }
}

impl VisibilityRegistry {
    /// Atomically: checks sealed → invokes `op` → mutates BTreeSet.
    /// If sealed, returns `HideAppOutcome::Failed` without calling `op`.
    /// `op` receives the identity and returns the outcome; the registry
    /// only inserts on `HiddenByStache`.
    pub(crate) fn hide_if_open(
        &self,
        identity: AppIdentity,
        op: impl FnOnce(AppIdentity) -> HideAppOutcome,
    ) -> HideAppOutcome {
        let mut state = self.inner.lock();
        if state.sealed {
            return HideAppOutcome::Failed;
        }
        let outcome = op(identity);
        if outcome == HideAppOutcome::HiddenByStache {
            state.owned.insert(identity);
        }
        outcome
    }

    /// Atomically: checks sealed → invokes `op` → mutates BTreeSet.
    /// If sealed, returns `UnhideAppOutcome::Failed` without calling `op`.
    /// Removes on `UnhiddenByStache` or `AlreadyShown`.
    pub(crate) fn unhide_if_open(
        &self,
        identity: AppIdentity,
        op: impl FnOnce(AppIdentity) -> UnhideAppOutcome,
    ) -> UnhideAppOutcome {
        let mut state = self.inner.lock();
        if state.sealed {
            return UnhideAppOutcome::Failed;
        }
        let outcome = op(identity);
        if matches!(outcome, UnhideAppOutcome::UnhiddenByStache | UnhideAppOutcome::AlreadyShown) {
            state.owned.remove(&identity);
        }
        outcome
    }

    /// Non-atomic insert/remove/contains/etc for test helpers and
    /// revalidation-only paths. Not used in the workspace hide/unhide
    /// hot path (which uses hide_if_open/unhide_if_open for atomicity).

    /// Inserts an identity after seal check. Returns true if newly inserted.
    pub(crate) fn insert(&self, identity: AppIdentity) -> bool {
        let mut state = self.inner.lock();
        if state.sealed { return false; }
        state.owned.insert(identity)
    }

    /// Removes an identity from the owned set.
    pub(crate) fn remove(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().owned.remove(identity)
    }

    /// Returns true if the identity is currently owned.
    pub(crate) fn contains(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().owned.contains(identity)
    }

    /// Returns the number of owned identities.
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.inner.lock().owned.len()
    }

    /// Returns true if the registry is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.inner.lock().owned.is_empty()
    }

    /// Returns whether the registry has been sealed.
    pub(crate) fn sealed(&self) -> bool {
        self.inner.lock().sealed
    }

    /// Atomically seals and drains in a single lock acquisition.
    /// This is the only mutation API exposed outside the actor/controller.
    /// Seal-and-drain occurs BEFORE actor teardown through the same mutex,
    /// so the actor cannot observe sealed=false while seal_and_drain runs.
    pub(crate) fn seal_and_drain(&self) -> Vec<AppIdentity> {
        let mut state = self.inner.lock();
        state.sealed = true;
        let mut result: Vec<_> = state.owned.iter().copied().collect();
        result.sort_unstable();
        state.owned.clear();
        result
    }
}
```

- [ ] **Step 3: Wire into `StateActor` / `StateActorHandle`**

In `actor/handle.rs:32`, add:

```rust
pub struct StateActorHandle {
    sender: mpsc::Sender<StateMessage>,
    registry: Arc<VisibilityRegistry>,  // NEW — private; accessed via seal_and_drain_visibility
}

impl StateActorHandle {
    /// Atomically seals the registry and drains all owned identities for restoration.
    /// The actor channel MUST be alive when called (seal happens before Task20 tiling
    /// shutdown closes the actor).
    pub(crate) fn seal_and_drain_visibility(&self) -> Vec<AppIdentity> {
        self.registry.seal_and_drain()
    }
}
```

Update `handle.rs` imports to include `std::sync::Arc` and
`crate::modules::tiling::visibility::VisibilityRegistry`; the old constructor
can no longer remain `const` because it allocates an `Arc`.

In `actor/mod.rs::StateActor::spawn()`, create the registry before the actor.
`StateActor` gains a `registry: Arc<VisibilityRegistry>` field. Because adding
the field makes the current one-field handle constructor incomplete, replace
it with these two concrete constructors. Existing handle unit tests can keep
calling `new(sender)`; production spawn uses `new_with_registry` so actor and
handle share the same `Arc`. Preserve Task15's reusable completion contract:
both spawn functions return `(StateActorHandle, CompletionLatch)`, and the
actor marks that latch only after its loop exits. `RuntimeFactory` consumes this
tuple unchanged; Task19C injects only the fresh registry and does not replace
Task15's lifecycle wiring:

```rust
// StateActor gains a registry field:
pub struct StateActor {
    state: TilingState,
    receiver: mpsc::Receiver<StateMessage>,
    registry: Arc<VisibilityRegistry>,  // NEW
}

impl StateActorHandle {
    pub(crate) fn new(sender: mpsc::Sender<StateMessage>) -> Self {
        Self::new_with_registry(sender, Arc::new(VisibilityRegistry::default()))
    }

    pub(crate) fn new_with_registry(
        sender: mpsc::Sender<StateMessage>,
        registry: Arc<VisibilityRegistry>,
    ) -> Self {
        Self { sender, registry }
    }
}

pub fn spawn() -> (StateActorHandle, CompletionLatch) {
    Self::spawn_with_registry(Arc::new(VisibilityRegistry::default()))
}

pub(crate) fn spawn_with_registry(
    registry: Arc<VisibilityRegistry>,
) -> (StateActorHandle, CompletionLatch) {
    tracing::debug!("tiling: spawning state actor");
    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
    let handle = StateActorHandle::new_with_registry(sender, Arc::clone(&registry));
    let stopped = CompletionLatch::new();
    let stopped_for_task = stopped.clone();

    let actor = Self {
        state: TilingState::new(),
        receiver,
        registry,
    };

    tauri::async_runtime::spawn(async move {
        actor.run().await;
        stopped_for_task.mark_complete();
    });

    (handle, stopped)
}
```

The actor stores its own `Arc<VisibilityRegistry>` clone. Runtime hide/unhide
operations use the closure-based `hide_if_open`/`unhide_if_open` methods —
never direct `inner` access. The `insert`/`remove` convenience methods are used
only for revalidation paths (AppShown handling) and test helpers, not for the
workspace hide/unhide hot path.

- [ ] **Step 4: Update re-exports**

At this point, `restore_stache_hidden_apps` still uses the old `HiddenAppTracker`
internally (it is replaced as part of the atomic Task 19D+19E cutover). Do NOT
re-export `restore_from_with`; it remains an internal test seam introduced in
Task 19D. Keep the existing public re-exports:

```rust
pub use visibility::{RestoreSummary, restore_stache_hidden_apps};
```

Add `VisibilityRegistry` to the existing re-exports in `tiling/mod.rs`:

```rust
pub use visibility::{RestoreSummary, restore_stache_hidden_apps, VisibilityRegistry};
```

> **Mutex discipline:** Every registry mutation from the actor uses
> `hide_if_open`/`unhide_if_open` — the closure-based API on `VisibilityRegistry` —
> which atomically checks sealed, runs the identity-validated OS call, and mutates
> the `BTreeSet`. The actor never accesses `inner`, `sealed`, or `owned` directly;
> these are private to `VisibilityRegistry`. No `run_on_main_thread` or other
> main-thread dispatch happens while the registry lock is held
> (NSRunningApplication hide/unhide does not perform main-thread callbacks).

- [ ] **Step 5: Run tests**

```bash
cargo test -p stache --lib modules::tiling::visibility::tests
cargo test -p stache --lib modules::tiling::actor::tests
cargo check -p stache
```

- [ ] **Step 6: Commit**

```bash
git add app/native/src/modules/tiling/visibility.rs \
  app/native/src/modules/tiling/actor/handle.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): VisibilityRegistry with sealed BTreeSet<AppIdentity>"
```

---

### Task 19D: Actor-owned workspace hide/unhide controller

**Files:**

- Modify: `app/native/src/modules/tiling/actor/handlers/window.rs` — actor hide/unhide via registry; `VisibilityDelta` for focus-path callers
- Modify: `app/native/src/modules/tiling/actor/handlers/workspace.rs` — `VisibilityDelta` collect/apply; workspace switch handler calls actor methods
- Modify: `app/native/src/modules/tiling/state/tiling_state.rs` — add `windows_identity_iter`
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — add exact `hide_app_instance_with_outcome`/`unhide_app_instance_with_outcome`; bare-PID wrappers remain only until Task19G removes them after the atomic cutover
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — wire hide/unhide through actor registry
- Modify: `app/native/src/modules/tiling/visibility.rs` — atomically cut shutdown restoration over to the same registry before committing the new hide path
- Modify: `app/native/src/modules/tiling/mod.rs` — preserve public restore API while using registry ownership
- Modify: `app/native/src/modules/tiling/init.rs` — make temporary pause drain
  the current generation's registry and make each fresh resume install a new,
  unsealed registry generation

- [ ] **Step 1: Add identity-aware helpers to `window_ops.rs`**

Add exact-instance hide/unhide helpers that take `AppIdentity` (not bare PID).
Each resolves one local `NSRunningApplication`, validates identity equality on
the same object, then reuses the shared detailed ObjC core (the same
hide/unhide implementation used by the existing PID-based helpers). The local
ObjC call runs under an autorelease pool; no reference escapes.

Keep the existing bare-PID helpers only as temporary compilation bridges while
Tasks19D and 19E are uncommitted in the same working tree. No new production
caller may use them. Task19G deletes them after the atomic exact-identity
cutover proves there are no remaining callers.

```rust
// In window_ops.rs — new identity-aware helpers.
use crate::modules::tiling::identity::AppIdentity;
use objc::runtime::{Class, Object, BOOL, YES};
use objc::{msg_send, sel, sel_impl};

#[must_use]
pub fn hide_app_instance_with_outcome(identity: AppIdentity) -> HideAppOutcome {
    objc::rc::autoreleasepool(|| unsafe {
        let app_class = Class::get("NSRunningApplication");
        let app_class = match app_class {
            Some(c) => c,
            None => return HideAppOutcome::Failed,
        };
        let app: *mut Object = msg_send![app_class,
            runningApplicationWithProcessIdentifier: identity.pid];
        if app.is_null() {
            return HideAppOutcome::Failed;
        }
        let actual = match AppIdentity::from_ns_running_app(app) {
            Some(a) => a,
            None => return HideAppOutcome::Failed,
        };
        if actual != identity {
            // PID-reuse: a different process now owns this PID.
            return HideAppOutcome::Failed;
        }
        let is_hidden: BOOL = msg_send![app, isHidden];
        if is_hidden == YES {
            return HideAppOutcome::AlreadyHidden;
        }
        let result: BOOL = msg_send![app, hide];
        if result == YES { HideAppOutcome::HiddenByStache } else { HideAppOutcome::Failed }
    })
}

#[must_use]
pub fn unhide_app_instance_with_outcome(identity: AppIdentity) -> UnhideAppOutcome {
    objc::rc::autoreleasepool(|| unsafe {
        let app_class = Class::get("NSRunningApplication");
        let app_class = match app_class {
            Some(c) => c,
            None => return UnhideAppOutcome::Failed,
        };
        let app: *mut Object = msg_send![app_class,
            runningApplicationWithProcessIdentifier: identity.pid];
        if app.is_null() {
            return UnhideAppOutcome::Failed;
        }
        let actual = match AppIdentity::from_ns_running_app(app) {
            Some(a) => a,
            None => return UnhideAppOutcome::Failed,
        };
        if actual != identity {
            return UnhideAppOutcome::Failed;
        }
        let is_hidden: BOOL = msg_send![app, isHidden];
        if is_hidden == YES {
            let result: BOOL = msg_send![app, unhide];
            if result == YES { UnhideAppOutcome::UnhiddenByStache } else { UnhideAppOutcome::Failed }
        } else {
            UnhideAppOutcome::AlreadyShown
        }
    })
}
```

> No `?` operators — the return type is `HideAppOutcome`/`UnhideAppOutcome`, not
> `Option` or `Result`. Full `match`/early-return instead.

- [ ] **Step 2: Add actor-owned workspace hide/unhide methods**

Add to `StateActor`. Each method delegates to the registry's closure-based
API (`hide_if_open`/`unhide_if_open`) which atomically checks sealed, calls
the identity-validated OS operation, and mutates the owned set — without
exposing `inner`, `sealed`, or `owned` to the actor module:

```rust
/// Runs inside the actor's message loop. Delegates to
/// `VisibilityRegistry::hide_if_open` which holds the mutex across sealed
/// check, OS call, and ownership mutation atomically.
fn handle_hide_for_workspace_with(
    &mut self,
    identity: AppIdentity,
    hide: impl FnOnce(AppIdentity) -> HideAppOutcome,
) -> HideAppOutcome {
    let outcome = self.registry.hide_if_open(identity, hide);

    // Registry lock already released — update window state on actor side
    if outcome == HideAppOutcome::HiddenByStache {
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = true);
        }
    }
    outcome
}

fn handle_hide_for_workspace(&mut self, identity: AppIdentity) -> HideAppOutcome {
    self.handle_hide_for_workspace_with(identity, hide_app_instance_with_outcome)
}

fn handle_unhide_for_workspace_with(
    &mut self,
    identity: AppIdentity,
    unhide: impl FnOnce(AppIdentity) -> UnhideAppOutcome,
) -> UnhideAppOutcome {
    let outcome = self.registry.unhide_if_open(identity, unhide);

    if matches!(outcome, UnhideAppOutcome::UnhiddenByStache | UnhideAppOutcome::AlreadyShown) {
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = false);
        }
    }
    outcome
}

fn handle_unhide_for_workspace(&mut self, identity: AppIdentity) -> UnhideAppOutcome {
    self.handle_unhide_for_workspace_with(identity, unhide_app_instance_with_outcome)
}
```

The private `_with` methods are deterministic actor-test seams; production
wrappers pass the real exact-identity ObjC operations. Tests therefore exercise
the same registry and actor-state mutation logic without requiring live macOS
applications.

Add `windows_identity_iter` to `TilingState` (no tuple destructuring — `Window`
is not stored as `(id, Window)` tuple but as part of a flat structure or map):

```rust
/// Returns window IDs whose identity matches the given value.
/// Uses `filter(|w| w.identity.as_ref() == Some(identity))` without
/// tuple destructuring (windows is not `HashMap<u32, Window>` tuple
/// iteration in this codebase — adjust to actual storage).
pub fn windows_identity_iter(&self, identity: &AppIdentity) -> Vec<u32> {
    self.windows.iter()
        .filter(|w| w.identity.as_ref() == Some(identity))
        .map(|w| w.id)
        .collect()
}
```

> If `self.windows` is `HashMap<u32, Window>`, iteration is
> `.iter().filter(|(_, w)| w.identity.as_ref() == Some(identity)).map(|(id, _)| *id)`;
> the documentation above avoids tuple destructuring by describing only the
> semantic intent. Adjust to the actual storage type.

- [ ] **Step 3: Route workspace visibility through actor**

In `actor/mod.rs`, the workspace switch message handler calls
`self.handle_hide_for_workspace(identity)` instead of the global
`hide_app_for_workspace(pid)`. The identity is obtained from the
window's stored identity.

Define a concrete `VisibilityDelta` beside the existing
`sync_window_visibility_for_workspaces` function in `handlers/window.rs`. The
free handlers return this value; only `StateActor` applies it. No handler calls
an actor method directly:

```rust
// In handlers/window.rs
/// Identity-based visibility change set produced by workspace analysis.
/// Every affected signature uses this type — no bare PID iteration.
#[derive(Debug, Default)]
pub struct VisibilityDelta {
    pub showing: Vec<AppIdentity>,
    pub hiding: Vec<AppIdentity>,
}
```

Change the existing collector to return the delta instead of performing OS
operations. Its current empty-input branch returns `VisibilityDelta::default()`:

```rust
pub fn sync_window_visibility_for_workspaces(
    state: &TilingState,
    becoming_visible: &[Uuid],
    becoming_hidden: &[Uuid],
) -> VisibilityDelta;
```

Change the three free handlers that currently trigger visibility to return a
delta while preserving all their existing state mutations, notifications, and
frontend events:

```rust
// handlers/workspace.rs — every early return uses VisibilityDelta::default().
pub fn on_switch_workspace(state: &mut TilingState, name: &str) -> VisibilityDelta;
pub fn on_send_workspace_to_screen(
    state: &mut TilingState,
    target_screen: &TargetScreen,
) -> VisibilityDelta;
pub fn on_cycle_workspace(
    state: &mut TilingState,
    direction: CycleDirection,
) -> VisibilityDelta;

// handlers/window.rs — every early return uses VisibilityDelta::default().
pub fn on_window_focused(state: &mut TilingState, window_id: u32) -> VisibilityDelta;
```

Each function replaces its current direct sync call with
`let delta = sync_window_visibility_for_workspaces(...)`, finishes its existing
notifications/focus/event work, and returns `delta` at the end. The only current
visibility-changing callsites are `workspace.rs` (workspace switch,
send-to-screen, and cycle) and `window.rs` (window-focused);
`handlers/focus.rs` does not call the sync function and is not changed. For
workspace cycling, compute the delta after marking `current_workspace_id`
hidden and `next_workspace_id` visible:

```rust
let delta = sync_window_visibility_for_workspaces(
    state,
    &[next_workspace_id],
    &[current_workspace_id],
);
// Preserve existing focus, subscriber notifications, and frontend event work.
delta
```

Every early return from `on_cycle_workspace` returns
`VisibilityDelta::default()`.

Preserve the existing semantic algorithm exactly: compute identities in
becoming-hidden workspaces MINUS identities with any window in ANY currently
visible workspace; put that difference in `hiding`, and compute `showing` from
identities newly exposed by becoming-visible workspaces. Do not narrow the
visible-workspace subtraction to one workspace, and do not hide first.

Update the concrete actor-owned callsites in `actor/mod.rs`:

```rust
// In actor/mod.rs — replaces the previous static
// sync_window_visibility_for_workspaces.
fn sync_visibility_for_workspaces(&mut self, delta: VisibilityDelta) {
    for identity in delta.showing {
        self.handle_unhide_for_workspace(identity);
    }
    for identity in delta.hiding {
        self.handle_hide_for_workspace(identity);
    }
}

fn on_switch_workspace(&mut self, name: &str) {
    let delta = handlers::on_switch_workspace(&mut self.state, name);
    self.sync_visibility_for_workspaces(delta);
}

fn on_send_workspace_to_screen(&mut self, target: &messages::TargetScreen) {
    let delta = handlers::on_send_workspace_to_screen(&mut self.state, target);
    self.sync_visibility_for_workspaces(delta);
}

fn on_cycle_workspace(&mut self, direction: CycleDirection) {
    let delta = handlers::on_cycle_workspace(&mut self.state, direction);
    self.sync_visibility_for_workspaces(delta);
}

// In StateMessage::WindowFocused dispatch:
let delta = handlers::on_window_focused(&mut self.state, window_id);
self.sync_visibility_for_workspaces(delta);
```

Add an exact-identity cycle test with one app only on the old workspace and one
only on the next workspace. Cycle once and assert the returned delta shows the
next identity, hides the old identity, and the actor registry owns only the old
hidden identity after applying it.

`sync_window_visibility_for_workspaces` preserves the current algorithm using
identity sets: `showing` is collected from becoming-visible workspaces;
`hidden_candidates` is collected from becoming-hidden workspaces;
`currently_visible` is collected from every window whose workspace appears in
`state.get_visible_workspaces()` after the state transition; and `hiding` is
`hidden_candidates.difference(&currently_visible)`. Sort both vectors for
deterministic tests. `sync_visibility_for_workspaces` shows every identity first
and hides every identity second.

Migrate the separate initialization path in the same change. Current
`StateActor::on_init_complete(&self)` calls PID-based
`sync_window_visibility(&self)`, which writes the old tracker. Change both to
`&mut self`; collect exact identities from windows (skip `None` fail-closed),
subtract identities present in any visible workspace from non-visible
candidates, sort, then call `handle_unhide_for_workspace` for visible identities
before `handle_hide_for_workspace` for hidden-only identities:

Add `use std::collections::{BTreeSet, HashSet};` and retain/import `Uuid` in
`actor/mod.rs` for the following implementation.

```rust
fn on_init_complete(&mut self) {
    self.sync_window_visibility();
    // existing layout notification logic remains unchanged
}

fn sync_window_visibility(&mut self) {
    let visible_ws_ids: HashSet<Uuid> = self.state
        .get_visible_workspaces().iter().map(|ws| ws.id).collect();
    let mut visible = BTreeSet::new();
    let mut non_visible = BTreeSet::new();
    for window in self.state.windows.iter() {
        let Some(identity) = window.identity else { continue; };
        if visible_ws_ids.contains(&window.workspace_id) {
            visible.insert(identity);
        } else {
            non_visible.insert(identity);
        }
    }
    for identity in visible.iter().copied() {
        self.handle_unhide_for_workspace(identity);
    }
    for identity in non_visible.difference(&visible).copied() {
        self.handle_hide_for_workspace(identity);
    }
}
```

Add an initialization test with one visible and one hidden-only exact identity;
apply the calculated operations through the private `_with` actor methods using
injected `AlreadyShown` and `HiddenByStache` outcomes. Assert the registry owns
only the hidden-only identity. Likewise, apply the cycle delta through these
injected seams rather than calling live NSRunningApplication APIs. Separately
assert injected `Failed` outcomes do not change ownership. No PID-based
visibility wrapper remains in `actor/mod.rs` after this task.

Integrate this registry with Task15's runtime slot in `init.rs` during the same
atomic cutover. `start_runtime`/actor spawn creates a fresh
`Arc<VisibilityRegistry>` for every generation and publishes it through that
generation's actor/handle; it must never reuse a registry that was sealed by a
previous pause. `pause_runtime` first transitions the lifecycle to `Stopping`
while leaving the running runtime in its slot, releases all locks, and calls the
existing public `restore_stache_hidden_apps()` so the current handle can seal,
drain, and restore ownership. Only after that idempotent call returns may it
take the runtime for teardown/quarantine. A retry from `Quarantined` skips or
repeats the now-empty restore safely. Terminal `app_shutdown` may call the same
public helper again and receives an empty summary rather than unhiding twice.

> **Note:** The old `forget_stache_hidden_app`, `forget_stache_hidden_app_terminated`,
> `classify_stache_hidden_app`, `classify_stache_hidden_app_with_state`,
> `ShownClassification`, and `PidState` symbols are NOT deleted here.
> They are still used by the EventProcessor until Task 19E strips the
> classifier imports, and the obsolete definitions are removed in Task 19G.
> At this stage the obsolete lifecycle classifier coexists with the new code,
> but workspace hide ownership and shutdown restoration both cut over
> atomically to `VisibilityRegistry` in this task.

- [ ] **Step 4: Atomically cut shutdown restoration over to the registry**

Do this in the SAME atomic Task19D+Task19E working-tree batch that switches the
workspace hide hot path. There must never be an intermediate commit where new
hides enter `VisibilityRegistry` while shutdown drains only the old tracker.
Add the exact-instance restoration helpers to `visibility.rs` now (Task19F adds
exhaustive tests, not the first production cutover):

```rust
use crate::modules::tiling::identity::AppIdentity;
use objc::runtime::{BOOL, YES};
use objc::{msg_send, sel, sel_impl};

#[must_use]
fn restore_one_exact_with<T>(
    owned: AppIdentity,
    resolve: impl FnOnce(i32) -> Option<T>,
    identity_of: impl FnOnce(&T) -> Option<AppIdentity>,
    hidden_of: impl FnOnce(&T) -> Option<bool>,
    unhide: impl FnOnce(&T) -> bool,
) -> bool {
    let Some(app) = resolve(owned.pid) else { return false; };
    if identity_of(&app) != Some(owned) || hidden_of(&app) != Some(true) {
        return false;
    }
    unhide(&app)
}

#[must_use]
fn restore_one_exact(owned: AppIdentity) -> bool {
    objc::rc::autoreleasepool(|| {
        restore_one_exact_with(
            owned,
            |pid| unsafe {
                let class = objc::runtime::Class::get("NSRunningApplication")?;
                let app: *mut objc::runtime::Object =
                    msg_send![class, runningApplicationWithProcessIdentifier: pid];
                (!app.is_null()).then_some(app)
            },
            |app| unsafe { AppIdentity::from_ns_running_app(*app) },
            |app| unsafe {
                let hidden: BOOL = msg_send![*app, isHidden];
                Some(hidden == YES)
            },
            |app| unsafe {
                let result: BOOL = msg_send![*app, unhide];
                result == YES
            },
        )
    })
}

#[must_use]
pub fn restore_from_with(
    identities: Vec<AppIdentity>,
    mut restore: impl FnMut(AppIdentity) -> bool,
) -> RestoreSummary {
    let attempted = identities.len();
    let mut restored = 0;
    for identity in identities {
        if restore(identity) {
            restored += 1;
        }
    }
    RestoreSummary { attempted, restored }
}

#[must_use]
pub fn restore_stache_hidden_apps() -> RestoreSummary {
    crate::modules::tiling::init::get_handle()
        .map(|handle| restore_from_with(handle.seal_and_drain_visibility(), restore_one_exact))
        .unwrap_or(RestoreSummary { attempted: 0, restored: 0 })
}
```

The raw `NSRunningApplication` pointer stays inside one autorelease pool and one
synchronous call. Identity equality and current hidden state are checked on the
same resolved object before unhide. `app_shutdown.rs` remains unchanged and
continues calling the same public `restore_stache_hidden_apps()` API.

- [ ] **Step 5: Build and run tests**

```bash
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::effects::window_ops::tests
cargo fmt --all -- --check
cargo check -p stache
```

- [ ] **Step 6: Do not commit; continue directly through Task19E**

Task19D's working tree is intentionally not a shippable checkpoint: the old
lifecycle classifier still cannot remove ownership from `VisibilityRegistry`.
Do not commit, quit/restart through this state, or hand it off independently.
Proceed immediately to Task19E, then commit the Task19D+19E file set together
after raw lifecycle forwarding/revalidation tests pass. This makes initial
sync, workspace hot paths, lifecycle relinquishment, and shutdown restoration
one atomic commit.

---

### Task 19E: Raw lifecycle forwarding and actor revalidation

**Files:**

- Modify: `app/native/src/modules/tiling/events/processor.rs`
- Modify: `app/native/src/modules/tiling/actor/handlers/app.rs`
- Modify: `app/native/src/modules/tiling/actor/mod.rs`
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — add `app_instance_is_hidden`

- [ ] **Step 1: Write actor revalidation tests**

The actor's `on_app_shown` must validate the identity against current OS state:

```rust
// In actor/handlers/app.rs tests
fn test_identity(pid: i32, v: u64) -> AppIdentity {
    AppIdentity {
        pid,
        launch_date: LaunchDateBits::from_time_interval_since_reference_date(v as f64).unwrap(),
    }
}

/// Sets up a test harness with an actor-owned registry and a minimal state.
/// The actor stores the registry and passes it to revalidation methods.
fn make_actor() -> (StateActor, Arc<VisibilityRegistry>) {
    let registry = Arc::new(VisibilityRegistry::default());
    let actor = StateActor { state: TilingState::new(), receiver: mpsc::channel(16).1, registry: Arc::clone(&registry) };
    (actor, registry)
}

#[test]
fn app_shown_visible_removes_owner_and_marks_windows() {
    let identity = test_identity(42, 100);
    let (mut actor, registry) = make_actor();
    registry.insert(identity);
    // Use real Window construction via Default + explicit required fields.
    // Window::default() provides sensible defaults for all non-identity fields.
    actor.state.upsert_window(Window {
        id: 1,
        pid: 42,
        app_id: "com.test.app".into(),
        app_name: "Test App".into(),
        title: "Window 1".into(),
        frame: Rect::new(0.0, 0.0, 800.0, 600.0),
        workspace_id: Uuid::now_v7(),
        identity: Some(identity),
        is_hidden: true,
        ..Window::default()
    });

    // Simulate AppShown with OS visible — actor-owned method
    actor.on_app_shown_revalidated(identity, |_| Some(false));

    assert!(!registry.contains(&identity));
    // All windows for this identity are marked visible
    for w in actor.state.windows.iter().filter(|w| w.identity == Some(identity)) {
        assert!(!w.is_hidden);
    }
}

#[test]
fn queued_app_shown_consumed_after_external_rehide_retains_owner() {
    let identity = test_identity(42, 100);
    let (mut actor, registry) = make_actor();
    registry.insert(identity);
    actor.state.upsert_window(Window {
        id: 1,
        pid: 42,
        app_id: "com.test.app".into(),
        app_name: "Test App".into(),
        title: "Window 1".into(),
        frame: Rect::new(0.0, 0.0, 800.0, 600.0),
        workspace_id: Uuid::now_v7(),
        identity: Some(identity),
        is_hidden: true,
        ..Window::default()
    });

    // Model the approved unavoidable-history case deterministically: Stache
    // owns the hide, an external show notification is queued, and the user
    // externally re-hides the app before the actor consumes that notification.
    // The injected read-only OS query therefore observes hidden.
    actor.on_app_shown_revalidated(identity, |_| Some(true));
    // OS says hidden — retain owner, no window change and no active hide call.
    assert!(registry.contains(&identity));
    for w in actor.state.windows.iter().filter(|w| w.identity == Some(identity)) {
        assert!(w.is_hidden);
    }
}

#[test]
fn app_shown_mismatch_retains_owner() {
    let stored = test_identity(42, 100);
    let different = test_identity(42, 200);
    let (mut actor, registry) = make_actor();
    registry.insert(stored);

    actor.on_app_shown_revalidated(different, |_| Some(false));

    assert!(registry.contains(&stored)); // retained
}

#[test]
fn delayed_termination_removes_only_exact_instance_resources() {
    let identity_a = test_identity(42, 100);
    let identity_b = test_identity(42, 200); // same PID, different launch date
    let (mut actor, registry) = make_actor();
    registry.insert(identity_a);
    registry.insert(identity_b);

    let workspace_a = Uuid::now_v7();
    let workspace_b = Uuid::now_v7();
    actor.state.upsert_workspace(Workspace {
        id: workspace_a,
        name: "A".into(),
        window_ids: smallvec![1],
        focused_window_index: Some(0),
        ..Workspace::default()
    });
    actor.state.upsert_workspace(Workspace {
        id: workspace_b,
        name: "B".into(),
        window_ids: smallvec![2],
        focused_window_index: Some(0),
        ..Workspace::default()
    });

    actor.state.upsert_window(Window {
        id: 1,
        pid: 42,
        identity: Some(identity_a),
        workspace_id: workspace_a,
        app_id: "com.test.a".into(),
        app_name: "A".into(),
        ..Window::default()
    });
    actor.state.upsert_window(Window {
        id: 2,
        pid: 42,
        identity: Some(identity_b),
        workspace_id: workspace_b,
        app_id: "com.test.b".into(),
        app_name: "B".into(),
        ..Window::default()
    });
    // Tab windows are deliberately NOT tracked in TilingState. Give each
    // same-PID application instance an independent untracked tab ID.
    tabs::clear_all_tabs();
    tabs::register_tab(101, identity_a);
    tabs::register_tab(202, identity_b);
    actor.state.record_focus_history(workspace_a, 1);
    actor.state.record_focus_history(workspace_b, 2);
    let mut invalidated = Vec::new();
    let mut cached_apps = HashSet::from([identity_a, identity_b]);

    // Inject only the cache side effect. The production helper itself selects
    // exact-instance window IDs and invokes both callbacks with those IDs.
    actor.on_app_terminated_exact_with(
        identity_a,
        tabs::clear_tabs_for_identity,
        |target| invalidated.push(target),
        |identity| { cached_apps.remove(&identity); },
    );

    assert!(!registry.contains(&identity_a));
    assert!(registry.contains(&identity_b));
    assert!(actor.state.get_window(1).is_none());
    assert_eq!(actor.state.get_window(2).and_then(|w| w.identity), Some(identity_b));
    assert!(!tabs::is_tab_for_identity(101, identity_a), "terminated instance tab must be cleared");
    assert!(tabs::is_tab_for_identity(202, identity_b), "replacement instance tab must survive");
    assert!(actor.state.get_workspace(workspace_a)
        .is_some_and(|ws| !ws.window_ids.contains(&1)));
    assert!(actor.state.get_workspace(workspace_b)
        .is_some_and(|ws| ws.window_ids.contains(&2)));
    assert_eq!(actor.state.get_focus_history(workspace_a), None);
    assert_eq!(actor.state.get_focus_history(workspace_b), Some(2));
    assert_eq!(
        invalidated,
        vec![WindowTarget { identity: identity_a, window_id: 1 }],
        "replacement cache target must not be invalidated",
    );
    assert!(!cached_apps.contains(&identity_a));
    assert!(cached_apps.contains(&identity_b), "replacement app cache must survive");
    tabs::clear_all_tabs();
}

#[test]
fn termination_clears_exact_untracked_tabs_without_tracked_windows() {
    let identity_a = test_identity(42, 100);
    let identity_b = test_identity(42, 200);
    let (mut actor, registry) = make_actor();
    registry.insert(identity_a);
    registry.insert(identity_b);

    // Neither tab ID exists in TilingState; both share a PID but differ by
    // exact application identity.
    tabs::clear_all_tabs();
    tabs::register_tab(101, identity_a);
    tabs::register_tab(202, identity_b);
    let mut invalidated = Vec::new();
    let mut cached_apps = HashSet::from([identity_a, identity_b]);

    actor.on_app_terminated_exact_with(
        identity_a,
        tabs::clear_tabs_for_identity,
        |target| invalidated.push(target),
        |identity| { cached_apps.remove(&identity); },
    );

    assert!(!registry.contains(&identity_a));
    assert!(registry.contains(&identity_b));
    assert!(!tabs::is_tab_for_identity(101, identity_a));
    assert!(tabs::is_tab_for_identity(202, identity_b));
    assert!(invalidated.is_empty(), "no tracked window cache IDs exist");
    assert!(!cached_apps.contains(&identity_a));
    assert!(cached_apps.contains(&identity_b), "replacement app cache must survive");
    tabs::clear_all_tabs();
}
```

- [ ] **Step 2: Run, verify RED**

```bash
cargo test -p stache --lib modules::tiling::actor::handlers::app::tests
```

Expected: RED — `on_app_shown_revalidated` is not yet implemented on
`StateActor`, so compilation fails.

- [ ] **Step 3: Strip EventProcessor classifier**

In `processor.rs`, remove:

- `use ...::ShownClassification` import
- `use ...::classify_stache_hidden_app` import
- `use ...::forget_stache_hidden_app_terminated` import
- `fn on_app_shown_with` (the seam/test helper)
- `ShownClassification` references

Replace with raw forwarding:

```rust
/// Forwards a raw AppShown event to the actor. The actor re-validates.
pub fn on_app_shown(&self, identity: AppIdentity, pid: i32) {
    tracing::trace!("App shown: identity={identity:?}, pid={pid}");
    let _ = self.actor_handle.send(StateMessage::AppShown { identity, pid });
}

pub fn on_app_hidden(&self, identity: AppIdentity, pid: i32) {
    tracing::trace!("App hidden: identity={identity:?}, pid={pid}");
    let _ = self.actor_handle.send(StateMessage::AppHidden { identity, pid });
}

pub fn on_app_terminated(&self, identity: AppIdentity, pid: i32) {
    tracing::trace!("App terminated: identity={identity:?}, pid={pid}");
    // No pre-actor forget: actor handles it via identity.
    let _ = self.actor_handle.send(StateMessage::AppTerminated { identity, pid });
}
```

- [ ] **Step 4: Implement actor revalidation**

The revalidation methods live on `StateActor` (which owns the registry), not
on `TilingState` or as standalone functions. `EventProcessor` forwards only
the raw identity — it never performs OS queries itself.

```rust
// In actor/mod.rs — on StateActor impl:
use std::collections::HashSet;
use uuid::Uuid;

/// Handles AppShown with identity revalidation.
/// The actor owns the registry; this method accesses it via `self.registry`.
fn on_app_shown_revalidated(
    &mut self,
    identity: AppIdentity,
    query_os: impl FnOnce(AppIdentity) -> Option<bool>,
) {
    let is_hidden = query_os(identity);
    match is_hidden {
        Some(false) => {
            // OS confirms visible — remove owner, mark windows shown
            self.registry.remove(&identity);
            for wid in self.state.windows_identity_iter(&identity) {
                self.state.update_window(wid, |w| w.is_hidden = false);
            }
        }
        Some(true) | None => {
            // OS hidden or unknown — retain owner, no state change
            // This is the conservative unavoidable-history policy.
        }
    }
}

/// Handles AppHidden — OS confirms hidden state.
/// Only updates if identity exactly matches (bare PID insufficient).
fn on_app_hidden_revalidated(&mut self, identity: AppIdentity, os_hidden: Option<bool>) {
    if os_hidden == Some(true) {
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = true);
        }
    }
    // If os_hidden is false/None (mismatch or unknown), no state change.
}

/// Handles AppTerminated — removes exact identity from registry AND performs
/// full termination cleanup for every exact-instance window: tab references,
/// cache entries, focus, workspace membership/layout references, and the
/// window record.
///
/// PID-reuse protection: the event carries the identity captured at
/// termination-notification time (ingress capture). A new process B that
/// inherits the same PID (different launch date) does NOT match. Only
/// windows whose identity is `Some(identity)` are affected — windows from
/// other processes with the same PID but different launch date are untouched.
///
/// OBSERVER REMOVAL happens on the main thread in `app_monitor.rs::on_app_terminated`
/// BEFORE this actor handler runs — identity is captured at ingress and stored
/// in the `remove_observer_for_identity` call there. This function only cleans
/// up actor-side state (tabs, cache, windows, registry).
fn on_app_terminated_exact(&mut self, identity: AppIdentity) {
    self.on_app_terminated_exact_with(
        identity,
        crate::modules::tiling::tabs::clear_tabs_for_identity,
        |target| {
            crate::modules::tiling::effects::get_window_cache()
                .invalidate_window(target);
        },
        |identity| {
            crate::modules::tiling::effects::get_window_cache()
                .invalidate_app(identity);
        },
    );
}

/// Test seam around side effects only. Exact window selection and cleanup
/// ordering remain production logic, so tests can prove a delayed event for A
/// never touches B's exact-instance tabs/cache IDs after PID reuse.
fn on_app_terminated_exact_with(
    &mut self,
    identity: AppIdentity,
    mut clear_tabs_for_identity: impl FnMut(AppIdentity),
    mut invalidate_window: impl FnMut(WindowTarget),
    mut invalidate_app: impl FnMut(AppIdentity),
) {
    // Tab windows are not represented in TilingState. Clear the exact
    // identity's tab records even when this app has no tracked windows.
    clear_tabs_for_identity(identity);

    // Application elements are keyed by exact identity. Invalidate A even if
    // it has no tracked windows; never invalidate another app sharing its PID.
    invalidate_app(identity);

    // 1. Collect windows to remove (exact identity match only).
    let window_ids: Vec<u32> = self.state.windows.iter()
        .filter(|w| w.identity.as_ref() == Some(&identity))
        .map(|w| w.id)
        .collect();

    if window_ids.is_empty() {
        // No windows tracked — just remove registry entry, no layout change
        self.registry.remove(&identity);
        return;
    }

    // 2. For each exact-instance tracked window, invalidate its AX cache entry
    // by exact target. Tabs were already cleared by exact identity above. Never
    // call clear_tabs_for_pid or invalidate_app: PID may belong to replacement B.
    for wid in &window_ids {
        invalidate_window(WindowTarget { identity, window_id: *wid });
    }

    // 3. Remove each window from workspace window lists, focus history, and
    // update focus/layout. Exact-instance B references must remain.
    let mut affected_workspaces: HashSet<Uuid> = HashSet::new();
    for wid in &window_ids {
        self.state.remove_window_from_focus_history(*wid);
        if let Some(ws_id) = self.state.get_window(*wid).map(|w| w.workspace_id) {
            affected_workspaces.insert(ws_id);
            self.state.update_workspace(ws_id, |ws| {
                // Preserve the focused window by ID. Retaining an earlier
                // entry shifts indices and must not silently focus a sibling.
                let focused_window_id = ws.focused_window_index
                    .and_then(|index| ws.window_ids.get(index).copied());
                ws.window_ids.retain(|id| *id != *wid);
                ws.focused_window_index = focused_window_id
                    .and_then(|focused_id| {
                        ws.window_ids.iter().position(|id| *id == focused_id)
                    })
                    .or_else(|| (!ws.window_ids.is_empty()).then_some(0));
            });
        }
    }

    // 4. Clear focus if any removed window was focused
    let current_focus = eyeball::Observable::get(&self.state.focus);
    if current_focus.focused_window_id.is_some_and(|fid| window_ids.contains(&fid)) {
        self.state.clear_focus();
    }

    // 5. Remove windows from state
    for wid in &window_ids {
        self.state.remove_window(*wid);
    }

    // 6. Notify subscriber to recompute layouts for affected workspaces
    if let Some(handle) = crate::modules::tiling::init::get_subscriber_handle() {
        for ws_id in &affected_workspaces {
            handle.notify_layout_changed(*ws_id, false);
        }
    }

    // 7. Remove from registry
    self.registry.remove(&identity);
}
```

Add a focused-index regression test in the same actor test module. Create one
workspace with `window_ids = [1, 2, 3]`, `focused_window_index = Some(1)`, A
owning window 1, and B owning windows 2 and 3. Terminate A exactly. Assert the
remaining IDs are `[2, 3]`, the focused index is `Some(0)`, and the focused ID
is still window 2. Keep this separate from the same-PID A/B resource test so an
index-shift regression has one unambiguous failure.

Add a read-only helper to `window_ops.rs` that validates identity equality
before returning isHidden — it never calls hide/unhide:

```rust
// In window_ops.rs
/// Returns the OS-level hidden state for an exact identity.
/// Validates identity on the same local NSRunningApplication object before
/// reading isHidden. Never mutates app state — lifecycle handlers use this.
/// Returns None if the app cannot be found or identity does not match.
///
/// # Autorelease safety
///
/// Creates its own autorelease pool because this is called from the actor's
/// task thread (no guarantee of an existing pool). The `objc::rc::autoreleasepool`
/// helper ensures autoreleased NSRunningApplication objects are drained before
/// returning.
#[must_use]
pub fn app_instance_is_hidden(identity: AppIdentity) -> Option<bool> {
    objc::rc::autoreleasepool(|| {
        unsafe {
            let app_class = objc::runtime::Class::get("NSRunningApplication")?;
            let app: *mut objc::runtime::Object =
                msg_send![app_class, runningApplicationWithProcessIdentifier: identity.pid];
            if app.is_null() {
                return None;
            }
            let actual = AppIdentity::from_ns_running_app(app)?;
            if actual != identity {
                return None; // PID-reuse or process mismatch
            }
            let is_hidden: BOOL = msg_send![app, isHidden];
            Some(is_hidden == YES)
        }
    })
}
```

Update the actor message handlers to use `app_instance_is_hidden` only
(read-only query, never hide/unhide while consuming lifecycle notifications):

```rust
StateMessage::AppShown { identity, pid: _ } => {
    // Read-only OS query — never hide/unhide during lifecycle handling.
    let os_hidden = app_instance_is_hidden(identity);
    self.on_app_shown_revalidated(identity, |_| os_hidden);
}
StateMessage::AppHidden { identity, pid: _ } => {
    let os_hidden = app_instance_is_hidden(identity);
    self.on_app_hidden_revalidated(identity, os_hidden);
}
StateMessage::AppTerminated { identity, pid: _ } => {
    self.on_app_terminated_exact(identity);
}
```

> **PID-reuse protection in terminated handling:** The identity was captured
> at event-forwarding time (by `extract_app_info` in `app_monitor.rs`). The
> actor removes only that exact identity. A new process that inherits the
> same PID but has a different launch date is unaffected.

Remove the old PID-only `actor/handlers/app.rs::on_app_terminated(state, pid)`
cleanup path (and its PID-wide tab/cache calls) from production dispatch. The
only termination cleanup path is `StateActor::on_app_terminated_exact(identity)`
with exact-identity tab clearing plus exact tracked-window cache invalidation.
Tab cleanup runs even when the terminated identity has no tracked windows.
Keep or rewrite tests so A and B share a PID but differ in launch date, use
untracked tab IDs for both identities, and assert B's tabs, cache, workspace
membership, focus history, focus, and state remain after delayed A termination.

- [ ] **Step 5: Build and run tests**

```bash
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo test -p stache --lib modules::tiling::actor::handlers::app::tests
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::visibility::tests
cargo fmt --all -- --check
cargo check -p stache
```

Expected: generation/FIFO classifier tests removed; identity-based tests pass.
The `classify_shown_with_state` tests from the old visibility.rs are deleted —
replaced by the actor revalidation tests.

- [ ] **Step 6: Commit**

```bash
git add app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/init.rs \
  app/native/src/modules/tiling/actor/handlers/app.rs \
  app/native/src/modules/tiling/actor/handlers/window.rs \
  app/native/src/modules/tiling/actor/handlers/workspace.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/state/tiling_state.rs \
  app/native/src/modules/tiling/effects/window_ops.rs \
  app/native/src/modules/tiling/visibility.rs \
  app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): atomically cut visibility ownership to actor registry"
```

---

### Thread-safety and deadlock constraints

Key invariants that must not be violated anywhere in Phase 6:

| Thread/Context                                         | What runs there                                                                        | Registry mutex?                                                       |
| ------------------------------------------------------ | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Actor's spawned task                                   | `handle_hide_for_workspace`, `handle_unhide_for_workspace`, `on_app_shown_revalidated` | Yes — held across OS call + insert/remove                             |
| Shutdown caller (Tauri main thread via `cleanup_once`) | `seal_and_drain()`                                                                     | Yes — single lock acquisition                                         |
| AX callback (main thread CFRunLoop)                    | Event handling + `WindowEvent` construction                                            | No — only reads identity from `OBSERVER_STATE` (brief `HashMap::get`) |
| NSWorkspace notification (main thread)                 | `on_app_launched`/`on_app_terminated` → `EventProcessor` → actor message               | No registry access                                                    |
| Signal handler (`ctrlc` thread)                        | Schedules work on Tauri main thread                                                    | No — only dispatches via `app.run_on_main_thread`                     |

**Deadlock rules:**

- The actor's task thread holds the registry mutex across OS calls. Ensure no
  OS call on that path synchronously dispatches main-thread work that could
  attempt to acquire the same mutex (NSRunningApplication hide/unhide is safe:
  it does not perform main-thread callbacks).
- The AX callback MUST NOT hold any mutex that the actor takes — but currently
  it only reads `OBSERVER_STATE` (a different mutex) briefly.
- `seal_and_drain` is called BEFORE the actor teardown, while the actor
  message loop is still active. The actor writes through the registry's
  closure APIs, not through the mutex directly — this prevents races
  during the seal-and-drain window.

### Task 19F: Shutdown restoration via registry seal/drain

**Files:**

- Test: `app/native/src/modules/tiling/visibility.rs` — add exhaustive tests for the restoration code already committed by Task19E
- Test: `app/native/src/modules/tiling/init.rs` — pause restores before actor
  teardown, repeated terminal cleanup is empty/idempotent, and resume publishes
  a fresh unsealed registry generation
- `app/native/src/modules/tiling/mod.rs` — unchanged; the existing public re-export remains valid
- `app/native/src/app_shutdown.rs` — unchanged (still calls `tiling::restore_stache_hidden_apps()`)

- [ ] **Step 1: Write restoration validation tests**

```rust
// In visibility.rs tests
fn bits(v: u64) -> LaunchDateBits {
    LaunchDateBits::from_time_interval_since_reference_date(v as f64).unwrap()
}

#[test]
fn restore_proves_identity_equality_before_action() {
    // Simulate PID-reuse: identity A (old launch date) vs identity A'
    // at the same PID (new launch date). The restore function checks
    // equality BEFORE performing the action.
    let identity_a = AppIdentity { pid: 42, launch_date: bits(100) };
    let identity_a_reused = AppIdentity { pid: 42, launch_date: bits(200) };

    let mut actions = Vec::new();

    let restored = restore_one_exact_with(
        identity_a,
        |_| Some(FakeApp { identity: identity_a_reused, hidden: true }),
        |app| Some(app.identity),
        |app| Some(app.hidden),
        |app| { actions.push(app.identity); true },
    );

    assert!(!restored);
    assert_eq!(actions.len(), 0, "no action performed for mismatched identity");
}

#[test]
fn restore_skips_same_pid_replacement_for_stale_owned_identity() {
    let identity_a = AppIdentity { pid: 42, launch_date: bits(100) };
    let identity_b = AppIdentity { pid: 42, launch_date: bits(200) };
    let registry = VisibilityRegistry::default();
    registry.insert(identity_a); // Simulates a missed A termination event.

    let mut unhide_calls = 0;
    let summary = restore_from_with(registry.seal_and_drain(), |owned| {
        restore_one_exact_with(
            owned,
            |_| Some(FakeApp { identity: identity_b, hidden: true }),
            |app| Some(app.identity),
            |app| Some(app.hidden),
            |_| { unhide_calls += 1; true },
        )
    });

    assert_eq!(summary, RestoreSummary { attempted: 1, restored: 0 });
    assert_eq!(unhide_calls, 0, "replacement process must remain untouched");
}

#[test]
fn restore_proves_correct_identity_is_unhidden() {
    let identity_a = AppIdentity { pid: 10, launch_date: bits(1) };
    let identity_b = AppIdentity { pid: 20, launch_date: bits(2) };

    let mut actions = Vec::new();
    let summary = restore_from_with(vec![identity_a, identity_b], |identity| {
        restore_one_exact_with(
            identity,
            |_| Some(FakeApp { identity, hidden: true }),
            |app| Some(app.identity),
            |app| Some(app.hidden),
            |app| { actions.push(app.identity); true },
        )
    });

    assert_eq!(summary.attempted, 2);
    assert_eq!(summary.restored, 2);
    assert_eq!(actions, vec![identity_a, identity_b]);
}

#[test]
fn restore_skips_not_hidden_without_unhide() {
    let identity = AppIdentity { pid: 10, launch_date: bits(1) };
    let mut actions = Vec::new();
    assert!(!restore_one_exact_with(
        identity,
        |_| Some(FakeApp { identity, hidden: false }),
        |app| Some(app.identity),
        |app| Some(app.hidden),
        |app| { actions.push(app.identity); true },
    ));
    assert!(actions.is_empty());
}

#[test]
fn restore_attempts_later_identities_after_failure() {
    let first = AppIdentity { pid: 10, launch_date: bits(1) };
    let second = AppIdentity { pid: 20, launch_date: bits(2) };
    let mut calls = Vec::new();
    let summary = restore_from_with(vec![first, second], |identity| {
        restore_one_exact_with(
            identity,
            |_| Some(FakeApp { identity, hidden: true }),
            |app| Some(app.identity),
            |app| Some(app.hidden),
            |app| { calls.push(app.identity); app.identity == second },
        )
    });
    assert_eq!(summary, RestoreSummary { attempted: 2, restored: 1 });
    assert_eq!(calls, vec![first, second]);
}

#[test]
fn empty_identities_list_is_no_op() {
    let summary = restore_from_with(vec![], |_| unreachable!());
    assert_eq!(summary, RestoreSummary { attempted: 0, restored: 0 });
}

#[test]
fn restore_works_through_registry_seal_and_drain() {
    let registry = VisibilityRegistry::default();
    registry.insert(AppIdentity { pid: 10, launch_date: bits(1) });
    registry.insert(AppIdentity { pid: 20, launch_date: bits(2) });
    let identities = registry.seal_and_drain();

    let mut restored: Vec<AppIdentity> = Vec::new();
    let summary = restore_from_with(identities, |identity| {
        restore_one_exact_with(
            identity,
            |_| Some(FakeApp { identity, hidden: true }),
            |app| Some(app.identity),
            |app| Some(app.hidden),
            |app| { restored.push(app.identity); true },
        )
    });

    assert_eq!(summary.attempted, 2);
    assert_eq!(summary.restored, 2);
    assert!(registry.is_empty());
}
```

In `init.rs` tests, use injected restore/teardown hooks rather than live AppKit:

1. Start generation 1, seed its registry with identity A, pause, and assert the
   restore hook sees A before any actor-shutdown hook executes.
2. Invoke terminal cleanup after that pause and assert no second identity is
   restored.
3. Resume generation 2 and assert its registry is a distinct Arc, is unsealed,
   accepts identity B, and does not contain A.
4. Force a completion timeout, assert generation 1 remains quarantined with its
   already-drained registry, then retry teardown and assert restoration remains
   idempotent.

- [ ] **Step 2: Verify the atomic Task19D seal/drain restore**

Do not add duplicate definitions in this task. The functions in the following
block were implemented in Task19D in the same commit as the hide hot-path
cutover; this block is the exact final implementation to audit while adding the
Task19F tests. The registry is obtained at shutdown through the already-existing static
handle. No `let registry = ...` intermediate, no static ambiguity, no actor
channel query — the handle exists because `tiling::shutdown` has not been
called yet:

```rust
/// Restores every owned identity with exact-instance validation.
///
/// Seals the registry, drains all identities, then validates and unhides
/// each. Uses `init::get_handle()` to access the `seal_and_drain_visibility`
/// method on `StateActorHandle`. The handle must be alive because
/// restoration runs *before* `tiling::shutdown` closes the actor channel.
#[must_use]
pub fn restore_stache_hidden_apps() -> RestoreSummary {
    crate::modules::tiling::init::get_handle()
        .map(|h| {
            let identities = h.seal_and_drain_visibility();
            restore_from_with(identities, restore_one_exact)
        })
        .unwrap_or(RestoreSummary { attempted: 0, restored: 0 })
}

/// Narrow decision seam. One resolved app value flows through identity lookup,
/// hidden-state lookup, and unhide, so production can use one local
/// NSRunningApplication pointer and tests execute the same ordering logic.
#[must_use]
fn restore_one_exact_with<T>(
    owned: AppIdentity,
    resolve: impl FnOnce(i32) -> Option<T>,
    identity_of: impl FnOnce(&T) -> Option<AppIdentity>,
    hidden_of: impl FnOnce(&T) -> Option<bool>,
    unhide: impl FnOnce(&T) -> bool,
) -> bool {
    let Some(app) = resolve(owned.pid) else {
        return false;
    };
    if identity_of(&app) != Some(owned) {
        return false;
    }
    if hidden_of(&app) != Some(true) {
        return false;
    }
    unhide(&app)
}

/// Production wrapper. The raw pointer exists only inside this autorelease
/// pool and this synchronous call; it is never stored or sent across threads.
#[must_use]
fn restore_one_exact(owned: AppIdentity) -> bool {
    use objc::runtime::{BOOL, YES};
    use objc::{msg_send, sel, sel_impl};

    objc::rc::autoreleasepool(|| {
        restore_one_exact_with(
            owned,
            |pid| unsafe {
                let class = objc::runtime::Class::get("NSRunningApplication")?;
                let app: *mut objc::runtime::Object =
                    msg_send![class, runningApplicationWithProcessIdentifier: pid];
                (!app.is_null()).then_some(app)
            },
            |app| unsafe { AppIdentity::from_ns_running_app(*app) },
            |app| unsafe {
                let hidden: BOOL = msg_send![*app, isHidden];
                Some(hidden == YES)
            },
            |app| unsafe {
                let result: BOOL = msg_send![*app, unhide];
                result == YES
            },
        )
    })
}

This is the exact-instance validation boundary: `restore_one_exact_with`
performs equality and hidden checks before calling `unhide`, while production
uses the same resolved object for every step. PID reuse between separate
lookups is therefore impossible.

/// Public restoration helper retained for Task20 and tests. It uses an
/// explicit loop so `FnMut(AppIdentity)` is called with an owned identity and
/// no `&AppIdentity`/`FnMut` iterator mismatch can occur. Every identity is
/// attempted even when an earlier restore fails.
#[must_use]
pub fn restore_from_with(
    identities: Vec<AppIdentity>,
    restore: impl FnMut(AppIdentity) -> bool,
) -> RestoreSummary {
    let attempted = identities.len();
    let mut restore = restore;
    let mut restored = 0;
    for identity in identities {
        let did_restore = restore(identity);
        tracing::debug!(
            pid = identity.pid,
            launch_date_bits = identity.launch_date.bits(),
            restored = did_restore,
            "shutdown restore result for Stache-owned application"
        );
        if did_restore {
            restored += 1;
        }
    }
    RestoreSummary { attempted, restored }
}
```

For `app_shutdown.rs`, the call `tiling::restore_stache_hidden_apps()` stays
unchanged — its internal implementation now uses the handle-based registry
access. Task20's `cleanup_once` function remains unchanged in structure.

- [ ] **Step 3: Build and run tests**

```bash
cargo test -p stache --lib modules::tiling::visibility::tests
cargo check -p stache
# Also run app_shutdown tests to confirm interface compatibility
cargo test -p stache --lib app_shutdown::tests
```

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/tiling/visibility.rs
git add app/native/src/modules/tiling/init.rs
git commit -m "test(tiling): verify exact registry shutdown restoration"
```

---

### Task 19G: Delete obsolete code and simplify

**Files:**

- Modify: `app/native/src/modules/tiling/visibility.rs` — remove generation/FIFO/ShownClassification
- `app/native/src/modules/tiling/events/processor.rs` — already cleaned in 19E
- Delete stale test references in other modules

- [ ] **Step 1: Remove deleted items from visibility.rs**

Delete these definitions and all related tests:

- `ShownClassification` enum
- `PidState` struct
- `PidState::is_zombie`
- All generation/FIFO fields in `TrackerState`; `sealed`/`owned` exist only in
  the replacement `RegistryState` from 19C
- `TrackerState::next_generation`, `log_seq`
- `HiddenAppTracker::hide_with` (the old one — replaced by actor-bound version)
- `HiddenAppTracker::unhide_with_outcome`
- `HiddenAppTracker::classify_shown_with`
- `HiddenAppTracker::classify_shown_with_state`
- `HiddenAppTracker::forget_terminated`
- `HiddenAppTracker::begin_shutdown_and_drain` (replaced by VisibilityRegistry)
- `HiddenAppTracker` struct itself (replaced by VisibilityRegistry)
- All generation/FIFO/OS-state-classifier tests

Keep:

- `RestoreSummary`
- `restore_stache_hidden_apps` (public API preserved, now uses `restore_from_with` internally)
- `restore_from_with` (replaces `restore_stache_hidden_apps_from`)
- New `VisibilityRegistry` (created in 19C)
- New identity-based tests

Result: `visibility.rs` goes from ~1268 lines to ~200-300 lines of clean
registry + restore code.

- [ ] **Step 2: Clean processor.rs imports**

Verify no `ShownClassification`, `classify_stache_hidden_app`, or
`forget_stache_hidden_app_terminated` remain. The `on_app_shown_with` seam
and its tests are removed.

- [ ] **Step 3: Remove bare-PID visibility APIs from window_ops.rs**

Retain `HideAppOutcome` and `UnhideAppOutcome`, plus only the exact-instance
helpers introduced by Task19D (`app_instance_is_hidden`,
`hide_app_instance_with_outcome`, and `unhide_app_instance_with_outcome`).
Delete the bare-PID production APIs `hide_app_with_outcome`,
`unhide_app_with_outcome`, `hide_app`, `unhide_app`, and `app_is_hidden`; after
the atomic actor cutover they have no production caller and would reintroduce
PID-reuse risk. Update or remove their old tests rather than keeping public
compatibility shims. Restoration continues through
`restore_stache_hidden_apps()` and the exact `restore_one_exact` seam.

- [ ] **Step 4: Build with full project lint**

```bash
cargo test -p stache --lib 2>&1 | tail -20
cargo clippy -p stache --lib -- -D warnings 2>&1 | tail -20
cargo fmt --all -- --check 2>&1 | tail -20
cargo check -p stache 2>&1 | tail -20
pnpm test
pnpm lint
pnpm format
```

Expected: all pass. Clippy may flag removed generational code — address each.
Run project-wide lint only after focused Cargo checks pass.

> **Protected files:** No changes to `app/native/src/modules/audio/device.rs`
> (unrelated audio code) or any file outside the Tasks 19A-19G change set.
> Phase 6 intentionally integrates with Task15's pause/resume runtime in
> `init.rs`; no other Tasks9-18 behavior is changed.

- [ ] **Step 5: Commit**

```bash
git add app/native/src/modules/tiling/visibility.rs \
  app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/effects/window_ops.rs
git commit -m "refactor(tiling): remove obsolete generation/FIFO/ShownClassification"
```

---

### Superseded commits note

Commits `f2e20c8..95fa2f5` implemented the per-PID generation/FIFO state
machine (`ShownClassification`, `PidState`, `classify_stache_hidden_app`,
`forget_stache_hidden_app_terminated`, `TrackerState::next_generation`,
etc.) and the FIFO generation-based restore. These are superseded by Tasks 19A-19G.

The detailed per-call `HideAppOutcome`/`UnhideAppOutcome` types from those
commits survive — they are useful for actor-owned exact-instance hide/unhide.
Bare-PID wrappers do not survive.
The overall `RestoreSummary` and `restore_stache_hidden_apps` public API
surface is preserved with updated internals.

Manual Task21 verification should now include rapid A→B→A→B (alternating
hide/unhide between two applications) and explicit PID-reuse validation
evidence if practical (e.g., terminating and relaunching an app while it
is owned, then confirming restoration targets the new instance correctly).

### Task 20: Centralize orderly shutdown and wire every supported exit path

**Status: COMPLETED / HISTORICAL.** The CAS-based terminal-request arbiter,
`cleanup_once`, signal handler, and all exit-path wiring (`tray/mod.rs`,
`config/watcher.rs`, `bar/ipc_listener.rs`) are implemented in
`app/native/src/app_shutdown.rs` (commits `fb3fe1b`, `12d913e` and earlier).
No further changes needed for this task.

Task 19D changes the internals of `tiling::restore_stache_hidden_apps()`
(visibility.rs) atomically with the hide hot-path cutover; Task19F adds the
exact restoration tests behind the existing public API surface. The cleanup order
(restore → tiling shutdown → IPC shutdown) and idempotency are already
enforced by `cleanup_once` — no `app_shutdown.rs` edits are required for
Phase 6.

> **Do not modify `app_shutdown.rs` in Tasks 19A–19G.** The CAS arbiter,
> signal escalation, and exit-path wiring through commit `fb3fe1b` are
> stable and must not be overwritten.

### Task 21: Manually verify supported shutdown paths

**Files:** none

- [ ] **Step 1: Prepare distinguishable application states**

Run `pnpm tauri:dev`, place one application's windows only on a non-visible Stache
workspace so Stache hides that application, minimize a window from another application,
and manually hide a third application before Stache attempts to manage it.

Also prepare the two conservative-behavior sequences:

1. **Actor observes visible before manual rehide:** Let Stache hide App A. Show it
   manually (AppShown fires, actor sees OS visible → removes ownership). Immediately
   hide it again manually (Stache has no ownership → the app remains hidden after
   shutdown because Stache never re-acquired ownership). Verification: on shutdown,
   `attempted` does not include App A.

2. **External show then external rehide before AppShown is consumed — ownership
   retained:** This timing is not deterministically controllable in a manual
   production run because Task21 adds no debug pause/barrier. Exercise it only
   as best-effort observation: let Stache hide App B, externally/user-show B,
   then immediately externally/user-rehide B without another Stache workspace
   hide. If the actor consumes `AppShown` after the rehide, its read-only
   `isHidden` query observes hidden, retains ownership under the approved
   Keep-hidden policy, and shutdown restores B. The deterministic proof is the
   Task19E unit test
   `queued_app_shown_consumed_after_external_rehide_retains_owner`, which injects
   hidden OS state and verifies ownership is retained without any active
   hide/unhide call. Record manual results as observational, not guaranteed.

3. **PID reuse, delivered termination:** Let Stache hide App C. Terminate App C
   normally and allow its exact termination event to be processed. Launch a
   different app that acquires the same PID. Shut Stache down. Verification:
   App C's identity is absent from the drained registry, so `attempted` does
   not include it and the replacement remains untouched. The separate missed
   termination case is covered below with an injected stale identity.

- [ ] **Step 2: Verify normal tray quit**

Choose “Quit Stache.” Confirm the Stache-hidden application becomes visible, while the
minimized window remains minimized and the independently hidden application remains
hidden.

- [ ] **Step 3: Verify every reload/restart entry point in a release build**

Build and launch the release app. From a fresh process/state for each case, hide an
application through a workspace switch and trigger:

1. Tray “Reload Stache”.
2. A config file change handled by `config/watcher.rs`.
3. CLI `stache reload`, which reaches `bar/ipc_listener.rs`.

Confirm restoration occurs before the replacement process starts in all three
cases. Use the per-identity debug event from `visibility.rs` (PID,
`launch_date_bits`, and `restored`) to identify the exact application instance
rather than relying only on aggregate counts.

- [ ] **Step 4: Verify SIGTERM and SIGINT**

For each signal, start from a fresh process and repeat the hidden-state setup:

```bash
kill -TERM "$(pgrep -x stache)"
kill -INT "$(pgrep -x stache)"
```

Confirm the process exits after restoring only Stache-owned hides. Check both
the aggregate `attempted`/`restored` counts and each per-identity restore result.

- [ ] **Step 5: PID-reuse validation (two scenarios)**

Two distinct PID-reuse scenarios must be validated:

1. **Normal termination, same-PID replacement:** Terminate App A that Stache
   has hidden. After App A terminates, launch App B that inherits the same PID
   (or wait for a system process to reuse it). After the delivered exact
   termination removes A from the registry, shut Stache down and confirm A is
   absent from `attempted`; B is never considered for restoration. Unit test:
   `delayed_termination_removes_only_exact_instance_resources` proves exact A
   resource removal while preserving B.

2. **Missed termination, same-PID replacement:** Seed stale owned identity A
   in a local `VisibilityRegistry` without delivering termination, then have
   the restoration seam resolve fake App B with the same PID and a different
   launch date. Drain and restore. Assert `attempted == 1`, `restored == 0`,
   and the unhide callback is never called. This deterministic
   `restore_skips_same_pid_replacement_for_stale_owned_identity` unit test
   proves the lost-event boundary; B's events and resources remain keyed by
   identity B and are untouched.

A manual variant for case 1: terminate a Stache-hidden app, allow the
termination event to be processed, note its PID and launch-date bits, launch
another app (or wait for reuse), then trigger Stache shutdown and verify no
per-identity restore event is emitted for the old identity. Case 2 is covered
deterministically by the stale-owned
identity restoration test because intentionally dropping an NSWorkspace event
is not a reliable manual procedure.

- [ ] **Step 6: Confirm and document the SIGKILL boundary**

Run `kill -KILL "$(pgrep -x stache)"` only after the supported-path checks. Confirm no
cleanup log is emitted. This is the expected POSIX limitation: SIGKILL cannot execute
in-process restoration and is not a failure of the implementation.

- [ ] **Step 7: Report results** — verification only. No commit.

---

## Verification Summary

| Phase | Verification                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Manual: menubar reaction ≤200ms (Task 3). Build passes.                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 2     | Manual: tracing spans emit under debug default (Task 8). No behavior change; existing tests pass (`pnpm test`).                                                                                                                                                                                                                                                                                                                                                                     |
| 3     | Blocked pending Phase 2 evidence; each confirmed fix gets a failing test → pass → commit.                                                                                                                                                                                                                                                                                                                                                                                           |
| 4     | Build passes; `pnpm test` passes; manual: each module's OS resource actually releases/reacquires (event tap disabled, listener removed, tiling `reset()` clears guard so resume succeeds).                                                                                                                                                                                                                                                                                          |
| 5     | Manual tray interaction (Task 18): toggle states correct, config-off locked, unavailable shows reason, restart resets.                                                                                                                                                                                                                                                                                                                                                              |
| 6     | Unit: AppIdentity capture/validation, registry seal/drain/concurrency, identity-keyed event forwarding, actor revalidation (hidden/visible/mismatch), exact-instance restoration validation, deterministic delayed/duplicate/missing/PID-reuse tests. PID-reuse: hide-rejected for old identity (fail-closed), restore does not unhide wrong process. Manual: tray quit, reload/restart, SIGTERM, and SIGINT restore only Stache-hidden applications; SIGKILL limitation confirmed. |

## Open Items / Risks

- **Phase 3 is blocked on Phase 2 evidence.** Do not write Phase 3 tasks until the logs from Task 8 are reviewed.
- **proxyAudio listener removal (Task 13)** is the highest-risk change — callback reference and `client_data` pointer MUST match the registration exactly.
- **tiling `INITIALIZED` change (Task 15)** converts a `OnceLock<bool>` to `Mutex<bool>`; all read/write sites in `init()` must be updated consistently.
- **Tray `CheckMenuItem` handle retention (Task 17)** may need a `TrayMenuState` managed struct if immediate state reflection is required; the minimal version relies on macOS re-querying the menu on open.
- **AppIdentity ObjC safety.** `NSRunningApplication` objects must never cross threads; identity extraction is a one-shot operation that saves only the `Send + Sync` bytes. AX callbacks must never lock the `VisibilityRegistry` (they briefly lock `OBSERVER_STATE` — a separate mutex — for identity lookup). The actor must never dispatch main-thread work while holding the `VisibilityRegistry` mutex.
- **SIGKILL cannot be handled in-process.** Phase 6 intentionally provides best-effort cleanup for orderly exits, explicit restart paths, SIGTERM, and SIGINT only.
