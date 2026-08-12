# Module Lifecycle + Tray — Implementation Plan (Tasks 10-14, 16-18)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the six toggleable modules (wallpapers, commandQuit, notunes, proxyAudio, menuAnywhere, tiling) real OS-resource-level pause/resume behind the existing `LifecycleModule` trait, register them in a fixed `Arc`-based registry wired before base modules, and build a tray "Modules" submenu that keeps Quit/Reload live immediately and installs module `CheckMenuItem`s only after background startup.

**Architecture:** Tasks 10-14 retain each module's OS handle (wallpaper timer generation, event tap, workspace observer, CoreAudio listener set) and implement `LifecycleModule` with `status()` derived from the _actual_ resource state, not a cached boolean. Task 16 adds a fixed registry of `Arc<dyn LifecycleModule>` whose `toggle` never holds a lock across OS calls, managed via `app.manage()` before `load_base_modules`. Task 17 builds the base tray menu immediately, retains the `TrayIcon` and `CheckMenuItem`s in `TrayMenuState`, installs the Modules submenu only after the background startup task finishes, preserves `app_shutdown::restart`/`exit`, and runs toggles off the main thread then refreshes the item from the resulting `ModuleStatus`. Task 18 is the release manual checklist.

**Tech Stack:** Rust (Tauri 2.11.2, `tray-icon` feature), `objc` v0.2.7, `core-foundation` 0.10.1, `objc2-core-audio` 0.3.2, `parking_lot`, `tracing`.

**Prerequisites (must be merged first):**

- `2026-08-12-restartable-tiling-runtime.md` Task 9: `crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus}` (derives `Debug, Clone, PartialEq, Eq`; `LifecycleModule: Send + Sync` with `name`/`id`/`start`/`pause`/`resume`/`status`).
- Task 15D: `tiling::TilingLifecycle::new(tauri::AppHandle)` implementing `LifecycleModule`, re-exported from `modules/tiling/mod.rs` together with `pause_runtime`/`resume`.

**Hard guardrail (applies to every commit):** `app/native/src/modules/audio/device.rs` is protected — never modify it, never stage it. Forbidden: `git add -A`, `git add .`, `git reset --hard`, `git checkout .`, `git stash`, `git commit -am`. Before every commit verify `git hash-object app/native/src/modules/audio/device.rs` prints `50451982cc8ec2064079a2acec1170dfb49dec38` and `git status --short` still shows it as ` M` (modified, unstaged).

**Verified Tauri 2.11.2 API contracts used below:** `CheckMenuItem::with_id(manager, id, text, enabled, checked, accelerator: Option<A>) -> crate::Result<CheckMenuItem<Wry>>`; `set_checked(bool)`/`set_enabled(bool)`/`set_text(S)` all return `crate::Result<()>`; `Submenu::with_items(manager, text, enabled, &[&dyn IsMenuItem<R>])` and `Submenu::append_items(&[&dyn IsMenuItem<R>])`; `Menu::with_items(manager, &[&dyn IsMenuItem<R>])`; `TrayIconBuilder::on_menu_event(|app: &AppHandle, event: MenuEvent|)` with `event.id.as_ref()` as `&str`; `TrayIcon<Wry>` is `Clone + Send + Sync`; `AppHandle::run_on_main_thread(FnOnce() + Send + 'static) -> crate::Result<()>`; `Manager::try_state::<T>() -> Option<State<T>>` and `Manager::state::<T>()`; `platform::thread::spawn_named_thread(name, task)`.

**File structure**

| File                                                    | Responsibility                                                                                                                                             |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app/native/src/modules/wallpaper/manager.rs`           | Task 10: `timer_generation: AtomicU64`; generation-safe `start_timer`/`stop_timer`/`reset_timer`; `wallpaper_status`; `WallpaperLifecycle`                 |
| `app/native/src/modules/wallpaper/mod.rs`               | Task 10: re-export `WallpaperLifecycle`                                                                                                                    |
| `app/native/src/modules/cmd_q/mod.rs`                   | Task 11: `RetainedEventTap` + `EVENT_TAP` static; `CGEventTapIsEnabled` FFI; `cmd_q_status`; `CmdQLifecycle`                                               |
| `app/native/src/modules/notunes/mod.rs`                 | Task 12: `ObserverPtr` + `OBSERVER` static; main-thread register/remove; `no_tunes_status`; `NoTunesLifecycle`                                             |
| `app/native/src/modules/audio/watcher.rs`               | Task 13: per-generation `RetainedListeners`/`RUNTIME`; `start_generation`/`remove_generation`; `proxy_audio_status`; `ProxyAudioLifecycle`                 |
| `app/native/src/modules/audio/mod.rs`                   | Task 13: re-export `ProxyAudioLifecycle`                                                                                                                   |
| `app/native/src/modules/menu_anywhere/event_monitor.rs` | Task 14: `RetainedEventTap` + `EVENT_TAP` static; `CGEventTapIsEnabled` FFI; `set_enabled`/`tap_state`                                                     |
| `app/native/src/modules/menu_anywhere/mod.rs`           | Task 14: `menu_anywhere_status`; `MenuAnywhereLifecycle`                                                                                                   |
| `app/native/src/modules/lifecycle_registry.rs`          | Task 16 (create): `LifecycleRegistry` (fixed `Vec<Arc<dyn LifecycleModule>>`, lock-free-across-OS-calls `toggle`)                                          |
| `app/native/src/modules/mod.rs`                         | Task 16: `pub mod lifecycle_registry;`                                                                                                                     |
| `app/native/src/lib.rs`                                 | Task 16: manage registry + register 6 modules before `load_base_modules`; call `tray::install_modules_submenu(&handle)` at end of background startup       |
| `app/native/src/modules/tray/mod.rs`                    | Task 17: base menu immediately (Quit/Reload + empty Modules submenu), retain `TrayIcon`; `TrayMenuState`; `install_modules_submenu`; async `handle_toggle` |
| `app/native/src/modules/audio/device.rs`                | **Protected — never modify or stage**                                                                                                                      |

---

## Task 10: Wallpapers — generation-safe timer + `WallpaperLifecycle`

**Files:**

- Modify: `app/native/src/modules/wallpaper/manager.rs`
- Modify: `app/native/src/modules/wallpaper/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block at the end of `manager.rs`:

```rust
    fn manager_with_interval(interval: u64) -> Arc<WallpaperManager> {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("wall.jpg");
        std::fs::write(&img, b"").unwrap();
        let config = WallpaperConfig {
            enabled: true,
            list: vec![img.display().to_string()],
            interval,
            ..Default::default()
        };
        Arc::new(WallpaperManager::new(&config).unwrap())
    }

    #[test]
    fn start_timer_is_idempotent_and_preserves_generation() {
        let manager = manager_with_interval(100);

        manager.start_timer();
        let gen = manager.timer_generation.load(Ordering::SeqCst);
        assert!(manager.timer_running.load(Ordering::SeqCst));

        manager.start_timer();
        assert!(manager.timer_running.load(Ordering::SeqCst));
        assert_eq!(manager.timer_generation.load(Ordering::SeqCst), gen);

        manager.stop_timer();
        assert!(!manager.timer_running.load(Ordering::SeqCst));
        assert_eq!(manager.timer_generation.load(Ordering::SeqCst), gen + 1);
    }

    #[test]
    fn reset_timer_restarts_immediately_without_arbitrary_sleep() {
        let manager = manager_with_interval(100);

        manager.start_timer();
        manager.reset_timer();
        // Must return without sleeping: running is re-claimed and generation moved forward.
        assert!(manager.timer_running.load(Ordering::SeqCst));
        manager.stop_timer();
    }

    #[test]
    fn wallpaper_status_maps_states() {
        let on_config = WallpaperConfig {
            enabled: true,
            list: vec![],
            interval: 100,
            ..Default::default()
        };
        assert_eq!(wallpaper_status(&WallpaperConfig::default(), None), ModuleStatus::ConfiguredOff);
        assert!(matches!(
            wallpaper_status(&on_config, None),
            ModuleStatus::Unavailable(reason) if reason.contains("no wallpapers")
        ));

        let manager = WallpaperManager::new(&on_config).unwrap();
        assert_eq!(wallpaper_status(&on_config, Some(&manager)), ModuleStatus::Paused);

        let fixed_config = WallpaperConfig { interval: 0, ..on_config.clone() };
        assert_eq!(wallpaper_status(&fixed_config, Some(&manager)), ModuleStatus::Running);

        manager.timer_running.store(true, Ordering::SeqCst);
        assert_eq!(wallpaper_status(&on_config, Some(&manager)), ModuleStatus::Running);
    }
```

