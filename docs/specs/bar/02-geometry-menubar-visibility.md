# Status Bar 02 — Geometry and Menu-Bar Visibility

> Status: 🟡 Draft · Open: MB1

## Purpose

This capability positions the bar and projects the observed macOS menu-bar visibility into the bar renderer.

## Scope

- Bar frame calculation and `get_bar_window_frame`.
- Display repositioning and menu-visibility event meaning.
- Frontend CSS visibility presentation.

### Out of Scope

| Excluded concern                   | Owner                                                                | Boundary note                                      |
| ---------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------- |
| Bar initialization and composition | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)            | This capability operates on its created window.    |
| Widget placement                   | [widgets/01 overlay lifecycle](../widgets/01-overlay-lifecycle.md)   | Widgets consume the frame command.                 |
| Event declaration transport        | [foundation/06 frontend events](../foundation/06-frontend-events.md) | This spec owns this event's meaning.               |
| System menu-bar policy             | Not a current capability                                             | Stache observes but does not control macOS policy. |

## Terminology

- **Bar frame** — logical `{ x, y, width, height }` used for the bar window.
- **Menu visible** — boolean emitted for the observed native menu bar.

## Data Contract

For logical screen width `W`, configured height `H`, and padding `P`, the frame is `(P, P, W - 2P, H)`. `get_bar_window_frame` returns those four camel-case fields or a `StacheError` when the bar window or screen size cannot be obtained.

`stache://menubar/visibility-changed` carries a boolean. `useBar` stores it under `['menubar-visibility']`; `false` applies `transform: translateY(100%)` and `opacity: 0` to bar content, while `true` leaves the base CSS class.

## Configuration Contract

Consumes `bar.height` and `bar.padding`; no range validation or fallback is implemented. The watcher fallback poll is 100 ms and is not configurable.

## Inputs

- Native display dimensions and display-reconfiguration callbacks.
- AppKit, pointer, and Core Graphics menu-bar visibility probes.
- Workspace/application notifications and fallback polling.

## State Transitions

| From         | Input                              | To                 | Effect                                     |
| ------------ | ---------------------------------- | ------------------ | ------------------------------------------ |
| Unpositioned | startup or display signal          | Positioned attempt | Recalculate and call the position adapter. |
| Watching(v)  | changed resolved visibility        | Watching(v')       | Emit the boolean to `bar`.                 |
| Watching(v)  | unchanged or unresolved visibility | Watching(v)        | Emit nothing; retain prior resolution.     |
| Query cache  | event `false`                      | CSS hidden         | Apply `barHidden`.                         |
| CSS hidden   | event `true`                       | CSS visible        | Remove `barHidden`.                        |

## Outputs

- A requested native frame and query result.
- A deduplicated visibility event for the `bar` webview.
- CSS transform/opacity presentation state; no native bar hide/show request is made by this flow.

## Derived Effects

A screen-watcher thread invokes repositioning. The menu watcher registers native observers, uses a 100 ms fallback poll, and emits through the bar webview when its resolved value changes.

## Failure & Recovery

Screen/frame lookup logs or returns the command error. Observer registration is best-effort; failures are logged and refresh continues where possible. Unresolved visibility preserves the prior result. The observer/runtime has no documented stop or restart operation.

## Cross-Module Contracts

[bar/01](01-bar-window-lifecycle.md) starts the watcher and hosts CSS. [widgets/01](../widgets/01-overlay-lifecycle.md) obtains the same frame command. [foundation/06](../foundation/06-frontend-events.md) declares transport, not the boolean's semantics.

## Acceptance Scenarios

1. Given width 1920, height 28, and padding 12, when a frame is calculated, then it is `(12, 12, 1896, 28)`.
2. Given a screen reconfiguration, when the watcher delivers it, then position is recalculated.
3. Given a new resolved boolean, when it differs from prior state, then one visibility event is emitted.
4. Given an unchanged or wholly unresolved reading, when refresh runs, then no duplicate event is emitted.
5. Given `false`, when the bar receives the event, then CSS transforms it down and makes it transparent rather than calling a native hide API.
6. Given frame-command failure, when a widget asks for geometry, then it receives the command error.
7. Given process exit, when observers remain process-lifetime, then no unsupported restart guarantee is claimed.

## Testing Seam

`calculate_window_frame` and `resolve_menu_bar_visible` are pure/native-reduction seams; frontend visibility is covered by `app/ui/renderer/bar/Bar.test.tsx`. Relevant native tests are in `app/native/src/modules/bar/window.rs:66-131` and `menubar.rs:462-792`.

## Open Decisions

| ID  | Current behavior                                                                                                                                                       | Documented intent                                                                                       | Rewrite consequence                                                                                                                           | Evidence                                                                                                                             |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| MB1 | The watcher emits state and `Bar.styles.ts` hides content with transform/opacity; `menubar.rs` and `bar/mod.rs` do not hide/show the bar window on visibility changes. | `status-bar.md:46-47` says the bar hides when the system menu bar is visible and returns when it hides. | A rewrite must choose content-only presentation or add a native window-visibility contract; it MUST NOT describe native hide/show as current. | `app/native/src/modules/bar/menubar.rs:67-137`; `app/ui/renderer/bar/Bar.state.ts:6-16`; `Bar.styles.ts:5-28`; `status-bar.md:46-47` |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                      | Implementation evidence                                        | Test evidence                                                                                                                                                                  | Intended documentation | Disposition  |
| ----------------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------- | ------------ |
| Frame formula and command errors                      | `app/native/src/modules/bar/window.rs:17-64`                   | `window.rs` — `calculate_window_frame_returns_correct_dimensions`; `calculate_window_frame_with_custom_config`                                                                 | `status-bar.md:31-37`  | Aligned      |
| Deduplicated visibility event                         | `app/native/src/modules/bar/menubar.rs:67-137,344-370`         | `menubar.rs` — `visibility_event_constant_is_correct`; `menu_bar_fallback_poll_interval_is_100ms`; `resolve_visible_when_nsmenu_true`; `resolve_preserves_prior_when_all_fail` | `events-and-ipc.md`    | Current-only |
| CSS content presentation versus claimed native hiding | `app/ui/renderer/bar/Bar.state.ts:6-16`; `Bar.styles.ts:24-28` | `Bar.test.tsx` — `renders correctly when menu is hidden`                                                                                                                       | `status-bar.md:46-47`  | Conflict     |
