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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

- [ ] **Step 2: Confirm reaction is ≤200ms**

The debug build already logs at `stache=debug`. Watch the menubar visibility event emission timing in the terminal. The reaction should now be near-instant (≤100ms poll + event dispatch). If it is still >200ms, re-check that `query_menu_bar_visible_via_nsmenu` is actually being called (add a temporary `tracing::debug!` around it, then remove it).

- [ ] **Step 3: Commit nothing** — verification only. Report result.

---

## Phase 2 — Diagnostic Tracing (No Fixes)

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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

- [ ] **Step 3: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/services/lifecycle.rs app/native/src/modules/services/mod.rs
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add app/native/src/modules/menu_anywhere/event_monitor.rs app/native/src/modules/menu_anywhere/mod.rs
git commit -m "feat(menuAnywhere): retain tap handle + implement LifecycleModule"
```

### Task 15: Add tiling `reset()` + implement trait

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs`

- [ ] **Step 1: Add `reset()` that clears the init guard**

After `shutdown()` (init.rs:175-184), add:

```rust
/// Clears the init guard and stops the actor/processor so the module can be
/// re-initialized by `resume()`. Called only from the LifecycleModule impl.
pub fn reset() {
    shutdown();
    let mut initialized = INITIALIZED.lock().unwrap();
    *initialized = false;
}
```

> **Important:** `INITIALIZED` must be changed from `OnceLock<bool>` to `Mutex<bool>` (see Step 2) so `reset()` can clear it. `OnceLock::set` would fail on a second call.

- [ ] **Step 2: Change `INITIALIZED` to a clearable type**

In `init.rs:63`, replace:

```rust
static INITIALIZED: OnceLock<bool> = OnceLock::new();
```

with:

```rust
static INITIALIZED: Mutex<bool> = Mutex::new(false);
```

Update all `INITIALIZED.get().is_some()` / `INITIALIZED.set(...)` usages in `init()` (init.rs:124, 134, 140, 156, 162) to `INITIALIZED.lock().unwrap()` reads/writes. For example, the guard at line 124 becomes:

```rust
if *INITIALIZED.lock().unwrap() {
    tracing::warn!("tiling: already initialized");
    return false;
}
```

and each `let _ = INITIALIZED.set(value);` becomes `*INITIALIZED.lock().unwrap() = value;`.

- [ ] **Step 3: Implement `LifecycleModule` for tiling**

At the end of `init.rs`:

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

impl LifecycleModule for Tiling {
    fn name(&self) -> &'static str { "Tiling" }
    fn id(&self) -> &'static str { "tiling" }

    fn start(&self) -> Result<(), String> {
        if !init(self.app_handle.clone()) {
            return Err("tiling failed to start (check accessibility permission)".to_string());
        }
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        shutdown();
        Ok(())
    }

    fn resume(&self) -> Result<(), String> {
        reset();
        if !init(self.app_handle.clone()) {
            return Err("tiling failed to resume".to_string());
        }
        Ok(())
    }

