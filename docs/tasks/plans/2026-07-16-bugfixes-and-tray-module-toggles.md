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

Expected: all pass. `AppIdentity::from_ns_running_app(std::ptr::null_mut())` fails
with `None` (null pointer deref on the first msg_send is technically UB in test — the
test we wrote only checks the validation layer, not the full ObjC path. The `null_mut`
test above is a placeholder for a compile-time verify; the real ObjC test can only run
with a real NSRunningApplication. Move the null-pointer test to integration once an
ObjC runtime is available.)

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
    let ident = AppIdentity { pid: 42, launch_date: LaunchDateBits(100) };
    let event = WindowEvent::new(WindowEventType::Created, 42, 0);
    // This must fail: WindowEvent::new signature must change.
    // After impl: assert_eq!(event.identity, ident);
}

#[test]
fn state_message_app_shown_carries_identity() {
    let ident = AppIdentity { pid: 42, launch_date: LaunchDateBits(100) };
    let msg = StateMessage::AppShown { identity: ident, pid: 42 };
    // After impl: assert_eq!(msg.identity(), ident);
}
```

- [ ] **Step 2: Run, verify RED**

```bash
cargo test -p stache --lib modules::tiling::events::types::tests
```

Expected: compilation fails — `identity` field missing from `WindowEvent`.

- [ ] **Step 3: Add identity field to each type**

**`Window`** (`state/types.rs:308`):

```rust
pub struct Window {
    pub id: u32,
    pub pid: i32,
    pub identity: AppIdentity,  // NEW
    pub app_id: String,
    // ... rest unchanged
}
```

Update `Window::default()` — add `identity: AppIdentity { pid: 0, launch_date: LaunchDateBits(0) }` or wrap in `Option<AppIdentity>`. Prefer `Option<AppIdentity>` for windows where capture hasn't happened yet; the spec requires identity on Window, so make it `pub identity: Option<AppIdentity>` to handle the initial window-scan path gracefully.

**`WindowCreatedInfo`** (`actor/messages.rs:271`):

```rust
pub struct WindowCreatedInfo {
    pub window_id: u32,
    pub pid: i32,
    pub identity: Option<AppIdentity>, // NEW
    // ... rest unchanged
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

In `ax_observer.rs::add_observer_for_pid`, when registering the callback for each
event type, capture the `AppIdentity` from the same `NSRunningApplication` used
to create the AX element. Store the identity in the callback's context so the
callback can copy it rather than re-resolving the PID:

```rust
// In the callback registration closure (approximate):
let identity = unsafe {
    let app: *mut Object = msg_send![class!(NSRunningApplication),
        runningApplicationWithProcessIdentifier: pid];
    AppIdentity::from_ns_running_app(app)
};
// Store identity in callback context; callback copies value.
```

For AX callbacks that produce `WindowEvent`, call `WindowEvent::new` with the
stored identity.

- [ ] **Step 5: Capture identity in NSWorkspace termination**

In `app_monitor.rs::extract_app_info`, after getting `running_app` from
`NSWorkspaceApplicationKey`, capture its identity before extracting PID:

```rust
let identity = unsafe { AppIdentity::from_ns_running_app(running_app) }
    .unwrap_or_else(|| AppIdentity { pid: 0, launch_date: LaunchDateBits(0) });
// ... existing PID/bundle extraction using the same running_app object.
```

Return identity alongside the tuple. Update `on_app_launched`/`on_app_terminated`
signatures to accept `AppIdentity`.

- [ ] **Step 6: Fix all call sites**

Every place that constructs `Window`, `WindowCreatedInfo`, `WindowEvent`, or sends
a lifecycle `StateMessage` must provide an identity. Key call sites:

- `ax_observer.rs::handle_window_created` — pass identity from observer record
- `ax_observer.rs` AX callback handlers for AppHidden/AppShown
- `app_monitor.rs::on_app_launched` → `processor.on_app_launched`
- `app_monitor.rs::on_app_terminated` → `processor.on_app_terminated`
- Initial window scan during tiling init — capture identity per window's PID

- [ ] **Step 7: Build and run specific tests**

```bash
cargo check -p stache 2>&1 | head -40
# Fix all type errors iteratively
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

- [ ] **Step 1: Write registry tests that define the API**

Write these before implementation — they must fail to compile:

```rust
// In visibility.rs tests
#[test]
fn registry_accepts_and_drains_identities() {
    let reg = VisibilityRegistry::default();
    let id1 = AppIdentity { pid: 10, launch_date: LaunchDateBits(1) };
    let id2 = AppIdentity { pid: 20, launch_date: LaunchDateBits(2) };

    assert!(!reg.sealed());
    reg.insert(id1);
    reg.insert(id2);
    assert!(reg.contains(id1));
    assert_eq!(reg.len(), 2);

    reg.seal();
    assert!(reg.sealed());

    // Late insert is a no-op (no panic)
    let id3 = AppIdentity { pid: 30, launch_date: LaunchDateBits(3) };
    reg.insert(id3);
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
        launch_date: LaunchDateBits(i as u64),
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
    let id = AppIdentity { pid: 10, launch_date: LaunchDateBits(1) };
    reg.insert(id);
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
    pub fn insert(&self, identity: AppIdentity) -> bool {
        let mut state = self.inner.lock().unwrap();
        if state.sealed { return false; }
        state.owned.insert(identity)
    }

    /// Removes an identity from the owned set.
    pub fn remove(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().unwrap().owned.remove(identity)
    }

    /// Returns `true` if the identity is currently owned.
    pub fn contains(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().unwrap().owned.contains(identity)
    }

    /// Returns the number of owned identities.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().owned.len()
    }

