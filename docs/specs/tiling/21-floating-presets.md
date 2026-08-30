# Tiling 21 — Floating Presets

> Status: ✅ Normative

## Purpose

Floating configuration supplies named presets and default positions; applying a named preset targets the focused window.

## Scope

- Owns the actor boundary described below: `ToggleFloating` and `ApplyPreset`..
- Owns the corresponding read boundary: Layoutable window queries..
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/16-frame-application.md](16-frame-application.md)                       | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Floating configuration supplies named presets and default positions; applying a named preset targets the focused window.

### Actor traceability

| Variant        | Producer                | Exact dispatch / handler                | State mutation                | Effect or OS operation         | Event / public route         | Test evidence                                     |
| -------------- | ----------------------- | --------------------------------------- | ----------------------------- | ------------------------------ | ---------------------------- | ------------------------------------------------- |
| ToggleFloating | CLI/hotkey actor handle | `handle_message` → `on_toggle_floating` | toggles window floating flag  | layout/native frame subscriber | CLI window route             | None — source-only evidence                       |
| ApplyPreset    | CLI/hotkey actor handle | `handle_message` → `on_apply_preset`    | applies named floating preset | native frame subscriber        | CLI `tiling window --preset` | `layout/floating.rs::tests::test_preset_centered` |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

| JSONC key                         | Rust type / serde name                                                            | Default          | Semantics                                                                                            | Test evidence                                                                                                       |
| --------------------------------- | --------------------------------------------------------------------------------- | ---------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `tiling.floating.defaultPosition` | `FloatingConfig.default_position: FloatingPosition` (`camelCase`, enum lowercase) | `center`         | New floating-window default position.                                                                | `config/types/tiling.rs::{FloatingConfig,FloatingPosition}`                                                         |
| `tiling.floating.presets`         | `Vec<FloatingPreset>`                                                             | `[]`             | Each named preset supplies `width` and `height`; optional `x`/`y` are ignored when `center` is true. | `config/types/tiling.rs::FloatingPreset`; `layout/floating.rs::tests::{test_preset_centered,test_preset_with_gaps}` |
| preset dimensions                 | `DimensionValue`: integer pixels or percentage string                             | no field default | `calculate_preset_frame` resolves them against the screen frame and applies gaps.                    | `layout/floating.rs::calculate_preset_frame`                                                                        |

## Inputs

- Producer: `config/types/tiling.rs::{FloatingConfig,FloatingPreset}`, `actor/handlers/preset.rs`, and `layout/floating.rs`.
- Actor input: `ToggleFloating` and `ApplyPreset`..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: Preset name is supplied by the actor handle/CLI route; no dedicated event exists..
- CLI `tiling window` applies a preset after swap and before resize/send (`app/native/src/cli/commands/tiling.rs::execute_window`).
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

Preset name is supplied by the actor handle/CLI route; no dedicated event exists.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Preset application schedules native geometry effects.

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

1. **Normal.** Given a running actor and a valid `ToggleFloating` and `ApplyPreset`. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `config/types/tiling.rs::{FloatingConfig,FloatingPreset}`, `actor/handlers/preset.rs`, and `layout/floating.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface | Implementation evidence                                                                                         | Test evidence               | Intended documentation                                            | Disposition  |
| ---------------- | --------------------------------------------------------------------------------------------------------------- | --------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract   | `config/types/tiling.rs::{FloatingConfig,FloatingPreset}`, `actor/handlers/preset.rs`, and `layout/floating.rs` | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary  | Preset name is supplied by the actor handle/CLI route; no dedicated event exists.                               | None — source-only evidence | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