    fn status(&self) -> ModuleStatus {
        if !config::get_config().tiling.is_enabled() {
            return ModuleStatus::ConfiguredOff;
        }
        if *INITIALIZED.lock().unwrap() {
            ModuleStatus::Running
        } else {
            ModuleStatus::Paused
        }
    }
}
```

> Adapt `Tiling` struct / `app_handle` field / `config` access to the actual file. If `tiling` is module functions, wrap in a `TilingLifecycle` struct holding the `AppHandle`.

- [ ] **Step 4: Build**

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add app/native/src/modules/tiling/init.rs
git commit -m "feat(tiling): add reset() + implement LifecycleModule"
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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

Run: `cd /Users/marcosmoura/Projects/stache && cargo build -p native 2>&1 | tail -20`
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
use std::cmp::Ordering;
use std::fmt;

/// Stable application identity combining PID and launch date.
///
/// Prevents PID-reuse races: after an app terminates the kernel may reuse
/// its PID for a different process. Binding ownership to `(pid, launch_date)`
/// ensures we never restore a wrong process that inherited the same PID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AppIdentity {
    pub pid: i32,
    pub launch_date: LaunchDateBits,
}

/// High-precision launch-date bits from
/// `NSRunningApplication.launchDate.timeIntervalSinceReferenceDate`.
///
/// Stored as `f64::to_bits` for `Send + Sync + Copy`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
        use objc::{msg_send, sel, sel_impl};
        use objc::runtime::{Object, BOOL, YES};

        if app.is_null() {
            return None;
        }

        let pid: i32 = msg_send![app, processIdentifier];
        if pid <= 0 {
            return None;
        }

        let launch_date: *mut Object = msg_send![app, launchDate];
        if launch_date.is_null() {
            return None;
        }

        let interval: f64 = msg_send![launch_date, timeIntervalSinceReferenceDate];
        let bits = LaunchDateBits::from_time_interval_since_reference_date(interval)?;

        Some(Self { pid, launch_date: bits })
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

- [ ] **Step 5: Add restore validation helpers to AppIdentity**

Add to `impl AppIdentity`:

```rust
impl AppIdentity {
    /// Fetches a fresh `NSRunningApplication` by PID, extracts identity from
    /// the same object, requires identity equality, checks the app is hidden,
    /// and calls `unhide` — all on the same one-shot local reference.
    ///
    /// Returns `true` if the app was successfully unhidden.
    ///
    /// # Safety
    ///
    /// Must be called on a thread with a valid ObjC runtime.
    #[must_use]
    pub unsafe fn restore_with_exact_validation(self) -> bool {
        use objc::{msg_send, sel, sel_impl};
        use objc::runtime::{Class, Object, BOOL, YES};

        let Some(app_class) = Class::get("NSRunningApplication") else {
            return false;
        };
        let app: *mut Object = msg_send![app_class, runningApplicationWithProcessIdentifier: self.pid];
        if app.is_null() {
            return false;
        }
        // Validate identity from this exact object
        let Some(actual) = Self::from_ns_running_app(app) else {
            return false;
        };
        if actual != self {
            return false; // PID-reuse or process mismatch
        }
        let is_hidden: BOOL = msg_send![app, isHidden];
        if is_hidden != YES {
            return false; // already visible, not our problem
        }
        let result: BOOL = msg_send![app, unhide];
        result == YES
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add app/native/src/modules/tiling/identity.rs app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): add AppIdentity with capture/validation"
```

---

### Task 19B: Propagate `AppIdentity` into Window, events, and StateMessage

**Files:**

- Modify: `app/native/src/modules/tiling/state/types.rs:308` — add `pub identity: AppIdentity` to `Window`
- Modify: `app/native/src/modules/tiling/actor/messages.rs:271` — add `pub identity: AppIdentity` to `WindowCreatedInfo`
- Modify: `app/native/src/modules/tiling/actor/messages.rs:54-68` — add identity to `AppLaunched`, `AppTerminated`, `AppHidden`, `AppShown`
- Modify: `app/native/src/modules/tiling/events/types.rs:139` — add `pub identity: AppIdentity` to `WindowEvent`
- Modify: `app/native/src/modules/tiling/events/ax_observer.rs` — capture identity at observer registration; callback copies stored identity
- Modify: `app/native/src/modules/tiling/events/app_monitor.rs` — capture identity from `NSWorkspaceApplicationKey` object, not fresh PID lookup
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

- [ ] **Step 4: Capture identity in AX observer registration**

All current `observer.rs` symbols used here: `OBSERVER_STATE` (type
`parking_lot::Mutex<Option<ObserverState>>`), `ObserverState` (has
`observers: HashMap<i32, ObserverRef>` by PID), `ObserverRef` (`*mut c_void`
wrapping `AXObserverRef`), `add_observer_for_pid` (at line ~250).
Callback at `ax_observer_callback` (line ~329) receives `observer: AXObserverRef`
and `refcon: *mut c_void` (carries bare PID). No fictional function names used.
`AXObserverRef` is the C `AXObserverRef` type alias.

Keyed by PID is a PID-reuse race: after the original process terminates the
kernel may reuse its PID for a different process. The callback already receives
the `AXObserverRef` as its first argument — use that stable address instead.

Change `ObserverState::observers` to `HashMap<usize, ObserverRecord>` keyed by
`AXObserverRef` address (cast to `usize`). `ObserverRecord` pairs the observer
with its captured identity:

```rust
struct ObserverRecord {
    observer: ObserverRef,
    identity: AppIdentity,
}
```

In `add_observer_for_pid`, capture `AppIdentity` from the same
`NSRunningApplication` object used to construct the AX element (PID already
confirmed valid). Store under `observer_ref as usize`:

```rust
// In add_observer_for_pid, after pid is confirmed valid:
let identity = unsafe {
    let app: *mut Object = msg_send![
        class!(NSRunningApplication),
        runningApplicationWithProcessIdentifier: pid
    ];
    AppIdentity::from_ns_running_app(app)
};
let Some(identity) = identity else {
    tracing::warn!(pid, "ax_observer: skipping observer for pid with no valid identity");
    return false;
};
let record = ObserverRecord { observer, identity };
OBSERVER_STATE.lock().unwrap().as_mut().unwrap().observers.insert(observer as usize, record);
```

**Callback derives identity from its `observer` argument**, not from PID:

```rust
unsafe extern "C" fn ax_observer_callback(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: *const c_void,
    refcon: *mut c_void,
) {
    let key = observer as usize;
    let (identity, observer_ref) = {
        let state = OBSERVER_STATE.lock().unwrap();
        let Some(ref state) = *state else { return; };
        let Some(record) = state.observers.get(&key) else { return; };
        (record.identity, record.observer) // Copy identity; observer stored for removal
    };
    // The callback now uses `identity` (not PID) for WindowEvent construction
    // and `observer_ref` for any removal. PID from `refcon` is only used for
    // AX API compatibility, never for ownership decisions.
}
```

This avoids holding the lock for more than the short lookup; the identity value
is copied (`AppIdentity` is `Copy`), and the callback constructs `WindowEvent`
with it. **Exact removal** uses the same key: `state.observers.remove(&(observer as usize))`
and drops the `ObserverRef` value.

For `ax_observer.rs::handle_window_created`, the `WindowEvent` is built by the
callback and already carries the identity before reaching the handler.

**Locking:** `OBSERVER_STATE` uses `parking_lot::Mutex` (already true). The
callback holds it for one `HashMap::get` (microseconds). There is no registry
lock (`VisibilityRegistry` mutex) involved here — the registry is only locked
by the actor task and the shutdown path.

**Important thread note:** The AX callback runs on the main thread's CFRunLoop.
The `OBSERVER_STATE` mutex is held for the duration of a single `HashMap::get`
lookup (microseconds). This is safe: the callback is re-entrant only from
other AX events on the same main-thread runloop, and no other thread writes to
`OBSERVER_STATE` for this observer address while the callback runs.

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
accept `Option<AppIdentity>`. The adapter drops events with `None` identity
before forwarding to `EventProcessor`:

```rust
fn on_app_launched(&self, identity: Option<AppIdentity>, pid: i32, bundle_id: Option<String>, name: Option<String>) {
    let Some(identity) = identity else {
        tracing::trace!(pid, "app_monitor: dropping launch event (no identity)");
        return;
    };
    self.processor.on_app_launched(identity, pid, bundle_id, name);
}

fn on_app_terminated(&self, identity: Option<AppIdentity>, pid: i32, bundle_id: Option<&str>, name: Option<&str>) {
    let Some(identity) = identity else {
        tracing::trace!(pid, "app_monitor: dropping terminate event (no identity)");
        return;
    };
    self.processor.on_app_terminated(identity, pid);
}
```

On `None` identity the event is dropped. No fallback to bare-PID forwarding —
fail-closed per spec.

- [ ] **Step 6: Fix all call sites — staged for compile safety**

Because the plan uses `Option<AppIdentity>` on `Window` and `WindowCreatedInfo`,
intermediate states compile. Fix call sites in this order (each step compiles):

1. **Add field to types only** — add `identity: Option<AppIdentity>` to `Window`,
   `WindowCreatedInfo`. Add `identity: AppIdentity` (non-optional because always
   captured) to `WindowEvent` and the four lifecycle `StateMessage` variants.
   At this point all existing call sites still compile (`Window { identity: None, .. }`
   in `Window::default()`, etc.).

2. **Capture in AX observer** (Step 4 above) — `ObserverRecord` stores identity,
   `WindowEvent::new` receives it from the callback.

3. **Capture in app_monitor** (Step 5 above) — `extract_app_info` returns
   `Option<AppIdentity>`, `on_app_launched`/`on_app_terminated` forward it.

4. **Fill in remaining call sites**:
   - `ax_observer.rs::handle_window_created` — pass identity from observer record
   - `ax_observer.rs` AX callback handlers for AppHidden/AppShown
   - `app_monitor.rs::on_app_launched` → `processor.on_app_launched`
   - `app_monitor.rs::on_app_terminated` → `processor.on_app_terminated`
   - Initial window scan during tiling init — capture identity per window's PID
     from `NSRunningApplication.runningApplicationWithProcessIdentifier`

Each step can be verified with `cargo check` before proceeding.

- [ ] **Step 7: Build and run specific tests**

```bash
cargo check -p stache 2>&1 | tail -20
# Fix all type errors iteratively, then:
cargo test -p stache --lib modules::tiling::events::types::tests
cargo test -p stache --lib modules::tiling::actor::messages::tests
cargo fmt --all -- --check
cargo check -p stache
```

- [ ] **Step 8: Commit**

```bash
git add app/native/src/modules/tiling/state/types.rs \
  app/native/src/modules/tiling/actor/messages.rs \
  app/native/src/modules/tiling/events/types.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/events/app_monitor.rs \
  # plus any other call-site fixes
git commit -m "feat(tiling): propagate AppIdentity through Window, events, StateMessage"
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
fn id10_1() -> AppIdentity { AppIdentity { pid: 10, launch_date: bits(1) } }
fn id20_2() -> AppIdentity { AppIdentity { pid: 20, launch_date: bits(2) } }
fn id30_3() -> AppIdentity { AppIdentity { pid: 30, launch_date: bits(3) } }

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

