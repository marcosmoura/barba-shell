# Tiling 13 — Layout Algorithms

> Status: ✅ Normative

## Purpose

Each selected layout delegates to its named pure layout module; output is an ordered set of window-ID/frame pairs for later effects.

## Scope

- Owns the actor boundary described below: Layout-changing messages cause recalculation through handlers..
- Owns the corresponding read boundary: `GetWindowLayout`, `GetWindowLayoutTargets`.
- Records the source-to-effect/public trace for this capability; adjacent state remains separately owned.

### Out of Scope

| Excluded concern             | Owner                                                                           | Boundary note                                                  |
| ---------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Adjacent tiling state policy | [tiling/14-gaps-usable-geometry.md](14-gaps-usable-geometry.md)                 | This capability consumes that state rather than redefining it. |
| Runtime lifecycle            | [tiling/27-runtime-lifecycle-quarantine.md](27-runtime-lifecycle-quarantine.md) | Initialization and shutdown orchestration remain there.        |

## Terminology

- **Actor message** — a `StateMessage` received by the single tiling state actor.
- **Bare external window ID** — a public `u32` window identifier; it is not an exact lifetime identity.
- **Exact window target** — `WindowTarget`, which binds a window ID to `AppIdentity`; use it where the actor/effect API requires it.

## Data Contract

Each selected layout delegates to its named pure layout module; output is an ordered set of window-ID/frame pairs for later effects.

### Algorithm dispatch and formulas

`layout/mod.rs::calculate_layout_full` routes the selected `LayoutType` after the caller supplies ordered layoutable IDs, visible frame, ratio, gaps, split ratios, and master position. The functions do not issue native operations.

| Layout             | Pure function                        | Current calculation / edge behavior                                                                                                                        | Focused tests                                                                                                           |
| ------------------ | ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `dwindle`          | `layout/dwindle.rs::layout`          | Recursively alternates split direction from screen orientation; uses supplied ratio or `0.5`, subtracts the matching inner gap, and preserves input order. | `tests::test_landscape_two_windows`, `test_dwindle_with_custom_ratio_two_windows`, `test_dwindle_preserves_order`       |
| `split`            | `layout/split.rs::layout_auto`       | Uses horizontal rows when width ≥ height and vertical rows otherwise; supplied cumulative ratios define segment edges.                                     | `tests::test_auto_landscape`, `test_auto_portrait`, `test_auto_square`                                                  |
| `split-horizontal` | `layout/split.rs::layout_horizontal` | Divides usable width into ordered columns, subtracting horizontal inner gaps; missing ratios are distributed by the function.                              | `tests::test_horizontal_two_windows`, `test_horizontal_with_custom_ratios`                                              |
| `split-vertical`   | `layout/split.rs::layout_vertical`   | Divides usable height into ordered rows, subtracting vertical inner gaps.                                                                                  | `tests::test_vertical_two_windows`, `test_vertical_with_custom_ratios`                                                  |
| `monocle`          | `layout/monocle.rs::layout`          | Returns the same supplied usable frame for every ordered ID; empty input returns no frames.                                                                | `tests::test_monocle_empty`, `test_monocle_multiple_windows`                                                            |
| `master`           | `layout/master.rs::layout`           | First ID occupies the selected master side at the clamped master ratio; remaining IDs divide the stack axis, with auto position based on orientation.      | `tests::test_auto_landscape_two_windows`, `test_left_position`, `test_master_ratio_clamping_low`                        |
| `grid`             | `layout/grid.rs::layout`             | Chooses its code-defined grid pattern for ordered IDs and truncates input beyond twelve IDs.                                                               | `tests::test_layout_four_windows_landscape`, `test_layout_twelve_windows_landscape`, `test_layout_max_windows_exceeded` |
| `floating`         | `layout/floating.rs`                 | The selected layout emits no tiled frame calculation; presets use `calculate_preset_frame` separately.                                                     | `tests::test_preset_centered`, `test_preset_with_gaps`                                                                  |

Shared rectangle formulas are `layout/helpers.rs::{split_horizontal,split_vertical}`: the first rect uses `ratio × available_axis`; the second starts after that size plus the gap. The gap is subtracted once from available axis before the split. Evidence: `tests::test_split_horizontal_with_gap`, `test_split_vertical_with_gap`.

### Actor traceability

| Variant                   | Producer                        | Exact dispatch / handler | State mutation | Effect or OS operation | Event / public route | Test evidence               |
| ------------------------- | ------------------------------- | ------------------------ | -------------- | ---------------------- | -------------------- | --------------------------- |
| No primary `StateMessage` | This spec owns a derived policy | —                        | —              | —                      | —                    | None — source-only evidence |