    /// Returns `true` if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().owned.is_empty()
    }

    /// Returns whether the registry has been sealed.
    pub fn sealed(&self) -> bool {
        self.inner.lock().unwrap().sealed
    }

    /// Seals the registry against future inserts.
    pub fn seal(&self) {
        self.inner.lock().unwrap().sealed = true;
    }

    /// Drains all identities in sorted order. After drain, returns empty.
    /// Caller should seal first.
    pub fn drain(&self) -> Vec<AppIdentity> {
        let mut state = self.inner.lock().unwrap();
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
    pub registry: Arc<VisibilityRegistry>,  // NEW
}
```

In `actor/mod.rs::StateActor::spawn()`, create the registry before the actor:

```rust
pub fn spawn() -> StateActorHandle {
    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
    let registry = Arc::new(VisibilityRegistry::default());
    let handle = StateActorHandle::new_with_registry(sender, Arc::clone(&registry));
    // Store registry in actor or pass via shared Arc
    let actor = Self { state: TilingState::new(), receiver, registry };
    // ...
}
```

The actor stores its own `Arc<VisibilityRegistry>` clone. Runtime operations use
`self.registry.insert(...)` / `self.registry.remove(...)`.

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
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — keep `hide_app_with_outcome`/`unhide_app_with_outcome`; `hide_app`/`unhide_app` compat wrappers unchanged
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — wire hide/unhide through actor registry

- [ ] **Step 1: Add actor-owned workspace hide/unhide methods**

Add to `StateActor` (or a new `handlers/visibility.rs`):

```rust
/// Runs inside the actor's message loop. Validates identity from a fresh
/// NSRunningApplication lookup, calls OS hide on that same object, then
/// records ownership.
fn handle_hide_for_workspace(&mut self, identity: AppIdentity) -> HideAppOutcome {
    // Fresh lookup from the same PID — validates identity before action
    let outcome = unsafe {
        let app_class = Class::get("NSRunningApplication")?;
        let app: *mut Object = msg_send![app_class,
            runningApplicationWithProcessIdentifier: identity.pid];
        if app.is_null() { return HideAppOutcome::Failed; }
        let actual = AppIdentity::from_ns_running_app(app)?;
        if actual != identity { return HideAppOutcome::Failed; } // PID-reuse
        let is_hidden: BOOL = msg_send![app, isHidden];
        if is_hidden == YES { return HideAppOutcome::AlreadyHidden; }
        let result: BOOL = msg_send![app, hide];
        if result == YES { HideAppOutcome::HiddenByStache } else { HideAppOutcome::Failed }
    };
    if outcome == HideAppOutcome::HiddenByStache {
        self.registry.insert(identity);
        // Update exact-identity windows in same turn
        for wid in self.state.get_windows_for_identity(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = true);
        }
    }
    outcome
}
```

Add `get_windows_for_identity` to `TilingState`:

```rust
pub fn get_windows_for_identity(&self, identity: &AppIdentity) -> Vec<u32> {
    self.windows.iter()
        .filter(|(_, w)| w.identity.as_ref() == Some(identity))
        .map(|(id, _)| *id)
        .collect()
}
```

- [ ] **Step 2: Route workspace visibility through actor**

In `actor/mod.rs`, the workspace switch message handler calls
`self.handle_hide_for_workspace(identity)` instead of the global
`hide_app_for_workspace(pid)`. The identity is obtained from the
window's stored identity.

For unhide, similarly:

```rust
fn handle_unhide_for_workspace(&mut self, identity: AppIdentity) -> UnhideAppOutcome {
    let outcome = unsafe {
        // Same fresh-lookup pattern
        // ...
    };
    if matches!(outcome, UnhideAppOutcome::UnhiddenByStache | UnhideAppOutcome::AlreadyShown) {
        self.registry.remove(&identity);
        for wid in self.state.get_windows_for_identity(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = false);
        }
    }
    outcome
}
```

- [ ] **Step 3: Remove static callback mutation**

Delete `forget_stache_hidden_app` and `forget_stache_hidden_app_terminated` from
the old visibility.rs (they relied on PID-keyed global state). The actor no
longer needs them — ownership is per-identity and the actor controls all
mutations.

Delete `classify_stache_hidden_app` / `classify_stache_hidden_app_with_state`
and the entire `ShownClassification` enum and generation/FIFO `PidState`.

- [ ] **Step 4: Update workspace visibility call sites**

In `handlers/window.rs` (around line 349-411), where `hide_app`/`unhide_app`
were called, the actor now has self-contained hide/unhide methods. Pass the
window's `identity` field rather than bare PID.

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
#[test]
fn app_shown_visible_removes_owner_and_marks_windows() {
    let identity = AppIdentity { pid: 42, launch_date: LaunchDateBits(100) };
    let (mut state, registry) = setup_with_identity(identity);
    registry.insert(identity);

    // Simulate AppShown with OS visible
    on_app_shown_revalidated(&mut state, identity, |id| {
        assert_eq!(id, identity);
        Some(false) // OS says visible
    });

    assert!(!registry.contains(&identity));
    // All windows for this identity are marked visible
    for wid in state.get_windows_for_identity(&identity) {
        assert!(!state.get_window(wid).unwrap().is_hidden);
    }
}

#[test]
fn app_shown_hidden_retains_owner() {
    let identity = AppIdentity { pid: 42, launch_date: LaunchDateBits(100) };
    let (mut state, registry) = setup_with_identity(identity);
    registry.insert(identity);
    state.mark_windows_hidden(&identity); // windows currently hidden

    on_app_shown_revalidated(&mut state, identity, |_| Some(true));
    // OS says hidden — retain owner, no window change
    assert!(registry.contains(&identity));
    for wid in state.get_windows_for_identity(&identity) {
        assert!(state.get_window(wid).unwrap().is_hidden);
    }
}

#[test]
fn app_shown_mismatch_retains_owner() {
    let stored = AppIdentity { pid: 42, launch_date: LaunchDateBits(100) };
    let different = AppIdentity { pid: 42, launch_date: LaunchDateBits(200) };
    let (mut state, registry) = setup_with_identity(stored);
    registry.insert(stored);

    // Misidentified event — OS has a different process at this PID
    on_app_shown_revalidated(&mut state, different, |_| Some(false));

    assert!(registry.contains(&stored)); // retained
}

#[test]
fn app_terminated_removes_exact_identity() {
    let identity = AppIdentity { pid: 42, launch_date: LaunchDateBits(100) };
    let (mut state, registry) = setup_with_identity(identity);
    registry.insert(identity);

    on_app_terminated(&mut state, identity);
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

In `handlers/app.rs`, add identity-aware versions:

```rust
/// Handles AppShown with identity revalidation.
pub fn on_app_shown_revalidated(
    state: &mut TilingState,
    identity: AppIdentity,
    query_os: impl FnOnce(AppIdentity) -> Option<bool>,
) {
    let is_hidden = query_os(identity);
    match is_hidden {
        Some(false) => {
            // OS confirms visible — remove owner, mark windows shown
            state.registry.remove(&identity);
            for wid in state.get_windows_for_identity(&identity) {
                state.update_window(wid, |w| w.is_hidden = false);
            }
        }
        Some(true) | None => {
            // OS hidden or unknown — retain owner, no state change
            // This is the conservative unavoidable-history policy.
        }
    }
}
```

Update the actor message handler for `AppShown` to get a fresh OS query:

```rust
StateMessage::AppShown { identity, .. } => {
    let os_hidden = unsafe {
        // One-shot identity capture from current OS state
        let app: *mut Object = msg_send![class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: identity.pid];
        if app.is_null() {
            None
        } else {
            let actual = AppIdentity::from_ns_running_app(app);
            if actual != Some(identity) {
                // Mismatch → treat as unknown (conservative)
                None
            } else {
                let is_hidden: BOOL = msg_send![app, isHidden];
                Some(is_hidden == YES)
            }
        }
    };
    on_app_shown_revalidated(&mut self.state, identity, |_| os_hidden);
}
```

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

### Task 19F: Shutdown restoration via registry seal/drain

**Files:**

- Modify: `app/native/src/modules/tiling/visibility.rs` — update `restore_stache_hidden_apps`
- Modify: `app/native/src/modules/tiling/mod.rs` — re-export updated restore
- `app/native/src/app_shutdown.rs` — unchanged (still calls `tiling::restore_stache_hidden_apps()`)

- [ ] **Step 1: Write restoration validation tests**

```rust
// In visibility.rs tests
#[test]
fn restore_uses_exact_identity_validation() {
    let registry = VisibilityRegistry::default();
    let id1 = AppIdentity { pid: 10, launch_date: LaunchDateBits(1) };
    let id2 = AppIdentity { pid: 20, launch_date: LaunchDateBits(2) };
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
    let id = AppIdentity { pid: 10, launch_date: LaunchDateBits(1) };
    registry.insert(id);
    registry.seal();
    assert!(!registry.insert(AppIdentity { pid: 99, launch_date: LaunchDateBits(9) }));
    let summary = restore_stache_hidden_apps_with(&registry, |_| true);
    assert_eq!(summary.attempted, 1);
    assert_eq!(summary.restored, 1);
}

