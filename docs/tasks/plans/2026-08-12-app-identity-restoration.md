# App Identity + Shutdown Restoration — Implementation Plan (Tasks 19A-19G)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the per-PID generation/FIFO hidden-app ownership machine with `AppIdentity`-keyed ownership through an actor-single-writer, sealed `VisibilityRegistry`, propagate exact `WindowTarget` through every delayed/tab/effect/cache/animation/drag/routing path, and restore only Stache-hidden applications before tiling teardown using exact-instance validation.

**Architecture:** `AppIdentity = (pid, launchDate f64 bits)` is captured once per `NSRunningApplication` object inside an autorelease pool and saved as `Send + Sync` bytes. `WindowTarget = (AppIdentity, window_id)` replaces every bare numeric ID on AX-derived, delayed, cached, and animated operations. A passive `Arc<VisibilityRegistry>` (sealed `BTreeSet<AppIdentity>` behind one `parking_lot::Mutex`) is shared between `StateActor` and `StateActorHandle`; the actor is the sole runtime writer through closure-based `hide_if_open`/`unhide_if_open`/`relinquish_if_open`, and the handle exposes the only external mutation (`seal_and_drain_visibility`). EventProcessor forwards raw lifecycle events keyed by identity; the actor revalidates each against current OS state. Tasks 19D+19E are one atomic uncommitted cutover.

**Tech Stack:** Rust (Tauri 2.x), `objc` v0.2.7 (`objc::rc::autoreleasepool`), `parking_lot`, `dashmap`, `eyeball`, existing `platform::objc::{nsstring, nsstring_to_string}`.

**Prerequisite (must be merged first):** `2026-08-12-restartable-tiling-runtime.md` — `RuntimeSlot::{Empty, Running(TilingRuntime), Quarantined(PartialRuntime)}`, `CompletionLatch`, `start_runtime(app_handle)`, `pause_runtime`, `get_handle` (owned), `current_generation`, `TilingLifecycle`. Task 19C consumes the `(StateActorHandle, CompletionLatch)` spawn contract.

**Spec:** `docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md` Phase 6.
**Supersedes:** the Task 19A-19G text in `docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md`, and commits `f2e20c8..95fa2f5` (generation/FIFO/PID-set ownership).

**Corrections applied vs. the old plan text (ba076a1):** workspace cycling (`on_cycle_workspace`) is untouched and excluded from the `VisibilityDelta` migration; `events/ax_observer.rs` only forwards identity (capture lives in `events/observer.rs`); all observer identity captures use `objc::rc::autoreleasepool`; registry relinquishment is seal-aware and actor-owned (`relinquish_if_open`); 19D+19E is one atomic uncommitted cutover; `app_shutdown.rs` is never edited; no lock is held across main-thread dispatch/waits.

**Protected:** `app/native/src/modules/audio/device.rs` (unstaged user edit, blob `50451982cc8ec2064079a2acec1170dfb49dec38`) and `app/native/src/app_shutdown.rs` (Task 20 CAS arbiter). Never `git add -A`/`git add .`/`git reset --hard`/`git checkout .`/`git stash`/`git commit -am`. Commit only explicit paths; verify `git hash-object` before every commit.

**File structure**

| File                                                                         | Responsibility                                                                                                                                                                                                                          |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app/native/src/modules/tiling/identity.rs`                                  | **New** (19A). `AppIdentity`, `LaunchDateBits`, `WindowTarget`; fail-closed capture/validation                                                                                                                                          |
| `app/native/src/modules/tiling/mod.rs`                                       | `pub mod identity;` (19A); re-export `VisibilityRegistry` (19C)                                                                                                                                                                         |
| `app/native/src/modules/tiling/state/types.rs`                               | `Window.identity: Option<AppIdentity>` (19B-1)                                                                                                                                                                                          |
| `app/native/src/modules/tiling/actor/messages.rs`                            | `WindowCreatedInfo.identity`; `StateMessage` lifecycle + AX-derived variants carry identity; `GeometryUpdate.identity`; `SetExpectedFrames` → `Vec<(WindowTarget, Rect)>`; `GetWindowLayoutTargets`/`GetFocusTargets` + results (19B-2) |
| `app/native/src/modules/tiling/events/types.rs`                              | `WindowEvent.identity: AppIdentity` (non-optional) + `WindowEvent::new` (19B-2)                                                                                                                                                         |
| `app/native/src/modules/tiling/events/observer.rs`                           | `ObserverState` address-keyed + `identity_to_observer` reverse index; `ObserverRecord`; autoreleasepool identity capture; `remove_observer_for_identity`; pure `take_observer_record_for_identity` seam (19B-2)                         |
| `app/native/src/modules/tiling/events/ax_observer.rs`                        | Forwards `event.identity` only (19B-2)                                                                                                                                                                                                  |
| `app/native/src/modules/tiling/events/app_monitor.rs`                        | `extract_app_info` returns `Option<AppIdentity>`; drop `None`; `remove_observer_for_identity` on termination (19B-2)                                                                                                                    |
| `app/native/src/modules/tiling/events/processor.rs`                          | Exact routing `DashMap<(AppIdentity, u32), u32>`; identity-keyed destroy detection; batched geometry with identity; raw forwarding (19B-2, 19E)                                                                                         |
| `app/native/src/modules/tiling/events/drag_state.rs`                         | `WindowSnapshot.target: WindowTarget`; exact drag identity (19B-2)                                                                                                                                                                      |
| `app/native/src/modules/tiling/tabs.rs`                                      | `window_to_identity: HashMap<u32, AppIdentity>`; exact tab APIs; post-scan identity revalidation (19B-2)                                                                                                                                |
| `app/native/src/modules/tiling/actor/handlers/window.rs`                     | Identity-upgrade + PID-reuse removal; `VisibilityDelta` (19B-2, 19D)                                                                                                                                                                    |
| `app/native/src/modules/tiling/actor/handlers/workspace.rs`                  | `on_switch_workspace`/`on_send_workspace_to_screen` return `VisibilityDelta`. **`on_cycle_workspace` untouched** (19D)                                                                                                                  |
| `app/native/src/modules/tiling/actor/handlers/app.rs`                        | Exact-identity revalidation + termination cleanup (19E)                                                                                                                                                                                 |
| `app/native/src/modules/tiling/actor/mod.rs`                                 | `registry` field; `spawn_with_registry`; actor hide/unhide/revalidate methods; `sync_visibility_for_workspaces` (19C, 19D+19E)                                                                                                          |
| `app/native/src/modules/tiling/actor/handle.rs`                              | `registry: Arc<VisibilityRegistry>`; `seal_and_drain_visibility`; `new_with_registry` (19C)                                                                                                                                             |
| `app/native/src/modules/tiling/state/tiling_state.rs`                        | `windows_identity_iter` (19B-1)                                                                                                                                                                                                         |
| `app/native/src/modules/tiling/effects/mod.rs`                               | Effect variants carry `WindowTarget`; `RefreshActiveBorder` replaces `UpdateBorder` (19B-2)                                                                                                                                             |
| `app/native/src/modules/tiling/effects/subscriber.rs`                        | Consumes `TargetLayout`/`TargetFocus`; `FloatingChanged` carries `WindowTarget`; no direct `borders::on_focus_changed` (19B-2)                                                                                                          |
| `app/native/src/modules/tiling/effects/executor.rs`                          | Exact-target batches; validates before border/focus/raise/visibility/SkyLight/AX (19B-2)                                                                                                                                                |
| `app/native/src/modules/tiling/effects/window_cache.rs`                      | `windows: DashMap<WindowTarget, …>`, `apps: DashMap<AppIdentity, …>`; exact resolve (19B-2)                                                                                                                                             |
| `app/native/src/modules/tiling/effects/window_ops.rs`                        | Exact-target ops; identity-aware hide/unhide/is-hidden helpers (19B-2, 19D, 19E, 19G)                                                                                                                                                   |
| `app/native/src/modules/tiling/effects/animation/{mod,state,transition}.rs`  | `WindowTransition.target: WindowTarget`; interrupted positions `DashMap<WindowTarget, Rect>` (19B-2)                                                                                                                                    |
| `app/native/src/modules/tiling/actor/handlers/{preset,focus,window_move}.rs` | Exact targets (19B-2)                                                                                                                                                                                                                   |
| `app/native/src/modules/tiling/rules/mod.rs`, `state/tiling_state.rs`        | `Window` literals gain `identity: None` (19B-1)                                                                                                                                                                                         |
| `app/native/src/modules/tiling/init.rs`                                      | Exact initial scan/focus targets; pause drains + fresh registry per resume; `on_init_complete(&mut self)` (19B-2, 19D, 19F)                                                                                                             |
| `app/native/src/modules/bar/components/tiling.rs`                            | Resolve stored `Window.identity` → `WindowTarget` (19B-2)                                                                                                                                                                               |
| `app/native/src/modules/tiling/visibility.rs`                                | 19C adds `VisibilityRegistry` beside old tracker; 19D cuts restoration over; 19F tests; 19G deletes old machine                                                                                                                         |

---

## Task 19A: `AppIdentity`, `LaunchDateBits`, `WindowTarget`

**Files:**

- Create: `app/native/src/modules/tiling/identity.rs`
- Modify: `app/native/src/modules/tiling/mod.rs` (add `pub mod identity;`)

- [ ] **Step 1: Add the module declaration and write the tests**

Add `pub mod identity;` to `tiling/mod.rs`. Create `identity.rs` with the implementation and tests:

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
    /// Returns `None` if pid ≤ 0, launchDate is null, or the time interval is
    /// not finite and positive. This is fail-closed: callers must skip the app
    /// rather than proceeding with an invalid identity.
    ///
    /// # Safety
    ///
    /// `app` must be a valid non-null `*mut Object` pointing to an
    /// `NSRunningApplication` instance for the duration of this call.
    #[must_use]
    pub unsafe fn from_ns_running_app(app: *mut objc::runtime::Object) -> Option<Self> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_from_null_fails_closed() {
        let identity = unsafe { AppIdentity::from_ns_running_app(std::ptr::null_mut()) };
        assert!(identity.is_none());
    }

    #[test]
    fn identity_ord_deterministic() {
        let a = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(1000.0).unwrap(),
        };
        let b = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(2000.0).unwrap(),
        };
        let c = AppIdentity {
            pid: 20,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(1000.0).unwrap(),
        };
        assert!(a < b);
        assert!(a < c);
        assert!(b < c);
    }

    #[test]
    fn identity_equality_pid_and_launch_date() {
        let a = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(42.0).unwrap(),
        };
        let b = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(42.0).unwrap(),
        };
        let c = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(43.0).unwrap(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn launch_date_bits_rejects_non_positive_and_non_finite() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(LaunchDateBits::from_time_interval_since_reference_date(bad).is_none());
        }
    }

    #[test]
    fn window_target_distinguishes_same_window_id() {
        let a = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(42.0).unwrap(),
        };
        let b = AppIdentity {
            pid: 10,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(43.0).unwrap(),
        };
        assert_ne!(
            WindowTarget { identity: a, window_id: 99 },
            WindowTarget { identity: b, window_id: 99 },
        );
    }
}
```

