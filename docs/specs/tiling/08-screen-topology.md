# Tiling 08 — Screen Topology

> Status: ✅ Normative

## Purpose

Screens use numeric `u32` display IDs. `SetScreens` accepts main-thread-detected records; `ScreensChanged` delegates detection to its handler.

## Scope

- Owns the actor boundary described below: `StateMessage::ScreensChanged`, `StateMessage::SetScreens`.
- Owns the corresponding read boundary: `GetAllScreens`, `GetScreen`, `GetWorkspacesForScreen`, `GetAllScreenIds`, and `HasScreen`..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/09-workspace-state.md](09-workspace-state.md)                           | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Screens use numeric `u32` display IDs. `SetScreens` accepts main-thread-detected records; `ScreensChanged` delegates detection to its handler.

### Actor traceability

| Variant          | Producer                               | Exact dispatch / handler                                      | State mutation                     | Effect or OS operation                                              | Event / public route                                                                                     | Test evidence               |
| ---------------- | -------------------------------------- | ------------------------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------- |
| `ScreensChanged` | `events/screen_monitor.rs`             | `StateActor::handle_message` → `handlers::on_screens_changed` | Reconciles detected display state. | Screen/workspace layout reconciliation owned by the screen handler. | `stache://tiling/screens-changed` remains declared-only; Event Catalog says emission is not implemented. | None — source-only evidence |
| `SetScreens`     | `init.rs` main-thread screen detection | `StateActor::handle_message` → `handlers::on_set_screens`     | Replaces/reconciles actor screens. | Screen/workspace layout reconciliation owned by the screen handler. | `stache://tiling/screens-changed` remains declared-only; Event Catalog says emission is not implemented. | None — source-only evidence |

### Query traceability

| Query                  | Exact `execute_query` arm                    | Result                    | Effect | Public route               | Test evidence               |
| ---------------------- | -------------------------------------------- | ------------------------- | ------ | -------------------------- | --------------------------- |
| GetAllScreens          | `screens.iter().cloned().collect()`          | `QueryResult::Screens`    | none   | no direct public route     | None — source-only evidence |
| GetScreen              | `state.get_screen(id)`                       | `QueryResult::Screen`     | none   | no direct public route     | None — source-only evidence |
| GetWorkspacesForScreen | `state.get_workspaces_for_screen(screen_id)` | `QueryResult::Workspaces` | none   | Tauri workspace projection | None — source-only evidence |
| GetAllScreenIds        | `state.get_all_screen_ids()`                 | `QueryResult::ScreenIds`  | none   | no direct public route     | None — source-only evidence |
| HasScreen              | `state.has_screen(id)`                       | `QueryResult::Exists`     | none   | no direct public route     | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

This capability consumes only the configuration decoded by `app/native/src/config/types/tiling.rs` or its focused config module. Configuration is immutable input to runtime state; it is not rewritten by actor commands. Where this capability has no focused key, it has **no owned configuration key**.

## Inputs

- Producer: `actor/messages.rs`, `actor/handlers/screen.rs`, and `events/screen_monitor.rs`.
- Actor input: `StateMessage::ScreensChanged`, `StateMessage::SetScreens`.
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport: `stache://tiling/screens-changed` is declared in Rust/TypeScript and is intentionally not emitted (`events-and-ipc.md#Event Catalog`).
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

`stache://tiling/screens-changed` is a declared public event with screen-object payload; its Event Catalog status is “Defined, but emission is not implemented.” It has no emitter today.

## Derived Effects

Screen reconciliation changes actor screen/workspace relationships.

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

1. **Normal.** Given a running actor and a valid `StateMessage::ScreensChanged`, `StateMessage::SetScreens` producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `actor/messages.rs`, `actor/handlers/screen.rs`, and `events/screen_monitor.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface    | Implementation evidence                                                                        | Test evidence                                                                           | Intended documentation                                                                                                 | Disposition  |
| ------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------ |
| Actor contract      | `actor/messages.rs`, `actor/handlers/screen.rs`, and `events/screen_monitor.rs`                | None — source-only evidence                                                             | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture`                                                      | Current-only |
| Declared-only event | `app/native/src/events.rs::tiling::SCREENS_CHANGED` is declared; no tiling emitter call exists | `events.rs::tests::test_all_events_have_stache_prefix` verifies declaration naming only | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md#Event-Catalog`: “Defined, but emission is not implemented” | Aligned      |
