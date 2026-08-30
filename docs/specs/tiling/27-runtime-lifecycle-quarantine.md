# Tiling 27 — Runtime Lifecycle and Quarantine

> Status: ✅ Normative

## Purpose

The actor receives enable, query, and shutdown messages; initialization emits `stache://tiling/initialized` with current `{ enabled: true, version: "v2" }` payload.

## Scope

- Owns the actor boundary described below: `SetEnabled`, `Query`, and `Shutdown`..
- Owns the corresponding read boundary: `GetEnabled` plus all actor query variants through `Query`..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/07-initial-reconciliation.md](07-initial-reconciliation.md)             | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

The actor receives enable, query, and shutdown messages; initialization emits `stache://tiling/initialized` with current `{ enabled: true, version: "v2" }` payload.

### Actor traceability

| Variant      | Producer                                                            | Exact `StateActor` arm / handler                                               | State mutation                                                        | Named effect / OS boundary                                                      | Literal public boundary                                                  | Test evidence               |
| ------------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | --------------------------- |
| `SetEnabled` | lifecycle/IPC control calls through `StateActorHandle::set_enabled` | `handle_message` → `StateActor::on_set_enabled` → `state.set_enabled(enabled)` | Sets actor enabled flag.                                              | No subscriber/native call in this dispatcher.                                   | Tauri `is_tiling_enabled` reads the flag through the query owner in T10. | None — source-only evidence |
| `Query`      | `StateActorHandle::query`                                           | `handle_message` → `execute_query` → response channel                          | Read-only; individual variants are mapped in T08/T09/T10/T11/T16/T17. | None.                                                                           | Public Tauri query routes are cross-linked in those exact query rows.    | None — source-only evidence |
| `Shutdown`   | `StateActorHandle::shutdown` / lifecycle owner                      | `StateActor::run` matches `Shutdown` before `handle_message` and returns.      | No dispatch mutation.                                                 | Actor receiver loop ends; no named subscriber/native teardown call in this arm. | No literal lifecycle event emitted by shutdown.                          | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

This capability consumes only the configuration decoded by `app/native/src/config/types/tiling.rs` or its focused config module. Configuration is immutable input to runtime state; it is not rewritten by actor commands. Where this capability has no focused key, it has **no owned configuration key**.

## Inputs

- Producer: `init.rs`, `actor/mod.rs`, `actor/handle.rs`, and `events.rs::tiling::INITIALIZED`.
- Actor input: `SetEnabled`, `Query`, and `Shutdown`..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: Tauri `is_tiling_enabled`; initialized event is consumed by the Spaces UI..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

Tauri `is_tiling_enabled`; initialized event is consumed by the Spaces UI.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Shutdown ends the actor loop; effects/monitor teardown follows their owning runtime paths.

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

1. **Normal.** Given a running actor and a valid `SetEnabled`, `Query`, and `Shutdown`. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `init.rs`, `actor/mod.rs`, `actor/handle.rs`, and `events.rs::tiling::INITIALIZED`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface | Implementation evidence                                                            | Test evidence               | Intended documentation                                            | Disposition  |
| ---------------- | ---------------------------------------------------------------------------------- | --------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract   | `init.rs`, `actor/mod.rs`, `actor/handle.rs`, and `events.rs::tiling::INITIALIZED` | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary  | Tauri `is_tiling_enabled`; initialized event is consumed by the Spaces UI.         | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