- [ ] **Step 2: Run tests, verify GREEN (with an intermediate RED)**

First run without creating `identity.rs` to observe RED, then create it:

```bash
cargo test -p stache --lib modules::tiling::identity::tests 2>&1 | tail -10
# Expected before creating the file: compilation fails (module/type missing).
# Expected after creating the file:
cargo test -p stache --lib modules::tiling::identity::tests
cargo fmt --all -- --check
cargo check -p stache
```

Expected: all pass. The null-pointer test proves the fail-closed path without an ObjC runtime.

- [ ] **Step 3: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/identity.rs app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): add AppIdentity with capture/validation"
```

---

## Task 19B-1: Optional identity on `Window` and `WindowCreatedInfo`

Additive; compiles and passes in isolation. All struct literals are updated in this same commit.

**Files:**

- Modify: `app/native/src/modules/tiling/state/types.rs:308` — `pub identity: Option<AppIdentity>` on `Window` and `Window::default()`
- Modify: `app/native/src/modules/tiling/actor/messages.rs:271` — `pub identity: Option<AppIdentity>` on `WindowCreatedInfo`
- Modify: `app/native/src/modules/tiling/state/tiling_state.rs` — `windows_identity_iter`
- Modify: every struct-literal call site: `actor/handlers/window.rs`, `init.rs` (initial scan), `rules/mod.rs`, `state/tiling_state.rs`, `events/ax_observer.rs` (`WindowCreatedInfo` in `handle_window_created`), and all `#[cfg(test)]` `Window { … }` literals under `app/native/src/modules/tiling/` and `app/native/src/modules/bar/components/tiling.rs`

- [ ] **Step 1: Write the failing test**

Add to `state/tiling_state.rs` tests:

```rust
    #[test]
    fn windows_identity_iter_filters_exact_identity() {
        use crate::modules::tiling::identity::{AppIdentity, LaunchDateBits};
        let a = AppIdentity { pid: 42, launch_date: LaunchDateBits::from_time_interval_since_reference_date(100.0).unwrap() };
        let b = AppIdentity { pid: 42, launch_date: LaunchDateBits::from_time_interval_since_reference_date(200.0).unwrap() };
        let mut state = TilingState::new();
        state.upsert_window(Window { id: 1, pid: 42, identity: Some(a), workspace_id: Uuid::nil(), ..Window::default() });
        state.upsert_window(Window { id: 2, pid: 42, identity: Some(b), workspace_id: Uuid::nil(), ..Window::default() });
        state.upsert_window(Window { id: 3, pid: 43, identity: None, workspace_id: Uuid::nil(), ..Window::default() });
        assert_eq!(state.windows_identity_iter(&a), vec![1]);
        assert_eq!(state.windows_identity_iter(&b), vec![2]);
        assert!(state.windows_identity_iter(
            &AppIdentity { pid: 99, launch_date: LaunchDateBits::from_time_interval_since_reference_date(1.0).unwrap() }
        ).is_empty());
    }
```

- [ ] **Step 2: Run, verify RED**

Run: `cargo test -p stache --lib modules::tiling::state::tests::windows_identity_iter_filters_exact_identity`
Expected: compilation fails — `Window.identity` field and `windows_identity_iter` missing.

- [ ] **Step 3: Add the field and iterator, update every literal**

In `state/types.rs` `Window`, insert after `pub pid: i32,`:

```rust
    /// Exact application identity (PID + launch date). `None` only for
    /// transitional windows before first capture; the actor never runs
    /// hide/unhide/terminate/restore against a window whose identity is `None`.
    pub identity: Option<AppIdentity>,
```

Add `identity: None,` to `Window::default()`. In `actor/messages.rs` `WindowCreatedInfo`, add `pub identity: Option<AppIdentity>,` after `pub pid: i32,`. Update every `Window { … }`/`WindowCreatedInfo { … }` literal with `identity: None,` (tests may use `Some(…)` where a concrete identity exists). In `state/tiling_state.rs`:

```rust
/// Returns window IDs whose identity equals the given value.
pub fn windows_identity_iter(&self, identity: &AppIdentity) -> Vec<u32> {
    self.windows
        .iter()
        .filter(|w| w.identity.as_ref() == Some(identity))
        .map(|w| w.id)
        .collect()
}
```

> `state.windows` is an `ObservableVector<Window>` (tiling_state.rs:46); `.iter()` yields `&Window`, so the filter/map above is final as written.

- [ ] **Step 4: Run, verify GREEN**

```bash
cargo test -p stache --lib modules::tiling::state::tests
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::actor::handlers::app::tests
cargo check -p stache
cargo fmt --all -- --check
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/state/types.rs \
  app/native/src/modules/tiling/actor/messages.rs \
  app/native/src/modules/tiling/state/tiling_state.rs \
  app/native/src/modules/tiling/actor/handlers/window.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/init.rs \
  app/native/src/modules/tiling/rules/mod.rs \
  app/native/src/modules/bar/components/tiling.rs
git commit -m "feat(tiling): add optional identity to Window and WindowCreatedInfo"
```

---

## Task 19B-2: Key events, observers, tabs, effects, cache, animation, drag, routing by `AppIdentity`

One forced-atomic commit: `WindowEvent.identity` is non-optional and the AX-derived `StateMessage` variants gain identity, so no intermediate commit can compile. Work through the ordered steps, commit once at the end.

**Files:**

- Modify: `events/types.rs`, `events/observer.rs`, `events/ax_observer.rs`, `events/app_monitor.rs`, `events/processor.rs`, `events/drag_state.rs`
- Modify: `tabs.rs`
- Modify: `actor/handlers/window.rs` (identity upgrade + PID-reuse removal), `actor/handlers/app.rs` (exact cache invalidation only; termination cleanup is 19E), `actor/handlers/focus.rs`, `actor/handlers/workspace.rs`, `actor/handlers/preset.rs`, `actor/handlers/window_move.rs`
- Modify: `actor/handle.rs` (`set_expected_frames`), `actor/messages.rs` (query variants + `SetExpectedFrames`), `actor/mod.rs`
- Modify: `effects/mod.rs`, `effects/subscriber.rs`, `effects/executor.rs`, `effects/window_cache.rs`, `effects/window_ops.rs`, `effects/animation/mod.rs`, `effects/animation/state.rs`, `effects/animation/transition.rs`
- Modify: `init.rs`, `app/native/src/modules/bar/components/tiling.rs`

- [ ] **Step 1: Write failing tests**

In `events/types.rs` tests (will not compile until `WindowEvent.identity` exists):

```rust
use crate::modules::tiling::identity::{AppIdentity, LaunchDateBits};

#[test]
fn window_event_carries_identity() {
    let ident = AppIdentity { pid: 42, launch_date: LaunchDateBits::from_time_interval_since_reference_date(100.0).unwrap() };
    let event = WindowEvent::new(WindowEventType::Created, 42, 0x1234, ident);
    assert_eq!(event.identity, ident);
}
```

In `events/observer.rs` tests (address-keyed removal seam):

```rust
#[test]
fn removal_by_identity_preserves_same_pid_replacement() {
    let mut state = ObserverState {
        observers: HashMap::new(),
        identity_to_observer: HashMap::new(),
    };
    let a = AppIdentity { pid: 42, launch_date: LaunchDateBits::from_time_interval_since_reference_date(100.0).unwrap() };
    let b = AppIdentity { pid: 42, launch_date: LaunchDateBits::from_time_interval_since_reference_date(200.0).unwrap() };
    state.observers.insert(0xAAA, ObserverRecord { observer: ObserverRef(0xAAA as *mut c_void), identity: a });
    state.observers.insert(0xBBB, ObserverRecord { observer: ObserverRef(0xBBB as *mut c_void), identity: b });
    state.identity_to_observer.insert(a, 0xAAA);
    state.identity_to_observer.insert(b, 0xBBB);

    let removed = take_observer_record_for_identity(&mut state, &a).expect("a present");
    assert_eq!(removed.observer.0, 0xAAA as *mut c_void);
    assert!(state.identity_to_observer.contains_key(&b));
    assert!(state.observers.contains_key(&0xBBB));
    assert!(state.observers.get(&0xAAA).is_none());
}
```

- [ ] **Step 2: Run, verify RED**

Run: `cargo test -p stache --lib modules::tiling::events::types::tests`
Expected: compilation fails — `WindowEvent::new` has no `identity` parameter.

- [ ] **Step 3: Add identity to event/message types and queries, atomically**

**`WindowEvent`** (`events/types.rs:139`):

```rust
pub struct WindowEvent {
    pub event_type: WindowEventType,
    pub pid: i32,
    /// Exact application identity. Never derived from a bare PID at callback time.
    pub identity: AppIdentity,
    pub element: usize,
}

impl WindowEvent {
    #[must_use]
    pub const fn new(
        event_type: WindowEventType,
        pid: i32,
        element: usize,
        identity: AppIdentity,
    ) -> Self {
        Self { event_type, pid, identity, element }
    }
}
```

Update `test_window_event_new` and add the `window_event_carries_identity` test.

**`StateMessage`** (`actor/messages.rs`) — lifecycle variants carry identity (keep `pid` for handler internals):

```rust
AppLaunched { identity: AppIdentity, pid: i32, bundle_id: String, name: String },
AppTerminated { identity: AppIdentity, pid: i32 },
AppHidden { identity: AppIdentity, pid: i32 },
AppShown { identity: AppIdentity, pid: i32 },
```

Every AX-derived ID-targeted variant gains `identity: AppIdentity`: `WindowDestroyed`, `WindowFocused`, `WindowUnfocused`, `WindowMoved`, `WindowResized`, `WindowMinimized`, `WindowTitleChanged`, `WindowFullscreenChanged`. `GeometryUpdate` gains `pub identity: AppIdentity`. `SetExpectedFrames { frames: Vec<(WindowTarget, Rect)> }`. Add:

```rust
StateQuery::GetWindowLayoutTargets { workspace_id: Uuid }
StateQuery::GetFocusTargets

QueryResult::TargetLayout(Vec<(WindowTarget, Rect)>)
QueryResult::TargetFocus {
    focused_window: Option<WindowTarget>,
    focused_workspace_id: Option<Uuid>,
}
```

`WindowCreatedInfo` sets `identity: Some(…)` from the observer capture (Step 4). `None` remains valid only for transitional initial-scan entries.

- [ ] **Step 4: Capture identity in `events/observer.rs` (autoreleasepool, address-keyed)**

Replace the `ObserverState` shape:

```rust
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

In `add_observer_for_pid(pid)`: capture identity inside an autorelease pool from one local `NSRunningApplication`; first duplicate check in a short critical section; create/register the observer **without** holding `OBSERVER_STATE`; re-resolve and revalidate the identity immediately before publication; insert both indices; drop the guard before `CFRunLoopAddSource`:

```rust
#[allow(clippy::significant_drop_tightening)]
pub fn add_observer_for_pid(pid: i32) -> Result<(), String> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    if !INITIALIZED.load(Ordering::SeqCst) {
        return Err("Observer system not initialized".to_string());
    }

    let identity = objc::rc::autoreleasepool(|| -> Option<AppIdentity> {
        unsafe {
            let app: *mut Object = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid
            ];
            if app.is_null() {
                return None;
            }
            AppIdentity::from_ns_running_app(app)
        }
    });
    let Some(identity) = identity else {
        return Err(format!("no valid identity for pid {pid}"));
    };

    {
        let state_guard = OBSERVER_STATE.lock();
        let state = state_guard.as_ref().ok_or("Observer state not initialized")?;
        if state.identity_to_observer.contains_key(&identity) {
            return Ok(());
        }
    }

    let mut observer: AXObserverRef = ptr::null_mut();
    let result =
        unsafe { AXObserverCreate(pid, observer_callback, std::ptr::addr_of_mut!(observer)) };
    if result != K_AX_ERROR_SUCCESS || observer.is_null() {
        return Err(format!("AXObserverCreate failed for pid {pid}: {result}"));
    }
    let app_element = unsafe { AXUIElementCreateApplication(pid) };
    if app_element.is_null() {
        unsafe { CFRelease(observer.cast()) };
        return Err(format!("AXUIElementCreateApplication failed for pid {pid}"));
    }

    for name in [
        notifications::WINDOW_CREATED,
        notifications::WINDOW_MOVED,
        notifications::WINDOW_RESIZED,
        notifications::WINDOW_MINIMIZED,
        notifications::WINDOW_UNMINIMIZED,
        notifications::FOCUSED_WINDOW_CHANGED,
        notifications::UI_ELEMENT_DESTROYED,
        notifications::TITLE_CHANGED,
        notifications::APP_ACTIVATED,
        notifications::APP_DEACTIVATED,
        notifications::APP_HIDDEN,
        notifications::APP_SHOWN,
    ] {
        let cf_name = CFString::new(name);
        let r = unsafe {
            AXObserverAddNotification(
                observer,
                app_element,
                cf_name.as_concrete_TypeRef().cast(),
                pid as *mut c_void,
            )
        };
        if r != K_AX_ERROR_SUCCESS {
            tracing::trace!("Failed to add notification {name} for pid {pid}: {r}");
        }
    }
    unsafe { CFRelease(app_element.cast()) };

    let source = unsafe { AXObserverGetRunLoopSource(observer) };
    if source.is_null() {
        unsafe { CFRelease(observer.cast()) };
        return Err(format!("observer has no run-loop source: pid {pid}"));
    }

    // Re-resolve immediately before publication to close the PID-reuse window.
    let current_identity = objc::rc::autoreleasepool(|| -> Option<AppIdentity> {
        unsafe {
            let app: *mut Object = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid
            ];
            if app.is_null() {
                return None;
            }
            AppIdentity::from_ns_running_app(app)
        }
    });
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

    unsafe {
        let run_loop = CFRunLoop::get_main();
        let mode = core_foundation::runloop::kCFRunLoopDefaultMode;
        CFRunLoopAddSource(run_loop.as_concrete_TypeRef().cast(), source, mode.cast());
    }
    tracing::trace!("Added observer for identity {identity:?}");
    Ok(())
}
```

Initialize both maps in `init()`: `ObserverState { observers: HashMap::new(), identity_to_observer: HashMap::new() }`.

**Callback derives identity from the observer address** (refcon stays PID for logging only; never holds the lock beyond one `HashMap::get`):

```rust
unsafe extern "C" fn observer_callback(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: *const c_void,
    refcon: *mut c_void,
) {
    unsafe {
        let cf_notification = notification as core_foundation::string::CFStringRef;
        let notification_str = CFString::wrap_under_get_rule(cf_notification);
        let notification_name = notification_str.to_string();
        let pid = refcon as i32;

        let Some(event_type) = WindowEventType::from_notification(&notification_name) else {
            tracing::trace!("Unknown notification: {notification_name}");
            return;
        };

        let identity = {
            let state_guard = OBSERVER_STATE.lock();
            let Some(ref state) = *state_guard else { return; };
            let Some(record) = state.observers.get(&(observer as usize)) else { return; };
            record.identity
        };

        let event = WindowEvent::new(event_type, pid, element as usize, identity);
        super::ax_observer::adapter_callback(event);
    }
}
```

**Exact removal seam** (pure, never calls Core Foundation) plus the production wrapper:

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

Add `CFRunLoopRemoveSource` to the `#[link(name = "CoreFoundation", kind = "framework")]` block (same shape as `CFRunLoopAddSource`). Delete `remove_observer_for_pid` — no production removal path scans by PID. Add the Step 1 address-keyed removal test.

> Keep the 15C-2 `observer::shutdown()` behavior: after 19B-2 it drains the address-keyed map (`state.observers.drain()` yields `(usize, ObserverRecord)` — adjust the loop accordingly) and still resets `INITIALIZED`.

- [ ] **Step 5: Forward identity in `events/ax_observer.rs` only**

`adapter_callback(event)` forwards the `WindowEvent` that already carries identity. Inside `AXObserverAdapter::handle_event`, forward `event.identity` (never re-capture, never key by bare PID):

```rust
    pub fn handle_event(&self, event: WindowEvent) {
        if !self.gate_open() {
            tracing::trace!("Ignoring event {:?} - adapter not active", event.event_type);
            return;
        }
        let ax_element = event.element as AXUIElementRef;
        match event.event_type {
            WindowEventType::Created => self.handle_window_created(event.pid, event.identity, ax_element),
            WindowEventType::Destroyed => self.handle_window_destroyed(event.pid, event.identity, ax_element),
            WindowEventType::Focused => self.handle_window_focused(event.pid, event.identity, ax_element),
            WindowEventType::Unfocused => self.handle_window_unfocused(event.pid, event.identity, ax_element),
            WindowEventType::Moved => self.handle_window_moved(event.pid, event.identity, ax_element),
            WindowEventType::Resized => self.handle_window_resized(event.pid, event.identity, ax_element),
            WindowEventType::Minimized => self.handle_window_minimized(event.pid, event.identity, ax_element, true),
            WindowEventType::Unminimized => self.handle_window_minimized(event.pid, event.identity, ax_element, false),
            WindowEventType::TitleChanged => self.handle_title_changed(event.pid, event.identity, ax_element),
            WindowEventType::AppActivated => self.handle_app_activated(event.pid, event.identity),
            WindowEventType::AppDeactivated => {}
            WindowEventType::AppHidden => self.processor.on_app_hidden(event.identity, event.pid),
            WindowEventType::AppShown => self.processor.on_app_shown(event.identity, event.pid),
        }
    }
```

In `handle_window_created(pid, identity, ax_element)`, set `identity: Some(identity)` on the `WindowCreatedInfo`. `handle_window_destroyed` calls `self.processor.on_window_destroyed(window_id, identity)` when the ID is known, else `self.processor.on_window_destroyed_for_identity(identity)` (never a PID variant). `handle_window_focused/moved/resized/minimized/title_changed` pass `event.identity` to the corresponding processor method.

- [ ] **Step 6: Capture identity in `events/app_monitor.rs`**

```rust
/// Extracts app info from an `NSNotification`.
/// Returns (identity, pid, bundle_id, app_name). `identity` is None if capture
/// fails (fail-closed — the caller drops the event).
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
        let identity = objc::rc::autoreleasepool(|| unsafe {
            AppIdentity::from_ns_running_app(running_app)
        });

        let pid: i32 = msg_send![running_app, processIdentifier];
        if pid <= 0 {
            return (None, 0, None, None);
        }
        let bundle_id: Option<String> = {
            let b: *mut Object = msg_send![running_app, bundleIdentifier];
            if b.is_null() { None } else { Some(nsstring_to_string(b)) }
        };
        let app_name: Option<String> = {
            let n: *mut Object = msg_send![running_app, localizedName];
            if n.is_null() { None } else { Some(nsstring_to_string(n)) }
        };
        (identity, pid, bundle_id, app_name)
    }
}
```

Update `ExtractAppInfo` if it exists. `on_app_launched(&self, identity: Option<AppIdentity>, pid: i32, bundle_id: Option<String>, name: Option<String>)` drops on `None`:

```rust
    let Some(identity) = identity else {
        tracing::trace!(pid, "app_monitor: dropping launch event (no identity)");
        return;
    };
    let bundle_id = bundle_id.unwrap_or_default();
    let name = name.unwrap_or_default();
    self.processor.on_app_launched(identity, pid, bundle_id, name);
```

`on_app_terminated(&self, identity: Option<AppIdentity>, pid: i32, bundle_id: Option<&str>, name: Option<&str>)` drops on `None`, then (main thread — safe) calls `crate::modules::tiling::events::observer::remove_observer_for_identity(&identity)`, then `self.processor.on_app_terminated(identity, pid)`. Update `test_extract_app_info_null_notification` to the 4-tuple.

- [ ] **Step 7: Exact routing/destroy/batching in `events/processor.rs`**

Replace `window_screen_map: DashMap<u32, u32>` with `DashMap<(AppIdentity, u32), u32>` and `pid_windows: Mutex<HashMap<i32, HashSet<u32>>>` with `Mutex<HashMap<AppIdentity, HashSet<u32>>>`:

```rust
pub fn set_window_screen(&self, identity: AppIdentity, window_id: u32, screen_id: u32) {
    self.window_screen_map.insert((identity, window_id), screen_id);
}

pub fn remove_window(&self, identity: AppIdentity, window_id: u32) {
    self.window_screen_map.remove(&(identity, window_id));
}

fn get_window_screen(&self, identity: AppIdentity, window_id: u32) -> u32 {
    self.window_screen_map
        .get(&(identity, window_id))
        .map_or_else(|| self.default_screen_id.load(Ordering::SeqCst), |e| *e)
}

pub fn on_window_destroyed(&self, window_id: u32, identity: AppIdentity) {
    let screen_id = self.window_screen_map.remove(&(identity, window_id)).map(|(_, id)| id);
    if let Some(screen_id) = screen_id
        && let Some(batch) = self.screen_batches.lock().get_mut(&screen_id)
    {
        batch.updates.remove(&(identity, window_id));
    }
    {
        let mut m = self.pid_windows.lock();
        if let Some(set) = m.get_mut(&identity) {
            set.remove(&window_id);
        }
    }
    let _ = self.actor_handle.send(StateMessage::WindowDestroyed { window_id, identity });
}

pub fn on_window_destroyed_for_identity(&self, identity: AppIdentity) {
    let tracked: Vec<u32> = self
        .pid_windows
        .lock()
        .get(&identity)
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default();
    if tracked.is_empty() {
        return;
    }
    let cache = crate::modules::tiling::effects::get_window_cache();
    let invalid = cache.find_invalid_windows(&tracked);
    for window_id in invalid {
        self.on_window_destroyed(window_id, identity);
    }
}
```

