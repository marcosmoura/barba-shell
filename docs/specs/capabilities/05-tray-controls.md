# Capabilities 05 — Tray Controls

> Status: 🟡 Draft · Open: TR1, TR2, C05-D1

## Purpose

Provide the macOS tray icon, process actions, and a status-projected module pause/resume menu.

## Scope

- Tray construction, base commands, module check items, toggle execution, and projection of module status.

### Out of Scope

| Excluded concern                  | Owner                                                 | Boundary note                                     |
| --------------------------------- | ----------------------------------------------------- | ------------------------------------------------- |
| Module-specific lifecycle effects | [Foundation 02](../foundation/02-module-lifecycle.md) | Individual modules implement pause/resume/status. |
| Wallpaper timer lifecycle         | [Wallpaper 01](../wallpaper/01-cycling.md)            | A tray item consumes its status.                  |
| Audio listener lifecycle          | [Audio 03](../audio/03-topology-reaction.md)          | A tray item consumes its status.                  |

## Terminology

- **Projected status** — the immediate `ModuleStatus` read rendered as checked/enabled/text.
- **Base menu** — Reload (release only), Modules placeholder, and Quit.

## Data Contract

`TrayMenuState` retains the icon, Modules submenu, and a mutex-protected map of module ID to `CheckMenuItem`. `Running` maps to checked/enabled; `Paused` unchecked/enabled; `ConfiguredOff` unchecked/disabled; `Unavailable(reason)` unchecked/disabled with `name (reason)`.

## Configuration Contract

No tray-specific config. Module configuration gates are read through each lifecycle module's `status`.

## Inputs

Tray init builds base menu immediately. `install_modules_submenu` later asks the stored `LifecycleRegistry` for modules and installs items once. Release reload calls app restart; Quit calls app exit. A module menu ID starts toggle handling.

## State Transitions

| From                | Input                                     | To                     | Result                                             |
| ------------------- | ----------------------------------------- | ---------------------- | -------------------------------------------------- |
| no tray             | init                                      | base menu installed    | Builder panics on creation/icon/build failures.    |
| base menu           | background module startup attempts return | module items installed | Empty/previously installed item set is a no-op.    |
| enabled module item | click                                     | temporarily disabled   | Off-thread pause/resume executes.                  |
| successful toggle   | main-thread refresh                       | projected status       | Item is set from returned status.                  |
| failed toggle       | main-thread return                        | enabled item           | Warning is logged; old checked/text state remains. |

## Outputs

The tray owns native menu state only; no public event is emitted. Module status is resource-derived or inferred by the individual lifecycle implementation, not independently verified by tray.

## Derived Effects

Toggle chooses pause for `Running`, resume for `Paused`, and rejects `ConfiguredOff`/`Unavailable`. It dispatches the module operation to a named thread and menu item changes back to the main thread.

## Failure & Recovery

Base menu/item/tray construction uses `expect` and can panic. Individual item creation or submenu append errors are logged; setup does not expose a retryable degraded construction state. Toggle failure logs and re-enables the preexisting item without rereading actual module status.

## Cross-Module Contracts

Module items are obtained through the current `LifecycleRegistry`, but background modules are initialized by direct module paths before submenu installation. [Foundation 02 owns the primary L3 decision](../foundation/02-module-lifecycle.md#open-decisions) about a uniform registry tray/control route; this spec owns the separate submenu-timing decision C05-D1.

## Acceptance Scenarios

1. **Normal item.** Given a running module, when its item is installed, then it is checked and enabled.
2. **Boundary disabled config.** Given `ConfiguredOff`, when installed/clicked, then it is unchecked/disabled and no pause/resume runs.
3. **Failure.** Given a module toggle error, when returned to main thread, then the item re-enables and a warning is logged without status refresh.
4. **Lifecycle.** Given modules installed once, when installation is requested again, then no duplicate check items are appended.

## Testing Seam

`item_state` is a pure mapping with unit tests. Native tray construction and app menu events require Tauri/macOS integration evidence.

## Open Decisions

| ID     | Current behavior                                                                                                                                           | Documented intent                                                                                                   | Rewrite consequence                                                                                                             | Evidence                                                                                                |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| TR1    | Toggle failure merely logs and re-enables the existing item; it does not refresh from actual lifecycle status.                                             | The Modules Submenu documentation says an item is refreshed with the new status after the operation.                | Do not claim failure-refresh behavior.                                                                                          | `tray/mod.rs:78-121`; `tray-and-lifecycle.md#The Modules Submenu`.                                      |
| TR2    | Base tray construction uses `expect`; item creation/append errors only log, and neither yields a retryable tray state.                                     | The tray documentation makes the Modules submenu always available as part of the tray menu.                         | Decide whether construction/append failure needs a fallible, retryable contract; do not claim one exists.                       | `tray/mod.rs:44-76,124-182`; `tray-and-lifecycle.md#Tray Menu`.                                         |
| C05-D1 | The submenu is installed after direct background initializer calls return; several initializers can start work without a module-readiness acknowledgement. | Documentation says installation follows all background modules having started so initial check states are accurate. | Specify the current initializer-return trigger, or add a readiness condition; do not equate it with confirmed module readiness. | `tray/mod.rs:184-189`; `lib.rs::lazy_load_modules:87-147`; `tray-and-lifecycle.md#The Modules Submenu`. |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                | Implementation evidence                            | Test evidence               | Intended documentation                                    | Disposition  |
| --------------------------------------------------------------- | -------------------------------------------------- | --------------------------- | --------------------------------------------------------- | ------------ |
| Base menu and item status projection                            | `tray/mod.rs:23-76,124-189`                        | `tray/mod.rs::tests`        | `tray-and-lifecycle.md#Tray Menu`, `#The Modules Submenu` | Aligned      |
| Toggle failure without a status refresh                         | `tray/mod.rs:78-121`                               | None — source-only evidence | `tray-and-lifecycle.md#The Modules Submenu`               | Conflict     |
| Construction panics and item/append error logging               | `tray/mod.rs:44-76,124-182`                        | None — source-only evidence | None — source-only evidence                               | Current-only |
| Submenu timing after initializer returns rather than completion | `tray/mod.rs:184-189`; `lib.rs::lazy_load_modules` | None — source-only evidence | `tray-and-lifecycle.md#The Modules Submenu`               | Conflict     |
