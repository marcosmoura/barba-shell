# Capabilities 03 — NoTunes

> Status: ✅ Normative

## Purpose

Block Apple Music or iTunes launches and optionally open a configured replacement music app.

## Scope

- Workspace observer, target mapping, replacement launch, and lifecycle status.

### Out of Scope

| Excluded concern              | Owner                                                  | Boundary note                          |
| ----------------------------- | ------------------------------------------------------ | -------------------------------------- |
| Tray UI                       | [Tray Controls](05-tray-controls.md)                   | Consumes lifecycle state.              |
| General application launching | [Foundation 10](../foundation/10-application-shell.md) | Owns user-facing allowlisted launches. |
| Media status display          | [Bar 04](../bar/04-media-item.md)                      | Is independent of launch interception. |

## Terminology

- **Blocked app** — bundle ID `com.apple.Music` or `com.apple.iTunes`.
- **Target** — `tidal`, `spotify`, `feishin`, or `none` replacement setting.

## Data Contract

`notunes.enabled` defaults false and `targetApp` defaults `spotify`. Target enum values map to known app paths/display names; `none` has no app path. The module retains an optional workspace observer, an atomic running flag, and a process-lifetime `OnceLock` target.

## Configuration Contract

Configuration is sampled when `init` first successfully begins. Later target changes are not applied to the existing `OnceLock` value.

## Inputs

`NSWorkspace.willLaunchApplicationNotification` invokes the callback. For a blocked bundle ID, it force-terminates that app and calls replacement launch. Start/resume/pause are tray lifecycle inputs.

## State Transitions

| From    | Input              | To       | Result                                                       |
| ------- | ------------------ | -------- | ------------------------------------------------------------ |
| stopped | enabled init/start | starting | Spawns main-thread observer setup and termination sweep.     |
| running | blocked launch     | running  | Force-terminates then optionally launches target.            |
| running | pause              | paused   | Removes retained observer on main thread and clears running. |
| paused  | resume             | running  | Calls observer setup and sets running.                       |

## Outputs

Publishes no frontend event. Target missing/not runnable and launch errors are tracing warnings/errors only.

## Derived Effects

Initialization removes existing Apple Music/iTunes processes after observer setup. Callback uses `forceTerminate`. A configured target is skipped for `none`, missing path, or an already-running matching bundle ID; otherwise `/usr/bin/open` is spawned.

## Failure & Recovery

Observer setup has no returned registration outcome; status is `ConfiguredOff` when disabled, `Running` when observer exists, `Unavailable("observer registration failed")` when running has no observer, otherwise `Paused`. There is no public confirmed generation, rollback, or retry guarantee.

## Cross-Module Contracts

Tray callers directly invoke `pause`/`resume`; status derives from retained observer/running state. NoTunes owns its workspace observer independently of generic lifecycle startup claims.

## Acceptance Scenarios

1. **Normal block.** Given a launch notification for Apple Music, when received, then the app is force-terminated and the configured installed target may be opened.
2. **Boundary target none.** Given `targetApp: none`, when a blocked launch occurs, then no replacement is launched.
3. **Failure.** Given a missing target path, when a blocked launch occurs, then replacement is skipped and a warning is logged.
4. **Lifecycle.** Given pause then resume, when they complete, then the observer is removed and then set up again on the main thread.

## Testing Seam

`is_music_app` and `no_tunes_status` are pure seams; workspace observer callbacks and process launching remain macOS effects.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                       | Implementation evidence                                   | Test evidence               | Intended documentation          | Disposition  |
| ---------------------------------------------------------------------- | --------------------------------------------------------- | --------------------------- | ------------------------------- | ------------ |
| Defaults, blocked bundle IDs, target mapping, and replacement behavior | `config/types/notunes.rs`; `notunes/mod.rs:25-53,179-259` | `notunes/mod.rs::tests`     | `notunes.md#Configuration`      | Aligned      |
| Lifecycle status and main-thread observer removal                      | `notunes/mod.rs:261-325`                                  | `notunes/mod.rs::tests`     | `tray-and-lifecycle.md#Modules` | Current-only |
| Asynchronous observer setup and inferred status                        | `notunes/mod.rs:55-83,114-177,261-325`                    | None — source-only evidence | None — source-only evidence     | Current-only |
