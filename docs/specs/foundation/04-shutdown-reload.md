# Foundation 04 — Shutdown and Reload

> Status: ✅ Normative

## Purpose

Arbitrate terminal exit/restart requests and perform restoration, tiling shutdown, and IPC stop at most once.

## Scope

- Atomic terminal-request arbitration, signal escalation, cleanup order, natural-exit precedence, and reload IPC emission.

### Out of Scope

| Excluded concern          | Owner                                                    | Boundary note                                  |
| ------------------------- | -------------------------------------------------------- | ---------------------------------------------- |
| Config parsing/watching   | [configuration contract](01-configuration-contract.md)   | It supplies the change cause.                  |
| Socket wire protocol      | [CLI control surface](05-cli-control-surface.md)         | This spec owns the terminal Reload effect.     |
| Feature cleanup internals | [feature specifications](../index.md#specs-capabilities) | Coordinator calls their exposed cleanup paths. |

## Terminology

- **Pending action**: an exit or restart claimed but not committed on the main thread.
- **Natural exit**: Tauri `RunEvent::Exit`, which suppresses pending actions.

## Data Contract

`TERMINAL_REQUEST: AtomicU8` has `NONE`, `RESTART_PENDING`, `EXIT_PENDING`, `RESTART_COMMITTED`, `EXIT_COMMITTED`, and `NATURAL_EXIT`. Restart claims only `NONE`; exit claims `NONE` or overrides `RESTART_PENDING`. Commit accepts only the matching pending state. `CLEANUP_STARTED: AtomicBool` admits one cleanup. `SIGNAL_COUNT` counts all termination signals.

## Configuration Contract

No owned configuration keys.

## Inputs

`exit`, `restart`, Tauri `RunEvent::Exit`, SIGINT/SIGTERM/SIGHUP through `ctrlc`, and `IpcCommand::Reload` handled by `modules/bar/ipc_listener.rs`.

## State Transitions

| From                 | Input              | To                 | Effect                                       |
| -------------------- | ------------------ | ------------------ | -------------------------------------------- |
| NONE                 | restart            | RESTART_PENDING    | Queue main-thread restart closure.           |
| NONE/RESTART_PENDING | exit               | EXIT_PENDING       | Exit wins over an uncommitted restart.       |
| Pending              | main-thread commit | matching COMMITTED | Run cleanup then terminal call.              |
| Pending              | dispatch failure   | NONE               | Release only matching claim.                 |
| NONE/Pending         | natural exit       | NATURAL_EXIT       | Suppress uncommitted terminal work.          |
| first signal         | termination signal | exit requested     | Queue orderly exit.                          |
| second signal        | termination signal | force exit         | Escalate because main thread may be stalled. |

## Outputs

Cleanup logs, terminal `app.exit`/restart behavior, and `stache://app/reload` with unit payload when the IPC listener accepts Reload. `app/ui/renderer/Renderer.state.ts` subscribes and invokes `window.location.reload()`; Tauri emission does not establish replay or delivery to a non-listening window.

## Derived Effects

`cleanup_once` restores hidden applications, restores Caps Lock remapping, shuts down tiling, and stops IPC in that order. `lib.rs::run` establishes natural-exit precedence before calling it.

## Failure & Recovery

CAS mismatch is a safe no-op; failed main-thread dispatch releases only its own pending claim. Repeated cleanup calls do nothing after the first. Signal installation returns `ctrlc::Error`; `run` currently expects successful installation. The CLI considers a parsed Reload response—including `InvalidResponse`—an acknowledgement; other IPC errors become command failure.

## Cross-Module Contracts

[Keybinding dispatch](08-keybinding-dispatch.md) supplies `hotkey::shutdown`; tiling supplies shutdown; [CLI control surface](05-cli-control-surface.md) transports reload; configuration watcher may initiate reload behavior but cannot bypass this arbiter.

## Acceptance Scenarios

1. Given a pending restart then exit, when exit claims, then restart cannot commit.
2. Given natural exit while restart is pending, when its queued closure runs, then it is suppressed.
3. Given two cleanup callers, when both run, then restore→Caps Lock→tiling→IPC executes once.
4. Given one termination signal, when delivered, then orderly exit is requested.
5. Given a second termination signal before completion, then force escalation is selected.
6. Given Reload IPC, when handled and the renderer is listening, then a unit reload event makes `Renderer.state.ts` call `window.location.reload()`; replay/delivery outside that condition is not guaranteed.

## Testing Seam

`app_shutdown.rs` atomic helpers and `run_cleanup_once` have focused tests: `cleanup_restores_before_stopping_tiling_and_ipc`, `cleanup_runs_only_once`, `second_termination_signal_forces_exit`, `exit_override_wins_before_restart_commit`, and `natural_exit_suppresses_pending_actions`; `ipc_listener` is the reload handler seam.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                    | Implementation evidence                                                                                                                                                      | Test evidence                                                                                                                                                                                                             | Intended documentation               | Disposition |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ----------- |
| Atomic arbitration, signal escalation, cleanup-once | `app/native/src/app_shutdown.rs::{try_claim_exit_on,try_claim_restart_on,try_commit_on,establish_exit_precedence,run_cleanup_once,install_signal_handler}`; `lib.rs:247-257` | `app_shutdown.rs::tests::{cleanup_restores_before_stopping_tiling_and_ipc,cleanup_runs_only_once,second_termination_signal_forces_exit,exit_override_wins_before_restart_commit,natural_exit_suppresses_pending_actions}` | `tray-and-lifecycle.md#Shutdown`     | Aligned     |
| Reload event and CLI acknowledgement                | `modules/bar/ipc_listener.rs::handle_command`; `events.rs::app::RELOAD`; `app/ui/renderer/Renderer.state.ts::onAppReload`; `cli/commands/mod.rs::Cli::execute`               | `cli/commands/mod.rs::tests::test_cli_parses_reload`                                                                                                                                                                      | `events-and-ipc.md#Events`; `cli.md` | Aligned     |