    reg.seal();
    assert!(reg.sealed());

    // Late insert is a no-op (no panic)
    reg.insert(id30_3());
    assert_eq!(reg.len(), 2);

    let drained = reg.drain();
    assert_eq!(drained, vec![id1, id2]); // sorted by Ord
    assert!(reg.drain().is_empty()); // empty after drain
}

#[test]
fn registry_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let reg = Arc::new(VisibilityRegistry::default());
    let ids: Vec<_> = (0..100).map(|i| AppIdentity {
        pid: i,
        launch_date: bits(i),
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
    reg.seal();
    let drained = reg.drain();
    assert_eq!(drained.len(), 100);
}

#[test]
fn registry_no_op_after_close() {
    let reg = VisibilityRegistry::default();
    reg.insert(id10_1());
    reg.seal();
    // drain consumes all
    let _ = reg.drain();
    // handle seal then closed channel scenario: drain returns empty
    assert!(reg.drain().is_empty());
}
```

- [ ] **Step 2: Implement `VisibilityRegistry`**

```rust
use std::collections::BTreeSet;
use std::sync::Mutex;

use super::identity::AppIdentity;

/// Passive registry shared between `StateActor` and `StateActorHandle`.
///
/// The actor is the sole runtime writer. The handle can seal and drain
/// only after the actor channel is closed.
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
    /// Inserts an identity into the owned set.
    /// Returns `true` if the identity was newly inserted.
    /// After seal, returns `false` without mutating.
    /// Only accessible within the tiling module (actor/controller).
    pub(crate) fn insert(&self, identity: AppIdentity) -> bool {
        let mut state = self.inner.lock().unwrap();
        if state.sealed { return false; }
        state.owned.insert(identity)
    }

    /// Removes an identity from the owned set.
    pub(crate) fn remove(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().unwrap().owned.remove(identity)
    }

    /// Returns `true` if the identity is currently owned.
    /// Not a public API — used by actor revalidation code.
    pub(crate) fn contains(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().unwrap().owned.contains(identity)
    }

    /// Returns the number of owned identities.
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.inner.lock().unwrap().owned.len()
    }

    /// Returns `true` if the registry is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().owned.is_empty()
    }

    /// Returns whether the registry has been sealed.
    pub(crate) fn sealed(&self) -> bool {
        self.inner.lock().unwrap().sealed
    }

    /// Atomically seals and drains in a single lock acquisition.
    /// This is the ONLY mutation API exposed outside the actor/controller.
    /// Prevents a late insert between `seal()` and `drain()` if they were separate.
    /// The actor channel must be alive when called; callers obtain the `Arc` via
    /// `handle.seal_and_drain_visibility()`.
    pub(crate) fn seal_and_drain(&self) -> Vec<AppIdentity> {
        let mut state = self.inner.lock().unwrap();
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

In `actor/mod.rs::StateActor::spawn()`, create the registry before the actor.
`StateActor` gains a `registry: Arc<VisibilityRegistry>` field. The handle
wraps the existing `pub(crate) const fn new(sender)` — we add a separate
constructor that also stores the registry clone:

```rust
// StateActor gains a registry field:
pub struct StateActor {
    state: TilingState,
    receiver: mpsc::Receiver<StateMessage>,
    registry: Arc<VisibilityRegistry>,  // NEW
}

pub fn spawn() -> StateActorHandle {
    tracing::debug!("tiling: spawning state actor");
    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
    let registry = Arc::new(VisibilityRegistry::default());
    let handle = StateActorHandle::new_with_registry(sender, Arc::clone(&registry));

    let actor = Self {
        state: TilingState::new(),
        receiver,
        registry,
    };

    tauri::async_runtime::spawn(async move {
        actor.run().await;
    });

    handle
}
```

The actor stores its own `Arc<VisibilityRegistry>` clone. Runtime operations use
`self.registry.insert(...)` / `self.registry.remove(...)` (pub(crate) methods)
or lock `self.registry.inner` directly for multi-step atomicity.

- [ ] **Step 4: Update re-exports**

In `tiling/mod.rs`, replace:

```rust
pub use visibility::{RestoreSummary, restore_stache_hidden_apps};
```

with:

```rust
pub use visibility::VisibilityRegistry;
// restore_stache_hidden_apps stays as a public function but its impl changes
```

> **Mutex discipline:** Every registry mutation from the actor holds `inner.lock()`
> across all three steps: sealed check, OS call (hide/unhide via `window_ops` helpers),
> and `BTreeSet` mutation. In production the actor locks `self.registry.inner`
> directly (not via the `insert`/`remove` convenience methods) so the OS call is
> atomic with the mutation. The `insert`/`remove` methods above are used only for
> test helpers and non-OS-mutation paths. The actor's runtime methods
> (`handle_hide_for_workspace`, `handle_unhide_for_workspace`) hold the lock across
> all three stages and never call an OS function while the lock is released.
> No `run_on_main_thread` or other main-thread dispatch happens while the registry
> lock is held (NSRunningApplication hide/unhide does not perform main-thread
> callbacks).

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

- Modify: `app/native/src/modules/tiling/actor/handlers/window.rs` — actor hide/unhide via registry
- Modify: `app/native/src/modules/tiling/actor/handlers/app.rs` — remove old forget_stache_hidden_app calls
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — add `hide_app_instance_with_outcome`/`unhide_app_instance_with_outcome`; existing `hide_app_with_outcome`/`unhide_app_with_outcome` (PID wrappers) kept as compat
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — wire hide/unhide through actor registry

- [ ] **Step 1: Add identity-aware helpers to `window_ops.rs`**

Add exact-instance hide/unhide helpers that take `AppIdentity` (not bare PID).
Each resolves one local `NSRunningApplication`, validates identity equality on
the same object, then reuses the shared detailed ObjC core (the same
hide/unhide implementation used by the existing PID-based helpers). The local
ObjC call runs under an autorelease pool; no reference escapes.

Existing `hide_app_with_outcome(pid)` / `unhide_app_with_outcome(pid)` remain
as single-path compatibility wrappers that internally obtain `AppIdentity` from
the PID — they are used by any non-actor code that still operates by PID.

```rust
// In window_ops.rs — new identity-aware helpers.

#[must_use]
pub fn hide_app_instance_with_outcome(identity: AppIdentity) -> HideAppOutcome {
    unsafe {
        let pool = objc::rc::autoreleasepool(|| {
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
        });
        pool
    }
}

#[must_use]
pub fn unhide_app_instance_with_outcome(identity: AppIdentity) -> UnhideAppOutcome {
    unsafe {
        let pool = objc::rc::autoreleasepool(|| {
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
        });
        pool
    }
}
```

> No `?` operators — the return type is `HideAppOutcome`/`UnhideAppOutcome`, not
> `Option` or `Result`. Full `match`/early-return instead.

- [ ] **Step 2: Add actor-owned workspace hide/unhide methods**

Add to `StateActor`. Each method locks the registry, calls the exact-instance
helper while the lock is held, mutates the owned set, unlocks, then updates
window state on the actor:

```rust
/// Runs inside the actor's message loop. Holds the registry mutex across
/// sealed check, OS call, and ownership mutation so no concurrent
/// seal/drain can observe an inconsistent state.
fn handle_hide_for_workspace(&mut self, identity: AppIdentity) -> HideAppOutcome {
    // Lock registry — held across check + OS call + mutation
    let mut registry_state = self.registry.inner.lock().unwrap();

    // Reject if sealed (shutdown in progress — don't make OS calls)
    if registry_state.sealed {
        return HideAppOutcome::Failed;
    }

    // Exact-instance call while lock held — autorelease pool inside,
    // no reference escapes, no run_on_main_thread while locked.
    let outcome = hide_app_instance_with_outcome(identity);

    if outcome == HideAppOutcome::HiddenByStache {
        registry_state.owned.insert(identity);
        drop(registry_state); // release lock before actor state mutation
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = true);
        }
    } else {
        drop(registry_state);
    }
    outcome
}

fn handle_unhide_for_workspace(&mut self, identity: AppIdentity) -> UnhideAppOutcome {
    let mut registry_state = self.registry.inner.lock().unwrap();
    if registry_state.sealed {
        return UnhideAppOutcome::Failed;
    }

    let outcome = unhide_app_instance_with_outcome(identity);

    if matches!(outcome, UnhideAppOutcome::UnhiddenByStache | UnhideAppOutcome::AlreadyShown) {
        registry_state.owned.remove(&identity);
        drop(registry_state);
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = false);
        }
    } else {
        drop(registry_state);
    }
    outcome
}
```

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

Replace `sync_window_visibility_for_workspaces` (currently in
`handlers/window.rs`) with an actor method that collects identities (not PIDs)
from windows in affected workspaces, then processes them through the
identity-aware controller methods:

```rust
// In actor/mod.rs workspace switch handler:
/// Collects identities from windows in the affected workspaces and
/// processes hide/unhide through the registry-locked controller.
fn sync_visibility_for_workspaces(&mut self, becoming_visible: &[Uuid], becoming_hidden: &[Uuid]) {
    // Collect identities from windows — no bare PID iteration
    let showing: Vec<AppIdentity> = becoming_visible.iter()
        .flat_map(|ws_id| self.state.windows.iter()
            .filter(|w| w.workspace_id == *ws_id)
            .filter_map(|w| w.identity))
        .collect();

    let visible_ws_ids: HashSet<Uuid> =
        self.state.get_visible_workspaces().iter().map(|ws| ws.id).collect();

    let hiding: Vec<AppIdentity> = becoming_hidden.iter()
        .flat_map(|ws_id| self.state.windows.iter()
            .filter(|w| w.workspace_id == *ws_id)
            .filter_map(|w| w.identity)
            .filter(|id| !self.state.windows.iter().any(|w| {
                w.identity == Some(*id) && visible_ws_ids.contains(&w.workspace_id)
            })))
        .collect();

    for identity in showing {
        self.handle_unhide_for_workspace(identity);
    }
    for identity in hiding {
        self.handle_hide_for_workspace(identity);
    }
}
```

Workspace-switch callers (focus change, workspace switch command) already call
through the actor's message loop, so this method replaces the previous static
function call. Focus-switch callers that currently call
`sync_window_visibility_for_workspaces` directly must route through the actor
instead.

- [ ] **Step 4: Remove static callback mutation**

Delete `forget_stache_hidden_app` and `forget_stache_hidden_app_terminated` from
the old visibility.rs (they relied on PID-keyed global state). The actor no
longer needs them — ownership is per-identity and the actor controls all
mutations.

Delete `classify_stache_hidden_app` / `classify_stache_hidden_app_with_state`
and the entire `ShownClassification` enum and generation/FIFO `PidState`.

- [ ] **Step 5: Build and run tests**

```bash
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::effects::window_ops::tests
cargo fmt --all -- --check
cargo check -p stache
```

- [ ] **Step 6: Commit**

```bash
git add app/native/src/modules/tiling/actor/handlers/window.rs \
  app/native/src/modules/tiling/actor/handlers/app.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/effects/window_ops.rs
git commit -m "feat(tiling): actor-owned workspace hide/unhide via registry"
```

---

### Task 19E: Raw lifecycle forwarding and actor revalidation

**Files:**

- Modify: `app/native/src/modules/tiling/events/processor.rs`
- Modify: `app/native/src/modules/tiling/actor/handlers/app.rs`
- Modify: `app/native/src/modules/tiling/actor/mod.rs`

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
    actor.state.add_window_with_identity(identity); // helper that sets Window.identity

    // Simulate AppShown with OS visible
    // on_app_shown_revalidated accepts registry ref and state ref:
    on_app_shown_revalidated(&registry, &mut actor.state, identity, |_| Some(false));

    assert!(!registry.contains(&identity));
    // All windows for this identity are marked visible
    for wid in actor.state.get_windows_for_identity(&identity) {
        assert!(!actor.state.get_window(wid).unwrap().is_hidden);
    }
}