#[test]
fn empty_restore_is_no_op() {
    let registry = VisibilityRegistry::default();
    let summary = restore_stache_hidden_apps_with(&registry, |_| unreachable!());
    assert_eq!(summary, RestoreSummary { attempted: 0, restored: 0 });
}
```

- [ ] **Step 2: Implement handle-based seal/drain restore**

```rust
/// Restores every owned identity with exact-instance validation.
///
/// Seals the registry, drains all identities, and attempts to unhide
/// each. Identity mismatch, nonexistent PID, or already-visible state
/// are silently skipped. Continues on per-identity errors. Registry
/// lock is released before restoration begins.
#[must_use]
pub fn restore_stache_hidden_apps() -> RestoreSummary {
    // Registry is obtained from the actor handle's shared Arc.
    // During shutdown, the caller has the handle.
    let registry = ...; // obtained from StateActorHandle::registry
    registry.seal();
    let identities = registry.drain();
    let attempted = identities.len();
    let restored = identities.into_iter()
        .filter(|id| unsafe { id.restore_with_exact_validation() })
        .count();
    RestoreSummary { attempted, restored }
}
```

The actual wiring: `StateActorHandle` has `pub registry: Arc<VisibilityRegistry>`.
`restore_stache_hidden_apps` takes the handle or the registry. The cleanest approach:

```rust
pub fn restore_stache_hidden_apps_from(registry: &VisibilityRegistry) -> RestoreSummary {
    registry.seal();
    let identities = registry.drain();
    let attempted = identities.len();
    let restored = identities.into_iter()
        .filter(|id| unsafe { id.restore_with_exact_validation() })
        .count();
    RestoreSummary { attempted, restored }
}
```

And `tiling::restore_stache_hidden_apps()` becomes a thin wrapper that obtains
the registry from the actor handle (stored in a static or passed at shutdown).

For `app_shutdown.rs`, the call `tiling::restore_stache_hidden_apps()` stays but
the internal implementation changes. Task20's `cleanup_once` function remains
unchanged in structure.

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

- [ ] **Step 4: Build with full lint**

```bash
cargo test -p stache
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
cargo check -p stache
```

Expected: all pass. Clippy may flag removed generational code — address each.

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

**Files:**

- Create: `app/native/src/app_shutdown.rs`
- Modify: `app/native/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `app/native/src/lib.rs:7-27,168-222`
- Modify: `app/native/src/modules/tray/mod.rs:50-59`
- Modify: `app/native/src/config/watcher.rs:94-98`
- Modify: `app/native/src/modules/bar/ipc_listener.rs:58-74`

