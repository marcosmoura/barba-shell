# Tiling 09 — Workspace State

> Status: ✅ Normative

## Purpose

Workspaces are UUID-keyed internal state associated with numeric screen IDs and named for user commands; they own visible/focused membership and window ordering.

## Scope

- Owns the actor boundary described below: `SwitchWorkspace`, `CycleWorkspace`, `SendWorkspaceToScreen`, and workspace-affecting window messages..
- Owns the corresponding read boundary: `GetAllWorkspaces`, `GetWorkspace`, `GetWorkspaceByName`, `GetWindowsForWorkspace`, `GetVisibleWorkspaces`, `GetFocusedWorkspace`, `GetAllWorkspaceIds`, `GetWindowIdsForWorkspace`, `GetVisibleWorkspaceIds`, and `HasWorkspace`..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/19-cross-workspace-moves.md](19-cross-workspace-moves.md)               | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Workspaces are UUID-keyed internal state associated with numeric screen IDs and named for user commands; they own visible/focused membership and window ordering.

### Actor traceability

| Variant                 | Producer                                                                                         | Exact `StateActor::handle_message` arm / handler                  | State mutation                                                     | Named effect / OS boundary                                               | Literal public boundary                                                                                            | Test evidence                                        |
| ----------------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------- |
| `SwitchWorkspace`       | `StateActorHandle::switch_workspace`; IPC `tilingFocusWorkspace`; Tauri `focus_tiling_workspace` | `handlers::on_switch_workspace(state, name)`                      | Changes focused workspace and returns visibility delta.            | `StateActor::sync_visibility_for_workspaces(delta)` → `visibility.rs`.   | `stache://tiling/workspace-changed` `{ workspace, screen, previousWorkspace }`; `init.rs::emit_workspace_changed`. | None — source-only evidence                          |
| `CycleWorkspace`        | CLI/hotkey through actor handle                                                                  | `StateActor::on_cycle_workspace` → `handlers::on_cycle_workspace` | Cycles workspace focus.                                            | No subscriber/native call in this dispatcher; handler owns state change. | No direct Tauri route; workspace event emitter is `init.rs::emit_workspace_changed`.                               | None — source-only evidence                          |
| `SendWorkspaceToScreen` | IPC `tilingWorkspaceSendToScreen`; CLI `tiling workspace --send-to-screen`                       | `handlers::on_send_workspace_to_screen(state, target_screen)`     | Reassigns workspace to target screen and returns visibility delta. | `StateActor::sync_visibility_for_workspaces(delta)` → `visibility.rs`.   | CLI route; no per-operation result event in this arm.                                                              | `actor/messages.rs::tests::test_target_screen_parse` |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

This capability consumes only the configuration decoded by `app/native/src/config/types/tiling.rs` or its focused config module. Configuration is immutable input to runtime state; it is not rewritten by actor commands. Where this capability has no focused key, it has **no owned configuration key**.

## Inputs

- Producer: `state/tiling_state.rs`, `state/types.rs`, and `actor/handlers/workspace.rs`.
- Actor input: `SwitchWorkspace`, `CycleWorkspace`, `SendWorkspaceToScreen`, and workspace-affecting window messages..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: `get_tiling_workspaces` and `stache://tiling/workspace-changed` expose selected projections..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

`get_tiling_workspaces` and `stache://tiling/workspace-changed` expose selected projections.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Visibility synchronization is delegated to tiling visibility policy.

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

1. **Normal.** Given a running actor and a valid `SwitchWorkspace`, `CycleWorkspace`, `SendWorkspaceToScreen`, and workspace-affecting window messages. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `state/tiling_state.rs`, `state/types.rs`, and `actor/handlers/workspace.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface | Implementation evidence                                                                      | Test evidence               | Intended documentation                                            | Disposition  |
| ---------------- | -------------------------------------------------------------------------------------------- | --------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract   | `state/tiling_state.rs`, `state/types.rs`, and `actor/handlers/workspace.rs`                 | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary  | `get_tiling_workspaces` and `stache://tiling/workspace-changed` expose selected projections. | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