> `tempfile` is a dev-dependency already used by the workspace tests; if the crate is not a dev-dependency of `stache`, create the fixture via `std::env::temp_dir()` + a unique `std::process::id`-suffixed directory instead, and `let _ = std::fs::remove_dir_all(&dir);` at the end.

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::wallpaper::manager::tests`
Expected: compilation fails — `timer_generation` field, `wallpaper_status`, `ModuleStatus` don't exist yet.

- [ ] **Step 3: Implement the generation-safe timer and lifecycle**

`timer_generation` needs `AtomicU64`; extend the atomic import (manager.rs:5) to `use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};`.

Add `timer_generation` to the struct:

```rust
pub struct WallpaperManager {
    wallpapers: Vec<PathBuf>,
    config: WallpaperConfig,
    current_index: AtomicUsize,
    timer_running: AtomicBool,
    timer_generation: AtomicU64,
    change_lock: Mutex<()>,
}
```

Set `timer_generation: AtomicU64::new(0),` in `WallpaperManager::new`. Replace `start_timer`/`stop_timer`/`reset_timer` (manager.rs:300-342):

```rust
    /// Starts the automatic wallpaper cycling timer.
    ///
    /// Does nothing if the interval is 0. A worker thread fires only while its
    /// captured generation still matches the manager's current generation, so a
    /// stopped timer can never apply another wallpaper, and restart needs no sleep.
    pub fn start_timer(self: &Arc<Self>) {
        if self.config.interval == 0 {
            return;
        }

        if self.timer_running.swap(true, Ordering::SeqCst) {
            // Timer already claimed by a live worker.
            return;
        }

        let generation = self.timer_generation.load(Ordering::SeqCst);
        let manager = Arc::clone(self);
        let interval = Duration::from_secs(self.config.interval);

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);

                if !manager.timer_running.load(Ordering::SeqCst)
                    || manager.timer_generation.load(Ordering::SeqCst) != generation
                {
                    break;
                }

                let next_index = manager.select_next_index();
                if let Err(err) = manager.set_wallpaper_at_index(next_index) {
                    tracing::warn!(error = %err, "wallpaper timer failed to set wallpaper");
                }
            }
        });
    }

    /// Stops the automatic wallpaper cycling timer.
    ///
    /// Bumping the generation invalidates any in-flight worker so it exits at
    /// its next wakeup without applying a wallpaper. No sleep is required
    /// before restart.
    pub fn stop_timer(&self) {
        self.timer_running.store(false, Ordering::SeqCst);
        self.timer_generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Resets the timer (stops and starts it again).
    pub fn reset_timer(self: &Arc<Self>) {
        self.stop_timer();
        self.start_timer();
    }
```

Add the lifecycle impl at the end of `manager.rs` (before the tests module):

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

/// Tray-toggleable lifecycle handle for the global wallpaper manager.
///
/// The manager stays lazily initialized in its `OnceLock`; this unit struct
/// resolves it through `get_manager()` on demand and never forces init.
pub struct WallpaperLifecycle;

impl WallpaperLifecycle {
    fn manager() -> Option<Arc<WallpaperManager>> { get_manager().cloned() }
}

/// Pure status decision so the global `OnceLock` is not required in tests.
fn wallpaper_status(config: &WallpaperConfig, manager: Option<&WallpaperManager>) -> ModuleStatus {
    if !config.is_enabled() {
        return ModuleStatus::ConfiguredOff;
    }
    let Some(manager) = manager else {
        return ModuleStatus::Unavailable("no wallpapers configured or load failed".into());
    };
    if config.interval == 0 || manager.timer_running.load(Ordering::SeqCst) {
        ModuleStatus::Running
    } else {
        ModuleStatus::Paused
    }
}

impl LifecycleModule for WallpaperLifecycle {
    fn name(&self) -> &'static str { "Wallpapers" }
    fn id(&self) -> &'static str { "wallpapers" }

    fn start(&self) -> Result<(), String> {
        match Self::manager() {
            Some(manager) => {
                manager.start_timer();
                Ok(())
            }
            None => Err("wallpaper manager not initialized".into()),
        }
    }

    fn pause(&self) -> Result<(), String> {
        match Self::manager() {
            Some(manager) => {
                manager.stop_timer();
                Ok(())
            }
            None => Err("wallpaper manager not initialized".into()),
        }
    }

    fn resume(&self) -> Result<(), String> { self.start() }

    fn status(&self) -> ModuleStatus {
        let config = &crate::config::get_config().wallpapers;
        wallpaper_status(config, Self::manager().as_deref())
    }
}
```

In `mod.rs`, add `WallpaperLifecycle` to the re-export list.

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::wallpaper::manager::tests`
Expected: all wallpaper tests pass. `start_timer` worker threads may log "wallpaper timer failed to set wallpaper" for the empty fixture image — expected and harmless.

Run: `cargo clippy -p stache --lib -- -D warnings` and `cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/wallpaper/manager.rs app/native/src/modules/wallpaper/mod.rs
git commit -m "feat(wallpapers): generation-safe timer and LifecycleModule"
```

---

## Task 11: commandQuit — retained tap with sound Send/Sync boundary + `CmdQLifecycle`

**Files:**

- Modify: `app/native/src/modules/cmd_q/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `cmd_q/mod.rs`:

```rust
    #[test]
    fn retained_event_tap_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RetainedEventTap>();
    }

    #[test]
    fn cmd_q_status_maps_states() {
        assert_eq!(
            cmd_q_status(false, true, Some(true)),
            ModuleStatus::ConfiguredOff
        );
        assert_eq!(cmd_q_status(true, true, Some(true)), ModuleStatus::Running);
        assert_eq!(cmd_q_status(true, true, Some(false)), ModuleStatus::Paused);
        assert_eq!(cmd_q_status(true, false, None), ModuleStatus::Paused);
        assert!(matches!(
            cmd_q_status(true, true, None),
            ModuleStatus::Unavailable(reason) if reason.contains("Accessibility")
        ));
    }
```

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::cmd_q::tests`
Expected: compilation fails — `RetainedEventTap`, `cmd_q_status`, `ModuleStatus` don't exist.

- [ ] **Step 3: Implement retention, actual status, and lifecycle**

Add the `CGEventTapIsEnabled` FFI declaration inside the existing `unsafe extern "C"` block (after `CGEventTapEnable`):

```rust
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
```

Add near the other statics (after `IS_RUNNING`):

```rust
/// Retained event tap port. `CFMachPort` is a raw-pointer wrapper that is not
/// `Send`/`Sync`; the explicit impls are sound because the retained port is only
/// touched via `CGEventTapEnable`/`CGEventTapIsEnabled` (thread-safe), and the
/// port was retained (`wrap_under_create_rule` takes ownership) so crossing
/// threads never invalidates it.
#[derive(Clone)]
struct RetainedEventTap(CFMachPort);
unsafe impl Send for RetainedEventTap {}
unsafe impl Sync for RetainedEventTap {}

static EVENT_TAP: Mutex<Option<RetainedEventTap>> = Mutex::new(None);
```

In `start_event_tap` (cmd_q/mod.rs:227-231), retain the port before entering the run loop:

```rust
        // Enable the event tap
        CGEventTapEnable(tap, true);

        // Retain the port so pause/resume can reach it via CGEventTapEnable.
        *EVENT_TAP.lock().unwrap() = Some(RetainedEventTap(tap_port.clone()));

        // Run the run loop
        CFRunLoop::run_current();
```

Add the lifecycle impl and the pure status function at the end of `cmd_q/mod.rs` (before the tests module):

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

/// Pure status decision: config gate, then the real tap state.
fn cmd_q_status(config_enabled: bool, running: bool, tap_state: Option<bool>) -> ModuleStatus {
    if !config_enabled {
        return ModuleStatus::ConfiguredOff;
    }
    match tap_state {
        None if running => ModuleStatus::Unavailable(
            "event tap creation failed — check Accessibility permission".into(),
        ),
        None => ModuleStatus::Paused,
        Some(true) => ModuleStatus::Running,
        Some(false) => ModuleStatus::Paused,
    }
}

/// Tray-toggleable lifecycle handle for the Hold-to-Quit event tap.
pub struct CmdQLifecycle {
    app_handle: tauri::AppHandle,
}

impl CmdQLifecycle {
    #[must_use]
    pub fn new(app_handle: tauri::AppHandle) -> Self { Self { app_handle } }
}

impl LifecycleModule for CmdQLifecycle {
    fn name(&self) -> &'static str { "Command Quit" }
    fn id(&self) -> &'static str { "commandQuit" }

    fn start(&self) -> Result<(), String> {
        if IS_RUNNING.load(Ordering::SeqCst) {
            return Ok(());
        }
        let config = crate::config::get_config().command_quit.clone();
        init(self.app_handle.clone(), &config);
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        let tap = EVENT_TAP.lock().unwrap().clone();
        match tap {
            Some(tap) => {
                unsafe { CGEventTapEnable(tap.0.as_concrete_TypeRef(), false) };
                Ok(())
            }
            None => Err("event tap handle not available".into()),
        }
    }

    fn resume(&self) -> Result<(), String> {
        let tap = EVENT_TAP.lock().unwrap().clone();
        match tap {
            Some(tap) => {
                unsafe { CGEventTapEnable(tap.0.as_concrete_TypeRef(), true) };
                Ok(())
            }
            None => Err("event tap handle not available".into()),
        }
    }

    fn status(&self) -> ModuleStatus {
        let config_enabled = crate::config::get_config().command_quit.is_enabled();
        let running = IS_RUNNING.load(Ordering::SeqCst);
        let tap_state = {
            let guard = EVENT_TAP.lock().unwrap();
            guard
                .as_ref()
                .map(|tap| unsafe { CGEventTapIsEnabled(tap.0.as_concrete_TypeRef()) })
        };
        cmd_q_status(config_enabled, running, tap_state)
    }
}
```

`pause`/`resume` clone the port out of the guard before the OS call, so no lock is held across `CGEventTapEnable`. `resume` re-enables the same retained tap and never re-runs `init`, so the timer loop and run loop are not duplicated.

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::cmd_q::tests`
Expected: all cmd_q tests pass.

Run: `cargo clippy -p stache --lib -- -D warnings` and `cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/cmd_q/mod.rs
git commit -m "feat(commandQuit): retain tap with Send/Sync boundary and real status"
```

---

## Task 12: noTunes — main-thread observer retention + `NoTunesLifecycle`

**Files:**

- Modify: `app/native/src/modules/notunes/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `notunes/mod.rs`:

```rust
    #[test]
    fn observer_ptr_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ObserverPtr>();
    }

    #[test]
    fn no_tunes_status_maps_states() {
        assert_eq!(no_tunes_status(false, true, true), ModuleStatus::ConfiguredOff);
        assert_eq!(no_tunes_status(true, true, true), ModuleStatus::Running);
        assert_eq!(no_tunes_status(true, false, false), ModuleStatus::Paused);
        assert!(matches!(
            no_tunes_status(true, false, true),
            ModuleStatus::Unavailable(reason) if reason.contains("observer")
        ));
    }
```

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::notunes::tests`
Expected: compilation fails — `ObserverPtr`, `no_tunes_status`, `ModuleStatus` don't exist.

- [ ] **Step 3: Implement retention, main-thread registration/removal, and lifecycle**

Add imports:

```rust
use std::sync::Mutex;

use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};
use crate::platform::thread::dispatch_on_main_sync;
```

Add near `IS_RUNNING`:

```rust
/// Retained NSWorkspace observer instance. The raw pointer is not `Send`/`Sync`;
/// the explicit impls are sound because the object is retained by
/// `NSNotificationCenter` for its whole lifetime and is only dereferenced via
/// `removeObserver:` on the main thread.
struct ObserverPtr(*mut Object);
unsafe impl Send for ObserverPtr {}
unsafe impl Sync for ObserverPtr {}