- [ ] **Step 1: Write failing cleanup-order and idempotency tests**

Create `app_shutdown.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn cleanup_restores_before_stopping_tiling_and_ipc() {
        let started = AtomicBool::new(false);
        let order = Mutex::new(Vec::new());

        assert!(run_cleanup_once(
            &started,
            || order.lock().unwrap().push("restore"),
            || order.lock().unwrap().push("tiling"),
            || order.lock().unwrap().push("ipc"),
        ));

        assert_eq!(*order.lock().unwrap(), ["restore", "tiling", "ipc"]);
    }

    #[test]
    fn cleanup_runs_only_once() {
        let started = AtomicBool::new(false);
        let calls = AtomicUsize::new(0);
        assert!(run_cleanup_once(
            &started,
            || { calls.fetch_add(1, Ordering::Relaxed); },
            || { calls.fetch_add(1, Ordering::Relaxed); },
            || { calls.fetch_add(1, Ordering::Relaxed); },
        ));
        assert!(!run_cleanup_once(
            &started,
            || { calls.fetch_add(1, Ordering::Relaxed); },
            || { calls.fetch_add(1, Ordering::Relaxed); },
            || { calls.fetch_add(1, Ordering::Relaxed); },
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn second_termination_signal_forces_exit() {
        let signals = AtomicU8::new(0);

        assert_eq!(next_signal_action(&signals), SignalAction::Orderly);
        assert_eq!(next_signal_action(&signals), SignalAction::Force);
    }
}
```