`on_window_created(info)` keys `pid_windows` by `info.identity` (require `Some`; on `None` log and drop). All geometry methods take `(identity, window_id, frame)`, key batches by `(AppIdentity, u32)` in `ScreenBatch.updates: HashMap<(AppIdentity, u32), GeometryUpdate>`, and send `GeometryUpdate { identity, window_id, frame, update_type }`. Destroy-detection tracking takes `(WindowTarget, …)` shapes. Update all `processor.rs` tests to pass `AppIdentity` values and exact targets. `set_expected_frames` becomes `Vec<(WindowTarget, Rect)>` end to end; the actor updates `expected_frame` only when the stored window identity equals the target identity.

- [ ] **Step 8: Exact tabs in `tabs.rs`**

Replace `window_to_pid: HashMap<u32, i32>` with `window_to_identity: HashMap<u32, AppIdentity>`. Public API:

```rust
pub fn register_tab(window_id: u32, identity: AppIdentity);
pub fn unregister_tab_for_identity(window_id: u32, identity: AppIdentity);
pub fn is_tab_for_identity(window_id: u32, identity: AppIdentity) -> bool;
pub fn tabs_for_identity(identity: AppIdentity) -> Vec<u32>;
pub fn clear_tabs_for_identity(identity: AppIdentity);
pub fn replace_tabs_for_identity(identity: AppIdentity, window_ids: impl IntoIterator<Item = u32>);
pub fn clear_all_tabs();
```

`scan_and_register_tabs_for_app(identity)` re-resolves the current `NSRunningApplication` from `identity.pid` inside an autorelease pool, requires the captured identity to match before scanning, and re-resolves again immediately before publishing via the deterministic seam:

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

`is_new_window_a_tab(identity: AppIdentity, new_window_id: u32, workspace_window_ids: &[u32])` compares siblings through `window_to_identity`, never PID. Add `TabRegistry` tests with identities A/B sharing PID 42 and different launch dates: register 101→A and 202→B; clear A and assert B remains; replacing A's scan cannot replace B's tabs; mismatched post-scan identity publishes nothing; same-window-ID 303 A→B replacement survives exact unregister of A and is removed by exact unregister of B.

- [ ] **Step 9: Actor ingress — identity upgrade + mismatch rejection**

In `actor/handlers/window.rs::on_window_created_internal`, before the "already tracked, updating" branch, compare identities; the function now returns a de-duplicated `Vec<Uuid>` of affected workspaces:

```rust
    let mut affected_workspaces: Vec<Uuid> = Vec::new();
    let existing_identity = state.get_window(info.window_id).and_then(|w| w.identity);
    match (existing_identity, info.identity) {
        (Some(existing), Some(incoming)) if existing != incoming => {
            // Window ID reused by another application instance: remove every
            // stale reference before creating the new instance.
            if let Some(workspace_id) = on_window_destroyed(state, info.window_id) {
                affected_workspaces.push(workspace_id);
            }
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

`on_window_created` iterates the returned vector and emits one layout notification per workspace; the silent batch caller ignores it. After normal new-window insertion:

```rust
    affected_workspaces.push(workspace_id);
    affected_workspaces.sort_unstable();
    affected_workspaces.dedup();
    affected_workspaces
```

The `Window { … }` literal gains `identity: info.identity`. Tab path uses `tabs::is_tab_for_identity(info.window_id, identity)` / `tabs::register_tab(info.window_id, identity)` / `tabs::is_new_window_a_tab(identity, …)`. `WindowDestroyed` ingress special-cases tabs first: `tabs::is_tab_for_identity(window_id, identity)` → `tabs::unregister_tab_for_identity(window_id, identity)` and return without layout mutation. In `actor/mod.rs`, every AX-derived `StateMessage` arm rejects when the stored window identity differs from `message.identity` before invoking the handler; batched `GeometryUpdate`s get the same independent check. The initial focus event in `init.rs` captures the focused window's identity from `window_infos` before the batch and sends `WindowFocused` only with `Some(identity)`, otherwise fail closed and log.

Add focused tests: existing `None` + incoming `Some(A)` upgrades without duplicating; existing `Some(A)` + incoming `Some(B)` at the same ID removes A's stale workspace/focus/cache references and creates B; delayed A destroy/move/resize/batched-geometry cannot mutate B; matching B destroy still cleans up.

- [ ] **Step 10: Exact effects, cache, animation, drag, routing**

`effects/mod.rs` — replace target-bearing values with `WindowTarget`, remove `UpdateBorder` and `effects_from_focus_change` entirely, add `RefreshActiveBorder`:

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
    workspace_id: Uuid,
    user_triggered: bool,
}
FocusChange {
    old_window: Option<WindowTarget>,
    new_window: Option<WindowTarget>,
    old_workspace_id: Option<Uuid>,
    new_workspace_id: Option<Uuid>,
}
```

Add `GetWindowLayoutTargets`/`GetFocusTargets` execution in `actor/mod.rs::execute_query`, producing `TargetLayout`/`TargetFocus` from stored identities (omit `None`, log at trace). The subscriber consumes only these; it no longer calls `borders::on_focus_changed` directly — the executor validates the `RefreshActiveBorder` target, then invokes the existing `borders::on_focus_changed(layout, is_window_floating)` helper. `SubscriberNotification::FloatingChanged` carries `WindowTarget`; `handlers/window_move.rs` re-reads the stored window, requires `Some(identity)`, and notifies exactly. `WindowElementCache` keys become `windows: DashMap<WindowTarget, CachedWindowElement>`, `apps: DashMap<AppIdentity, CachedAppElement>`; every API takes exact targets; resolution obtains one local `NSRunningApplication` for `target.identity.pid`, captures `AppIdentity` from that same object, requires exact equality, then enumerates only that app's AX windows for `target.window_id`. Delete `get_window_pid(window_id)` and the global first-matching-window-ID fallbacks; `focus_window`/`raise_window`/`get_window_frame`/`set_window_frame`/`set_window_frame_fast`/`set_window_frames_batch`/`get_window_minimum_size`/`set_window_frame_verified` accept `WindowTarget`. `WindowTransition.target: WindowTarget`; interrupted positions `DashMap<WindowTarget, Rect>`; `drag_state::WindowSnapshot` stores `target: WindowTarget`; `DragInfo` stores the exact identity; `SwapWindows`/`UserResizeCompleted` carry exact targets; mouse-up skips missing/mismatched identities, never falling back to a numeric window ID. `bar/components/tiling.rs` resolves the stored `Window.identity`, builds `WindowTarget`, and focuses exactly.

- [ ] **Step 11: Fix every remaining call site, then GREEN**

```bash
cargo check -p stache 2>&1 | tail -30
cargo test -p stache --lib modules::tiling::events::types::tests
cargo test -p stache --lib modules::tiling::events::observer::tests
cargo test -p stache --lib modules::tiling::tabs::tests
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::effects::tests
cargo test -p stache --lib modules::tiling::effects::executor::tests
cargo test -p stache --lib modules::tiling::effects::window_cache::tests
cargo test -p stache --lib modules::tiling::effects::animation
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo fmt --all -- --check
```

Expected: all green, `cargo check` clean. Then:

```bash
rg -n "UpdateBorder|effects_from_focus_change|on_window_destroyed_for_pid|remove_observer_for_pid|get_window_pid|clear_tabs_for_pid" app/native/src/modules/tiling
```

Expected: no matches.

- [ ] **Step 12: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/events/types.rs \
  app/native/src/modules/tiling/events/observer.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/events/app_monitor.rs \
  app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/events/drag_state.rs \
  app/native/src/modules/tiling/tabs.rs \
  app/native/src/modules/tiling/actor/messages.rs \
  app/native/src/modules/tiling/actor/handlers/window.rs \
  app/native/src/modules/tiling/actor/handlers/app.rs \
  app/native/src/modules/tiling/actor/handlers/focus.rs \
  app/native/src/modules/tiling/actor/handlers/workspace.rs \
  app/native/src/modules/tiling/actor/handlers/preset.rs \
  app/native/src/modules/tiling/actor/handlers/window_move.rs \
  app/native/src/modules/tiling/actor/handle.rs \
  app/native/src/modules/tiling/actor/mod.rs \
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
git commit -m "feat(tiling): key events, observers, tabs, effects by AppIdentity"
```

---

## Task 19C: `VisibilityRegistry` — shared, sealed `BTreeSet<AppIdentity>`

Safe staging: the registry is added **beside** the old generation/FIFO `HiddenAppTracker`; both compile, old tests still pass, and no production code calls the new registry until 19D.

**Files:**

- Modify: `app/native/src/modules/tiling/visibility.rs` (append registry; keep old code)
- Modify: `app/native/src/modules/tiling/actor/handle.rs` — `registry: Arc<VisibilityRegistry>`; `new_with_registry`; `seal_and_drain_visibility`
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — `StateActor.registry` field; `spawn_with_registry`
- Modify: `app/native/src/modules/tiling/mod.rs` — re-export `VisibilityRegistry`

- [ ] **Step 1: Write the failing registry tests**

Append to `visibility.rs` tests:

```rust
fn bits(v: u64) -> LaunchDateBits {
    LaunchDateBits::from_time_interval_since_reference_date(v as f64).unwrap()
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
    assert!(reg.insert(id1));
    assert!(reg.insert(id2));
    assert!(reg.contains(&id1));
    assert_eq!(reg.len(), 2);
    let drained = reg.seal_and_drain();
    assert_eq!(drained, vec![id1, id2]); // sorted by Ord
    assert!(reg.is_empty());
    reg.insert(id30_3());
    assert!(reg.is_empty(), "late insert after seal is a no-op");
}

