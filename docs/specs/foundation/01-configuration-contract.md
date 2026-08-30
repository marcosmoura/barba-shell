# Foundation 01 — Configuration Contract

> Status: 🟡 Draft · Open: C3

## Purpose

Select one process-lifetime configuration snapshot, parse it as JSONC, and expose the file-management commands without deciding feature-section semantics.

## Scope

- Discovery, explicit-path precedence, parsing, defaults, template creation, snapshot publication, watching, and schema/config CLI output.

### Out of Scope

| Excluded concern                  | Owner                                              | Boundary note                                          |
| --------------------------------- | -------------------------------------------------- | ------------------------------------------------------ |
| Feature key meanings and defaults | [feature specifications](../index.md#specs-tiling) | This loader only deserializes the root value.          |
| Restart arbitration               | [shutdown and reload](04-shutdown-reload.md)       | The watcher requests the feature-specific reload path. |
| CLI transport and tiling grammar  | [CLI control surface](05-cli-control-surface.md)   | This spec owns only config/schema command behavior.    |

## Terminology

- **Custom path**: the one `--config`/`-c` path accepted before configuration initialization.
- **Snapshot**: the `OnceLock<StacheConfig>` value returned for the process lifetime.

## Data Contract

`config_paths()` orders `config.jsonc` before `config.json` at `$XDG_CONFIG_HOME/stache` (when set), `~/.config/stache`, `~/Library/Application Support/stache`, then legacy `~/.stache.jsonc`/`.stache.json`. `load_config()` returns the first existing parseable candidate or its first read/parse error. `load_config_from_path` strips `//` and `/* … */` comments before serde JSON parsing. `StacheConfig` is `#[serde(default)]`; omitted root fields use their type defaults. Config-field semantics remain with their feature owner.

`CUSTOM_CONFIG_PATH`, `CONFIG`, and `CONFIG_PATH` are `OnceLock`s. A custom path can be set once before `init()`/`get_config()`; later calls cannot replace the published snapshot or path.

## Configuration Contract

`--config PATH`/`-c PATH` must name an existing path in both CLI and desktop entry paths; it bypasses discovery. `config init` writes the generated template to an explicit `--path`, otherwise the first discovery path; `--stdout` prints instead and takes precedence; an existing destination needs `--force`. `config path` prints the search paths and their active/existing markers. `schema` prints generated JSON Schema to stdout.

## Inputs

Filesystem paths and bytes, `XDG_CONFIG_HOME`, home-directory resolution, custom-path CLI arguments, and file-system notifications for the loaded file.

## State Transitions

| From             | Input                           | To                                           | Effect                                                                                                                       |
| ---------------- | ------------------------------- | -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Uninitialized    | Existing custom path            | Loaded                                       | Parse only that path.                                                                                                        |
| Uninitialized    | Discovered path                 | Loaded                                       | Parse the first existing candidate.                                                                                          |
| Uninitialized    | No path                         | Defaulted                                    | Best-effort template creation and `StacheConfig::default()`.                                                                 |
| Loaded/Defaulted | `init` or `get_config`          | Same                                         | Return the same snapshot.                                                                                                    |
| Loaded           | matching parent-directory event | Debug: unchanged; release: restart requested | Debug records the 200 ms debounce timestamp and logs; release calls `app_shutdown::restart` without updating that timestamp. |

## Outputs

The immutable snapshot and optional active path; template/schema/path command text; no frontend event.

## Derived Effects

`load_or_default` creates the parent/template on a not-found result and sets `CONFIG_PATH` only after a successful load or template write. `watch_config_file` starts from `lib.rs::load_base_modules`, watches the active path's parent non-recursively, filters by filename, and logs watcher errors. In debug it debounces for 200 ms and logs; in release it requests restart. The generated schema and template derive from root config types (`schema.rs`, `config/template.rs`).

## Failure & Recovery

A missing explicit path returns `ConfigError` before command execution. A missing discovered config is first-run behavior and defaults still publish if template writing fails. `ConfigError` distinguishes not-found, I/O, and parse errors. C3 exclusively decides the desktop outcome for a non-not-found load error. No reload updates a live `OnceLock` snapshot.

## Cross-Module Contracts

[Startup orchestration](03-startup-orchestration.md) calls `config::init` before feature initialization. [Keybinding dispatch](08-keybinding-dispatch.md) and feature owners consume the snapshot. [Shutdown and reload](04-shutdown-reload.md) owns terminal action after a reload request.

## Acceptance Scenarios

1. Given JSONC and JSON in one location, when discovery runs, then JSONC wins.
2. Given an existing `--config`, when either entry path starts, then it is the only load target.
3. Given a missing `--config`, when starting, then the process reports that path and does not fall back.
4. Given no discovery candidate, when loading, then defaults publish and template writing is attempted.
5. Given comments and omitted fields, when parsing, then comments are stripped and root defaults apply.
6. Given a malformed or unreadable discovered configuration, when desktop loading runs, then C3 is the required decision boundary before a rewrite asserts a user-facing outcome.
7. Given repeated snapshot access or a later custom-path attempt, then the first snapshot/path remains authoritative.
8. Given matching parent-directory events in debug, when they arrive within 200 ms, then only the first logs; in release each matching event requests restart until terminal arbitration suppresses duplicates.

## Testing Seam

`config/types/root.rs::{config_paths,load_config_from_path}` and `config/mod.rs` tests are stable parsing/error seams; `config/watcher.rs::tests::{config_debounce_duration_is_reasonable,config_debounce_duration_creates_valid_duration}` cover only debounce constants, not filesystem events.

## Open Decisions

| ID  | Current behavior                                                      | Documented intent                                          | Rewrite consequence                                                                      | Evidence                                                                                                                  |
| --- | --------------------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| C3  | Desktop `load_or_default` logs I/O/parse errors and returns defaults. | Configuration documentation describes strict format rules. | Choose fail-fast startup or explicitly retain fallback; do not describe both as settled. | `app/native/src/config/mod.rs::load_or_default`; `/Users/marcosmoura/Documents/stache-docs/configuration.md#Format Rules` |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface               | Implementation evidence                                                                                                           | Test evidence                                                                                                                  | Intended documentation                           | Disposition |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------ | ----------- |
| Discovery and JSONC parsing    | `config/types/root.rs::{config_paths,load_config,load_config_from_path}`                                                          | None — source-only evidence                                                                                                    | `configuration.md#File Locations; #Format Rules` | Aligned     |
| Custom-path precedence         | `cli/commands/mod.rs::Cli::execute`; `config/mod.rs::set_custom_config_path`; `main.rs::{extract_config_path,should_run_desktop}` | `cli/commands/mod.rs::tests::test_cli_parses_config_flag`                                                                      | `configuration.md#File Locations`                | Aligned     |
| First-run template/defaults    | `config/mod.rs::{load_or_default,create_default_config_file}`                                                                     | None — source-only evidence                                                                                                    | `configuration.md#First Run`                     | Aligned     |
| Immutable snapshot and watcher | `config/mod.rs::{CONFIG,CONFIG_PATH,init,get_config}`; `config/watcher.rs::watch_config_file`                                     | `config/watcher.rs::tests::{config_debounce_duration_is_reasonable,config_debounce_duration_creates_valid_duration}`           | `configuration.md#Hot Reload`                    | Aligned     |
| Parse/I/O fallback             | `config/mod.rs::load_or_default`; `config/types/root.rs::ConfigError`                                                             | `config/mod.rs::tests::test_config_error`                                                                                      | `configuration.md#Format Rules`                  | Conflict    |
| Config commands/schema         | `cli/commands/{config_cmd,mod}.rs::{execute,print_config_template,show_config_path}`                                              | `cli/commands/config_cmd.rs::tests::test_config_paths_returns_non_empty`; `cli/commands/mod.rs::tests::test_cli_parses_schema` | `cli.md`; `configuration.md`                     | Aligned     |