static OBSERVER: Mutex<Option<ObserverPtr>> = Mutex::new(None);
```

Update `setup_workspace_observer` to retain the observer pointer:

```rust
unsafe fn setup_workspace_observer() {
    // Get the workspace and notification center
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let notification_center: *mut Object = msg_send![workspace, notificationCenter];

    // Create the notification name
    let notification_name = unsafe { nsstring("NSWorkspaceWillLaunchApplicationNotification") };

    // Create an observer object
    let observer = unsafe { create_observer_object() };
    *OBSERVER.lock().unwrap() = Some(ObserverPtr(observer));

    let _: () = msg_send![
        notification_center,
        addObserver: observer
        selector: sel!(handleAppLaunch:)
        name: notification_name
        object: null_mut::<Object>()
    ];
}
```

Update `init` so registration happens on the main thread:

```rust
    spawn_named_thread("notunes-init", move || {
        // SAFETY: NSWorkspace/NSNotificationCenter must be touched on the main thread.
        dispatch_on_main_sync(|| unsafe {
            setup_workspace_observer();
            // Also terminate any already-running instances
            terminate_music_apps();
        });
    });
```

Add the lifecycle impl and pure status function at the end of `notunes/mod.rs` (before the tests module):

```rust
/// Removes the retained observer. Must be called on the main thread.
unsafe fn remove_observer() {
    let Some(observer) = OBSERVER.lock().unwrap().take() else {
        return;
    };
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let center: *mut Object = msg_send![workspace, notificationCenter];
    let _: () = msg_send![center, removeObserver: observer.0];
}

/// Re-registers the observer. Must be called on the main thread.
fn start_observer() -> Result<(), String> {
    dispatch_on_main_sync(|| unsafe { setup_workspace_observer() });
    IS_RUNNING.store(true, Ordering::SeqCst);
    Ok(())
}

/// Pure status decision so the global `OBSERVER` slot is not required in tests.
fn no_tunes_status(config_enabled: bool, observer_present: bool, running: bool) -> ModuleStatus {
    if !config_enabled {
        return ModuleStatus::ConfiguredOff;
    }
    if observer_present {
        return ModuleStatus::Running;
    }
    if running {
        ModuleStatus::Unavailable("observer registration failed".into())
    } else {
        ModuleStatus::Paused
    }
}

