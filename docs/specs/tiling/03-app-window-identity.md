# Tiling 03 — Application and Window Identity

> Status: ✅ Normative

## Purpose

A public window ID is a bare `u32`; exact internal targets are `WindowTarget` values tied to `AppIdentity` (PID and launch-date identity) plus window ID.

## Scope

- Owns the actor boundary described below: Identity-bearing window events: `WindowDestroyed`, `WindowFocused`, `WindowUnfocused`, `WindowMoved`, `WindowResized`, `WindowMinimized`, `WindowTitleChanged`, and `WindowFullscreenChanged`..
- Owns the corresponding read boundary: `StateQuery::GetWindow`, `GetAllWindowIds`, `HasWindow`; exact queries use `GetWindowLayoutTargets` and `GetFocusTargets`..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/10-window-state-machine.md](10-window-state-machine.md)                 | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

A public window ID is a bare `u32`; exact internal targets are `WindowTarget` values tied to `AppIdentity` (PID and launch-date identity) plus window ID.

### Actor traceability

| Variant                   | Producer                                        | Exact `StateActor::handle_message` arm / handler                             | State mutation                                                              | Named effect / OS boundary                                                                                                                | Literal public boundary                                                                                                              | Test evidence               |
| ------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------- |
| `WindowDestroyed`         | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_destroyed(state, window_id, identity)` | Removes the matching tracked window and returns its workspace when present. | `get_subscriber_handle().notify_window_destroyed(window_id)` then `notify_layout_changed(workspace_id, false)` in `actor/mod.rs:168-186`. | `stache://tiling/window-untracked` `{ windowId, workspace }`; emitter `init.rs::emit_window_untracked`.                              | None — source-only evidence |
| `WindowFocused`           | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_focused`                               | Changes focus state and returns a visibility delta.                         | `StateActor::sync_visibility_for_workspaces(delta)`; native hide/show is `visibility.rs` policy.                                          | `stache://tiling/window-focus-changed` `{ windowId, workspace }`; emitter `init.rs::emit_window_focus_changed`.                      | None — source-only evidence |
| `WindowUnfocused`         | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_unfocused`                             | Clears/updates focus state.                                                 | No subscriber/native call in this dispatch arm.                                                                                           | No literal public event emitted by this arm.                                                                                         | None — source-only evidence |
| `WindowMoved`             | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_moved`                                 | Stores observed frame.                                                      | No subscriber/native call in this dispatch arm.                                                                                           | No literal public event emitted by this arm.                                                                                         | None — source-only evidence |
| `WindowResized`           | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_resized`                               | Stores observed frame.                                                      | No subscriber/native call in this dispatch arm.                                                                                           | No literal public event emitted by this arm.                                                                                         | None — source-only evidence |
| `WindowMinimized`         | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_minimized`                             | Sets minimized state.                                                       | No subscriber/native call in this dispatch arm.                                                                                           | `stache://tiling/workspace-windows-changed` is the workspace projection emitter, owned by `init.rs::emit_workspace_windows_changed`. | None — source-only evidence |
| `WindowTitleChanged`      | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_title_changed`                         | Replaces tracked title.                                                     | No subscriber/native call in this dispatch arm.                                                                                           | `stache://tiling/window-title-changed` `{ windowId, title }`; emitter `init.rs::emit_window_title_changed`.                          | None — source-only evidence |
| `WindowFullscreenChanged` | `events/ax_observer.rs` → `events/processor.rs` | identity guard → `handlers::on_window_fullscreen_changed`                    | Sets fullscreen state.                                                      | No subscriber/native call in this dispatch arm.                                                                                           | No literal public event emitted by this arm.                                                                                         | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

This capability consumes only the configuration decoded by `app/native/src/config/types/tiling.rs` or its focused config module. Configuration is immutable input to runtime state; it is not rewritten by actor commands. Where this capability has no focused key, it has **no owned configuration key**.

## Inputs

- Producer: `identity.rs::{AppIdentity,WindowTarget}`, `actor/messages.rs`, and `actor/mod.rs::window_event_matches`.
- Actor input: Identity-bearing window events: `WindowDestroyed`, `WindowFocused`, `WindowUnfocused`, `WindowMoved`, `WindowResized`, `WindowMinimized`, `WindowTitleChanged`, and `WindowFullscreenChanged`..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: The Tauri bar API accepts bare `windowId: u32`; it does not carry `WindowTarget`..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

The Tauri bar API accepts bare `windowId: u32`; it does not carry `WindowTarget`.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Stale identity-bearing events are dropped before handler dispatch.

Effect execution is separated from actor mutation by `app/native/src/modules/tiling/effects/{subscriber,executor,window_ops}.rs`; a state update alone is not proof that macOS accepted the operation.

## Failure & Recovery

- A closed actor channel returns the handle's send/query error; it is not silently retried.
- Missing, stale, or unsupported targets follow the owning handler/adapter error path and leave no documented compensating transaction.
- Native-operation failure recovery is limited to code present in the named adapter; source provides no generic retry, rollback, quarantine, or aggregate degraded-state guarantee.

## Cross-Module Contracts

- `actor/messages.rs` owns message/query shapes; this spec owns their capability-specific interpretation.
- `effects` receives derived requests, not user intent.
- [tiling/03-app-window-identity.md](03-app-window-identity.md) owns the distinction between bare external IDs and exact targets.
- [foundation/05-cli-control-surface.md](../foundation/05-cli-control-surface.md) owns socket framing, CLI output, and exit status.

## Acceptance Scenarios

1. **Normal.** Given a running actor and a valid Identity-bearing window events: `WindowDestroyed`, `WindowFocused`, `WindowUnfocused`, `WindowMoved`, `WindowResized`, `WindowMinimized`, `WindowTitleChanged`, and `WindowFullscreenChanged`. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `identity.rs::{AppIdentity,WindowTarget}`, `actor/messages.rs`, and `actor/mod.rs::window_event_matches`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface | Implementation evidence                                                                                  | Test evidence               | Intended documentation                                            | Disposition  |
| ---------------- | -------------------------------------------------------------------------------------------------------- | --------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract   | `identity.rs::{AppIdentity,WindowTarget}`, `actor/messages.rs`, and `actor/mod.rs::window_event_matches` | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary  | The Tauri bar API accepts bare `windowId: u32`; it does not carry `WindowTarget`.                        | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
