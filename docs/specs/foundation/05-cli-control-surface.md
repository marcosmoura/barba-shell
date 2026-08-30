# Foundation 05 — CLI Control Surface

> Status: ✅ Normative

## Purpose

Expose the current `stache` command grammar and its bounded JSON-lines Unix-socket protocol for tiling query/control operations.

## Scope

- CLI dispatch, human/JSON output boundaries, IPC request/response framing, limits, and control acknowledgement.

### Out of Scope

| Excluded concern       | Owner                                                  | Boundary note                                                      |
| ---------------------- | ------------------------------------------------------ | ------------------------------------------------------------------ |
| Config file meaning    | [configuration contract](01-configuration-contract.md) | Only the global path flag and config commands cross this boundary. |
| Reload arbitration     | [shutdown and reload](04-shutdown-reload.md)           | Reload transport is here; terminal effect is there.                |
| Tiling state semantics | [tiling specifications](../index.md#specs-tiling)      | This spec owns framing and CLI flags, not query values.            |

## Terminology

- **JSON line**: one serialized `IpcMessage` followed by `\n`.
- **Acknowledgement**: any parsed IPC response for Reload, including client `InvalidResponse` handling.

## Data Contract

`IpcMessage` is untagged `Query(IpcQuery)` or `Command(IpcCommand)`; every frame is one JSON line and every response is exactly `{"data": <JSON>}` or `{"error": <string>}`. `lib.rs::load_base_modules` routes queries to `tiling::init::handle_ipc_query` and controls to `modules::bar::ipc_listener::handle_command`.

| Kind    | `type` tag and JSON payload                                                                                              | Defaults                            | CLI producer                                                  | Handler boundary                               |
| ------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------- | ------------------------------------------------------------- | ---------------------------------------------- |
| query   | `screens`                                                                                                                | none                                | `tiling query screens`                                        | `handle_ipc_query` → `handle_screens_query`    |
| query   | `workspaces`, `screen?: string`, `focusedScreen?: bool`                                                                  | omitted screen; false               | `tiling query workspaces [--screen NAME \| --focused-screen]` | `handle_ipc_query` → `handle_workspaces_query` |
| query   | `windows`, `screen?: string`, `workspace?: string`, `focusedScreen?: bool`, `focusedWorkspace?: bool`, `detailed?: bool` | omitted strings; all booleans false | `tiling query windows` filters / `--detailed`                 | `handle_ipc_query` → `handle_windows_query`    |
| query   | `apps`                                                                                                                   | none                                | `tiling query apps`                                           | `handle_ipc_query` → `handle_apps_query`       |
| query   | `ping`                                                                                                                   | none                                | Not a current CLI producer                                    | `handle_ipc_query` → `"pong"`                  |
| query   | `v2State`                                                                                                                | none                                | Not a current CLI producer                                    | `handle_ipc_query`                             |
| query   | `v2Screens`                                                                                                              | none                                | Not a current CLI producer                                    | `handle_ipc_query`                             |
| query   | `v2Workspaces`                                                                                                           | none                                | Not a current CLI producer                                    | `handle_ipc_query`                             |
| query   | `v2Windows`, `workspaceId?: string`                                                                                      | omitted workspaceId                 | Not a current CLI producer                                    | `handle_ipc_query`                             |
| query   | `v2Enabled`                                                                                                              | none                                | Not a current CLI producer                                    | `handle_ipc_query`                             |
| control | `reload`                                                                                                                 | none                                | `reload`                                                      | `handle_command` → reload notification/event   |
| control | `tilingFocusWorkspace`, `workspace: string`                                                                              | none                                | `tiling workspace --focus`                                    | `handle_command` → `switch_workspace`          |
| control | `tilingSetLayout`, `layout: string`                                                                                      | none                                | `tiling workspace --layout`                                   | `handle_command` → `set_layout`                |
| control | `tilingWindowFocus`, `target: string`                                                                                    | none                                | `tiling window --focus`                                       | `handle_command` → `focus_window`              |
| control | `tilingWindowSwap`, `direction: string`                                                                                  | none                                | `tiling window --swap`                                        | `handle_command` → `swap_window_in_direction`  |
| control | `tilingWindowResize`, `dimension: string`, `amount: i32`                                                                 | none                                | repeated `tiling window --resize DIMENSION AMOUNT`            | `handle_command` → `resize_focused_window`     |
| control | `tilingWindowPreset`, `preset: string`                                                                                   | none                                | `tiling window --preset`                                      | `handle_command` → `apply_preset`              |
| control | `tilingWindowSendToWorkspace`, `workspace: string`                                                                       | none                                | `tiling window --send-to-workspace`                           | `handle_command` → workspace lookup/send       |
| control | `tilingWindowSendToScreen`, `screen: string`                                                                             | none                                | `tiling window --send-to-screen`                              | `handle_command` → `send_window_to_screen`     |
| control | `tilingWorkspaceBalance`                                                                                                 | none                                | `tiling workspace --balance`                                  | `handle_command` → `balance_workspace`         |
| control | `tilingWorkspaceSendToScreen`, `screen: string`                                                                          | none                                | `tiling workspace --send-to-screen`                           | `handle_command` → `send_workspace_to_screen`  |

## Configuration Contract

Global `--config PATH`/`-c PATH` applies before subcommand execution and requires an existing file. Feature config keys remain feature-owned.

## Inputs

`wallpaper`, `cache`, `audio`, `tiling`, `config`, `reload`, `schema`, `completions --shell {bash,elvish,fish,powershell,zsh}`, and hidden `--desktop`. Tiling query accepts `--json/-j`, `--detailed/-d`, and its declared mutually exclusive screen/workspace filters. Window combined operations run focus → swap → preset → each resize → send; workspace combined operations run focus → layout → balance → send.

## State Transitions

| From        | Input                                 | To           | Effect                                                                                                                                             |
| ----------- | ------------------------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI process | no args/`--desktop`/bundle executable | Desktop      | `main.rs` calls `run`.                                                                                                                             |
| CLI process | other grammar-valid command           | Executing    | `cli::run` dispatches subcommand.                                                                                                                  |
| IPC client  | connection attempt                    | Sent/failed  | At most three total attempts: initial plus two `AppNotRunning` retries, with 100 ms waits; timeout, I/O, and invalid-response errors do not retry. |
| server      | complete JSON line                    | Responded    | Handler returns data/error JSON line.                                                                                                              |
| server      | over-64 KiB or invalid/timeout input  | Error/closed | Bounded handler rejects it.                                                                                                                        |

## Outputs

Schema/completions print stdout. Every `tiling query` helper returns `Ok(())` after rendering its result. For example, `stache tiling query screens --json` sends `{"type":"screens"}\n`: a `data` response prints highlighted JSON; an `error` response prints `{"error":"<server error>"}`; `AppNotRunning` prints `{"error":"Stache app is not running"}`; and every other caught `IpcError` prints `{"error":"<error>"}`. In human mode those corresponding paths render a table/empty message, `Error: <server error>`, `Stache app is not running.`, or `Error: <error>`. All those query outcomes exit 0. Command paths that return `Err` reach `main.rs`, which prints `stache: {error}` to stderr and exits 1. `config`, cache, audio, and wallpaper output is owned by their feature/config specs.

## Derived Effects

The desktop server binds `get_cache_dir()/stache.sock`, chmods it `0600`, bounds servicing at eight connections, applies five-second read/write deadlines, caps a line at 64 KiB, and removes stale socket path before bind. Client reads/writes newline-delimited JSON.

## Failure & Recovery

Query transport errors and server error responses are rendered by each helper and return `Ok(())`; they do not use the `stache:` stderr/exit-1 path. Invalid Clap grammar is a Clap parse failure. `reload` accepts any `Ok(IpcResponse)` or `Err(InvalidResponse(_))`; other IPC errors become `StacheError::IpcError`. Command paths that return `Err` are the paths reserved for main's stderr/exit-1 handling.

## Cross-Module Contracts

[Startup orchestration](03-startup-orchestration.md) installs the server. [Shutdown and reload](04-shutdown-reload.md) owns Reload handling. Tiling owns the handler meaning/results, while this surface maps CLI forms to `IpcQuery`/`IpcCommand`.

## Acceptance Scenarios

1. Given no argument, `--desktop`, or bundle execution, when main dispatches, then desktop mode runs.
2. Given `stache tiling query screens --json`, when it serializes, then its exact wire frame is `{"type":"screens"}\n`.
3. Given any query helper result—data, `IpcResponse::Error`, `AppNotRunning`, or another caught `IpcError`—when it renders, then it prints the corresponding JSON/human result and exits 0.
4. Given nine held connections, when the ninth arrives, then handling is queued rather than an unbounded worker being created.
5. Given a line larger than 64 KiB or a five-second stalled I/O, when served, then it is rejected/closed.
6. Given multiple window flags, when executing, then their source order is focus, swap, preset, resize(s), send.
7. Given Reload returns malformed response, when the client detects `InvalidResponse`, then reload still exits successfully.
8. Given another execution error, when main receives it, then stderr is prefixed `stache:` and exit code is 1.

## Testing Seam

`platform/ipc_socket.rs` protocol seams include `test_query_round_trip_over_socket`, `test_command_round_trip_over_socket`, `test_oversized_request_rejected`, and `test_concurrent_connections_bounded`; Clap parser tests live in `cli/commands/{mod,tiling}.rs`. No cited end-to-end test proves every output path.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                            | Implementation evidence                                                                                           | Test evidence                                                                                                                                                                                                                                                            | Intended documentation | Disposition |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------- | ----------- |
| Desktop/CLI dispatch and grammar            | `app/native/src/main.rs::{main,should_run_desktop}`; `cli/commands/mod.rs::{Cli,Commands,Cli::execute}`           | `cli/commands/mod.rs::tests::{test_cli_parses_config_flag,test_cli_parses_reload,test_cli_parses_completions_bash}`                                                                                                                                                      | `cli.md`               | Aligned     |
| Tiling flags/order/output                   | `cli/commands/tiling.rs::{TilingCommands,TilingQueryCommands,TilingWindowArgs,TilingWorkspaceArgs,execute_query}` | `cli/commands/tiling.rs::tests::test_cli_parses_tiling_query_screens`; `cli/commands/mod.rs::tests::test_cli_parses_tiling_window_focus`                                                                                                                                 | `cli.md#Tiling`        | Aligned     |
| Socket frames, limits, permissions, retries | `platform/ipc_socket.rs::{IpcMessage,IpcResponse,init,handle_connection,send_message_once}`                       | `platform/ipc_socket.rs::tests::{test_ipc_query_serialization,test_ipc_response_serialization,test_ipc_command_serialization,test_query_round_trip_over_socket,test_command_round_trip_over_socket,test_oversized_request_rejected,test_concurrent_connections_bounded}` | `cli.md#IPC`           | Aligned     |
