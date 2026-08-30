# Tiling 14 — Gaps and Usable Geometry

> Status: ✅ Normative

## Purpose

Gap config accepts scalar or per-axis values and computes usable geometry before layout frames are calculated.

## Scope

- Owns the actor boundary described below: No dedicated message; geometry is consumed during layout calculation..
- Owns the corresponding read boundary: `GetWindowLayout`.
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/13-layout-algorithms.md](13-layout-algorithms.md)                       | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Gap config accepts scalar or per-axis values and computes usable geometry before layout frames are calculated.

### Geometry transform

`layout/gaps.rs::Gaps::from_config` selects configured gaps and adds the bar offset to the top outer gap. `Gaps::apply_outer` transforms a frame to `(x + left, y + top, width - left - right, height - top - bottom)`; inner gaps are consumed by each layout split. `Gaps::with_top_offset` changes top only. Focused evidence: `layout/gaps.rs::tests::{test_gaps_apply_outer,test_gaps_apply_outer_asymmetric,test_gaps_with_top_offset}`.

### Actor traceability

| Variant                   | Producer                        | Exact dispatch / handler | State mutation | Effect or OS operation | Event / public route | Test evidence               |
| ------------------------- | ------------------------------- | ------------------------ | -------------- | ---------------------- | -------------------- | --------------------------- |
| No primary `StateMessage` | This spec owns a derived policy | —                        | —              | —                      | —                    | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

| JSONC key     | Rust type / serde name                             | Default                        | Semantics                                                               | Test evidence                                                                                |
| ------------- | -------------------------------------------------- | ------------------------------ | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `tiling.gaps` | `GapsConfigValue` untagged global/per-screen union | global `GapsConfig::default()` | A per-screen record selects by `screen`; otherwise global values apply. | `config/types/gaps.rs::{GapsConfigValue,ScreenGapsConfig}`; `tests::test_gap_value_as_inner` |
| `inner`       | `GapValue` untagged uniform/per-axis/per-side      | `0`                            | Inner per-side values reduce to average horizontal/vertical values.     | `GapValue::as_inner`; `tests::test_gap_value_as_inner`                                       |
| `outer`       | `GapValue` untagged uniform/per-axis/per-side      | `0`                            | Outer values resolve as top/right/bottom/left.                          | `GapValue::as_outer`; `tests::test_gap_value_as_outer`                                       |

## Inputs

- Producer: `config/types/gaps.rs` and `layout/gaps.rs`.
- Actor input: No dedicated message; geometry is consumed during layout calculation..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: No direct Tauri command or event..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

No direct Tauri command or event.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Gap computation changes proposed frames only.

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

1. **Normal.** Given a running actor and a valid No dedicated message; geometry is consumed during layout calculation. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `config/types/gaps.rs` and `layout/gaps.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                  | Implementation evidence                                                                                     | Test evidence                                                                                               | Intended documentation                                            | Disposition  |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract                    | `config/types/gaps.rs` and `layout/gaps.rs`                                                                 | None — source-only evidence                                                                                 | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary                   | No direct Tauri command or event.                                                                           | None — source-only evidence                                                                                 | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
| Gap selection and frame transform | `config/types/gaps.rs::{GapValue,GapsConfigValue}`; `layout/gaps.rs::{Gaps::from_config,Gaps::apply_outer}` | `layout/gaps.rs::tests::{test_gaps_apply_outer,test_gaps_apply_outer_asymmetric,test_gaps_with_top_offset}` | `/Users/marcosmoura/Documents/stache-docs/configuration.md#Gaps`  | Aligned      |
