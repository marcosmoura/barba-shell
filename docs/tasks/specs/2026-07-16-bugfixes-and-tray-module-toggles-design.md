# Bug Fixes + Tray Module Toggles — Design

## Summary

One combined project with six sequential phases:

1. Fix menubar show/hide reaction latency (currently up to ~2s, target ≤200ms).
2. Add targeted diagnostic tracing for two intermittent bugs (Ghostty windows ignored;
   floating windows undetectable after workspace switch) — **no fixes yet**.
3. Fix those two bugs, root-caused from Phase 2 evidence.
4. Introduce a uniform module lifecycle contract (`LifecycleModule` trait) so every
   module supports safe pause/resume.
5. Add a tray icon submenu exposing runtime toggles for 6 modules, built on top of
   Phase 4.
6. Restore windows hidden by Stache before supported shutdown paths complete.

Phases are ordered by dependency: 2 must complete (with real log evidence) before 3
starts. 4 does not depend on 2/3 and could run in parallel, but 5 depends on 4. Phase 6
uses the tiling lifecycle and must restore windows before tiling shuts down.

## Phase 1 — Menubar Latency Fix

**Root cause (confirmed):** `app/native/src/modules/bar/menubar.rs` and
`watcher.rs::run_refresh_loop` rely on a 2-second fallback poll using
`CGWindowListCopyWindowInfo` window enumeration as the only mechanism that detects
menu-bar auto-show/hide from mouse movement. The six registered NSNotifications never
fire for this interaction.

**Fix:**

- Call macOS's own `NSMenu.menuBarVisible()` (via `objc2`) from the main thread instead
  of the `CGWindowList` heuristic.
- Replace the 2-second fallback poll interval with a fast poll (~100ms) against this
  API.
- Keep the existing NSNotification fast-path (workspace/app changes) as an
  immediate-trigger optimization on top of the poll.
- No permission changes required (this API needs none).

**Verification:** Add temporary timestamp logging around the visibility flip and the
resulting Tauri event emission. Manually reproduce mouse-in/mouse-out at the menu bar
and confirm p95 latency ≤200ms from logs. Remove/gate temporary logging behind existing
debug tracing conventions once confirmed.

## Phase 2 — Diagnostic Tracing (No Fixes)

Both remaining bugs currently have only hypotheses, not confirmed root causes:

- **Ghostty windows intermittently ignored**: possible causes include `AXUnknown`
  subrole filtering in `modules/tiling/window.rs` excluding Ghostty windows, tab
  misclassification in `modules/tiling/tabs.rs`, or an observer/init race in
  `modules/tiling/init.rs`.
- **Floating windows undetectable after switching workspace**: layout is applied to
  all windows on workspace visibility change, with floating windows excluded via
  `is_layoutable()` rather than in `handle_visibility_changed` itself. The exact
  interaction between that exclusion and workspace-switch visibility handling in
  `modules/tiling/effects/subscriber.rs` is not yet understood — root cause is unclear
  and the tracing below targets this path directly rather than assuming a mechanism.

**Scope of this phase:** add targeted, low-noise `tracing::debug!` spans only —
no behavior changes. Instrument:

- Window enumeration: subrole, size-filter, and PiP-filter decisions per window
  (`modules/tiling/window.rs`).
- Tab classification decisions (`modules/tiling/tabs.rs`), specifically
  `is_new_window_a_tab()`.
- AXObserver registration ordering relative to initial window batch registration
  (`modules/tiling/init.rs`).
- `modules/tiling/effects/subscriber.rs::handle_visibility_changed` and the
  `is_layoutable()` check specifically for floating windows across a workspace switch
  (log window id, floating flag, and whether it was included/excluded from the
  layout/visibility pass).

Ship this phase alone. The user will reproduce both bugs with `RUST_LOG=stache=debug`
(the existing logging convention — debug builds already default to this level; no new
env var is introduced), capture logs, and share them before Phase 3 begins.

## Phase 3 — Fix Ghostty + Floating-Window Bugs

Deferred until Phase 2 log evidence is available. The actual mechanism may differ from
the hypotheses above; this phase's concrete change list will be written once root cause
is confirmed via evidence, not assumption. No further detail is speculated here per
root-cause debugging discipline.