#[test]
fn app_shown_hidden_retains_owner() {
    let identity = test_identity(42, 100);
    let (mut actor, registry) = make_actor();
    registry.insert(identity);
    actor.state.add_window_with_identity(identity);
    for wid in actor.state.get_windows_for_identity(&identity) {
        actor.state.update_window(wid, |w| w.is_hidden = true);
    }

    on_app_shown_revalidated(&registry, &mut actor.state, identity, |_| Some(true));
    // OS says hidden — retain owner, no window change
    assert!(registry.contains(&identity));
    for wid in actor.state.get_windows_for_identity(&identity) {
        assert!(actor.state.get_window(wid).unwrap().is_hidden);
    }
}

#[test]
fn app_shown_mismatch_retains_owner() {
    let stored = test_identity(42, 100);
    let different = test_identity(42, 200);
    let (mut actor, registry) = make_actor();
    registry.insert(stored);

    on_app_shown_revalidated(&registry, &mut actor.state, different, |_| Some(false));

    assert!(registry.contains(&stored)); // retained
}

#[test]
fn app_terminated_removes_exact_identity() {
    let identity = test_identity(42, 100);
    let (mut actor, registry) = make_actor();
    registry.insert(identity);

    on_app_terminated(&registry, &mut actor.state, identity);
    assert!(!registry.contains(&identity));
}
```

- [ ] **Step 2: Run, verify RED**

```bash
cargo test -p stache --lib modules::tiling::actor::handlers::app::tests
```

Expected: `on_app_shown_revalidated`, `setup_with_identity` don't exist.

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

/// Handles AppTerminated — removes exact identity from registry.
/// PID-reuse scenario: the event's identity matches a dead process;
/// after termination a new process may inherit the same PID. The identity
/// comparison prevents removing the new process's ownership.
fn on_app_terminated_exact(&mut self, identity: AppIdentity) {
    self.registry.remove(&identity);
}
```

