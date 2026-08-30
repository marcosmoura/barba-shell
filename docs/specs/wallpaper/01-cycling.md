# Wallpaper 01 — Cycling

> Status: 🟡 Draft · Open: W01-D1

## Purpose

Rotate a configured wallpaper collection and permit manual wallpaper actions.

## Scope

- Wallpaper selection, timer lifecycle, and the wallpaper module's tray status.
- Configuration values consumed by the manager.

### Out of Scope

| Excluded concern                             | Owner                                                | Boundary note                       |
| -------------------------------------------- | ---------------------------------------------------- | ----------------------------------- |
| Collection discovery and path interpretation | [Wallpaper 02](02-discovery-collection.md)           | Supplies the manager's collection.  |
| Image processing and cache files             | [Wallpaper 03](03-processing-cache.md)               | Produces the image passed to macOS. |
| Per-screen macOS application                 | [Wallpaper 04](04-application-adapter.md)            | Owns the OS setter.                 |
| Tray presentation                            | [Tray Controls](../capabilities/05-tray-controls.md) | Consumes lifecycle status.          |

## Terminology

- **Manual action** — a `Random`, `RandomForScreen`, `File`, or `FileForScreen` request.
- **Generation** — the timer counter captured by a worker when it starts.

## Data Contract

The manager owns an immutable nonempty `Vec<PathBuf>`, a cloned `WallpaperConfig`, atomic sequential index, atomic running flag, atomic generation, and a mutex serializing changes. Sequential selection advances `(current + 1) % length`; random selection samples the collection.

## Configuration Contract

`wallpapers.enabled` defaults to `false`; `path` to `""`; `list` to `[]`; `interval` to `0`; `mode` to `random`; and `blur` and `radius` to `0`. `interval` is seconds and zero prevents timer creation. `mode` is `random` or `sequential`.

## Inputs

`Random` chooses one random item for every current screen. `RandomForScreen(usize)` rejects an index outside `0..screen_count`. `File(String)` resolves a case-insensitive filename or stem and applies it through the all-screen path; `FileForScreen` resolves it and applies it to one screen. Unknown names produce `FileNotFound`.

## State Transitions

| From                  | Input                                              | To                          | Result                                             |
| --------------------- | -------------------------------------------------- | --------------------------- | -------------------------------------------------- |
| stopped               | `start_timer`, interval `0`                        | stopped                     | No worker is created.                              |
| stopped               | `start_timer`, interval `> 0`                      | running                     | One worker captures the current generation.        |
| running               | `start_timer`                                      | running                     | The atomic claim prevents a second worker.         |
| running or stopped    | `stop_timer`                                       | stopped                     | Clears running and increments generation.          |
| running or stopped    | `reset_timer`                                      | running when interval `> 0` | Stops then starts.                                 |
| any initialized state | successful global `perform_action`, interval `> 0` | running                     | Calls `reset_timer`, including after a tray pause. |

A worker exits after waking when its generation no longer matches or `timer_running` is false. Timed setter failures are logged and do not stop later ticks.

## Outputs

Publishes no wallpaper event today.

## Derived Effects

Each selected source is processed by [Wallpaper 03](03-processing-cache.md) then supplied to [Wallpaper 04](04-application-adapter.md). The global `setup` creates no manager when disabled or without configured wallpapers; creation failure is logged. `init` sets the initial wallpaper only outside debug builds and then starts the timer.

## Failure & Recovery

Construction returns `NoWallpapers` or `InvalidPath`; actions can return `NotInitialized`, `FileNotFound`, `InvalidScreen`, processing, or macOS errors. A failed manual action does not reach the timer reset. There is no retry policy beyond later ticks or later caller actions.

## Cross-Module Contracts

`WallpaperLifecycle.status` returns `ConfiguredOff` when config is off; `Unavailable("no wallpapers configured or load failed")` when no manager exists; `Running` for interval zero or a running timer; otherwise `Paused`. `pause` calls `stop_timer`; `resume` calls `start_timer`.

## Acceptance Scenarios

1. **Normal rotation.** Given a sequential collection, when a tick succeeds, then the next wrapped index is processed and applied.
2. **Boundary interval.** Given interval zero, when started or resumed, then no timer worker is created.
3. **Failure.** Given a processing failure on a tick, when it occurs, then it is logged and the worker continues.
4. **Lifecycle/manual conflict.** Given a tray-paused manager and interval greater than zero, when a manual global action succeeds, then the global wrapper resets and starts the timer.

## Testing Seam

`WallpaperManager` isolates selection, generation, and errors from macOS; `manager.rs` unit tests cover timer idempotence and lifecycle status. Platform application remains a separate seam.

## Open Decisions

| ID     | Current behavior                                          | Documented intent                                          | Rewrite consequence                                                                                  | Evidence                                        |
| ------ | --------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| W01-D1 | Per-screen random draws are independent and may coincide. | Documentation calls the image for each screen “different”. | Clarify whether uniqueness is required; current implementation guarantees only independent sampling. | `manager.rs:215-229`; `wallpapers.md#Rotation`. |

## Resolved Decisions

- **W1 — Manual action timer reset.** Outcome: A successful manual action passed to `perform_action` with `interval > 0` calls `reset_timer`, restarting a tray-paused timer. Basis: current code and the rotation documentation both say a CLI-set wallpaper restarts the timer. Evidence: `manager.rs:423-433`; `wallpapers.md#Rotation`.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                        | Implementation evidence                                            | Test evidence                                                           | Intended documentation        | Disposition |
| ------------------------------------------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------------- | ----------------------------- | ----------- |
| Defaults, modes, and interval gate                      | `config/types/wallpaper.rs::WallpaperConfig`; `manager.rs:304-355` | `manager.rs::tests` timer tests                                         | `wallpapers.md#Configuration` | Aligned     |
| Generation-guarded timer and tick warning               | `manager.rs:304-349`                                               | `manager.rs::tests::start_timer_is_idempotent_and_preserves_generation` | `wallpapers.md#Rotation`      | Aligned     |
| Manual action restarts a paused timer                   | `manager.rs:423-433`                                               | None — source-only evidence                                             | `wallpapers.md#Rotation`      | Aligned     |
| Independent rather than unique per-screen random values | `manager.rs:215-229`                                               | None — source-only evidence                                             | `wallpapers.md#Rotation`      | Conflict    |