Run:

```bash
cargo test -p stache --lib app_shutdown::tests
```

Expected: compilation fails because `run_cleanup_once` is missing.

- [ ] **Step 2: Add the signal dependency**

Add to `app/native/Cargo.toml` in alphabetical order:

```toml
ctrlc = { version = "3.4.5", features = ["termination"] }
```

Run `cargo check -p stache` once to update `Cargo.lock`.

- [ ] **Step 3: Implement one cleanup path and safe signal bridging**

Add above the tests in `app_shutdown.rs`:

```rust
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tauri::{AppHandle, Runtime};

use crate::{modules::tiling, platform};

static CLEANUP_STARTED: AtomicBool = AtomicBool::new(false);
static SIGNAL_COUNT: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownAction {
    Exit,
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalAction {
    Orderly,
    Force,
}

fn next_signal_action(signals: &AtomicU8) -> SignalAction {
    if signals.fetch_add(1, Ordering::AcqRel) == 0 {
        SignalAction::Orderly
    } else {
        SignalAction::Force
    }
}

fn run_cleanup_once(
    started: &AtomicBool,
    restore: impl FnOnce(),
    stop_tiling: impl FnOnce(),
    stop_ipc: impl FnOnce(),
) -> bool {
    if started.swap(true, Ordering::AcqRel) {
        return false;
    }
    restore();
    stop_tiling();
    stop_ipc();
    true
}

pub fn cleanup_once() {
    let _ = run_cleanup_once(
        &CLEANUP_STARTED,
        || {
            let summary = tiling::restore_stache_hidden_apps();
            tracing::info!(
                attempted = summary.attempted,
                restored = summary.restored,
                "restored applications hidden by Stache"
            );
        },
        tiling::shutdown,
        platform::ipc_socket::stop_server,
    );
}

fn request<R: Runtime>(app: &AppHandle<R>, action: ShutdownAction) {
    let action_handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        cleanup_once();
        match action {
            ShutdownAction::Exit => action_handle.exit(0),
            ShutdownAction::Restart => action_handle.restart(),
        }
    }) {
        tracing::error!(%error, "failed to dispatch orderly shutdown to main thread");
    }
}

pub fn exit<R: Runtime>(app: &AppHandle<R>) { request(app, ShutdownAction::Exit); }

pub fn restart<R: Runtime>(app: &AppHandle<R>) { request(app, ShutdownAction::Restart); }

pub fn install_signal_handler<R: Runtime>(app: AppHandle<R>) -> Result<(), ctrlc::Error> {
    ctrlc::set_handler(move || {
        match next_signal_action(&SIGNAL_COUNT) {
            SignalAction::Orderly => exit(&app),
            SignalAction::Force => std::process::exit(1),
        }
    })
}
```