A bare external `u32` window ID is never an exact `WindowTarget`; `WindowTarget` carries the stored `AppIdentity` and window ID. `StateActor::handle_message` and `StateActor::execute_query` in `actor/mod.rs` are the dispatch seams named in every row.

## Configuration Contract

| JSONC key                | Rust type / serde name                  | Default               | Semantics                                                                                           | Evidence                                                                                                                               |
| ------------------------ | --------------------------------------- | --------------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `tiling.master.ratio`    | `MasterConfig.ratio: u32` (`camelCase`) | `60`                  | `compute_layout` divides by 100 for the runtime master ratio; the layout clamps its accepted ratio. | `config/types/tiling.rs::MasterConfig`; `tests::test_master_config_default`; `layout/master.rs::tests::test_master_ratio_clamping_low` |
| `tiling.master.position` | `MasterPosition` (`kebab-case`)         | `auto`                | Auto selects left when width ≥ height and top otherwise.                                            | `config/types/tiling.rs::MasterPosition`; `layout/master.rs::tests::test_square_screen_auto_uses_left`                                 |
| workspace `layout`       | `LayoutType`                            | `dwindle` through T12 | `calculate_layout_full` dispatches the selected pure function.                                      | `layout/mod.rs::calculate_layout_full`; `tests::test_calculate_layout_routes_correctly`                                                |

## Inputs

- Producer: `layout/{dwindle,split,grid,monocle,master,floating,gaps,helpers}.rs` and `layout/mod.rs`.
- Actor input: Layout-changing messages cause recalculation through handlers..
- Handler: `app/native/src/modules/tiling/actor/mod.rs::StateActor::handle_message`; query dispatch uses `StateActor::execute_query`.
- Public transport, if any: No direct command; layout choice is public through workspace commands/events..
- The actor rejects or drops inputs that fail its existing identity/existence checks; it does not invent an acknowledgement protocol.

## State Transitions

| From                | Input                | To                         | Observable result                                                                |
| ------------------- | -------------------- | -------------------------- | -------------------------------------------------------------------------------- |
| Current actor state | Valid owned message  | Handler-defined next state | Handler may request a later effect or event.                                     |
| Current actor state | Missing/stale target | Current state              | Message is rejected/dropped where the handler performs that check.               |
| Current actor state | Owned query          | Current state              | A `QueryResult` snapshot or projection is returned through the response channel. |
| Running             | `Shutdown`           | Actor loop exited          | No additional actor messages are processed.                                      |

## Outputs

No direct command; layout choice is public through workspace commands/events.

Only events explicitly emitted by `app/native/src/modules/tiling/init.rs` are delivery promises. Declaration in `app/native/src/events.rs` alone does not establish emission.

## Derived Effects

Algorithms return frames; `effects` owns OS application.

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

1. **Normal.** Given a running actor and a valid Layout-changing messages cause recalculation through handlers. producer input, when the handler accepts it, then only the named state/effect boundary is used.
2. **Boundary.** Given a bare `u32` public window ID, when an exact identity is required, then the operation does not claim that the number alone is a `WindowTarget`.
3. **Failure.** Given a missing, stale, or adapter-rejected target, when processing reaches the relevant handler/effect, then no unimplemented retry, rollback, or partial-result contract is inferred.
4. **Lifecycle.** Given actor shutdown, when `StateMessage::Shutdown` is processed, then the actor loop exits and later sends use the closed-channel error path.

## Testing Seam

The stable seam is the actor message/query boundary in `app/native/src/modules/tiling/actor/messages.rs`, with focused handler/module tests adjacent to `layout/{dwindle,split,grid,monocle,master,floating,gaps,helpers}.rs` and `layout/mod.rs`. Existing evidence is **None — source-only evidence** unless a focused test is named in the Evidence Base; a rewrite should test observable handler/effect behavior rather than source text.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface               | Implementation evidence                                                                                    | Test evidence                                                                                                 | Intended documentation                                            | Disposition  |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------ |
| Actor contract                 | `layout/{dwindle,split,grid,monocle,master,floating,gaps,helpers}.rs` and `layout/mod.rs`                  | None — source-only evidence                                                                                   | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Architecture` | Current-only |
| Public boundary                | No direct command; layout choice is public through workspace commands/events.                              | None — source-only evidence                                                                                   | `/Users/marcosmoura/Documents/stache-docs/events-and-ipc.md`      | Current-only |
| Eight layout dispatch/formulas | `app/native/src/modules/tiling/layout/mod.rs::calculate_layout_full` and the eight listed layout functions | `layout/mod.rs::tests::test_calculate_layout_routes_correctly`; focused function tests named in Data Contract | `/Users/marcosmoura/Documents/stache-docs/tiling.md#Layouts`      | Aligned      |