## Phase 4 — Uniform Module Lifecycle Contract

**Problem:** All 6 modules (`wallpapers`, `commandQuit`, `notunes`, `proxyAudio`,
`menuAnywhere`, `tiling`) currently initialize once via `OnceLock`/atomic guards at
startup and have no safe, uniform way to stop and restart. Tiling has a partial
`shutdown()` but no re-init path. A tray toggle needs real pause/resume, not just a
UI-facing flag, otherwise OS resources (event taps, listeners, observers) keep running
even when the tray shows a module as paused.

**Design:** Define a new, minimal trait (not reusing the existing but unused
`modules/services/traits.rs` `Module`/`BackgroundService` definitions):

```rust
pub trait LifecycleModule {
    fn start(&self) -> Result<(), String>;
    fn pause(&self) -> Result<(), String>;
    fn resume(&self) -> Result<(), String>;
    fn status(&self) -> ModuleStatus;
}

pub enum ModuleStatus {
    ConfiguredOff,
    Running,
    Paused,
    Unavailable(String), // reason, e.g. missing Accessibility permission
}
```

**Handle retention requirement:** Today, several modules never retain a handle to their
own OS resource after registering it (e.g. `commandQuit`/`menuAnywhere` call
`CGEventTapEnable(tap, true)` once at init with no stored tap handle reachable later;
`notunes` registers an NSWorkspace observer with no stored reference to unregister it;
`proxyAudio` has zero `AudioObjectRemovePropertyListener` calls anywhere in the
codebase today — listeners are registered and never removed). Implementing this trait
is not simply wrapping an existing call — each module's struct must be changed to
**store** its tap/observer/listener handle(s) so `pause()`/`resume()` have something to
act on. This handle-plumbing is required, real work for every module below, not just
tiling.

Each module implements this trait, wrapping its (now handle-retaining) internals behind
`start`/`pause`/`resume` instead of module-specific one-off logic:

| Module       | `pause()` behavior                                                                                                              | `resume()` behavior                                                                                                                                                                        |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| wallpapers   | Stop cycling timer (existing `stop_timer()`)                                                                                    | Restart timer                                                                                                                                                                              |
| commandQuit  | Retain the `CGEventTap` handle at creation; `CGEventTapEnable(tap, false)`                                                      | `CGEventTapEnable(tap, true)` on the retained handle                                                                                                                                       |
| notunes      | Retain the NSWorkspace observer reference; unregister it                                                                        | Re-register the observer                                                                                                                                                                   |
| proxyAudio   | Retain the 3 listener callback references; add matching `AudioObjectRemovePropertyListener` calls (new code — none exist today) | Re-add listeners with `AudioObjectAddPropertyListener`, using the same retained callback references                                                                                        |
| menuAnywhere | Retain the `CGEventTap` handle at creation; `CGEventTapEnable(tap, false)`                                                      | `CGEventTapEnable(tap, true)` on the retained handle                                                                                                                                       |
| tiling       | Full `shutdown()` (existing)                                                                                                    | Add a `reset()` that clears the `INITIALIZED: OnceLock<bool>` guard (and any other init-once state), then fully re-run the `init.rs` sequence (screens → workspaces → windows → observers) |

Tiling is the highest-risk module: `init()` currently refuses re-entry outright when
`INITIALIZED` is already set, so `resume()` cannot just call `init()` again as-is. This
phase must add an explicit reset/teardown of that guard as part of implementing
`resume()` for tiling — this is new work beyond the existing `shutdown()`.

**State transitions:**

- `ConfiguredOff` never transitions to `Running` without an app restart/reload
  (config is immutable for the process lifetime).
- `Running ⇄ Paused` is user-toggleable at runtime via the trait's `pause`/`resume`.
- `Unavailable(reason)` applies when config says a module is on, but its `start()`
  failed (e.g. missing Accessibility permission, no usable wallpapers found). This
  state is not user-toggleable; it reflects a real failure.
- All runtime toggles reset to the config-driven default on process restart or
  `reload` (config is re-read from disk only via full restart, per existing
  architecture — this project does not add hot config reload).