/// Tray-toggleable lifecycle handle for noTunes.
pub struct NoTunesLifecycle;

impl LifecycleModule for NoTunesLifecycle {
    fn name(&self) -> &'static str { "NoTunes" }
    fn id(&self) -> &'static str { "notunes" }

    fn start(&self) -> Result<(), String> {
        if IS_RUNNING.load(Ordering::SeqCst) {
            return Ok(());
        }
        init();
        Ok(())
    }

    fn pause(&self) -> Result<(), String> {
        dispatch_on_main_sync(|| unsafe { remove_observer() });
        IS_RUNNING.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn resume(&self) -> Result<(), String> { start_observer() }

    fn status(&self) -> ModuleStatus {
        let config_enabled = crate::config::get_config().notunes.is_enabled();
        let observer_present = OBSERVER.lock().unwrap().is_some();
        let running = IS_RUNNING.load(Ordering::SeqCst);
        no_tunes_status(config_enabled, observer_present, running)
    }
}
```

`pause` removes and `start_observer` registers strictly on the main thread via `dispatch_on_main_sync`, and `OBSERVER` retains the pointer between the two. `resume` re-registers without re-running `terminate_music_apps`.

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::notunes::tests`
Expected: all notunes tests pass.

Run: `cargo clippy -p stache --lib -- -D warnings` and `cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/notunes/mod.rs
git commit -m "feat(notunes): main-thread observer retention and LifecycleModule"
```

---

## Task 13: proxyAudio — per-generation listeners with exact removal and worker completion + `ProxyAudioLifecycle`

**Files:**

- Modify: `app/native/src/modules/audio/watcher.rs`
- Modify: `app/native/src/modules/audio/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append a new `mod tests` block at the end of `watcher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::services::lifecycle::ModuleStatus;

    #[test]
    fn proxy_audio_status_maps_states() {
        assert_eq!(proxy_audio_status(false, true), ModuleStatus::ConfiguredOff);
        assert_eq!(proxy_audio_status(true, true), ModuleStatus::Running);
        assert_eq!(proxy_audio_status(true, false), ModuleStatus::Paused);
    }

    #[test]
    fn retained_listeners_holds_its_generation() {
        let (tx, _rx) = channel();
        let listeners = RetainedListeners {
            generation: 3,
            addresses: [AudioObjectPropertyAddress {
                mSelector: kAudioHardwarePropertyDefaultOutputDevice,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            }; 3],
            sender: Box::new(tx),
            worker: std::thread::spawn(|| {}),
        };
        assert_eq!(listeners.generation, 3);
        assert_eq!(listeners.addresses.len(), 3);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::audio::watcher::tests`
Expected: compilation fails — `proxy_audio_status` and `RetainedListeners` are undefined.

- [ ] **Step 3: Implement the per-generation listener runtime and lifecycle**

In `watcher.rs`, replace the header statics (`LISTENER_SENDER`, `AUDIO_WATCHER_ONCE` at watcher.rs:29-32) with:

```rust
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
```

Both `start_generation`'s rollback loop and `remove_generation` call `AudioObjectRemovePropertyListener`, which is not currently imported. Extend the existing `objc2_core_audio` import (watcher.rs:11-16):

```rust
use objc2_core_audio::{
    AudioDeviceID, AudioObjectAddPropertyListener, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectRemovePropertyListener, AudioObjectSetPropertyData, kAudioHardwareNoError,
    kAudioHardwarePropertyDefaultInputDevice, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
    kAudioObjectSystemObject,
};
```

```rust
/// One active generation of audio property listeners: the exact three
/// `AudioObjectPropertyAddress` values, the owned `Sender` whose heap address
/// is the `client_data` passed to CoreAudio, and the watcher worker so pause
/// can join it. A fresh value is built on every resume; pause removes the
/// listeners, disconnects the channel, and joins the worker before `RUNTIME`
/// is left empty.
struct RetainedListeners {
    generation: u64,
    addresses: [AudioObjectPropertyAddress; 3],
    sender: Box<Sender<()>>,
    worker: std::thread::JoinHandle<()>,
}

static RUNTIME: Mutex<Option<RetainedListeners>> = Mutex::new(None);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);
```

Remove the now-unused `OnceLock` and `spawn_named_thread` imports if the final code no longer references them. Replace `register_audio_listeners`, `init_audio_device_watcher`, and `start` with:

```rust
/// Builds the three default-device/device-list property addresses.
fn listener_addresses() -> [AudioObjectPropertyAddress; 3] {
    let scope = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let input = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let devices = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    [scope, input, devices]
}

/// Starts one generation of the audio watcher: registers the three listeners
/// with the exact same callback and `client_data` pointer, spawns the worker
/// thread, and publishes the runtime. Rolls back any already-registered
/// listeners on partial failure.
fn start_generation(config: &ProxyAudioConfig) -> Result<(), String> {
    if RUNTIME.lock().unwrap().is_some() {
        return Err("proxyAudio watcher already running".into());
    }

    let (tx, rx) = channel();
    let sender = Box::new(tx);
    // The Box's heap address is stable; this is the pointer CoreAudio holds.
    let tx_ptr: *mut c_void =
        std::ptr::from_ref::<Sender<()>>(sender.as_ref()).cast_mut().cast();

    let addresses = listener_addresses();
    let mut registered = 0usize;
    for addr in &addresses {
        let status = unsafe {
            AudioObjectAddPropertyListener(
                kAudioObjectSystemObject as AudioObjectID,
                NonNull::from(addr),
                Some(audio_device_property_listener),
                tx_ptr,
            )
        };
        if status != kAudioHardwareNoError {
            break;
        }
        registered += 1;
    }

    if registered != addresses.len() {
        for addr in &addresses[..registered] {
            unsafe {
                AudioObjectRemovePropertyListener(
                    kAudioObjectSystemObject as AudioObjectID,
                    NonNull::from(addr),
                    Some(audio_device_property_listener),
                    tx_ptr,
                );
            }
        }
        return Err("failed to register audio property listeners".into());
    }

    let generation = NEXT_GENERATION.fetch_add(1, Ordering::SeqCst);
    let config = config.clone();
    let worker = std::thread::Builder::new()
        .name("stache-audio-device-watcher".into())
        .spawn(move || {
            while rx.recv().is_ok() {
                on_audio_device_change(&config);
            }
        })
        .map_err(|e| format!("failed to spawn audio watcher thread: {e}"))?;

    *RUNTIME.lock().unwrap() = Some(RetainedListeners {
        generation,
        addresses,
        sender,
        worker,
    });
    Ok(())
}

/// Removes the current generation: unregisters the three listeners with the
/// exact callback/client_data used to register, disconnects the channel (the
/// worker's `recv` then errors and the worker exits), and joins the worker.
fn remove_generation() -> Result<(), String> {
    let Some(runtime) = RUNTIME.lock().unwrap().take() else {
        return Ok(());
    };
    let tx_ptr: *mut c_void =
        std::ptr::from_ref::<Sender<()>>(runtime.sender.as_ref()).cast_mut().cast();

    for addr in &runtime.addresses {
        let status = unsafe {
            AudioObjectRemovePropertyListener(
                kAudioObjectSystemObject as AudioObjectID,
                NonNull::from(addr),
                Some(audio_device_property_listener),
                tx_ptr,
            )
        };
        if status != kAudioHardwareNoError {
            tracing::warn!(status, "proxyAudio: listener removal reported an error");
        }
    }

    drop(runtime.sender);
    if runtime.worker.join().is_err() {
        tracing::error!("proxyAudio: audio watcher worker panicked");
    }
    Ok(())
}

/// Starts the audio device watcher (idempotent when a generation is running).
pub fn start(config: ProxyAudioConfig) {
    if RUNTIME.lock().unwrap().is_some() {
        return;
    }
    on_audio_device_change(&config);
    if let Err(e) = start_generation(&config) {
        tracing::error!(error = %e, "proxyAudio: failed to start watcher");
    }
}
```

Add the lifecycle impl and pure status function at the end of `watcher.rs` (before the tests module):

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

/// Pure status decision so the global `RUNTIME` slot is not required in tests.
fn proxy_audio_status(config_enabled: bool, runtime_present: bool) -> ModuleStatus {
    if !config_enabled {
        return ModuleStatus::ConfiguredOff;
    }
    if runtime_present {
        ModuleStatus::Running
    } else {
        ModuleStatus::Paused
    }
}

/// Tray-toggleable lifecycle handle for proxyAudio.
pub struct ProxyAudioLifecycle;

impl LifecycleModule for ProxyAudioLifecycle {
    fn name(&self) -> &'static str { "Proxy Audio" }
    fn id(&self) -> &'static str { "proxyAudio" }

    fn start(&self) -> Result<(), String> {
        if RUNTIME.lock().unwrap().is_some() {
            return Ok(());
        }
        let config = crate::config::get_config().proxy_audio.clone();
        on_audio_device_change(&config);
        start_generation(&config)
    }

    fn pause(&self) -> Result<(), String> { remove_generation() }

    fn resume(&self) -> Result<(), String> { self.start() }

    fn status(&self) -> ModuleStatus {
        let config_enabled = crate::config::get_config().proxy_audio.is_enabled();
        proxy_audio_status(config_enabled, RUNTIME.lock().unwrap().is_some())
    }
}
```

In `audio/mod.rs`, add the re-export:

```rust
pub use watcher::ProxyAudioLifecycle;
```

`start_generation` builds a fresh channel/worker per resume; `remove_generation` unregisters with the exact `Some(audio_device_property_listener)` callback and the exact `client_data` pointer, drops the sender to unblock `recv`, then `join()`s the worker before clearing the slot. No CoreAudio call happens while any lock is held.

> Keep `audio::init()` behavior: `watcher::start(config.proxy_audio.clone())` is still the startup entry; the lifecycle wrapper and `init` share the same `RUNTIME` slot so startup and tray start are mutually idempotent.

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::audio::watcher::tests`
Expected: both tests pass.

Run: `cargo check -p stache`, `cargo clippy -p stache --lib -- -D warnings`, and `cargo fmt --all -- --check`
Expected: clean (fix any unused-import warnings left by removing `OnceLock`/`spawn_named_thread`).

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/audio/watcher.rs app/native/src/modules/audio/mod.rs
git commit -m "feat(proxyAudio): per-generation listeners, exact removal, worker join"
```

---

## Task 14: menuAnywhere — retained tap with sound Send/Sync boundary + `MenuAnywhereLifecycle`

**Files:**

- Modify: `app/native/src/modules/menu_anywhere/event_monitor.rs`
- Modify: `app/native/src/modules/menu_anywhere/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `event_monitor.rs`:

```rust
    #[test]
    fn retained_event_tap_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RetainedEventTap>();
    }
