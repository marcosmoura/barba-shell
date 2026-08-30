# Capabilities 04 — Keep Awake

> Status: 🟡 Draft · Open: KA1, KA3

## Purpose

Hold a macOS wake assertion while desired and react to observed session lock transitions.

## Scope

- Wake assertion controller, two Tauri commands, lock watcher, event payload, and current frontend consumer defect.

### Out of Scope

| Excluded concern                | Owner                                                  | Boundary note                                   |
| ------------------------------- | ------------------------------------------------------ | ----------------------------------------------- |
| Keep Awake status-bar rendering | [Bar 01](../bar/01-bar-window-lifecycle.md)            | Consumes commands/query state.                  |
| General application lifecycle   | [Foundation 10](../foundation/10-application-shell.md) | Registers Tauri state.                          |
| Tray module controls            | [Tray Controls](05-tray-controls.md)                   | This controller is not a tray lifecycle module. |

## Terminology

- **Desired awake** — the controller's intent flag.
- **Actual awake** — currently represented by presence of the `KeepAwake` handle.

## Data Contract

Controller state is `{ desired_awake: bool, handle: Option<KeepAwake> }`. Lock-change payload is exactly `{ locked: bool, desired_awake: bool }`. It has no `actual` or `error` field.

## Configuration Contract

No user configuration key is consumed.

## Inputs

`invoke('toggle_system_awake')` and `invoke('is_system_awake')` take no frontend arguments because Tauri injects controller state. Both return `Result<bool, StacheError>`. Startup calls `enable_awake`; lock observation uses distributed notifications plus a two-second best-effort polling fallback.

## State Transitions

| From                    | Input  | To                                      | Result                                         |
| ----------------------- | ------ | --------------------------------------- | ---------------------------------------------- |
| desired false/no handle | toggle | desired true/handle                     | Acquires wake assertion and returns true.      |
| desired true/handle     | toggle | desired false/no handle                 | Drops handle and returns false.                |
| any                     | lock   | handle none                             | Emits locked payload retaining desired intent. |
| desired true            | unlock | handle restored if acquisition succeeds | Emits unlocked payload.                        |

Watcher initialization is process-lifetime: a `OnceLock` prevents another lock watcher after the first `init`.

## Outputs

`stache://keepawake/state-changed` is emitted through the app handle on lock-state changes with `{ locked, desired_awake }`. The frontend listener is incorrectly generic `boolean` and stores the object payload as query data. Toggle and query command results are booleans.

## Derived Effects

Wake acquisition asks for display, idle, and sleep prevention with Stache reason/identity. The watcher registers two distributed notifications and polls session state; a changed lock bit invokes handle release or attempted reacquisition.

## Failure & Recovery

Failed startup acquisition is logged. Failed lock polling is logged. An unlock reacquisition failure prevents payload emission because `handle_system_unlocked_event` returns error; current behavior supplies no observable acquisition-error snapshot or retry policy beyond later lock transitions/toggles.

## Cross-Module Contracts

The UI bar fetches `is_system_awake`, toggles via `toggle_system_awake`, and listens to the declared event. Its event generic must eventually match the Rust payload; it does not today.

## Acceptance Scenarios

1. **Normal toggle.** Given no wake handle, when toggle succeeds, then it returns true and retains a handle.
2. **Boundary lock.** Given desired awake, when session locks, then the handle is cleared and emitted payload keeps `desired_awake: true`.
3. **Failure.** Given wake assertion acquisition failure on unlock, when handled, then a warning is logged and no error payload is emitted.
4. **Lifecycle.** Given two `init` calls, when the second runs, then it does not start another lock watcher.

## Testing Seam

`KeepAwakeController` tests `test_keep_awake_controller_is_awake_initially_false`, `test_keep_awake_controller_handle_system_locked`, and `test_keep_awake_changed_payload_serialization` make observable state/payload behavior explicit. Notification registration is platform evidence; the TypeScript listener is a direct regression seam.

## Open Decisions

| ID  | Current behavior                                                                                                          | Documented intent                                                                                                    | Rewrite consequence                                                                                          | Evidence                                                                                                                          |
| --- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| KA1 | Rust emits an object with `locked` and `desired_awake`; `KeepAwake.state.ts` types/stores payload as a boolean.           | Documentation explicitly identifies the lock-state UI synchronization defect.                                        | Keep the object-as-boolean consumer defect explicit; do not invent a four-field/generated manifest contract. | `keepawake.rs:31-44,107-127,272-288`; `KeepAwake.state.ts:24-26`; `keep-awake.md#Missing Feature: Lock-State UI Synchronization`. |
| KA3 | Acquisition/reacquisition failures are logged and suppress an event; no observable error snapshot or retry policy exists. | The current intended documentation specifies lock reacquisition, but no observable acquisition-error/retry contract. | Do not promise observable acquisition errors or retry behavior.                                              | `keepawake.rs:57-86,154-169,272-288`; `keep-awake.md#Behavior`.                                                                   |

## Resolved Decisions

- **KA2 — Watcher lifetime.** Outcome: watcher initialization is process-lifetime through `LOCK_WATCHER_ONCE`; no generation-gated callback claim is retained. Basis: current code. Evidence: `keepawake.rs:152-169`.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                             | Implementation evidence                                                     | Test evidence                                                                                                                         | Intended documentation                                         | Disposition  |
| -------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ------------ |
| Boolean Tauri commands and command failures  | `keepawake.rs:130-150`                                                      | `keepawake.rs::tests::test_keep_awake_controller_is_awake_initially_false`, `test_keep_awake_controller_toggle_changes_desired_state` | `events-and-ipc.md#Tauri Commands`; `keep-awake.md#Behavior`   | Aligned      |
| Lock transitions and two-field event payload | `keepawake.rs:31-44,107-127,171-288`; `events.rs::keepawake::STATE_CHANGED` | `keepawake.rs::tests::test_keep_awake_changed_payload_serialization`, `test_keep_awake_controller_handle_system_locked`               | `events-and-ipc.md#Event Catalog`; `keep-awake.md#Behavior`    | Aligned      |
| Process-lifetime watcher once guard          | `keepawake.rs:152-169`                                                      | `keepawake.rs::tests::test_lock_watcher_once_initialization`                                                                          | None — source-only evidence                                    | Current-only |
| Object payload stored as boolean in frontend | `KeepAwake.state.ts:24-26`                                                  | None — source-only evidence                                                                                                           | `keep-awake.md#Missing Feature: Lock-State UI Synchronization` | Known defect |
| Observable acquisition errors/retry          | `keepawake.rs:57-86,272-288`                                                | None — source-only evidence                                                                                                           | `keep-awake.md#Behavior`                                       | Conflict     |
