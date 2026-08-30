# Tiling 26 — Focused Borders

> Status: ✅ Normative

## Purpose

Border configuration and focus state drive native border rendering; borders remain a side-effect adapter concern.

## Scope

- Owns the actor boundary described below: Focus/window state changes can cause border updates..
- Owns the corresponding read boundary: Focus and layout queries are inputs; no border query is exposed..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/17-focus-navigation.md](17-focus-navigation.md)                         | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Border configuration and focus state drive native border rendering; borders remain a side-effect adapter concern.

### Actor traceability

| Variant                   | Producer                        | Exact dispatch / handler | State mutation | Effect or OS operation | Event / public route | Test evidence               |
| ------------------------- | ------------------------------- | ------------------------ | -------------- | ---------------------- | -------------------- | --------------------------- |
| No primary `StateMessage` | This spec owns a derived policy | —                        | —              | —                      | —                    | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

| JSONC key                                     | Rust type / serde name                                       | Default                                                                                   | Semantics                                                             | Evidence                                                                                  |
| --------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `tiling.borders.enabled`                      | `BordersConfig.enabled` (`camelCase`)                        | `false`                                                                                   | Enables border rendering.                                             | `config/types/borders.rs::BordersConfig`                                                  |
| `focused`, `unfocused`, `monocle`, `floating` | `BorderStateConfig` untagged false/solid/gradient/glow union | focused/unfocused/monocle/floating constructors use width `4` and their configured colors | State-specific border appearance.                                     | `BorderStateConfig::{default_focused,default_unfocused,default_monocle,default_floating}` |
| gradient `from`, `to`, `angle`                | `GradientConfig`                                             | angle `90.0`                                                                              | Converts colors to native RGBA; invalid hex returns an adapter error. | `GradientConfig`; `BorderStateConfig::to_gradient_rgba`                                   |
| gradient `animation`                          | `BorderAnimationConfig`                                      | easing `linear`                                                                           | Optional color-swap animation configuration.                          | `BorderAnimationConfig`                                                                   |

## Inputs

- Producer: `borders.rs`, `config/types/borders.rs`, and `effects/subscriber.rs`.
- Actor input: Focus/window state changes can cause border updates..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: No Tauri border command or public border event..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

No Tauri border command or public border event.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Border creation/update/removal is native side effect.

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

1. **Normal.** Given a running actor and a valid Focus/window state changes can cause border updates. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `borders.rs`, `config/types/borders.rs`, and `effects/subscriber.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface | Implementation evidence                                              | Test evidence               | Intended documentation                                            | Disposition  |
| ---------------- | -------------------------------------------------------------------- | --------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract   | `borders.rs`, `config/types/borders.rs`, and `effects/subscriber.rs` | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary  | No Tauri border command or public border event.                      | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