```

Append to the `mod tests` block in `menu_anywhere/mod.rs`:

```rust
    #[test]
    fn menu_anywhere_status_maps_states() {
        use crate::modules::services::lifecycle::ModuleStatus;

        assert_eq!(
            menu_anywhere_status(false, true, true, Some(true)),
            ModuleStatus::ConfiguredOff
        );
        assert!(matches!(
            menu_anywhere_status(true, false, false, None),
            ModuleStatus::Unavailable(reason) if reason.contains("Accessibility")
        ));
        assert_eq!(
            menu_anywhere_status(true, true, true, Some(true)),
            ModuleStatus::Running
        );
        assert_eq!(
            menu_anywhere_status(true, true, true, Some(false)),
            ModuleStatus::Paused
        );
        assert_eq!(menu_anywhere_status(true, true, false, None), ModuleStatus::Paused);
        assert!(matches!(
            menu_anywhere_status(true, true, true, None),
            ModuleStatus::Unavailable(reason) if reason.contains("Accessibility")
        ));
    }
```

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::menu_anywhere`
Expected: compilation fails — `RetainedEventTap`, `menu_anywhere_status`, `ModuleStatus` are undefined.

- [ ] **Step 3: Implement retention, actual status, and lifecycle**

In `event_monitor.rs`, add the `CGEventTapIsEnabled` FFI declaration (after `CGEventTapEnable`):

```rust
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
```

Add after the pre-computed config statics (event_monitor.rs:70-71):

```rust
/// Retained event tap port. `CFMachPort` is a raw-pointer wrapper that is not
/// `Send`/`Sync`; the explicit impls are sound because the retained port is only
/// touched via `CGEventTapEnable`/`CGEventTapIsEnabled` (thread-safe) and the
/// port was retained (`wrap_under_create_rule` takes ownership).
#[derive(Clone)]
struct RetainedEventTap(CFMachPort);
unsafe impl Send for RetainedEventTap {}
unsafe impl Sync for RetainedEventTap {}

static EVENT_TAP: Mutex<Option<RetainedEventTap>> = Mutex::new(None);
```

In `start` (event_monitor.rs:104-107), retain the port before entering the run loop:

```rust
        let run_loop = CFRunLoop::get_current();
        run_loop.add_source(&run_loop_source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        *EVENT_TAP.lock() = Some(RetainedEventTap(tap_port.clone()));
        CFRunLoop::run_current();
```

Add at the end of `event_monitor.rs` (before the tests module):

```rust
/// Enables or disables the retained event tap.
///
/// # Errors
///
/// Returns an error if the tap was never created.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let tap = EVENT_TAP.lock().clone();
    match tap {
        Some(tap) => {
            unsafe { CGEventTapEnable(tap.0.as_concrete_TypeRef(), enabled) };
            Ok(())
        }
        None => Err("event tap handle not available".into()),
    }
}

/// Returns the real tap state: `Some(bool)` if the tap exists, `None` if the
/// tap was never created.
#[must_use]
pub fn tap_state() -> Option<bool> {
    let guard = EVENT_TAP.lock();
    guard.as_ref().map(|tap| unsafe { CGEventTapIsEnabled(tap.0.as_concrete_TypeRef()) })
}
```

In `menu_anywhere/mod.rs`, add the lifecycle impl and pure status function at the end (before the tests module):

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

/// Pure status decision: config gate, then accessibility, then the real tap state.
fn menu_anywhere_status(
    config_enabled: bool,
    accessibility: bool,
    running: bool,
    tap_state: Option<bool>,
) -> ModuleStatus {
    if !config_enabled {
        return ModuleStatus::ConfiguredOff;
    }
    if !accessibility {
        return ModuleStatus::Unavailable("Accessibility permission required".into());
    }
    match tap_state {
        None if running => ModuleStatus::Unavailable(
            "event tap creation failed — check Accessibility permission".into(),
        ),
        None => ModuleStatus::Paused,
        Some(true) => ModuleStatus::Running,
        Some(false) => ModuleStatus::Paused,
    }
}

/// Tray-toggleable lifecycle handle for MenuAnywhere.
pub struct MenuAnywhereLifecycle {
    app_handle: tauri::AppHandle,
}

impl MenuAnywhereLifecycle {
    #[must_use]
    pub fn new(app_handle: tauri::AppHandle) -> Self { Self { app_handle } }
}