**Registry:** A small collection (e.g. `Vec<Box<dyn LifecycleModule>>` or a struct with
named fields) held via `app.manage()` lets Phase 5's tray code iterate all 6 modules
uniformly without per-module special-casing.

## Phase 5 — Tray UI

- Retain the `TrayIcon` handle (currently discarded in `modules/tray/mod.rs::init()`)
  and create one
  `CheckMenuItem` per module using the Phase 4 registry.
- Initial item state is derived from each module's `status()` at tray-build time:
  - `ConfiguredOff` → `enabled: false`, unchecked, locked (cannot be toggled).
  - `Running` → `checked: true`, `enabled: true`.
  - `Paused` → `checked: false`, `enabled: true`.
  - `Unavailable(reason)` → `enabled: false`, item text includes the reason.
- `on_menu_event` matches each module's menu item ID, calls the corresponding
  `pause()`/`resume()` on the registry entry, then updates that item's `checked`/
  `enabled`/`text` via the retained `CheckMenuItem` handle (`set_checked`,
  `set_enabled`, `set_text`) — no full menu rebuild needed for state changes.
- Existing "Reload Stache" / "Quit" items are unchanged and unaffected by this work.
  Note: "Reload Stache" only exists in release builds (`#[cfg(not(debug_assertions))]`)
  — it will not appear when manually testing tray toggles in a debug build.

## Phase 6 — Restore Stache-Hidden Windows on Shutdown

**Problem:** Workspace switching hides applications with
`NSRunningApplication.hide()`. Stache currently exits without reliably unhiding those
applications, which can leave their windows hidden after the window manager is gone.

**Scope:** Restore only applications that Stache itself successfully hid during
workspace switching. Preserve windows that the user minimized and applications that
were already hidden independently of Stache.

**Design:**

Stache assigns every running application a stable identity at first
observation. That identity combines the PID and the launch date into a single
`AppIdentity` value. This is the fundamental ownership unit — a bare PID can
be reused by the kernel for a different process after the original terminates,
so identity prevents PID-reuse races without generations or FIFO state machines.

```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AppIdentity { pub pid: i32, pub launch_date: LaunchDateBits }
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LaunchDateBits(u64);
```

Identity is captured from a single local `NSRunningApplication` object:
`pid > 0`, non-null `launchDate`, `timeIntervalSinceReferenceDate` finite and

> 0, stored as `f64::to_bits`. Failure at any step is fail-closed. The ObjC
> object (`StrongPtr`) is `!Send`/`!Sync` and must not cross threads — identity
> extraction and restoration are one-shot operations, saving only the
> `AppIdentity` bytes for reuse across threads.

**Actor-single-writer ownership.** The `StateActor` is the sole runtime
mutator of ownership state. A passive `Arc<VisibilityRegistry>` is shared
with `StateActorHandle` only for synchronous terminal seal/drain. The
registry holds a `BTreeSet<AppIdentity>` behind a single `Mutex`, gated by a
`sealed` flag:

```rust
pub struct VisibilityRegistry { inner: Mutex<RegistryState> }
struct RegistryState { sealed: bool, owned: BTreeSet<AppIdentity> }
```

**Event forwarding is raw.** `EventProcessor` dispatches lifecycle events
(AppLaunched, AppTerminated, AppShown, AppHidden) keyed by identity, not
by bare PID, and never mutates ownership. The actor consumes each event and
re-validates the identity against current OS state using a local
`NSRunningApplication` lookup. The actor policy is conservative:

- **AppShown on an exact identity — OS visible:** remove owner + mark exact
  windows shown. **OS hidden or mismatch:** retain owner, no state change.
- **AppHidden — OS hidden:** mark exact windows hidden. **OS visible or
  unknown:** ignore.
- **AppTerminated:** remove exact identity only.
- **AppShown where OS state is hidden and identity matches:** the event is
  an unobserved user show→rehide within the visibility notification gap.
  Stache retains ownership ("Keep hidden") per the conservative unavoidable-
  history policy.

This deterministic revalidation replaces the generation/FIFO classifier and
its `ShownClassification` seam entirely.

