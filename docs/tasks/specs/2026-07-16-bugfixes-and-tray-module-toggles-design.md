# Bug Fixes + Tray Module Toggles — Design

## Summary

One combined project with five sequential phases:

1. Fix menubar show/hide reaction latency (currently up to ~2s, target ≤200ms).
2. Add targeted diagnostic tracing for two intermittent bugs (Ghostty windows ignored;
   floating windows undetectable after workspace switch) — **no fixes yet**.
3. Fix those two bugs, root-caused from Phase 2 evidence.
4. Introduce a uniform module lifecycle contract (`LifecycleModule` trait) so every
   module supports safe pause/resume.
5. Add a tray icon submenu exposing runtime toggles for 6 modules, built on top of
   Phase 4.

Phases are ordered by dependency: 2 must complete (with real log evidence) before 3
starts. 4 does not depend on 2/3 and could run in parallel, but 5 depends on 4.

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
  subrole filtering in `tiling/window.rs` excluding Ghostty windows, tab
  misclassification in `tiling/tabs.rs`, or an observer/init race in `tiling/init.rs`.
- **Floating windows undetectable after switching workspace**: `tiling/effects/
subscriber.rs::handle_visibility_changed` only processes non-floating/layoutable
  windows, but `TilingState` itself still retains floating windows, so this alone does
  not fully explain the reported symptom. Root cause is unclear.

**Scope of this phase:** add targeted, low-noise `tracing::debug!` spans only —
no behavior changes. Instrument:

- Window enumeration: subrole, size-filter, and PiP-filter decisions per window
  (`tiling/window.rs`).
- Tab classification decisions (`tiling/tabs.rs`), specifically
  `is_new_window_a_tab()`.
- AXObserver registration ordering relative to initial window batch registration
  (`tiling/init.rs`).
- `effects/subscriber.rs::handle_visibility_changed` behavior for floating windows
  specifically across a workspace switch (log window id, floating flag, and whether it
  was included/excluded from the visibility pass).

Ship this phase alone. The user will reproduce both bugs with `STACHE_LOG=debug`,
capture logs, and share them before Phase 3 begins.

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
`services/traits.rs` `Module`/`BackgroundService` definitions):

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

Each module implements this trait, wrapping its existing internals behind
`start`/`pause`/`resume` instead of module-specific one-off logic:

| Module       | `pause()` behavior                                      | `resume()` behavior                                                            |
| ------------ | ------------------------------------------------------- | ------------------------------------------------------------------------------ |
| wallpapers   | Stop cycling timer (existing `stop_timer()`)            | Restart timer                                                                  |
| commandQuit  | `CGEventTapEnable(tap, false)`                          | `CGEventTapEnable(tap, true)`                                                  |
| notunes      | Unregister NSWorkspace observer                         | Re-register observer                                                           |
| proxyAudio   | `AudioObjectRemovePropertyListener` for all 3 listeners | Re-add listeners                                                               |
| menuAnywhere | `CGEventTapEnable(tap, false)`                          | `CGEventTapEnable(tap, true)`                                                  |
| tiling       | Full `shutdown()` (existing)                            | Full re-run of `init.rs` sequence (screens → workspaces → windows → observers) |

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

- Retain the `TrayIcon` handle (currently discarded in `tray::init()`) and create one
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

## Out of Scope

- Hot config reload without process restart.
- Adopting/refactoring the existing dead `services/traits.rs` code (left as-is,
  unrelated to the new trait).
- Any UI change beyond the tray menu (no changes to the bar/status widgets).
- Ghostty/floating-window fixes beyond what Phase 2 evidence supports (Phase 3 scope is
  intentionally left open pending that evidence).

## Verification Summary

| Phase | Verification                                                                                                                                                                                                       |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1     | Timestamp logging around visibility flip → event emission; manual repro; p95 ≤200ms                                                                                                                                |
| 2     | Confirm tracing spans emit expected data under `STACHE_LOG=debug` during manual repro of both bugs; no behavior change (existing test suite still passes)                                                          |
| 3     | TBD once root cause is confirmed; will include a regression test/repro case per fixed bug                                                                                                                          |
| 4     | Unit tests per module's `pause`/`resume` where OS calls can be exercised or mocked; existing test suite must still pass; manual check that OS resources are actually released/reacquired (e.g. event tap disabled) |
| 5     | Manual tray interaction test: toggle each module, confirm menu item state updates and underlying module actually pauses/resumes; confirm config-off items are locked and unavailable items show reason text        |