impl LifecycleModule for MenuAnywhereLifecycle {
    fn name(&self) -> &'static str { "Menu Anywhere" }
    fn id(&self) -> &'static str { "menuAnywhere" }

    fn start(&self) -> Result<(), String> {
        if IS_RUNNING.load(Ordering::SeqCst) {
            return Ok(());
        }
        init(self.app_handle.clone());
        Ok(())
    }

    fn pause(&self) -> Result<(), String> { event_monitor::set_enabled(false) }

    fn resume(&self) -> Result<(), String> { event_monitor::set_enabled(true) }

    fn status(&self) -> ModuleStatus {
        let config = crate::config::get_config();
        let config_enabled = config.menu_anywhere.is_enabled();
        let accessibility = crate::is_accessibility_granted();
        let running = IS_RUNNING.load(Ordering::SeqCst);
        let tap_state = event_monitor::tap_state();
        menu_anywhere_status(config_enabled, accessibility, running, tap_state)
    }
}
```

`resume` re-enables the same retained tap and never re-runs `init`, so the run-loop thread is not duplicated. `pause`/`resume` clone the port out of the guard before the OS call.

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::menu_anywhere`
Expected: all menu_anywhere tests pass.

Run: `cargo clippy -p stache --lib -- -D warnings` and `cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/menu_anywhere/event_monitor.rs app/native/src/modules/menu_anywhere/mod.rs
git commit -m "feat(menuAnywhere): retain tap with Send/Sync boundary and real status"
```

---

## Task 16: Fixed `Arc` lifecycle registry + startup wiring

**Files:**

- Create: `app/native/src/modules/lifecycle_registry.rs`
- Modify: `app/native/src/modules/mod.rs`
- Modify: `app/native/src/lib.rs`

- [ ] **Step 1: Write the failing tests and implementation**

Create `lifecycle_registry.rs`:

```rust
use std::sync::Arc;

use parking_lot::Mutex;

use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

/// Fixed collection of all toggleable modules, registered once at startup and
/// never mutated afterwards. `toggle` clones the `Arc` out of the guard and
/// drops the lock before calling `pause`/`resume`, so slow OS calls never block
/// concurrent lookups or registrations.
pub struct LifecycleRegistry {
    modules: Mutex<Vec<Arc<dyn LifecycleModule>>>,
}

impl LifecycleRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { modules: Mutex::new(Vec::new()) }
    }

    /// Registers a module. Call only during startup, before any reader.
    pub fn register(&self, module: Arc<dyn LifecycleModule>) {
        self.modules.lock().push(module);
    }

    /// Snapshot of all registered modules.
    #[must_use]
    pub fn modules(&self) -> Vec<Arc<dyn LifecycleModule>> {
        self.modules.lock().clone()
    }

    /// Finds a module by id without calling into it.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<dyn LifecycleModule>> {
        self.modules.lock().iter().find(|m| m.id() == id).cloned()
    }

    /// Toggles a module: pause if running, resume if paused.
    ///
    /// Rejects `ConfiguredOff` and `Unavailable` modules. The lock is dropped
    /// before any OS call.
    ///
    /// # Errors
    ///
    /// Returns an error if the module is missing, not toggleable, or the
    /// pause/resume call fails.
    pub fn toggle(&self, id: &str) -> Result<ModuleStatus, String> {
        let module = self.get(id).ok_or_else(|| format!("unknown module {id:?}"))?;
        match module.status() {
            ModuleStatus::ConfiguredOff => Err(format!("{id} is disabled in config")),
            ModuleStatus::Unavailable(reason) => Err(format!("{id} is unavailable: {reason}")),
            ModuleStatus::Running => {
                module.pause()?;
                Ok(module.status())
            }
            ModuleStatus::Paused => {
                module.resume()?;
                Ok(module.status())
            }
        }
    }
}

impl Default for LifecycleRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    struct MockModule {
        id: &'static str,
        name: &'static str,
        state: parking_lot::Mutex<ModuleStatus>,
    }

    impl MockModule {
        fn new(id: &'static str, name: &'static str, state: ModuleStatus) -> Arc<Self> {
            Arc::new(Self { id, name, state: parking_lot::Mutex::new(state) })
        }
    }

    impl LifecycleModule for MockModule {
        fn name(&self) -> &'static str { self.name }
        fn id(&self) -> &'static str { self.id }
        fn start(&self) -> Result<(), String> {
            *self.state.lock() = ModuleStatus::Running;
            Ok(())
        }
        fn pause(&self) -> Result<(), String> {
            *self.state.lock() = ModuleStatus::Paused;
            Ok(())
        }
        fn resume(&self) -> Result<(), String> {
            *self.state.lock() = ModuleStatus::Running;
            Ok(())
        }
        fn status(&self) -> ModuleStatus { self.state.lock().clone() }
    }

    /// Pause blocks until released, simulating a slow OS call (e.g. tiling teardown).
    struct BlockingPauseModule {
        id: &'static str,
        name: &'static str,
        state: parking_lot::Mutex<ModuleStatus>,
        started: mpsc::Sender<()>,
        release: std::sync::Mutex<mpsc::Receiver<()>>,
    }

    impl LifecycleModule for BlockingPauseModule {
        fn name(&self) -> &'static str { self.name }
        fn id(&self) -> &'static str { self.id }
        fn start(&self) -> Result<(), String> {
            *self.state.lock() = ModuleStatus::Running;
            Ok(())
        }
        fn pause(&self) -> Result<(), String> {
            let _ = self.started.send(());
            let _ = self.release.lock().unwrap().recv();
            *self.state.lock() = ModuleStatus::Paused;
            Ok(())
        }
        fn resume(&self) -> Result<(), String> {
            *self.state.lock() = ModuleStatus::Running;
            Ok(())
        }
        fn status(&self) -> ModuleStatus { self.state.lock().clone() }
    }

    #[test]
    fn register_and_get_by_id() {
        let registry = LifecycleRegistry::new();
        registry.register(MockModule::new("a", "A", ModuleStatus::Running));
        assert!(registry.get("a").is_some());
        assert!(registry.get("missing").is_none());
    }

    #[test]
    fn modules_snapshot_returns_all() {
        let registry = LifecycleRegistry::new();
        registry.register(MockModule::new("a", "A", ModuleStatus::Running));
        registry.register(MockModule::new("b", "B", ModuleStatus::Paused));
        assert_eq!(registry.modules().len(), 2);
    }

    #[test]
    fn toggle_running_pauses_and_returns_paused() {
        let registry = LifecycleRegistry::new();
        registry.register(MockModule::new("a", "A", ModuleStatus::Running));
        assert_eq!(registry.toggle("a").unwrap(), ModuleStatus::Paused);
    }

    #[test]
    fn toggle_paused_resumes_and_returns_running() {
        let registry = LifecycleRegistry::new();
        registry.register(MockModule::new("a", "A", ModuleStatus::Paused));
        assert_eq!(registry.toggle("a").unwrap(), ModuleStatus::Running);
    }

    #[test]
    fn toggle_unknown_id_errors() {
        let registry = LifecycleRegistry::new();
        assert!(registry.toggle("missing").unwrap_err().contains("unknown module"));
    }

    #[test]
    fn toggle_configured_off_errors() {
        let registry = LifecycleRegistry::new();
        registry.register(MockModule::new("a", "A", ModuleStatus::ConfiguredOff));
        assert!(registry.toggle("a").unwrap_err().contains("disabled"));
    }

    #[test]
    fn toggle_unavailable_errors() {
        let registry = LifecycleRegistry::new();
        registry.register(MockModule::new("a", "A", ModuleStatus::Unavailable("perm".into())));
        assert!(registry.toggle("a").unwrap_err().contains("unavailable"));
    }

    #[test]
    fn toggle_never_holds_registry_lock_during_os_call() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let registry = Arc::new(LifecycleRegistry::new());
        registry.register(Arc::new(BlockingPauseModule {
            id: "a",
            name: "A",
            state: parking_lot::Mutex::new(ModuleStatus::Running),
            started: started_tx,
            release: std::sync::Mutex::new(release_rx),
        }));

        let registry2 = Arc::clone(&registry);
        let handle = std::thread::spawn(move || {
            registry2.toggle("a").unwrap();
        });

        // pause() is now in flight, blocked on its release channel.
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        // The registry lock must be free while toggle() runs the OS call.
        assert!(registry.get("a").is_some());
        registry.register(MockModule::new("b", "B", ModuleStatus::Paused));
        assert_eq!(registry.modules().len(), 2);

        release_tx.send(()).unwrap();
        handle.join().unwrap();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::lifecycle_registry::tests`
