# Tiling 10 — Window State Machine

> Status: ✅ Normative

## Purpose

Tracked windows are keyed by bare `u32` in public state; identity-bearing events are checked against the stored application identity before mutation.

## Scope

- Owns the actor boundary described below: Creation/destruction, focus, geometry, minimize, title, fullscreen, `ToggleFloating`, and `Query`..
- Owns the corresponding read boundary: `GetAllWindows`, `GetWindow`, `GetWindowsForPid`, `GetAllWindowIds`, and `HasWindow`..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/03-app-window-identity.md](03-app-window-identity.md)                   | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Tracked windows are keyed by bare `u32` in public state; identity-bearing events are checked against the stored application identity before mutation.

### Actor traceability

| Variant | Producer                                                                            | Exact `StateActor` arm / handler                                      | State mutation | Named effect / OS boundary | Literal public boundary                                                                                                                                                                                                | Test evidence               |
| ------- | ----------------------------------------------------------------------------------- | --------------------------------------------------------------------- | -------------- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| `Query` | `StateActorHandle::query`; Tauri bar commands in `modules/bar/components/tiling.rs` | `handle_message` → `execute_query(query)` → `respond_to.send(result)` | Read-only.     | None.                      | `get_tiling_windows`, `get_tiling_current_workspace_windows`; window event projections are emitted by `init.rs::{emit_window_tracked,emit_window_untracked,emit_window_title_changed,emit_workspace_windows_changed}`. | None — source-only evidence |

Window lifecycle message ownership is explicit in [tiling/01-window-eligibility.md](01-window-eligibility.md) (`WindowCreated`) and [tiling/03-app-window-identity.md](03-app-window-identity.md) (identity-bearing updates); this spec owns their stored-window query/public projections only.

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

This capability consumes only the configuration decoded by `app/native/src/config/types/tiling.rs` or its focused config module. Configuration is immutable input to runtime state; it is not rewritten by actor commands. Where this capability has no focused key, it has **no owned configuration key**.

## Inputs

- Producer: `state/types.rs::Window`, `actor/mod.rs`, and `actor/handlers/window.rs`.
- Actor input: `StateMessage::Query`; lifecycle message ownership is cross-linked in the Actor Traceability table.
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: Tauri `get_tiling_windows` and `get_tiling_current_workspace_windows`; events `window-tracked`, `window-untracked`, `window-title-changed`, and `workspace-windows-changed`..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

Tauri `get_tiling_windows` and `get_tiling_current_workspace_windows`; `stache://tiling/window-tracked` carries `{ windowId: u32, workspace: String }`, `stache://tiling/window-untracked` carries `{ windowId: u32, workspace: String }`, `stache://tiling/window-title-changed` carries `{ windowId: u32, title: String }`, and `stache://tiling/workspace-windows-changed` carries `{ workspace: String, windows: Vec<u32> }`.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

State changes can request frames and emit the named tiling window events.

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

1. **Normal.** Given a running actor and a valid Creation/destruction, focus, geometry, minimize, title, fullscreen, `ToggleFloating`, and `Query`. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `state/types.rs::Window`, `actor/mod.rs`, and `actor/handlers/window.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                 | Implementation evidence                                                                                                                                                                                        | Test evidence               | Intended documentation                                                     | Disposition  |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | -------------------------------------------------------------------------- | ------------ |
| Actor contract                   | `state/types.rs::Window`, `actor/mod.rs`, and `actor/handlers/window.rs`                                                                                                                                       | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture`          | Current-only |
| Public events and window queries | `modules/bar/components/tiling.rs::{get_tiling_windows,get_tiling_current_workspace_windows}`; `init.rs::{emit_window_tracked,emit_window_untracked,emit_window_title_changed,emit_workspace_windows_changed}` | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md#Event-Catalog` | Aligned      |
