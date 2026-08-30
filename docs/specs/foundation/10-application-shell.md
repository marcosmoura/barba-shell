# Foundation 10 — Application Shell

> Status: 🟡 Draft · Open: F10-D1

## Purpose

Define Stache’s macOS runtime package boundary: desktop selection, bundle identity, windows, installed plugins/capabilities/resources, and the allowlisted `open_app` bridge used by bar items.

## Scope

- macOS-only executable boundary, desktop/CLI selection, product/bundle metadata, generated windows and renderer routes, plugins/managed state, CSP/capabilities/resources/plist declarations, and `open_app`.

### Out of Scope

| Excluded concern                       | Owner                                                       | Boundary note                                                                             |
| -------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Bar composition and click presentation | [bar window lifecycle](../bar/01-bar-window-lifecycle.md)   | This shell exposes the launch command used by the item.                                   |
| CPU Activity Monitor interaction       | [CPU item](../bar/09-cpu-item.md)                           | CPU owns its click; `open_app` owns allowlisting/launch.                                  |
| Feature command payloads               | [capability specifications](../index.md#specs-capabilities) | Handler registration does not change feature state contracts.                             |
| Contributor release procedure          | Not a current capability                                    | Build/release instructions are evidence unless they change the packaged runtime artifact. |

## Terminology

- **Desktop mode**: no non-config argument, first remaining argument `--desktop`, or an executable path containing `.app/Contents/MacOS`.
- **Allowed app**: one canonical display name/launch target admitted by `open_app` after ASCII case-insensitive trimmed matching.

## Data Contract

The bundle is `Stache`, identifier `com.marcosmoura.stache`, version `0.25.0`, with `macOSPrivateApi: true` and an active app bundle target. F10-D1 exclusively decides the public minimum macOS version. The generated Tauri windows are `bar`/`#/bar` and `widgets`/`#/widgets`: initially hidden, transparent, always-on-top, nonfocusable, undecorated, nonresizable, skipped from taskbar, and visible on all workspaces. Their frontend route selects the renderer surface; feature specs own subsequent window manipulation.

`open_app` takes `{ name: string }` and returns `Result<(), StacheError>`. It trims then resolves case-insensitively against exactly:

| Display name       | Target                                                           |
| ------------------ | ---------------------------------------------------------------- |
| Activity Monitor   | `open -a "Activity Monitor"`                                     |
| Clock              | `open -a "Clock"`                                                |
| Microsoft Edge Dev | `open -a "Microsoft Edge Dev"`                                   |
| Spotify            | `open -a "Spotify"`                                              |
| Tidal              | `open -a "Tidal"`                                                |
| Weather            | `open -a "Weather"`                                              |
| Battery            | `x-apple.systempreferences:com.apple.Battery-Settings.extension` |
| Wi-Fi              | `x-apple.systempreferences:com.apple.wifi-settings-extension`    |

Empty/unknown names return `InvalidArguments("Application … is not allowed")`. Spawn/await failure, nonzero `open` status, and signal termination return `ShellError`. Success means the macOS `open` command exited successfully, not that the destination UI became visible.

## Configuration Contract

The shell consumes Tauri configuration, not user JSONC feature fields. `tauri.conf.json` fixes windows, CSP, bundle files, and product metadata. Capability bindings are `capabilities/default.json`, `bar.json`, and `widgets.json`; they govern the generated runtime permission surface.

## Inputs

Process arguments, executable location, Tauri generated context, `open_app` invokes, macOS `open`, bundle resources, and platform permission declarations.

## State Transitions

| From           | Input                         | To                 | Effect                                                                                                            |
| -------------- | ----------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------- |
| process start  | desktop predicate true        | desktop runtime    | Validate optional custom config then call `stache_lib::run`.                                                      |
| process start  | predicate false               | CLI runtime        | Call `stache_lib::cli::run`.                                                                                      |
| Tauri builder  | setup                         | configured runtime | Install single-instance, Zustand, shell, global-shortcut plugins; manage KeepAwake controller; register handlers. |
| open_app input | allowed target + zero status  | launched           | Return `Ok(())`.                                                                                                  |
| open_app input | blank/unknown or open failure | rejected/failed    | Return the exact typed error described above.                                                                     |

## Outputs

The configured app bundle, `bar` and `widgets` window shells, registered Tauri handlers, and the `open_app` command result. The invoke list is registered in `lib.rs::run`; `open_app` is the shared application-launch security boundary, not an Apps bar item.

## Derived Effects

The builder installs `tauri_plugin_single_instance`, `tauri_plugin_zustand`, `tauri_plugin_shell`, and the global-shortcut plugin and manages `KeepAwakeController`. CSP permits self by default, Tauri IPC/`http://ipc.localhost`, listed weather/geolocation hosts, self/data/blob images, self/inline styles, self/data fonts, and forbids object/base/frame sources. Bundling includes `resources/lib` and `resources/Frameworks`, including the MediaRemoteAdapter framework/media-control dependencies. `Info.plist` declares `LSUIElement`, high-resolution capability, utility category, and location usage descriptions.

## Failure & Recovery

A missing desktop `--config` writes `stache: configuration file not found: …` and exits 1 before runtime. Desktop builder failures currently panic through `expect`. `open_app` refuses unallowlisted input before spawning any command and reports launch errors; it does not retry. Plugin/window/permission configuration failures have no general runtime recovery contract in this shell.

## Cross-Module Contracts

[Foundation 06](06-frontend-events.md) inventories handler registration. [Bar window lifecycle](../bar/01-bar-window-lifecycle.md) and [CPU item](../bar/09-cpu-item.md) call this command by name. [Platform capabilities](09-platform-capabilities.md) owns runtime Accessibility checking; this shell owns package declarations. [Startup orchestration](03-startup-orchestration.md) owns setup sequencing after the builder begins.

## Acceptance Scenarios

1. Given no arguments, `--desktop` after a config flag, or a bundled executable, when main dispatches, then it starts desktop runtime.
2. Given a regular subcommand outside a bundle, when main dispatches, then it uses CLI runtime.
3. Given the packaged runtime, when bundle metadata is read, then name/identifier are Stache/`com.marcosmoura.stache`; F10-D1 decides the public minimum-version requirement.
4. Given either configured window, when created, then its label/route are `bar`/`#/bar` or `widgets`/`#/widgets` and it starts hidden/nonfocusable/transparent.
5. Given a valid case-insensitive allowed name with surrounding whitespace, when `open_app` runs, then its canonical app or URL target is opened.
6. Given `Battery` or `Wi-Fi`, when opened, then the respective System Settings URL—not an arbitrary executable—is used.
7. Given an empty or unrecognized name, when invoked, then it returns `InvalidArguments` without spawning `open`.
8. Given `open` exits nonzero or is signaled, when awaited, then `open_app` returns `ShellError`.

## Testing Seam

`main.rs::{should_run_desktop,extract_config_path,is_running_from_app_bundle}` is a deterministic dispatch seam. `modules/bar/components/apps.rs::resolve_allowed_app` has focused resolution tests; launching `open` is source-only integration evidence. Tauri config/plist/capabilities are static artifact seams.

## Open Decisions

| ID     | Current behavior                          | Documented intent                                                     | Rewrite consequence                                                                           | Evidence                                                                                                                |
| ------ | ----------------------------------------- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| F10-D1 | Bundle configuration requires macOS 14.0. | README states macOS 10.15 while external getting-started states 14.0. | Update or qualify stale README minimum; do not promote one minimum as universally documented. | `app/native/tauri.conf.json:126-133`; `README.md:33`; `/Users/marcosmoura/Documents/stache-docs/getting-started.md:7-8` |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                     | Implementation evidence                                                                                                                     | Test evidence                                                                                                                                                                                                                           | Intended documentation                                               | Disposition |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ----------- |
| macOS-only desktop selection                         | `app/native/src/{main,lib}.rs::{main,should_run_desktop,is_running_from_app_bundle,run}`                                                    | None — source-only evidence                                                                                                                                                                                                             | `getting-started.md#Desktop`                                         | Aligned     |
| Product identity, windows, renderer routes           | `app/native/tauri.conf.json:3-6,18-92`                                                                                                      | None — source-only evidence                                                                                                                                                                                                             | `architecture.md#Windows`; `getting-started.md#Desktop`              | Aligned     |
| Plugins/managed state and registered command surface | `app/native/src/lib.rs:181-207::run`                                                                                                        | None — source-only evidence                                                                                                                                                                                                             | `architecture.md#Application`                                        | Aligned     |
| CSP, capabilities, resources, plist declarations     | `tauri.conf.json:94-133`; `capabilities/{default,bar,widgets}.json`; `Info.plist:5-14`; `resources/Frameworks/MediaRemoteAdapter.framework` | None — source-only evidence                                                                                                                                                                                                             | `architecture.md#Security; #Media`; `getting-started.md#Permissions` | Aligned     |
| `open_app` allowlist, targets, and errors            | `modules/bar/components/apps.rs:40-119::{ALLOWED_APPS,resolve_allowed_app,run_open_command,open_app}`                                       | `modules/bar/components/apps.rs::tests::{resolve_allowed_app_finds_application_case_insensitively,resolve_allowed_app_handles_url_entries,resolve_allowed_app_finds_wifi_url_entry,resolve_allowed_app_rejects_empty_or_unknown_names}` | `status-bar.md#Openable Targets`                                     | Aligned     |
| Minimum macOS version                                | `tauri.conf.json:126-133`; `README.md:33`                                                                                                   | None — source-only evidence                                                                                                                                                                                                             | `README.md:33`; `getting-started.md:7-8`                             | Conflict    |