Update the actor message handler for `AppShown` to get a fresh OS query
(via `unhide_app_instance_with_outcome` helper, which validates identity
before querying). No `setup_with_identity` seam — the test harness creates
a real `StateActor` with a real `VisibilityRegistry`:

```rust
StateMessage::AppShown { identity, pid: _ } => {
    // Query OS via the exact-instance helper (returns unhide outcome,
    // but we only need the OS hidden state).
    let outcome = unhide_app_instance_with_outcome(identity);
    let os_hidden = match outcome {
        UnhideAppOutcome::UnhiddenByStache => Some(false),   // was hidden, now shown
        UnhideAppOutcome::AlreadyShown => Some(false),       // already visible
        UnhideAppOutcome::Failed => None,                    // unknown/mismatch
    };
    self.on_app_shown_revalidated(identity, |_| os_hidden);
}
StateMessage::AppHidden { identity, pid: _ } => {
    let outcome = hide_app_instance_with_outcome(identity);
    let os_hidden = match outcome {
        HideAppOutcome::HiddenByStache => Some(true),
        HideAppOutcome::AlreadyHidden => Some(true),
        HideAppOutcome::Failed => None,
    };
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

- [ ] **Step 5: Build and run tests**

```bash
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo test -p stache --lib modules::tiling::actor::handlers::app::tests
cargo fmt --all -- --check
cargo check -p stache
```

Expected: generation/FIFO classifier tests removed; identity-based tests pass.
The `classify_shown_with_state` tests from the old visibility.rs are deleted —
replaced by the actor revalidation tests.

- [ ] **Step 6: Commit**

```bash
git add app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/actor/handlers/app.rs \
  app/native/src/modules/tiling/actor/mod.rs
