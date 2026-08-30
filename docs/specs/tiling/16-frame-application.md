# Tiling 16 — Frame Application

> Status: ✅ Normative

## Purpose

Layoutable windows map to frames, then the effect subscriber/executor applies native window operations; query variants expose calculated layouts.

## Scope

- Owns the actor boundary described below: Layout-affecting messages and `SetExpectedFrames` feed this boundary..
- Owns the corresponding read boundary: `GetLayoutableWindows`, `GetWindowLayout`, `GetWindowLayoutTargets`, and `GetLayoutableWindowIds`..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Layoutable windows map to frames, then the effect subscriber/executor applies native window operations; query variants expose calculated layouts.

### Frame/effect boundary

`StateActor::compute_layout` obtains layoutable windows, `Gaps::from_config`, runtime-or-config master ratio, and calls `calculate_layout_full`; it then applies the minimum-size branch before returning bare-ID frames. `compute_layout_targets` omits a window with no stored identity (fail-closed) and produces `(WindowTarget, Rect)` for the effect subscriber. `effects/subscriber.rs` consumes the request and `effects/{executor,window_ops}.rs` owns native frame application/animation.

### Actor traceability

| Variant                | Producer                  | Exact dispatch / handler                   | State mutation                | Effect or OS operation             | Event / public route          | Test evidence               |
| ---------------------- | ------------------------- | ------------------------------------------ | ----------------------------- | ---------------------------------- | ----------------------------- | --------------------------- |
| GetWindowLayout        | `StateActorHandle::query` | `execute_query` → `compute_layout`         | read-only computed frames     | none                               | internal/CLI query projection | None — source-only evidence |
| GetWindowLayoutTargets | effect subscriber query   | `execute_query` → `compute_layout_targets` | read-only exact target frames | effect subscriber receives targets | not a bare-ID public contract | None — source-only evidence |

### Query traceability

| Query                  | Exact `execute_query` arm             | Result                      | Effect                                   | Public route          | Test evidence               |
| ---------------------- | ------------------------------------- | --------------------------- | ---------------------------------------- | --------------------- | --------------------------- |
| GetLayoutableWindows   | `state.get_layoutable_windows(id)`    | `QueryResult::Windows`      | none                                     | internal layout input | None — source-only evidence |
| GetWindowLayout        | `compute_layout(id)`                  | `QueryResult::Layout`       | none                                     | internal/CLI query    | None — source-only evidence |
| GetWindowLayoutTargets | `compute_layout_targets(id)`          | `QueryResult::TargetLayout` | effect subscriber receives exact targets | not bare-ID public    | None — source-only evidence |
| GetLayoutableWindowIds | `state.get_layoutable_window_ids(id)` | `QueryResult::WindowIds`    | none                                     | internal layout input | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

| JSONC key                    | Rust type / serde name                  | Default    | Semantics                                                                         | Test evidence                                                                     |
| ---------------------------- | --------------------------------------- | ---------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `tiling.animations.enabled`  | `AnimationConfig.enabled` (`camelCase`) | `false`    | Enables the animation effect path.                                                | `config/types/tiling.rs::AnimationConfig`; `tests::test_animation_config_default` |
| `tiling.animations.duration` | `AnimationConfig.duration: u32`         | `200` ms   | Large moves use this duration; smaller moves scale down in the animation adapter. | `AnimationConfig`; `tests::test_animation_config_default`                         |
| `tiling.animations.easing`   | `EasingType` (`kebab-case`)             | `ease-out` | Chooses the configured easing enum.                                               | `EasingType`; `tests::test_easing_type_default_is_ease_out`                       |

## Inputs

- Producer: `effects/{subscriber,executor,window_ops}.rs`, `actor/handlers/layout.rs`, and `config/types/tiling.rs::AnimationConfig`.
- Actor input: Layout-affecting messages and `SetExpectedFrames` feed this boundary..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: No direct frame-setting Tauri command..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

No direct frame-setting Tauri command.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Native frame operations and optional animation are executed here.

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

1. **Normal.** Given a running actor and a valid Layout-affecting messages and `SetExpectedFrames` feed this boundary. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `effects/{subscriber,executor,window_ops}.rs`, `actor/handlers/layout.rs`, and `config/types/tiling.rs::AnimationConfig`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                 | Implementation evidence                                                                                                  | Test evidence               | Intended documentation                                            | Disposition  |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract                   | `effects/{subscriber,executor,window_ops}.rs`, `actor/handlers/layout.rs`, and `config/types/tiling.rs::AnimationConfig` | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary                  | No direct frame-setting Tauri command.                                                                                   | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
| Identity-keyed frame application | `actor/mod.rs::{compute_layout,compute_layout_targets}`; `effects/{subscriber,executor,window_ops}.rs`                   | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