#[test]
fn registry_thread_safety() {
    use std::sync::Arc;
    use std::thread;
    let reg = Arc::new(VisibilityRegistry::default());
    let ids: Vec<_> = (1..=100_i32)
        .map(|i| AppIdentity { pid: i, launch_date: bits(i as u64) })
        .collect();
    let mut handles = Vec::new();
    for chunk in ids.chunks(25) {
        let r = Arc::clone(&reg);
        let c = chunk.to_vec();
        handles.push(thread::spawn(move || {
            for id in c {
                r.insert(id);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(reg.len(), 100);
    assert_eq!(reg.seal_and_drain().len(), 100);
}

#[test]
fn registry_seal_rejects_actor_ops() {
    let reg = VisibilityRegistry::default();
    reg.seal_and_drain();
    assert_eq!(
        reg.hide_if_open(id10_1(), |id| { panic!("no OS op after seal: {id:?}") }),
        HideAppOutcome::Failed
    );
    assert_eq!(
        reg.unhide_if_open(id10_1(), |id| { panic!("no OS op after seal: {id:?}") }),
        UnhideAppOutcome::Failed
    );
    assert!(!reg.relinquish_if_open(&id10_1()), "relinquishment after seal is a no-op");
}

#[test]
fn registry_relinquishment_is_seal_aware() {
    let reg = VisibilityRegistry::default();
    reg.insert(id10_1());
    assert!(reg.relinquish_if_open(&id10_1()));
    assert!(!reg.contains(&id10_1()));
    assert!(!reg.relinquish_if_open(&id10_1()), "absent identity relinquishes nothing");
}
```

- [ ] **Step 2: Run, verify RED**

Run: `cargo test -p stache --lib modules::tiling::visibility::tests`
Expected: compilation fails — `VisibilityRegistry` and `relinquish_if_open` do not exist.

- [ ] **Step 3: Implement the registry**

Append to `visibility.rs` (add `use std::collections::BTreeSet;` and the identity imports):

```rust
/// Passive registry shared between `StateActor` and `StateActorHandle`.
///
/// The actor is the sole runtime writer. The handle's `seal_and_drain_visibility`
/// is the only external mutation API. Raw `RegistryState` fields are private —
/// actor code uses the closure methods `hide_if_open`/`unhide_if_open` and the
/// seal-aware `relinquish_if_open`.
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
    /// Atomically: sealed check → OS op → BTreeSet mutation.
    /// If sealed, returns `HideAppOutcome::Failed` without calling `op`.
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

    /// Atomically: sealed check → OS op → BTreeSet mutation.
    /// If sealed, returns `UnhideAppOutcome::Failed` without calling `op`.
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

    /// Actor-owned, seal-aware relinquishment. Returns false (no mutation) if
    /// the registry is sealed. Used by the actor's revalidation and exact
    /// termination paths; EventProcessor never mutates ownership.
    pub(crate) fn relinquish_if_open(&self, identity: &AppIdentity) -> bool {
        let mut state = self.inner.lock();
        if state.sealed {
            return false;
        }
        state.owned.remove(identity)
    }

    /// Inserts an identity after a seal check. Test/revalidation-only; not the
    /// workspace hide/unhide hot path.
    pub(crate) fn insert(&self, identity: AppIdentity) -> bool {
        let mut state = self.inner.lock();
        if state.sealed {
            return false;
        }
        state.owned.insert(identity)
    }

    pub(crate) fn contains(&self, identity: &AppIdentity) -> bool {
        self.inner.lock().owned.contains(identity)
    }

    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.inner.lock().owned.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.lock().owned.is_empty()
    }

    pub(crate) fn sealed(&self) -> bool {
        self.inner.lock().sealed
    }

    /// Atomically seals and drains in a single lock acquisition.
    /// This is the only mutation API exposed outside the actor/controller.
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

> The `Mutex` import must already exist in `visibility.rs` (the old tracker uses one); otherwise add `use parking_lot::Mutex;`.

- [ ] **Step 4: Wire into `StateActorHandle` and `StateActor`**

`actor/handle.rs` — add `use std::sync::Arc;` and the registry field; the constructor can no longer be `const`:

```rust
pub struct StateActorHandle {
    sender: mpsc::Sender<StateMessage>,
    registry: Arc<VisibilityRegistry>,
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

    /// Atomically seals the registry and drains all owned identities for
    /// restoration. The actor channel MUST be alive when called (seal happens
    /// before tiling shutdown closes the actor).
    pub(crate) fn seal_and_drain_visibility(&self) -> Vec<AppIdentity> {
        self.registry.seal_and_drain()
    }
}
```

`actor/mod.rs` — `StateActor` gains `registry: Arc<VisibilityRegistry>`; Task 15 spawn already returns the tuple:

```rust
pub(crate) fn spawn() -> (StateActorHandle, crate::modules::tiling::init::CompletionLatch) {
    Self::spawn_with_registry(Arc::new(VisibilityRegistry::default()))
}

pub(crate) fn spawn_with_registry(
    registry: Arc<VisibilityRegistry>,
) -> (StateActorHandle, crate::modules::tiling::init::CompletionLatch) {
    tracing::debug!("tiling: spawning state actor");
    let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
    let handle = StateActorHandle::new_with_registry(sender, Arc::clone(&registry));
    let stopped = crate::modules::tiling::init::CompletionLatch::new();
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

`mod.rs` re-exports:

```rust
pub use visibility::{RestoreSummary, restore_stache_hidden_apps, VisibilityRegistry};
```

- [ ] **Step 5: Run, verify GREEN**

```bash
cargo test -p stache --lib modules::tiling::visibility::tests
cargo test -p stache --lib modules::tiling::actor::tests
cargo check -p stache
cargo fmt --all -- --check
```

Expected: new registry tests pass; all old tracker tests still pass (coexistence).

- [ ] **Step 6: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/visibility.rs \
  app/native/src/modules/tiling/actor/handle.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): VisibilityRegistry with sealed BTreeSet<AppIdentity>"
```

---

## Task 19D + 19E: Atomic actor-owned visibility cutover

> **⚠️ CUTOVER PROHIBITIONS (both tasks, one working tree):**
>
> - Do **NOT** commit any part of 19D alone. The 19D tree is intentionally unshippable: the old lifecycle classifier cannot yet remove ownership from `VisibilityRegistry`.
> - Do **NOT** run `pnpm tauri:dev`, quit/restart the app, or hand off during 19D.
> - Proceed directly through 19E; commit the combined 19D+19E file set only after every 19E test passes. This is the plan's only atomicity exception.
> - Old symbols (`hide_app_for_workspace`, `unhide_app_for_workspace`, `HiddenAppTracker`, `classify_*`, `forget_stache_hidden_app_terminated`) remain compilation-alive through this commit and are deleted in 19G; transient dead-code warnings here are expected and are not errors for `cargo test`/`cargo check`.

### Task 19D: Actor-owned workspace hide/unhide + shutdown restoration cutover

**Files:**

- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — exact-instance helpers
- Modify: `app/native/src/modules/tiling/actor/handlers/window.rs` — `VisibilityDelta`; collector returns delta; `on_window_focused` returns delta
- Modify: `app/native/src/modules/tiling/actor/handlers/workspace.rs` — `on_switch_workspace`/`on_send_workspace_to_screen` return delta; **`on_cycle_workspace` untouched**
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — actor hide/unhide methods; `sync_visibility_for_workspaces`; `on_init_complete(&mut self)`; identity-based `sync_window_visibility`
- Modify: `app/native/src/modules/tiling/visibility.rs` — `restore_one_exact[_with]`, `restore_from_with`, registry-backed `restore_stache_hidden_apps`
- Modify: `app/native/src/modules/tiling/init.rs` — pause drains current-generation registry; resume installs a fresh unsealed registry

- [ ] **Step 1: Write the failing tests**

In `actor/handlers/window.rs` tests:

```rust
#[test]
fn sync_visibility_produces_identity_delta() {
    use crate::modules::tiling::identity::{AppIdentity, LaunchDateBits};
    let (mut state, visible_id, hidden_id) = make_visible_and_hidden_workspaces();
    let a = AppIdentity { pid: 10, launch_date: LaunchDateBits::from_time_interval_since_reference_date(1.0).unwrap() };
    let b = AppIdentity { pid: 20, launch_date: LaunchDateBits::from_time_interval_since_reference_date(2.0).unwrap() };
    state.upsert_window(Window { id: 1, pid: 10, identity: Some(a), workspace_id: visible_id, ..Window::default() });
    state.upsert_window(Window { id: 2, pid: 20, identity: Some(b), workspace_id: hidden_id, ..Window::default() });

    let delta = sync_window_visibility_for_workspaces(&state, &[visible_id], &[hidden_id]);
    assert_eq!(delta.showing, vec![a]);
    assert_eq!(delta.hiding, vec![b]);
}
```

`make_visible_and_hidden_workspaces` builds two workspaces on the same screen; the visible one has `is_visible: true`. In `actor/mod.rs` tests add: an init test with one visible and one hidden-only identity driven through `handle_hide_for_workspace_with`/`handle_unhide_for_workspace_with` with injected `HiddenByStache`/`AlreadyShown` outcomes — assert the registry owns only the hidden-only identity; a cycle test asserting `on_cycle_workspace` produces **no** registry change; and a `Failed`-outcome test asserting ownership is unchanged.

- [ ] **Step 2: Run, verify RED**

Run: `cargo test -p stache --lib modules::tiling::actor::handlers::window::tests`
Expected: compilation fails — `VisibilityDelta` and delta-returning signatures do not exist.

- [ ] **Step 3: Exact-instance hide/unhide helpers in `window_ops.rs`**

Bare-PID helpers remain only as temporary compilation bridges for this uncommitted tree; no new production caller may use them (19G deletes them).

```rust
use crate::modules::tiling::identity::AppIdentity;
use objc::runtime::{Class, Object, BOOL, YES};
use objc::{msg_send, sel, sel_impl};

#[must_use]
pub fn hide_app_instance_with_outcome(identity: AppIdentity) -> HideAppOutcome {
    objc::rc::autoreleasepool(|| unsafe {
        let Some(app_class) = Class::get("NSRunningApplication") else {
            return HideAppOutcome::Failed;
        };
        let app: *mut Object = msg_send![
            app_class,
            runningApplicationWithProcessIdentifier: identity.pid
        ];
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
        let Some(app_class) = Class::get("NSRunningApplication") else {
            return UnhideAppOutcome::Failed;
        };
        let app: *mut Object = msg_send![
            app_class,
            runningApplicationWithProcessIdentifier: identity.pid
        ];
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

- [ ] **Step 4: `VisibilityDelta` + collector return; cycle excluded**

In `actor/handlers/window.rs`:

```rust
#[derive(Debug, Default, PartialEq, Eq)]
pub struct VisibilityDelta {
    pub showing: Vec<AppIdentity>,
    pub hiding: Vec<AppIdentity>,
}

/// Identity-based collector. `showing` = identities in becoming-visible
/// workspaces; `hidden_candidates` = identities in becoming-hidden workspaces;
/// `currently_visible` = identities in ANY visible workspace after the
/// transition; `hiding` = candidates minus currently-visible. Sorted for
/// deterministic tests.
pub fn sync_window_visibility_for_workspaces(
    state: &TilingState,
    becoming_visible: &[Uuid],
    becoming_hidden: &[Uuid],
) -> VisibilityDelta {
    use std::collections::{BTreeSet, HashSet};
    let mut delta = VisibilityDelta::default();
    if becoming_visible.is_empty() && becoming_hidden.is_empty() {
        return delta;
    }
    let visible_ws_ids: HashSet<Uuid> =
        state.get_visible_workspaces().iter().map(|ws| ws.id).collect();
    let mut showing = BTreeSet::new();
    for ws_id in becoming_visible {
        for w in state.windows.iter().filter(|w| w.workspace_id == *ws_id) {
            if let Some(i) = w.identity {
                showing.insert(i);
            }
        }
    }
    let mut hidden_candidates = BTreeSet::new();
    for ws_id in becoming_hidden {
        for w in state.windows.iter().filter(|w| w.workspace_id == *ws_id) {
            if let Some(i) = w.identity {
                hidden_candidates.insert(i);
            }
        }
    }
    let mut currently_visible = BTreeSet::new();
    for w in state.windows.iter() {
        if visible_ws_ids.contains(&w.workspace_id)
            && let Some(i) = w.identity
        {
            currently_visible.insert(i);
        }
    }
    delta.showing = showing.into_iter().collect();
    delta.hiding = hidden_candidates.difference(&currently_visible).copied().collect();
    delta
}
```

`on_window_focused(state, window_id) -> VisibilityDelta` replaces its direct `sync_window_visibility_for_workspaces(...)` call with `let delta = sync_window_visibility_for_workspaces(...)` and returns `delta` after its existing notifications/events.

`actor/handlers/workspace.rs`: `on_switch_workspace(state, name) -> VisibilityDelta` and `on_send_workspace_to_screen(state, target) -> VisibilityDelta`; every early return becomes `return VisibilityDelta::default();` and the final call site becomes `let delta = sync_window_visibility_for_workspaces(state, &workspaces_becoming_visible, &workspaces_becoming_hidden); delta`. **`on_cycle_workspace(state, direction)` keeps its existing signature and body — it does not call the visibility collector today and must not begin to.**

- [ ] **Step 5: Actor-owned methods and dispatch**

In `actor/mod.rs`:

```rust
fn handle_hide_for_workspace_with(
    &mut self,
    identity: AppIdentity,
    hide: impl FnOnce(AppIdentity) -> HideAppOutcome,
) -> HideAppOutcome {
    let outcome = self.registry.hide_if_open(identity, hide);
    // Registry lock already released — update window state on actor side.
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

fn sync_visibility_for_workspaces(&mut self, delta: VisibilityDelta) {
    for identity in delta.showing {
        self.handle_unhide_for_workspace(identity);
    }
    for identity in delta.hiding {
        self.handle_hide_for_workspace(identity);
    }
}
```

Dispatch arms (the `CycleWorkspace` arm is unchanged):

```rust
StateMessage::SwitchWorkspace { name } => {
    let delta = handlers::on_switch_workspace(&mut self.state, &name);
    self.sync_visibility_for_workspaces(delta);
}
StateMessage::SendWorkspaceToScreen { target_screen } => {
    let delta = handlers::on_send_workspace_to_screen(&mut self.state, &target_screen);
    self.sync_visibility_for_workspaces(delta);
}
// StateMessage::WindowFocused { window_id } arm:
let delta = handlers::on_window_focused(&mut self.state, window_id);
self.sync_visibility_for_workspaces(delta);
```

Migrate initialization to `&mut self` (import `use std::collections::{BTreeSet, HashSet}; use uuid::Uuid;`):

```rust
fn on_init_complete(&mut self) {
    self.sync_window_visibility();
    // existing layout notification logic unchanged
}

fn sync_window_visibility(&mut self) {
    let visible_ws_ids: HashSet<Uuid> = self
        .state
        .get_visible_workspaces()
        .iter()
        .map(|ws| ws.id)
        .collect();
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

Remove the `hide_app_for_workspace`/`unhide_app_for_workspace` imports from `actor/mod.rs` and `handlers/window.rs`. No PID-based visibility wrapper remains in the actor.

Delete the now-unreferenced private wrappers in `actor/mod.rs`: `on_switch_workspace` (actor/mod.rs:505-507) and `on_send_workspace_to_screen` (actor/mod.rs:561-563). Every other wrapper method (`on_cycle_workspace`, `on_send_window_to_screen`, `on_set_layout`, …) keeps its dispatch arm and stays. Without this deletion the combined 19D+19E tree fails `cargo clippy -p stache --lib -- -D warnings` (19G Step 4) on `dead_code` for the two orphaned methods.

**`init.rs` runtime integration:** `start_runtime` creates a fresh `Arc<VisibilityRegistry>` for every generation and publishes it through that generation's actor/handle (`spawn_with_registry`) — never reuse a registry sealed by a previous pause. Fresh resume publishes a new unsealed registry generation.

**Restore ordering (supersedes the 15D-1 `pause_runtime` code):** the restore must run while the running runtime is still published. The 15D-1 code does `mem::replace(&mut *slot, RuntimeSlot::Empty)` **before** the restore stage, so `get_handle()` returns `None` and the registry is never sealed/drained — Stache-hidden apps would never be restored on tray-pause. Replace the 15D-1 restore stage with:

```rust
/// Restores Stache-hidden apps while the current generation's handle is
/// still published. Called before the runtime is taken from the slot so
/// `get_handle()` still resolves. Returns whether a restore was performed,
/// so the caller can mark the stage done on the runtime it takes.
fn restore_visibility_if_pending() -> bool {
    let pending = match &*RUNTIME.lock() {
        RuntimeSlot::Empty => false,
        RuntimeSlot::Running(rt) => !rt.teardown.visibility_restored,
        RuntimeSlot::Quarantined(partial) => !partial.teardown.visibility_restored,
    };
    if !pending {
        return false;
    }
    let summary = super::visibility::restore_stache_hidden_apps();
    tracing::info!(
        "tiling: restored {} of {} hidden apps",
        summary.restored,
        summary.attempted
    );
    true
}
```

In `pause_runtime` the flow becomes: lock `LIFECYCLE` only (transition `Running → Stopping`), call `restore_visibility_if_pending()`, _then_ take the runtime with the existing `mem::replace` match, and `if restored { runtime.teardown.visibility_restored = true; }` — drop the old `if !runtime.teardown.visibility_restored { … }` stage. No lock is held across the restore: `restore_visibility_if_pending` takes and releases `RUNTIME` briefly to peek, then `restore_stache_hidden_apps()` re-acquires it via `get_handle()` and seals/drains under the registry's own lock. A retry from `Quarantined` skips the restore when the stage was already marked done, or repeats it as a now-empty no-op (the registry is sealed by then); terminal `app_shutdown` on an already-paused runtime gets an empty summary.

- [ ] **Step 6: Atomically cut shutdown restoration over to the registry**

Add to `visibility.rs`:

```rust
use crate::modules::tiling::identity::AppIdentity;
use objc::runtime::{BOOL, YES};
use objc::{msg_send, sel, sel_impl};

/// Narrow decision seam. One resolved app value flows through identity lookup,
/// hidden-state lookup, and unhide, so tests run the same ordering logic.
#[must_use]
fn restore_one_exact_with<T>(
    owned: AppIdentity,
    resolve: impl FnOnce(i32) -> Option<T>,
    identity_of: impl FnOnce(&T) -> Option<AppIdentity>,
    hidden_of: impl FnOnce(&T) -> Option<bool>,
    unhide: impl FnOnce(&T) -> bool,
) -> bool {
    let Some(app) = resolve(owned.pid) else { return false; };
    if identity_of(&app) != Some(owned) { return false; }
    if hidden_of(&app) != Some(true) { return false; }
    unhide(&app)
}

/// Production wrapper. The raw pointer exists only inside this autorelease
/// pool and synchronous call; it is never stored or sent across threads.
#[must_use]
fn restore_one_exact(owned: AppIdentity) -> bool {
    objc::rc::autoreleasepool(|| {
        restore_one_exact_with(
            owned,
            |pid| unsafe {
                let class = objc::runtime::Class::get("NSRunningApplication")?;
                let app: *mut objc::runtime::Object = msg_send![
                    class,
                    runningApplicationWithProcessIdentifier: pid
                ];
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

/// Explicit loop so `FnMut(AppIdentity)` is called with an owned identity.
/// Every identity is attempted even when an earlier restore fails.
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

/// Seals, drains, then restores each owned identity with exact-instance
/// validation. The handle must be alive (restoration runs before tiling
/// shutdown closes the actor channel). No registry lock is held during
/// restoration.
#[must_use]
pub fn restore_stache_hidden_apps() -> RestoreSummary {
    crate::modules::tiling::init::get_handle()
        .map(|handle| {
            let identities = handle.seal_and_drain_visibility();
            restore_from_with(identities, restore_one_exact)
        })
        .unwrap_or(RestoreSummary { attempted: 0, restored: 0 })
}
```

`app_shutdown.rs` is **unchanged** and keeps calling `tiling::restore_stache_hidden_apps()`.

> **Deadlock rules (unconditional):** the actor holds the registry mutex across `NSRunningApplication` hide/unhide only — that API performs no main-thread callbacks, so no lock spans a main-thread dispatch/await. `seal_and_drain_visibility` is a single lock acquisition completed before restoration begins; `restore_one_exact` holds no registry lock. No lifecycle/runtime/visibility lock is held across `dispatch_on_main`/`run_on_main_thread` waits anywhere in Phase 6.

- [ ] **Step 7: Run focused tests (still uncommitted)**

```bash
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::actor::handlers::workspace::tests
cargo test -p stache --lib modules::tiling::actor::tests
cargo test -p stache --lib modules::tiling::effects::window_ops::tests
cargo fmt --all -- --check
cargo check -p stache
```

Expected: green except tests still depending on the old classifier, which 19E removes. **Do not commit.**

### Task 19E: Raw lifecycle forwarding and actor revalidation

**Files:**

- Modify: `app/native/src/modules/tiling/events/processor.rs` — strip classifier; raw forwarding
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — revalidation methods
- Modify: `app/native/src/modules/tiling/actor/handlers/app.rs` — remove PID-only termination path from production dispatch
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs` — `app_instance_is_hidden`

- [ ] **Step 1: Write the failing actor revalidation tests**

In `actor/handlers/app.rs` tests:

```rust
use std::sync::Arc;
use crate::modules::tiling::actor::StateActor;
use crate::modules::tiling::identity::{AppIdentity, LaunchDateBits, WindowTarget};
use crate::modules::tiling::visibility::VisibilityRegistry;

fn test_identity(pid: i32, v: u64) -> AppIdentity {
    AppIdentity {
        pid,
        launch_date: LaunchDateBits::from_time_interval_since_reference_date(v as f64).unwrap(),
    }
}

fn make_actor() -> (StateActor, Arc<VisibilityRegistry>) {
    let registry = Arc::new(VisibilityRegistry::default());
    let actor = StateActor {
        state: TilingState::new(),
        receiver: tokio::sync::mpsc::channel(16).1,
        registry: Arc::clone(&registry),
    };
    (actor, registry)
}

#[test]
fn app_shown_visible_removes_owner_and_marks_windows() {
    let identity = test_identity(42, 100);
    let (mut actor, registry) = make_actor();
    registry.insert(identity);
    actor.state.upsert_window(Window {
        id: 1,
        pid: 42,
        identity: Some(identity),
        is_hidden: true,
        workspace_id: Uuid::nil(),
        ..Window::default()
    });
    actor.on_app_shown_revalidated(identity, |_| Some(false));
    assert!(!registry.contains(&identity));
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
        identity: Some(identity),
        is_hidden: true,
        workspace_id: Uuid::nil(),
        ..Window::default()
    });
    // OS reports hidden — retain owner, no window change and no hide call.
    actor.on_app_shown_revalidated(identity, |_| Some(true));
    assert!(registry.contains(&identity), "keep ownership under unavoidable-history policy");
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
    assert!(registry.contains(&stored));
}

#[test]
fn delayed_termination_removes_only_exact_instance_resources() {
    let a = test_identity(42, 100);
    let b = test_identity(42, 200); // same PID, different launch date
    let (mut actor, registry) = make_actor();
    registry.insert(a);
    registry.insert(b);

    let wa = Uuid::now_v7();
    let wb = Uuid::now_v7();
    actor.state.upsert_workspace(Workspace {
        id: wa,
        name: "A".into(),
        window_ids: smallvec![1],
        focused_window_index: Some(0),
        ..Workspace::default()
    });
    actor.state.upsert_workspace(Workspace {
        id: wb,
        name: "B".into(),
        window_ids: smallvec![2],
        focused_window_index: Some(0),
        ..Workspace::default()
    });
    actor.state.upsert_window(Window {
        id: 1,
        pid: 42,
        identity: Some(a),
        workspace_id: wa,
        app_id: "com.test.a".into(),
        app_name: "A".into(),
        ..Window::default()
    });
    actor.state.upsert_window(Window {
        id: 2,
        pid: 42,
        identity: Some(b),
        workspace_id: wb,
        app_id: "com.test.b".into(),
        app_name: "B".into(),
        ..Window::default()
    });

    crate::modules::tiling::tabs::clear_all_tabs();
    crate::modules::tiling::tabs::register_tab(101, a);
    crate::modules::tiling::tabs::register_tab(202, b);
    actor.state.record_focus_history(wa, 1);
    actor.state.record_focus_history(wb, 2);
    let mut invalidated = Vec::new();
    let mut cached_apps = std::collections::HashSet::from([a, b]);

    actor.on_app_terminated_exact_with(
        a,
        crate::modules::tiling::tabs::clear_tabs_for_identity,
        |target| invalidated.push(target),
        |identity| { cached_apps.remove(&identity); },
    );

    assert!(!registry.contains(&a));
    assert!(registry.contains(&b));
    assert!(actor.state.get_window(1).is_none());
    assert_eq!(actor.state.get_window(2).and_then(|w| w.identity), Some(b));
    assert!(!crate::modules::tiling::tabs::is_tab_for_identity(101, a));
    assert!(crate::modules::tiling::tabs::is_tab_for_identity(202, b));
    assert!(actor.state.get_workspace(wa).is_some_and(|ws| !ws.window_ids.contains(&1)));
    assert!(actor.state.get_workspace(wb).is_some_and(|ws| ws.window_ids.contains(&2)));
    assert_eq!(actor.state.get_focus_history(wa), None);
    assert_eq!(actor.state.get_focus_history(wb), Some(2));
    assert_eq!(invalidated, vec![WindowTarget { identity: a, window_id: 1 }]);
    assert!(!cached_apps.contains(&a));
    assert!(cached_apps.contains(&b));
    crate::modules::tiling::tabs::clear_all_tabs();
}
```

Add a focused-index regression: one workspace with `window_ids = [1, 2, 3]`, `focused_window_index = Some(1)`, A owning window 1 and B owning 2 and 3; terminate A exactly; assert remaining IDs `[2, 3]`, index `Some(0)`, focused window still 2.

- [ ] **Step 2: Run, verify RED**

Run: `cargo test -p stache --lib modules::tiling::actor::handlers::app::tests`
Expected: compilation fails — `on_app_shown_revalidated`/`on_app_terminated_exact_with` not implemented.

- [ ] **Step 3: Strip the EventProcessor classifier**

In `processor.rs`, remove the `ShownClassification`/`classify_stache_hidden_app`/`forget_stache_hidden_app_terminated` imports, `on_app_shown_with`, and its tests. Replace with raw forwarding:

```rust
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
    // No pre-actor forget: the actor removes exact ownership.
    let _ = self.actor_handle.send(StateMessage::AppTerminated { identity, pid });
}
```

- [ ] **Step 4: Implement actor revalidation + exact termination**

In `window_ops.rs`:

```rust
/// OS hidden state for an exact identity, validated on the same local
/// `NSRunningApplication`. Read-only — lifecycle handlers never hide/unhide.
/// Creates its own autorelease pool (called from the actor's task thread).
#[must_use]
pub fn app_instance_is_hidden(identity: AppIdentity) -> Option<bool> {
    objc::rc::autoreleasepool(|| unsafe {
        let app_class = objc::runtime::Class::get("NSRunningApplication")?;
        let app: *mut objc::runtime::Object = msg_send![
            app_class,
            runningApplicationWithProcessIdentifier: identity.pid
        ];
        if app.is_null() {
            return None;
        }
        let actual = AppIdentity::from_ns_running_app(app)?;
        if actual != identity {
            return None; // PID-reuse or process mismatch
        }
        let is_hidden: BOOL = msg_send![app, isHidden];
        Some(is_hidden == YES)
    })
}
```

On `StateActor` in `actor/mod.rs` (import `use std::collections::HashSet; use uuid::Uuid;`):

```rust
/// AppShown with identity revalidation. OS visible → relinquish ownership and
/// mark windows shown; OS hidden/unknown → retain (unavoidable-history policy).
fn on_app_shown_revalidated(
    &mut self,
    identity: AppIdentity,
    query_os: impl FnOnce(AppIdentity) -> Option<bool>,
) {
    if query_os(identity) == Some(false) {
        self.registry.relinquish_if_open(&identity);
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = false);
        }
    }
}

/// AppHidden — OS confirms hidden. Mismatch/unknown → no state change.
fn on_app_hidden_revalidated(&mut self, identity: AppIdentity, os_hidden: Option<bool>) {
    if os_hidden == Some(true) {
        for wid in self.state.windows_identity_iter(&identity) {
            self.state.update_window(wid, |w| w.is_hidden = true);
        }
    }
}

/// Exact termination cleanup. Observer removal already ran on the main thread
/// in `app_monitor` before this handler.
fn on_app_terminated_exact(&mut self, identity: AppIdentity) {
    self.on_app_terminated_exact_with(
        identity,
        crate::modules::tiling::tabs::clear_tabs_for_identity,
        |target| crate::modules::tiling::effects::get_window_cache().invalidate_window(target),
        |identity| crate::modules::tiling::effects::get_window_cache().invalidate_app(identity),
    );
}

fn on_app_terminated_exact_with(
    &mut self,
    identity: AppIdentity,
    mut clear_tabs_for_identity: impl FnMut(AppIdentity),
    mut invalidate_window: impl FnMut(WindowTarget),
    mut invalidate_app: impl FnMut(AppIdentity),
) {
    clear_tabs_for_identity(identity);
    invalidate_app(identity);

    let window_ids: Vec<u32> = self
        .state
        .windows
        .iter()
        .filter(|w| w.identity.as_ref() == Some(&identity))
        .map(|w| w.id)
        .collect();

    if window_ids.is_empty() {
        self.registry.relinquish_if_open(&identity);
        return;
    }

    let mut affected_workspaces: HashSet<Uuid> = HashSet::new();
    for wid in &window_ids {
        invalidate_window(WindowTarget { identity, window_id: *wid });
        self.state.remove_window_from_focus_history(*wid);
        if let Some(ws_id) = self.state.get_window(*wid).map(|w| w.workspace_id) {
            affected_workspaces.insert(ws_id);
            self.state.update_workspace(ws_id, |ws| {
                // Preserve the focused window by ID; index shifts must not
                // silently focus a sibling.
                let focused_window_id = ws
                    .focused_window_index
                    .and_then(|index| ws.window_ids.get(index).copied());
                ws.window_ids.retain(|id| id != wid);
                ws.focused_window_index = focused_window_id
                    .and_then(|focused_id| {
                        ws.window_ids.iter().position(|id| *id == focused_id)
                    })
                    .or_else(|| (!ws.window_ids.is_empty()).then_some(0));
            });
        }
    }

    let current_focus = eyeball::Observable::get(&self.state.focus);
    if current_focus.focused_window_id.is_some_and(|fid| window_ids.contains(&fid)) {
        self.state.clear_focus();
    }
    for wid in &window_ids {
        self.state.remove_window(*wid);
    }
    if let Some(handle) = crate::modules::tiling::init::get_subscriber_handle() {
        for ws_id in &affected_workspaces {
            handle.notify_layout_changed(*ws_id, false);
        }
    }
    self.registry.relinquish_if_open(&identity);
}
```

Dispatch (read-only OS query — never hide/unhide while consuming lifecycle notifications):

```rust
StateMessage::AppShown { identity, pid: _ } => {
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

Remove the PID-only `actor/handlers/app.rs::on_app_terminated(state, pid)` production dispatch path (its `clear_tabs_for_pid`/PID-wide cache calls) — the only termination path is `on_app_terminated_exact`. Keep/rewrite its tests around exact identities.

- [ ] **Step 5: Run the full 19D+19E batch, verify GREEN**

```bash
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo test -p stache --lib modules::tiling::actor::handlers::app::tests
cargo test -p stache --lib modules::tiling::actor::handlers::window::tests
cargo test -p stache --lib modules::tiling::actor::handlers::workspace::tests
cargo test -p stache --lib modules::tiling::visibility::tests
cargo test -p stache --lib modules::tiling::actor::tests
cargo fmt --all -- --check
cargo check -p stache
```

Expected: generation/FIFO classifier tests removed; identity-based tests pass. Remove any `classify_shown_with_state` tests orphaned by the cutover now.

- [ ] **Step 6: Commit (the one atomic 19D+19E commit)**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
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

## Task 19F: Exhaustive restoration tests

**Files:**

- Test: `app/native/src/modules/tiling/visibility.rs`
- Test: `app/native/src/modules/tiling/init.rs` — pause restores before actor teardown; repeated terminal cleanup empty/idempotent; resume publishes a fresh unsealed registry generation
- `app/native/src/modules/tiling/mod.rs` — unchanged
- `app/native/src/app_shutdown.rs` — **unchanged**

- [ ] **Step 1: Write the restoration validation tests**

Append to `visibility.rs` tests (uses `#[derive(Clone, Copy)] struct FakeApp { identity: AppIdentity, hidden: bool }`):

```rust
#[test]
fn restore_proves_identity_equality_before_action() {
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
    assert!(actions.is_empty(), "no action for mismatched identity");
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
    let mut restored = Vec::new();
    let summary = restore_from_with(registry.seal_and_drain(), |identity| {
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

#[test]
fn relinquishment_is_seal_aware_after_drain() {
    let registry = VisibilityRegistry::default();
    registry.insert(AppIdentity { pid: 10, launch_date: bits(1) });
    registry.seal_and_drain();
    assert!(!registry.relinquish_if_open(&AppIdentity { pid: 10, launch_date: bits(1) }));
}

#[test]
fn fresh_registry_generation_is_unsealed_and_distinct() {
    // A resume must install a new unsealed registry generation that does not
    // inherit the drained contents of the previous one.
    let first = VisibilityRegistry::default();
    first.insert(AppIdentity { pid: 10, launch_date: bits(1) });
    let drained = first.seal_and_drain();
    assert_eq!(drained.len(), 1);

    let second = VisibilityRegistry::default();
    assert!(!second.sealed());
    assert!(second.insert(AppIdentity { pid: 20, launch_date: bits(2) }));
    assert!(!second.contains(&AppIdentity { pid: 10, launch_date: bits(1) }));
}
```

In `init.rs` tests (serialize with `TEST_LIFECYCLE_LOCK` from the runtime plan; keep `set_stop_timeout`):

```rust
    #[test]
    fn pause_restores_then_teardown_is_idempotent_and_terminal_cleanup_empty() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        set_stop_timeout(Some(Duration::from_millis(30)));

        // A quarantined generation with restore pending: the first pause runs
        // the public restore (empty summary — no running handle) and advances
        // the stage flag; teardown then times out on the unfinished subscriber
        // latch and retains the quarantine.
        let unfinished = CompletionLatch::new();
        let partial = PartialRuntime {
            generation: 9,
            actor: None,
            actor_stopped: None,
            processor: None,
            subscriber: None,
            subscriber_stopped: Some(unfinished.clone()),
            app_monitor: None,
            screen_monitor: None,
            ax_adapter: None,
            teardown: TeardownProgress {
                main_thread_sources_removed: true,
                processor_stopped: true,
                transient_services_paused: true,
                ..TeardownProgress::default()
            },
        };
        *RUNTIME.lock() = RuntimeSlot::Quarantined(partial);
        *LIFECYCLE.lock() = LifecycleState::Stopping;

        let err = pause_runtime().unwrap_err();
        assert!(err.contains("subscriber"), "{err}");
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Quarantined(_)));

        // Retry after completing the latch: teardown finishes, restore stage is
        // already marked done so nothing is restored a second time.
        unfinished.mark_complete();
        pause_runtime().unwrap();
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Empty));
        assert_eq!(*LIFECYCLE.lock(), LifecycleState::Stopped);

        // Terminal cleanup: a further pause on the stopped runtime errors and
        // performs no restore.
        assert!(pause_runtime().is_err());
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Empty));

        set_stop_timeout(None);
    }
```

This second test guards the 19D restore ordering: the registry must be sealed and drained **while the runtime is still published** (a `Running` slot whose handle owns a populated registry). All non-visibility teardown stages are pre-marked done so only the restore stage runs; the mandatory `TilingRuntime` fields are built from existing pub constructors:

```rust
    #[test]
    fn pause_restores_via_published_handle_before_teardown() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        set_stop_timeout(Some(Duration::from_millis(30)));

        let identity = AppIdentity {
            pid: 42,
            launch_date: LaunchDateBits::from_time_interval_since_reference_date(1.0).unwrap(),
        };
        let registry = Arc::new(VisibilityRegistry::default());
        assert!(registry.insert(identity));

        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let handle = StateActorHandle::new_with_registry(tx, Arc::clone(&registry));
        let processor = Arc::new(EventProcessor::new(handle.clone()));
        let (_, subscriber) = EffectSubscriber::new(handle.clone(), EffectExecutor::new());
        let runtime = TilingRuntime {
            generation: 7,
            actor: handle,
            actor_stopped: CompletionLatch::new(),
            processor: Arc::clone(&processor),
            subscriber,
            subscriber_stopped: CompletionLatch::new(),
            app_monitor: Arc::new(AppMonitorAdapter::new(Arc::clone(&processor))),
            screen_monitor: Arc::new(ScreenMonitorAdapter::new(Arc::clone(&processor))),
            ax_adapter: Arc::new(AXObserverAdapter::new(Arc::clone(&processor))),
            teardown: TeardownProgress {
                main_thread_sources_removed: true,
                processor_stopped: true,
                transient_services_paused: true,
                subscriber_stopped: true,
                actor_stopped: true,
                caches_cleared: true,
                ..TeardownProgress::default()
            },
        };
        *RUNTIME.lock() = RuntimeSlot::Running(runtime);
        *LIFECYCLE.lock() = LifecycleState::Running;

        pause_runtime().unwrap();
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Empty));
        assert_eq!(*LIFECYCLE.lock(), LifecycleState::Stopped);

        // The published handle sealed and drained the populated registry.
        // This fails if pause restores after taking the runtime (get_handle
        // → None → empty summary → registry never sealed).
        assert!(registry.sealed());
        assert!(!registry.contains(&identity));
        assert!(registry.is_empty());

        set_stop_timeout(None);
    }
```

Needed imports in the test module: `std::sync::Arc`, `crate::modules::tiling::identity::{AppIdentity, LaunchDateBits}`, `crate::modules::tiling::visibility::VisibilityRegistry`, `crate::modules::tiling::actor::StateActorHandle`, `crate::modules::tiling::events::EventProcessor`, `crate::modules::tiling::effects::{EffectExecutor, EffectSubscriber}`, `crate::modules::tiling::events::{AppMonitorAdapter, AXObserverAdapter, ScreenMonitorAdapter}`.

> `restore_one_exact` looks up `NSRunningApplication` for the fake PID; the `launch_date` (2001-01-01) can never match a live process, so the restore resolves to `Failed`/`false` — the summary is populated (attempted 1) but nothing real is unhidden. The assertions target registry state, not the OS result.

- [ ] **Step 2: Build and run tests**

```bash
cargo test -p stache --lib modules::tiling::visibility::tests
cargo test -p stache --lib modules::tiling::init::tests
cargo test -p stache --lib app_shutdown::tests
cargo check -p stache
cargo fmt --all -- --check
```

Expected: all pass; `app_shutdown` interface compatibility confirmed without editing the file.

- [ ] **Step 3: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/visibility.rs app/native/src/modules/tiling/init.rs
git commit -m "test(tiling): verify exact registry shutdown restoration"
```

---

## Task 19G: Delete obsolete code and simplify

**Files:**

- Modify: `app/native/src/modules/tiling/visibility.rs`
- Modify: `app/native/src/modules/tiling/effects/window_ops.rs`
- `app/native/src/modules/tiling/events/processor.rs` — already cleaned in 19E; verify only

- [ ] **Step 1: Remove deleted items from `visibility.rs`**

Delete `ShownClassification`, `PidState`, `PidState::is_zombie`, `TrackerState` (incl. `next_generation`, `log_seq`, `shutting_down`, `pids`), `HiddenAppTracker`, `STACHE_HIDDEN_APPS`, `tracker()`, `hide_app_for_workspace`, `unhide_app_for_workspace`, `classify_stache_hidden_app`, `classify_stache_hidden_app_with_state`, `forget_stache_hidden_app_terminated`, `restore_with`, and all generation/FIFO/OS-state-classifier tests. Remove old imports (`HashMap`, `VecDeque`, `OnceLock`, `app_is_hidden`, `hide_app_with_outcome`, `unhide_app`, `unhide_app_with_outcome`). Keep `RestoreSummary`, `VisibilityRegistry` (19C), `restore_stache_hidden_apps`, `restore_from_with`, `restore_one_exact`, `restore_one_exact_with`, and the identity-based tests. Result: `visibility.rs` shrinks to the clean registry + restore code.

- [ ] **Step 2: Verify `processor.rs` is clean**

Run: `rg -n "ShownClassification|classify_stache_hidden_app|forget_stache_hidden_app_terminated|on_app_shown_with" app/native/src/modules/tiling/events/processor.rs`
Expected: no matches.

- [ ] **Step 3: Remove bare-PID visibility APIs from `window_ops.rs`**

Delete `hide_app_with_outcome`, `unhide_app_with_outcome`, `hide_app`, `unhide_app`, `app_is_hidden`, `hide_apps`, `unhide_apps` and their old tests. Keep `HideAppOutcome`, `UnhideAppOutcome`, `hide_app_instance_with_outcome`, `unhide_app_instance_with_outcome` (19D Step 3), `app_instance_is_hidden` (19E), and their tests. The exact-instance helpers are self-contained — they do not share a classification helper — so nothing else is retained.

- [ ] **Step 4: Build with full project lint, then run final searches**

```bash
cargo test -p stache --lib 2>&1 | tail -20
cargo clippy -p stache --lib -- -D warnings 2>&1 | tail -20
cargo fmt --all -- --check 2>&1 | tail -20
cargo check -p stache 2>&1 | tail -20
```

Expected: all pass. Final searches proving the bare-PID APIs and the old classifier are gone:

```bash
rg -n "classify_stache_hidden_app|classify_stache_hidden_app_with_state|ShownClassification|PidState|forget_stache_hidden_app_terminated|HiddenAppTracker|begin_shutdown_and_drain|hide_app_for_workspace|unhide_app_for_workspace" app/native/src/modules/tiling
rg -n "hide_app_with_outcome|unhide_app_with_outcome|\bhide_app\b|\bunhide_app\b|app_is_hidden|hide_apps|unhide_apps" app/native/src/modules/tiling
rg -n "remove_observer_for_pid|clear_tabs_for_pid|get_window_pid\b|on_window_destroyed_for_pid" app/native/src/modules/tiling
rg -n "UpdateBorder|effects_from_focus_change" app/native/src/modules/tiling
```

Expected: no matches in all four.

> **Protected files:** No changes to `app/native/src/modules/audio/device.rs`, `app/native/src/app_shutdown.rs`, or any file outside the 19A-19G change set. `app_shutdown.rs` keeps calling `tiling::restore_stache_hidden_apps()` unchanged.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/visibility.rs \
  app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/effects/window_ops.rs
git commit -m "refactor(tiling): remove obsolete generation/FIFO/ShownClassification"
```

---

## Phase 6 invariant table (verify after 19G)

| Thread/Context                         | Runs there                                                                                                        | Registry mutex                                                                                     |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Actor task thread                      | `handle_hide_for_workspace`, `handle_unhide_for_workspace`, `on_app_shown_revalidated`, `on_app_terminated_exact` | Yes — held across OS call + set mutation only; `relinquish_if_open` is the seal-aware removal path |
| Shutdown caller (Tauri main thread)    | `seal_and_drain_visibility()`                                                                                     | Yes — one lock acquisition, released before restore                                                |
| AX callback (main thread CFRunLoop)    | `observer_callback` → `WindowEvent`                                                                               | No — reads `OBSERVER_STATE` (separate mutex), one address-keyed `get`                              |
| NSWorkspace notification (main thread) | `on_app_launched`/`on_app_terminated` → `EventProcessor` → actor message                                          | No registry access                                                                                 |
| Signal handler (`ctrlc` thread)        | Schedules work on Tauri main thread                                                                               | No — only dispatches                                                                               |

**Deadlock rules:** the actor never waits on the main thread; all main-thread removal work completes before completion waits; `seal_and_drain` runs before actor teardown while the actor loop is still alive; no lifecycle/runtime/visibility lock is held across `dispatch_on_main`/`run_on_main_thread` waits.

## Verification summary (Tasks 19A-19G)

```bash
cargo test -p stache --lib
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
cargo check -p stache
git status --short            # device.rs must remain  M and unstaged
git diff --cached --name-only # must never list device.rs
```

Manual Task 21 verification (after the module/tray plan lands) should include rapid A→B→A→B alternating hide/unhide between two applications and PID-reuse validation: terminate and relaunch an app while it is owned, then confirm restoration targets the new instance correctly and never unhides a same-PID replacement.