The `ctrlc` closure runs on the crate's dedicated thread; it only schedules work on
Tauri's main thread through `request`. Explicit restart calls from the config watcher
and IPC listener use the same dispatch path. AppKit/AX restoration never runs inside a
raw signal handler or on a background thread.
`ctrlc::set_handler` installs one process-global handler and this function is called
exactly once during app startup. The first SIGINT/SIGTERM requests orderly restoration;
a second signal is an explicit last-resort forced exit if the main thread is stalled.
If scheduling the first request fails, Stache logs the error and does not restart or
pretend restoration occurred.

- [ ] **Step 4: Expose the module and install the signal bridge before `App::run`**

In `lib.rs`, add `mod app_shutdown;`. Refactor the final builder chain so `.build(...)`
is assigned to `app`, then install the handler and run the app:

```rust
let app = tauri::Builder::default()
    // existing plugins, handlers, and setup remain unchanged
    .build(context)
    .expect("error while building tauri application");

app_shutdown::install_signal_handler(app.handle().clone())
    .expect("failed to install SIGINT/SIGTERM handler");

app.run(|_app, event| {
    if matches!(event, tauri::RunEvent::Exit) {
        tracing::info!("application exiting, cleaning up");
        app_shutdown::cleanup_once();
    }
});
```

- [ ] **Step 5: Route explicit quit and restart paths through cleanup**

Replace only the terminal calls at these sites:

```rust
// modules/tray/mod.rs
RELOAD_ID => crate::app_shutdown::restart(app),
QUIT_ID => crate::app_shutdown::exit(app),

// config/watcher.rs release branch
crate::app_shutdown::restart(&app_handle);

// modules/bar/ipc_listener.rs release branch
crate::app_shutdown::restart(app_handle);
```

Keep existing logging and frontend reload emission. Do not alter debug-only behavior.

- [ ] **Step 6: Run focused tests and compile all wired paths**

Run:

```bash
cargo test -p stache --lib app_shutdown::tests
cargo test -p stache --lib modules::tiling::visibility::tests
cargo fmt --all -- --check
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
```

Expected: all commands pass. The cleanup test proves restoration occurs before tiling
and IPC teardown, and repeated Tauri exit notifications cannot run cleanup twice.

- [ ] **Step 7: Commit**

```bash
git add app/native/Cargo.toml Cargo.lock app/native/src/app_shutdown.rs \
  app/native/src/lib.rs app/native/src/modules/tray/mod.rs \
  app/native/src/config/watcher.rs app/native/src/modules/bar/ipc_listener.rs
git commit -m "feat: restore Stache-hidden apps before shutdown"
```

### Task 21: Manually verify supported shutdown paths

**Files:** none

- [ ] **Step 1: Prepare distinguishable application states**

Run `pnpm tauri:dev`, place one application's windows only on a non-visible Stache
workspace so Stache hides that application, minimize a window from another application,
and manually hide a third application before Stache attempts to manage it.

Also prepare the relinquished-ownership sequence: let Stache hide an application, show
it manually, then hide it manually again. On every shutdown path below, confirm this
application remains hidden because the app-shown event removed Stache's ownership.

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

- [ ] **Step 5: Confirm and document the SIGKILL boundary**

Run `kill -KILL "$(pgrep -x stache)"` only after the supported-path checks. Confirm no
cleanup log is emitted. This is the expected POSIX limitation: SIGKILL cannot execute
in-process restoration and is not a failure of the implementation.

- [ ] **Step 6: Report results** — verification only. No commit.

---

## Verification Summary

| Phase | Verification                                                                                                                                                                                                                                                                                                                                                                        |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Manual: menubar reaction ≤200ms (Task 3). Build passes.                                                                                                                                                                                                                                                                                                                             |
| 2     | Manual: tracing spans emit under debug default (Task 8). No behavior change; existing tests pass (`pnpm test`).                                                                                                                                                                                                                                                                     |
| 3     | Blocked pending Phase 2 evidence; each confirmed fix gets a failing test → pass → commit.                                                                                                                                                                                                                                                                                           |
| 4     | Build passes; `pnpm test` passes; manual: each module's OS resource actually releases/reacquires (event tap disabled, listener removed, tiling `reset()` clears guard so resume succeeds).                                                                                                                                                                                          |
| 5     | Manual tray interaction (Task 18): toggle states correct, config-off locked, unavailable shows reason, restart resets.                                                                                                                                                                                                                                                              |
| 6     | Unit: AppIdentity capture/validation, registry seal/drain/concurrency, identity-keyed event forwarding, actor revalidation (hidden/visible/mismatch), exact-instance restoration validation, deterministic delayed/duplicate/missing/PID-reuse tests. Manual: tray quit, reload/restart, SIGTERM, and SIGINT restore only Stache-hidden applications; SIGKILL limitation confirmed. |

## Open Items / Risks

- **Phase 3 is blocked on Phase 2 evidence.** Do not write Phase 3 tasks until the logs from Task 8 are reviewed.
- **proxyAudio listener removal (Task 13)** is the highest-risk change — callback reference and `client_data` pointer MUST match the registration exactly.
- **tiling `INITIALIZED` change (Task 15)** converts a `OnceLock<bool>` to `Mutex<bool>`; all read/write sites in `init()` must be updated consistently.
- **Tray `CheckMenuItem` handle retention (Task 17)** may need a `TrayMenuState` managed struct if immediate state reflection is required; the minimal version relies on macOS re-querying the menu on open.
- **AppIdentity ObjC safety.** `NSRunningApplication` objects must never cross threads; identity extraction is a one-shot operation that saves only the `Send + Sync` bytes. AX callbacks must never lock the registry. The actor must never dispatch main-thread work while holding the registry lock.
- **SIGKILL cannot be handled in-process.** Phase 6 intentionally provides best-effort cleanup for orderly exits, explicit restart paths, SIGTERM, and SIGINT only.