**Identity propagation.** `AppIdentity` is stored on `Window`,
`WindowCreatedInfo`, and flows through `WindowEvent`, `StateMessage`
lifecycle variants, AX observer callback records, and NSWorkspace
termination handlers. The callback closure captures identity at observer
registration time rather than resolving PID at callback time.

**Workspace hide/unhide** remain actor operations. The actor validates the
identity against a freshly looked-up `AppIdentity` from one local
`NSRunningApplication`, then calls the OS hide/unhide on that same object,
then mutates registry ownership under the `VisibilityRegistry` mutex. No
notification ack is needed — the actor updates exact-identity windows in
the same turn:

- **Hide:** `HiddenByStache` inserts identity. `AlreadyHidden`/`Failed`
  preserve existing state.
- **Unhide:** `UnhiddenByStache`/`AlreadyShown` remove identity. `Failed`
  preserves it.

**Shutdown restoration.** When the actor channel is closed, the caller
obtains the shared `Arc<VisibilityRegistry>`, calls `seal`, then `drain` to
receive sorted `AppIdentity` values. For each identity, restoration obtains
one local `NSRunningApplication` by PID, captures `AppIdentity` from that
same object, requires equality with the drained identity, requires the app
to be hidden, and calls `unhide` on the same object before releasing the
reference. Errors are per-identity, best-effort. The registry lock is
released before restoration begins so late writes are impossible.

**Existing supported shutdown paths** (normal quit, tray quit, tray reload,
config-watcher restart, IPC reload, SIGTERM/SIGINT) are unchanged. The
signal handler installs one thread-safe handler that schedules work on
Tauri's main thread; AppKit/AX calls never run inside a raw signal handler.

Because Stache hides whole applications rather than moving windows off-screen,
no frame capture or repositioning is required. `unhide_app` returns their
existing windows to their current frames, while minimized state remains
controlled by macOS.

**Hard limitation:** SIGKILL cannot be caught, delayed, or handled by an in-process
application, so cleanup cannot run after `kill -9` or any Force Quit path implemented
with SIGKILL. Crash-safe recovery through a watchdog or restoration on the next launch
is outside this phase.

## Out of Scope

- Hot config reload without process restart.
- Adopting/refactoring the existing dead `modules/services/traits.rs` code (left as-is,
  unrelated to the new trait).
- Any UI change beyond the tray menu (no changes to the bar/status widgets).
- Persistent recovery or an external watchdog for SIGKILL and unrecoverable crashes.
- Ghostty/floating-window fixes beyond what Phase 2 evidence supports (Phase 3 scope is
  intentionally left open pending that evidence).
- `menuAnywhere`'s `IS_RUNNING` flag is currently set but never checked as a guard
  (unlike other modules' init guards); this inconsistency is noted but not fixed here
  since it doesn't block the trait wrapping.

## Verification Summary

| Phase | Verification                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Timestamp logging around visibility flip → event emission; manual repro; p95 ≤200ms                                                                                                                                                                                                                                                                                                                                                                                                 |
| 2     | Confirm tracing spans emit expected data under `RUST_LOG=stache=debug` during manual repro of both bugs; no behavior change (existing test suite still passes)                                                                                                                                                                                                                                                                                                                      |
| 3     | TBD once root cause is confirmed; will include a regression test/repro case per fixed bug                                                                                                                                                                                                                                                                                                                                                                                           |
| 4     | Unit tests per module's `pause`/`resume` where OS calls can be exercised or mocked; existing test suite must still pass; manual check that OS resources are actually released/reacquired (e.g. event tap disabled, tiling `reset()` actually clears init guard so resume succeeds)                                                                                                                                                                                                  |
| 5     | Manual tray interaction test: toggle each module, confirm menu item state updates and underlying module actually pauses/resumes; confirm config-off items are locked and unavailable items show reason text                                                                                                                                                                                                                                                                         |
| 6     | Unit: AppIdentity capture/validation, registry seal/drain, identity-keyed lifecycle event forwarding, actor revalidation (hidden/visible/mismatch), exact-instance restoration validation, deterministic delayed/duplicate/missing/PID-reuse tests, and best-effort continuation after individual failures. Manually verify normal quit, reload/restart, SIGTERM, and SIGINT unhide only applications hidden by Stache while preserving minimized and independently hidden windows. |