git commit -m "feat(tiling): raw lifecycle forwarding + actor identity revalidation"
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
- `seal_and_drain` is called after the actor channel is closed, so there is
  no concurrent actor writer when the seal runs.

### Task 19F: Shutdown restoration via registry seal/drain

**Files:**

- Modify: `app/native/src/modules/tiling/visibility.rs` — update `restore_stache_hidden_apps`
- Modify: `app/native/src/modules/tiling/mod.rs` — re-export updated restore
- `app/native/src/app_shutdown.rs` — unchanged (still calls `tiling::restore_stache_hidden_apps()`)

- [ ] **Step 1: Write restoration validation tests**

```rust
// In visibility.rs tests
fn bits(v: u64) -> LaunchDateBits {
    LaunchDateBits::from_time_interval_since_reference_date(v as f64).unwrap()
}

#[test]
fn restore_uses_exact_identity_validation() {
    let registry = VisibilityRegistry::default();
    let id1 = AppIdentity { pid: 10, launch_date: bits(1) };
    let id2 = AppIdentity { pid: 20, launch_date: bits(2) };
    registry.insert(id1);
    registry.insert(id2);

    // Mock restore: identity id1 matches, id2 does not
    let mut restored = Vec::new();
    let summary = restore_stache_hidden_apps_with(&registry, |identity| {
        let ok = identity == id1;
        restored.push(identity);
        ok
    });

    assert_eq!(summary.attempted, 2);
    assert_eq!(summary.restored, 1);
    assert_eq!(restored, vec![id1, id2]); // sorted
    assert!(registry.is_empty());
}

#[test]
fn restore_after_seal_rejects_late_insert() {
    let registry = VisibilityRegistry::default();
    registry.insert(AppIdentity { pid: 10, launch_date: bits(1) });
    registry.seal_and_drain();
    let summary = restore_stache_hidden_apps_with(&registry, |_| unreachable!());
    assert_eq!(summary.attempted, 0);
    assert_eq!(summary.restored, 0);
}

#[test]
fn empty_restore_is_no_op() {
    let registry = VisibilityRegistry::default();
    let summary = restore_stache_hidden_apps_with(&registry, |_| unreachable!());
    assert_eq!(summary, RestoreSummary { attempted: 0, restored: 0 });
}
```