Expected: compilation fails — `crate::modules::lifecycle_registry` is not declared in `modules/mod.rs`.

- [ ] **Step 3: Declare the module and wire startup**

In `modules/mod.rs`, add (alphabetical):

```rust
pub mod lifecycle_registry;
```

In `lib.rs`, add imports:

```rust
use std::sync::Arc;

use modules::lifecycle_registry::LifecycleRegistry;
```

In the `.setup` closure, after the activation-policy block and **before** `load_base_modules(app);`:

```rust
            // Phase 4: the registry must exist before base modules and before
            // the background startup reads it. Registration starts nothing —
            // each module stays lazily initialized.
            let registry = LifecycleRegistry::new();
            registry.register(Arc::new(wallpaper::WallpaperLifecycle));
            registry.register(Arc::new(cmd_q::CmdQLifecycle::new(app.handle().clone())));
            registry.register(Arc::new(notunes::NoTunesLifecycle));
            registry.register(Arc::new(audio::ProxyAudioLifecycle));
            registry.register(Arc::new(menu_anywhere::MenuAnywhereLifecycle::new(
                app.handle().clone(),
            )));
            registry.register(Arc::new(tiling::TilingLifecycle::new(app.handle().clone())));
            app.manage(registry);
```

In `lazy_load_modules`, at the end of the async task (after the tiling `if` block, before `tracing::info!("background initialization complete");`):

```rust
            // Install the Modules submenu only after every background module has
            // had a chance to start, so the initial check states are accurate.
            tray::install_modules_submenu(&handle);
```

> To keep this commit compiling before Task 17 lands, add a temporary stub in `tray/mod.rs`:

```rust
/// Placeholder; replaced by Task 17.
pub fn install_modules_submenu(_app: &tauri::AppHandle) {}
```

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::lifecycle_registry::tests`
Expected: all registry tests pass, including the lock-availability test.

Run: `cargo check -p stache`, `cargo clippy -p stache --lib -- -D warnings`, `cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/lifecycle_registry.rs app/native/src/modules/mod.rs app/native/src/lib.rs
git commit -m "feat(lifecycle): fixed Arc registry wired before base modules"
```

---

## Task 17: Retained tray + deferred Modules submenu + async toggles

**Files:**

- Modify: `app/native/src/modules/tray/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append a new `mod tests` block at the end of `tray/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_state_maps_running() {
        assert_eq!(
            item_state(&ModuleStatus::Running, "Wallpapers"),
            (true, true, "Wallpapers".to_string())
        );
    }

    #[test]
    fn item_state_maps_paused() {
        assert_eq!(
            item_state(&ModuleStatus::Paused, "NoTunes"),
            (false, true, "NoTunes".to_string())
        );
    }

    #[test]
    fn item_state_maps_configured_off() {
        assert_eq!(
            item_state(&ModuleStatus::ConfiguredOff, "Tiling"),
            (false, false, "Tiling".to_string())
        );
    }

    #[test]
    fn item_state_maps_unavailable_with_reason() {
        assert_eq!(
            item_state(
                &ModuleStatus::Unavailable("Accessibility permission required".into()),
                "Menu Anywhere"
            ),
            (false, false, "Menu Anywhere (Accessibility permission required)".to_string())
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (RED)**

Run: `cargo test -p stache --lib modules::tray::tests`
Expected: compilation fails — `item_state` is undefined.

- [ ] **Step 3: Implement the retained tray state and deferred submenu**

Replace the whole body of `tray/mod.rs` (keep `RELOAD_ID`/`QUIT_ID` and the `app_shutdown` routing):

```rust
//! System tray module for Stache.
//!
//! Provides a system tray icon with a menu for quick access to app actions and
//! module pause/resume toggles.

use std::collections::HashMap;

use parking_lot::Mutex;
use tauri::{App, Manager};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::Wry;

use crate::modules::lifecycle_registry::LifecycleRegistry;
use crate::modules::services::lifecycle::ModuleStatus;

/// Menu item ID for the reload action (production only).
#[cfg(not(debug_assertions))]
const RELOAD_ID: &str = "reload";

/// Menu item ID for the quit action.
const QUIT_ID: &str = "quit";

/// Retained tray state so the icon and check items survive after setup and can
/// be updated after a toggle.
pub struct TrayMenuState {
    tray: TrayIcon<Wry>,
    modules_submenu: Submenu<Wry>,
    check_items: Mutex<HashMap<String, CheckMenuItem<Wry>>>,
}

/// Maps a module status to the `(checked, enabled, text)` a CheckMenuItem needs.
fn item_state(status: &ModuleStatus, name: &str) -> (bool, bool, String) {
    match status {
        ModuleStatus::Running => (true, true, name.to_string()),
        ModuleStatus::Paused => (false, true, name.to_string()),
        ModuleStatus::ConfiguredOff => (false, false, name.to_string()),
        ModuleStatus::Unavailable(reason) => (false, false, format!("{name} ({reason})")),
    }
}

impl TrayMenuState {
    /// Builds one CheckMenuItem per registered module and appends them to the
    /// retained Modules submenu. Idempotent.
    fn install_modules(&self, app: &tauri::AppHandle, registry: &LifecycleRegistry) {
        {
            let guard = self.check_items.lock();
            if !guard.is_empty() {
                return;
            }
        }

        let mut items: Vec<CheckMenuItem<Wry>> = Vec::new();
        for module in registry.modules() {
            let (checked, enabled, text) = item_state(&module.status(), module.name());
            let Ok(item) =
                CheckMenuItem::with_id(app, module.id(), text, enabled, checked, None::<&str>)
            else {
                tracing::error!(module = module.id(), "tray: failed to create check menu item");
                continue;
            };
            items.push(item.clone());
            self.check_items.lock().insert(module.id().to_string(), item);
        }
        if items.is_empty() {
            return;
        }

        let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
            items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        if let Err(e) = self.modules_submenu.append_items(&refs) {
            tracing::error!(error = %e, "tray: failed to append module items");
        }
    }

