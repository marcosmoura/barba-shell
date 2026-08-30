# Foundation 07 — Cache Management

> Status: 🟡 Draft · Open: CA1, CA2

## Purpose

Expose cache paths and clearing behavior exactly as implemented, including the destructive relationship to the live IPC socket.

## Scope

- Root/subdirectory path helpers, byte accounting/formatting, and `cache clear`/`cache path` CLI behavior.

### Out of Scope

| Excluded concern               | Owner                                                 | Boundary note                                 |
| ------------------------------ | ----------------------------------------------------- | --------------------------------------------- |
| IPC server protocol            | [CLI control surface](05-cli-control-surface.md)      | Its socket happens to live beneath this root. |
| Wallpaper/media cache contents | [feature specifications](../index.md#specs-wallpaper) | This spec owns root deletion only.            |

## Terminology

- **Cache root**: `~/Library/Caches/{APP_BUNDLE_ID}` or `/tmp/{APP_BUNDLE_ID}` fallback.
- **Live socket**: `stache.sock` at the cache root while IPC server runs.

## Data Contract

`get_cache_dir` returns the cache root and `get_cache_subdir_str` returns a trailing-separator string. `clear_cache` reports freed bytes or an I/O error. CA1 decides clear interaction with live runtime endpoints; CA2 decides namespace/path-containment guarantees.

## Configuration Contract

No owned configuration keys.

## Inputs

`stache cache clear` and `stache cache path`; host cache directory availability and filesystem permissions.

## State Transitions

| From         | Input           | To                   | Effect                                                          |
| ------------ | --------------- | -------------------- | --------------------------------------------------------------- |
| root absent  | clear           | root absent          | Return zero; CLI reports nothing to clear.                      |
| root present | clear succeeds  | root cleared         | Clear result and live-endpoint interaction are governed by CA1. |
| root present | removal failure | root present/partial | Return cache error.                                             |

## Outputs

`cache path` prints the root and succeeds. `cache clear` prints `Cache directory does not exist. Nothing to clear.` for absence or a human-formatted byte total after success. No event is published.

## Derived Effects

Cache clearing is a root-level filesystem effect. CA1 decides whether live runtime endpoints must be preserved, refused, or coordinated.

## Failure & Recovery

I/O/permission errors propagate from sizing/removal and the CLI returns `CacheError` (main prints error and exits nonzero). CA2 decides path-containment validation; CA1 decides a refusal/preservation rule for a live runtime endpoint.

## Cross-Module Contracts

[CLI control surface](05-cli-control-surface.md) owns process exit/transport. Wallpaper and media own cache consumers. IPC owns server lifecycle but shares this root.

## Acceptance Scenarios

1. Given a usable macOS cache directory, when root is requested, then it is under `~/Library/Caches/com.marcosmoura.stache`.
2. Given unavailable cache resolution, when root is requested, then `/tmp/com.marcosmoura.stache` is used.
3. Given an absent root, when clear runs, then zero is returned and CLI prints the nothing-to-clear text.
4. Given a clear request while a live runtime endpoint might share the root, when a rewrite specifies clearing, then CA1 must be resolved before endpoint behavior is asserted.
5. Given a filesystem failure, when clear runs, then it returns an error rather than claiming success.
6. Given a namespace/path-containment requirement, when a rewrite specifies it, then CA2 must be resolved before the guarantee is asserted.

## Testing Seam

`cache.rs` pure path/size/formatting helpers and `cli/commands/cache.rs` parser tests are existing seams; CA1/CA2 behavior is decision-only and has no cited acceptance test.

## Open Decisions

| ID  | Current behavior                                                                      | Documented intent                                                                  | Rewrite consequence                                                    | Evidence                                                                                                                                     |
| --- | ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| CA1 | `clear_cache` deletes the whole root, which includes a live `stache.sock`.            | Cache documentation describes cache cleanup without an IPC-socket safety contract. | Choose refusal, server coordination, or explicit destructive behavior. | `app/native/src/cache.rs::clear_cache`; `platform/ipc_socket.rs::get_socket_path`; `/Users/marcosmoura/Documents/stache-docs/cache.md#Clear` |
| CA2 | `get_cache_subdir` directly joins caller input; no containment rejection is verified. | Cache-layout documentation describes component namespaces.                         | Do not claim path containment until it is implemented.                 | `app/native/src/cache.rs::get_cache_subdir`; `cache.md#Cache Layout`                                                                         |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface              | Implementation evidence                                                                               | Test evidence                                                                                     | Intended documentation  | Disposition |
| ----------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------- | ----------- |
| Root and CLI output           | `app/native/src/cache.rs::{get_cache_dir,clear_cache,format_bytes}`; `cli/commands/cache.rs::execute` | `cache.rs::tests`; `cli/commands/cache.rs::tests::{test_cache_clear_parse,test_cache_path_parse}` | `cache.md#CLI`          | Aligned     |
| Root deletion includes socket | `cache.rs::clear_cache`; `platform/ipc_socket.rs::get_socket_path`                                    | None — source-only evidence                                                                       | `cache.md#Clear`        | Conflict    |
| Containment/safe namespaces   | `cache.rs::get_cache_subdir`                                                                          | None — source-only evidence                                                                       | `cache.md#Cache Layout` | Conflict    |