- [ ] **Step 2: Implement handle-based seal/drain restore**

The registry is obtained at shutdown through the already-existing static
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
            restore_from(identities)
        })
        .unwrap_or(RestoreSummary { attempted: 0, restored: 0 })
}

/// Helper that operates on a drained identity list (testable without the
/// actor handle or registry).
fn restore_from(identities: Vec<AppIdentity>) -> RestoreSummary {
    let attempted = identities.len();
    let restored = identities.into_iter()
        .filter(|id| unsafe { id.restore_with_exact_validation() })
        .count();
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
git add app/native/src/modules/tiling/visibility.rs app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): shutdown restoration via registry seal/drain + identity validation"
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
- All generation/FIFO fields in `TrackerState` (but keep `sealed`/`owned` from 19C)
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
- `restore_stache_hidden_apps` / `restore_stache_hidden_apps_from`
- New `VisibilityRegistry` (created in 19C)
- New identity-based tests

Result: `visibility.rs` goes from ~1268 lines to ~200-300 lines of clean
registry + restore code.

- [ ] **Step 2: Clean processor.rs imports**

Verify no `ShownClassification`, `classify_stache_hidden_app`, or
`forget_stache_hidden_app_terminated` remain. The `on_app_shown_with` seam
and its tests are removed.

- [ ] **Step 3: Clean window_ops.rs if needed**