    /// Handles a module toggle: disables the item immediately on the main
    /// thread, runs the OS pause/resume off-thread, then refreshes the item on
    /// the main thread.
    fn handle_toggle(&self, app: &tauri::AppHandle, id: &str) {
        let Some(item) = self.check_items.lock().get(id).cloned() else {
            return;
        };
        let _ = item.set_enabled(false);

        let Some(registry) = app.try_state::<LifecycleRegistry>() else {
            let _ = item.set_enabled(true);
            return;
        };
        let Some(module) = registry.get(id) else {
            let _ = item.set_enabled(true);
            return;
        };

        let module_name = module.name();
        let id_owned = id.to_string();
        let app = app.clone();
        crate::platform::thread::spawn_named_thread("tray-toggle", move || {
            let result = match module.status() {
                ModuleStatus::ConfiguredOff => Err(format!("{id_owned} is disabled in config")),
                ModuleStatus::Unavailable(reason) => {
                    Err(format!("{id_owned} is unavailable: {reason}"))
                }
                ModuleStatus::Running => module.pause().and_then(|_| Ok(module.status())),
                ModuleStatus::Paused => module.resume().and_then(|_| Ok(module.status())),
            };
            let _ = app.run_on_main_thread(move || match result {
                Ok(status) => {
                    let (checked, enabled, text) = item_state(&status, module_name);
                    let _ = item.set_enabled(enabled);
                    let _ = item.set_checked(checked);
                    let _ = item.set_text(text);
                }
                Err(err) => {
                    tracing::warn!(module = %id_owned, error = %err, "tray: module toggle failed");
                    let _ = item.set_enabled(true);
                }
            });
        });
    }
}

/// Initializes the system tray icon and menu.
///
/// The base menu (Reload in release, Modules placeholder, Quit) is installed
/// immediately; module check items are appended by [`install_modules_submenu`]
/// only after background startup completes.
pub fn init(app: &App) {
    let handle = app.handle();

    let quit_item = MenuItem::with_id(handle, QUIT_ID, "Quit Stache", true, None::<&str>)
        .expect("failed to create quit menu item");

    #[cfg(not(debug_assertions))]
    let reload_item = MenuItem::with_id(handle, RELOAD_ID, "Reload Stache", true, None::<&str>)
        .expect("failed to create reload menu item");

    let empty_items: [&dyn tauri::menu::IsMenuItem<Wry>; 0] = [];
    let modules_submenu = Submenu::with_items(handle, "Modules", true, &empty_items)
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
        .on_menu_event(|app, event| match event.id.as_ref() {
            #[cfg(not(debug_assertions))]
            RELOAD_ID => crate::app_shutdown::restart(app),
            QUIT_ID => crate::app_shutdown::exit(app),
            id => {
                if let Some(state) = app.try_state::<TrayMenuState>() {
                    state.handle_toggle(app, id);
                }
            }
        })
        .build(handle)
        .expect("failed to build system tray icon");

    app.manage(TrayMenuState {
        tray,
        modules_submenu,
        check_items: Mutex::new(HashMap::new()),
    });

    tracing::debug!("system tray initialized");
}

/// Appends the Modules submenu check items. Called after background startup.
pub fn install_modules_submenu(app: &tauri::AppHandle) {
    let state = app.state::<TrayMenuState>();
    let registry = app.state::<LifecycleRegistry>();
    state.install_modules(app, &registry);
}
```

This retains the `TrayIcon` and every `CheckMenuItem` (in `TrayMenuState`), keeps Quit/Reload immediately usable and routed through `crate::app_shutdown::exit/restart`, installs the Modules items only from `install_modules_submenu`, and makes toggles async: item disabled on the main thread in the event handler, OS call on a named worker thread, then item refreshed on the main thread via `run_on_main_thread`.

- [ ] **Step 4: Run tests to verify they pass (GREEN)**

Run: `cargo test -p stache --lib modules::tray::tests`
Expected: all four `item_state` tests pass.

Run: `cargo test -p stache --lib`, `cargo clippy -p stache --lib -- -D warnings`, `cargo fmt --all -- --check`
Expected: full library test suite green, clippy clean, fmt clean. Verify `git status --short` still shows ` M app/native/src/modules/audio/device.rs` unstaged.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tray/mod.rs
git commit -m "feat(tray): retained tray and check items, deferred Modules submenu, async toggles"
```

---

## Task 18: Release manual verification

**Files:** none (manual verification; no commit)

- [ ] **Step 1: Preflight**

Run: `git status --short` and `git hash-object app/native/src/modules/audio/device.rs`
Expected: `device.rs` is ` M` (unstaged) and the hash is `50451982cc8ec2064079a2acec1170dfb49dec38`. Never stage or modify it during this session.

Run: `pnpm test` and `cargo clippy -p stache --lib -- -D warnings`
Expected: full suite green, clippy clean, before opening the app.

- [ ] **Step 2: Build and launch release**

Run: `cd /Users/marcosmoura/Projects/stache && pnpm tauri:build && open target/release/stache.app`
Grant Accessibility when prompted (tray toggles for tiling/menuAnywhere and event taps need it). Confirm the tray icon appears immediately — **before** the background modules finish — with "Quit Stache" (and "Reload Stache", release only).

- [ ] **Step 3: Wait for the Modules submenu**

Within a few seconds the "Modules" submenu appears with six items. Initial state must match config: enabled modules show a checkmark; disabled (`enabled: false`) modules are grayed/locked.

- [ ] **Step 4: Toggle every module and verify the OS resource actually stops**

- **Wallpapers**: with `interval > 0`, pause, wait longer than one interval, confirm the wallpaper does not change; resume and confirm cycling resumes.
- **commandQuit**: while running, press ⌘Q → the "Hold ⌘Q to quit" alert appears and the frontmost app is not quit; while paused, ⌘Q behaves normally (no suppression, no alert).
- **notunes**: while running, `open -a Music` → the process is force-terminated (and the target app opens if configured); while paused, `open -a Music` stays running.
- **proxyAudio**: while running, plug/unplug an output device → priority routing applies; while paused, no routing happens. Re-attach the device to re-verify on resume.
- **menuAnywhere**: while running, trigger the configured mouse+modifier combo → the frontmost app's menu pops at the cursor; while paused, nothing happens.
- **tiling**: while running, drag/resize a window and change workspaces → layout and borders react; while paused, no reactions and Stache-hidden apps are restored.

After each toggle the item must re-enable, the checkmark must reflect the new state, and the label must stay unchanged.

- [ ] **Step 5: Verify Unavailable and locked states**

Revoke or block Accessibility (or configure wallpapers with no loadable images), then relaunch: affected items show a parenthesized reason (e.g. `Command Quit (event tap creation failed — check Accessibility permission)`) and are grayed/locked — clicking them must not change state.

- [ ] **Step 6: Verify Quit/Reload and reset**

Click "Quit Stache" → the app exits after orderly cleanup (Stache-hidden apps restored, tiling stopped, IPC stopped). Click "Reload Stache" (release) → the app restarts. After any restart, all Modules toggles return to their config-driven default state.

- [ ] **Step 7: Report and guard the tree**

Report results. Run `git status --short` — expect ` M app/native/src/modules/audio/device.rs` (protected, still unstaged) plus the already-committed changes. No commit in this task.

---

## Verification summary (Tasks 10-14, 16-18)

```bash
cargo test -p stache --lib modules::wallpaper::manager::tests
cargo test -p stache --lib modules::cmd_q::tests
cargo test -p stache --lib modules::notunes::tests
cargo test -p stache --lib modules::audio::watcher::tests
cargo test -p stache --lib modules::menu_anywhere
cargo test -p stache --lib modules::lifecycle_registry::tests
cargo test -p stache --lib modules::tray::tests
cargo test -p stache --lib
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
git status --short            # device.rs must remain  M and unstaged
git diff --cached --name-only # must never list device.rs
```

The protected `app/native/src/modules/audio/device.rs` (blob `50451982cc8ec2064079a2acec1170dfb49dec38`) is never modified, staged, or committed; every commit above stages only explicit paths.
