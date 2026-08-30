# Tiling 12 — Layout Selection

> Status: 🟡 Draft · Open: T12-D1

## Purpose

The eight serialized layout names are `dwindle`, `split`, `split-vertical`, `split-horizontal`, `monocle`, `master`, `grid`, and `floating`; configured `LayoutType::default()` is Dwindle.

## Scope

- Owns the actor boundary described below: `SetLayout`, `CycleLayout`.
- Owns the corresponding read boundary: `StateQuery::GetWindowLayout`.
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

The eight serialized layout names are `dwindle`, `split`, `split-vertical`, `split-horizontal`, `monocle`, `master`, `grid`, and `floating`; configured `LayoutType::default()` is Dwindle.

### Actor traceability

| Variant       | Producer                              | Exact dispatch / handler                                     | State mutation                                             | Effect or OS operation                                                                              | Event / public route                                                    | Test evidence               |
| ------------- | ------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | --------------------------- |
| `SetLayout`   | CLI/workspace command or actor handle | `StateActor::handle_message` → `StateActor::on_set_layout`   | Sets workspace layout and resets incompatible ratio state. | The handler can drive layout recomputation; no current call reaches `init.rs::emit_layout_applied`. | CLI `tiling workspace --layout`; layout-change delivery is open T12-D1. | None — source-only evidence |
| `CycleLayout` | CLI/workspace command or actor handle | `StateActor::handle_message` → `StateActor::on_cycle_layout` | Selects the next layout.                                   | The handler can drive layout recomputation; no current call reaches `init.rs::emit_layout_applied`. | CLI `tiling workspace`; layout-change delivery is open T12-D1.          | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

| JSONC key              | Rust type / serde name                                        | Default                | Precedence and semantics                                                                                            | Evidence                                                                                                            |
| ---------------------- | ------------------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `tiling.enabled`       | `TilingConfig.enabled` (`camelCase`)                          | `false`                | Controls whether tiling initialization is requested.                                                                | `config/types/tiling.rs::TilingConfig`; `tests::test_tiling_config_default`                                         |
| `tiling.defaultLayout` | `TilingConfig.default_layout: LayoutType` (`kebab-case` enum) | `dwindle`              | Used for a workspace that has no explicit layout; an existing workspace's runtime selection is not config mutation. | `config/types/tiling.rs:199-206`; `tests::test_layout_type_default_is_dwindle`, `test_default_layout_serialization` |
| workspace `layout`     | `WorkspaceConfig` layout override                             | no independent default | The workspace value, if present, is selected at workspace creation; otherwise `defaultLayout` applies.              | `config/types/workspaces.rs`; `actor/handlers/workspace.rs`                                                         |

The eight accepted serialized names are `dwindle`, `split`, `split-vertical`, `split-horizontal`, `monocle`, `master`, `grid`, and `floating` (`LayoutType::as_str`; `tests::test_layout_type_as_str`).

## Inputs

- Producer: `config/types/tiling.rs::{LayoutType,TilingConfig}`, `actor/handlers/layout.rs`, and `layout/mod.rs`.
- Actor input: `SetLayout`, `CycleLayout`.
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport: CLI `tiling workspace --layout`; no current emitted layout-change event.
- CLI `tiling workspace` executes combined operations in source order: focus → layout → balance → send (`app/native/src/cli/commands/tiling.rs::execute_workspace`).
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

The current application publishes no observed `stache://tiling/layout-changed` delivery: the symbol is declared and `init.rs::emit_layout_applied` exists, but that helper has no in-tree caller and `Spaces.state.ts` has no subscription.

## Derived Effects

Selection delegates frame calculation; it does not directly set native geometry.

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

1. **Normal.** Given a running actor and a valid `SetLayout`, `CycleLayout` producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `config/types/tiling.rs::{LayoutType,TilingConfig}`, `actor/handlers/layout.rs`, and `layout/mod.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

| ID     | Current behavior                                                                                                                                                                                                        | Documented intent                                                                                   | Rewrite consequence                                                                                                                                       | Evidence                                                                                                                                                                                                           |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| T12-D1 | `events.rs::tiling::LAYOUT_CHANGED` is declared and `init.rs::emit_layout_applied` constructs `{ workspace, layout, windowCount }`, but no in-tree caller reaches the helper and `Spaces.state.ts` has no subscription. | The Event Catalog says `stache://tiling/layout-changed` is emitted when a workspace layout changes. | Do not specify delivery, subscriber refresh, or emission timing as current behavior; retain the complete intended payload outside the normative contract. | `app/native/src/events.rs`; `app/native/src/modules/tiling/init.rs::emit_layout_applied`; `app/ui/renderer/bar/Spaces/Spaces.state.ts`; `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md#Event-Catalog` |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                    | Implementation evidence                                                                                                                                                                        | Test evidence                                                                                            | Intended documentation                                                                                                                                            | Disposition  |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| Actor contract                                                      | `config/types/tiling.rs::{LayoutType,TilingConfig}`, `actor/handlers/layout.rs`, and `layout/mod.rs`                                                                                           | None — source-only evidence                                                                              | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture`                                                                                                 | Current-only |
| `stache://tiling/layout-changed` declaration, payload, and delivery | `events.rs::tiling::LAYOUT_CHANGED` is declared; dormant `init.rs::emit_layout_applied` constructs `{ workspace, layout, windowCount }`; no in-tree caller and no `Spaces.state.ts` subscriber | None — source-only evidence                                                                              | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md#Event-Catalog` promises emission on workspace layout change with `{ workspace, layout, windowCount }` | Conflict     |
| `tiling.defaultLayout`                                              | `app/native/src/config/types/tiling.rs::TilingConfig.default_layout` and `LayoutType::default`                                                                                                 | `config/types/tiling.rs::tests::{test_layout_type_default_is_dwindle,test_default_layout_serialization}` | `/Users/marcosmoura/Documents/stache-docs/configuration.md#Tiling`                                                                                                | Aligned      |