`HideAppOutcome`, `UnhideAppOutcome`, `hide_app_with_outcome`, `unhide_app_with_outcome`,
`hide_app`, `unhide_app`, `app_is_hidden` all survive — they are still used by the
actor hide/unhide operations. The restore path now uses
`AppIdentity::restore_with_exact_validation` instead of bare `unhide_app`.

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
> Tasks 9-18 (Phases 4-5 lifecycle/tray changes) are unchanged by Phase 6.

- [ ] **Step 5: Commit**

```bash
git add app/native/src/modules/tiling/visibility.rs \
  app/native/src/modules/tiling/events/processor.rs
git commit -m "refactor(tiling): remove obsolete generation/FIFO/ShownClassification"
```

---

### Superseded commits note

Commits `f2e20c8..95fa2f5` implemented the per-PID generation/FIFO state
machine (`ShownClassification`, `PidState`, `classify_stache_hidden_app`,
`forget_stache_hidden_app_terminated`, `TrackerState::next_generation`,
etc.) and the PID-set-based restore. These are superseded by Tasks 19A-19G.

The detailed per-call `HideAppOutcome`/`UnhideAppOutcome` types from those
commits survive — they are useful for the actor-owned hide/unhide operations.
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

Task 19F changes only the internals of `tiling::restore_stache_hidden_apps()`
(visibility.rs) behind the existing public API surface. The cleanup order
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

2. **AppShown consumed after hidden — ownership retained:** Let Stache hide App B.
   Minimize App B's only window (the AppHidden event fires while windows are still
   hidden). The actor sees OS hidden → retains ownership. Then manually show App B
   (AppShown fires). The actor calls `on_app_shown_revalidated`, queries OS, sees
   visible → removes ownership and marks windows shown. Manual rehide before shutdown
   means ownership is gone. Verification: shutdown does NOT restore App B.

3. **PID reuse:** Let Stache hide App C. Terminate App C. Launch a different app that
   acquires the same PID. Shut Stache down — the old `AppIdentity` (with App C's
   launch date) does not match the new process's identity. Verification: `attempted`
   increments but `restored` does not include the PID-reused identity.

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

Confirm restoration occurs before the replacement process starts in all three cases.

- [ ] **Step 4: Verify SIGTERM and SIGINT**

For each signal, start from a fresh process and repeat the hidden-state setup:

```bash
kill -TERM "$(pgrep -x stache)"
kill -INT "$(pgrep -x stache)"
```

Confirm the process exits after restoring only Stache-owned hides. Check logs for the
`attempted` and `restored` counts.

- [ ] **Step 5: PID-reuse validation**

Design a test that simulates PID-reuse: terminate an application that Stache has
hidden, then launch a different application that happens to get the same PID.
Shut Stache down and confirm the new application is NOT unhidden (its identity's
launch date does not match the drained `AppIdentity`). This test is most reliably
done as a unit test via `restore_stache_hidden_apps_from` with a mocked
`restore_with_exact_validation` that returns `false` for mismatched identities.
A manual variant: terminate a Stache-hidden app, note its PID, launch another
app (or wait for the system to reuse the PID), then trigger Stache shutdown and
verify logs show `attempted` but no `restored` for that PID.

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
