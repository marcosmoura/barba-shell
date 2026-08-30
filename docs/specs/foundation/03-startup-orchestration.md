# Foundation 03 — Startup Orchestration

> Status: 🟡 Draft · Open: S1, S2

## Purpose

Define the actual desktop initialization order and its deliberately detached background boundary.

## Scope

- Desktop bootstrap, base-module ordering, background work, tiling sequencing, and startup command dispatch.

### Out of Scope

| Excluded concern        | Owner                                                  | Boundary note                                   |
| ----------------------- | ------------------------------------------------------ | ----------------------------------------------- |
| Configuration selection | [configuration contract](01-configuration-contract.md) | Startup consumes its snapshot.                  |
| Registry operations     | [module lifecycle](02-module-lifecycle.md)             | Registry registration is not a startup barrier. |
| Cleanup and restart     | [shutdown and reload](04-shutdown-reload.md)           | Startup does not arbitrate terminal actions.    |

## Terminology

- **Base modules**: watcher, IPC socket, tray, bar, and widgets.
- **Background initialization**: the async task spawned by `lazy_load_modules`.

## Data Contract

The setup/background boundary exposes no settled public Ready/Degraded contract; S2 decides whether a common startup snapshot is part of the rewrite surface.

## Configuration Contract

Consumes `exec_on_startup`, `tiling`, `command_quit`, and feature configuration from the immutable snapshot; it never reparses configuration.

## Inputs

Desktop launch, cached accessibility result, configuration snapshot, Tauri setup, and module initialization results/panics.

## State Transitions

1. `run` initializes logging, config, cached Accessibility check, and `wallpaper::setup`.
2. It builds Tauri plugins/handlers, then setup registers hotkeys, sets prohibited activation policy, registers lifecycle objects, and runs `load_base_modules`.
3. `load_base_modules` synchronously calls watcher → socket init → tray init → bar init → widgets init.
4. Setup calls `lazy_load_modules` and returns.
5. The spawned task delegates nonempty `execOnStartup` to [keybinding dispatch](08-keybinding-dispatch.md); S1 decides any startup-completion barrier.
6. The task coordinates five background module attempts before tiling sequencing; S2 decides common failure/status aggregation.
7. Only after that join does enabled tiling initialize through `dispatch_on_main_sync`; then the Modules submenu is installed.

## Outputs

Tauri setup completion, logs, feature-owned effects/events, and eventual Modules submenu installation. S2 decides any aggregate startup event, barrier acknowledgement, or common lifecycle snapshot.

## Derived Effects

Starts watcher/socket/tray/windows, global hotkeys, background blocking work, and main-thread tiling work. Startup-command execution semantics are delegated to [keybinding dispatch](08-keybinding-dispatch.md); S1 decides whether it is a startup barrier.

## Failure & Recovery

Background task failures and recovery remain feature-owned. S2 decides any coordinator-level retry or common degraded-status policy. Tiling starts only when `tiling_config.is_enabled()`.

## Cross-Module Contracts

[Configuration](01-configuration-contract.md) supplies the snapshot; [keybinding dispatch](08-keybinding-dispatch.md) owns detached command execution; [module lifecycle](02-module-lifecycle.md) owns registration facts and the primary [L2 decision](02-module-lifecycle.md#open-decisions); tiling owns its own runtime and event semantics.

## Acceptance Scenarios

1. Given desktop launch, when startup begins, then logging/config/accessibility run before Tauri setup.
2. Given setup, when base loading runs, then watcher, socket, tray, bar, and widgets are called in source order.
3. Given slow background initialization, when setup returns, then it has not waited for the join.
4. Given `execOnStartup`, when a rewrite requires ordering against module readiness, then S1 must be resolved before a completion barrier is specified.
5. Given all five background attempts reach the tiling sequencing point, when tiling is enabled, then tiling starts on the main-thread dispatch seam.
6. Given a background task failure, when a rewrite needs common lifecycle status, then S2 must be resolved before the publication is specified.
7. Given tiling is disabled, when background work completes, then only submenu installation follows.

## Testing Seam

`lib.rs::{load_base_modules,lazy_load_modules}` is the source seam; no automated orchestration test is cited.

## Open Decisions

| ID  | Current behavior                                                                   | Documented intent                                       | Rewrite consequence                                                 | Evidence                                                                                                                           |
| --- | ---------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| S1  | `execute_shortcut_commands` spawns a thread and returns before command completion. | Startup documentation implies ordered startup work.     | Do not promise a command-completion barrier without implementation. | `app/native/src/lib.rs:79-85`; `modules/hotkey/mod.rs:233-257`; `/Users/marcosmoura/Documents/stache-docs/architecture.md#Startup` |
| S2  | Panics are logged; no common `Unavailable`/`Degraded` result is published.         | Lifecycle documentation describes visible module state. | Specify feature-local failure facts or implement aggregation.       | `app/native/src/lib.rs:117-149`; `architecture.md#Startup`; `tray-and-lifecycle.md`                                                |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                      | Implementation evidence                                                                        | Test evidence               | Intended documentation    | Disposition |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------- | ------------------------- | ----------- |
| Base order                                            | `app/native/src/lib.rs::load_base_modules`                                                     | None — source-only evidence | `architecture.md#Startup` | Aligned     |
| Background join, tiling-after-join, detached commands | `app/native/src/lib.rs::lazy_load_modules`; `modules/hotkey/mod.rs::execute_shortcut_commands` | None — source-only evidence | `architecture.md#Startup` | Conflict    |
| Registry setup                                        | `app/native/src/lib.rs::run:217-239`                                                           | None — source-only evidence | `architecture.md#Modules` | Conflict    |
