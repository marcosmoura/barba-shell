# Wallpaper 04 — Application Adapter

> Status: ✅ Normative

## Purpose

Apply processed wallpaper paths to macOS desktops and translate wallpaper CLI target syntax.

## Scope

- macOS all-screen and one-screen setters and `wallpaper set` argument/result behavior.

### Out of Scope

| Excluded concern  | Owner                                      | Boundary note                        |
| ----------------- | ------------------------------------------ | ------------------------------------ |
| Collection lookup | [Wallpaper 02](02-discovery-collection.md) | Resolves requested names.            |
| Cache processing  | [Wallpaper 03](03-processing-cache.md)     | Produces setter inputs.              |
| Timer reset       | [Wallpaper 01](01-cycling.md)              | Happens after global action success. |

## Terminology

- **Screen selector** — CLI `all`, `main`, or a positive 1-based screen number.
- **All-screen setter** — the adapter function that validates a path and delegates once to the aggregate `wallpaper::set_from_path` API.

## Data Contract

The CLI accepts exactly one of optional positional `PATH` and `--random`. `--screen` defaults to `all`; `main` maps to the main screen and a positive integer is converted to the internal zero-based index. Backend manual action variants use zero-based indexes.

## Configuration Contract

No adapter-specific config. It uses [Wallpaper 01](01-cycling.md)'s selected collection and processing settings.

## Inputs

`stache wallpaper set PATH [--screen SELECTOR]` requests a named file action. `stache wallpaper set --random [--screen SELECTOR]` requests a random action. Missing both or supplying both is invalid arguments.

## State Transitions

No owned persistent state. A CLI invocation validates selector/action, calls the manager, then prints success or returns the mapped error.

## Outputs

On success the CLI prints exactly `Wallpaper set successfully.`. It maps manager errors to `StacheError`; an error is a nonzero CLI result. No UI event is published.

## Derived Effects

`macos::set_wallpaper` validates a path then delegates once to `wallpaper::set_from_path`, whose macOS behavior is aggregate. `WallpaperAction::Random` is separately implemented by the manager as an explicit per-screen loop using `set_wallpaper_for_screen`.

## Failure & Recovery

Invalid CLI arguments are rejected before manager application. Invalid internal indexes and macOS setter errors are returned. The aggregate setter has no per-screen outcome structure, partial-result object, or retry contract.

## Cross-Module Contracts

[Wallpaper 01](01-cycling.md) performs global actions and therefore resets the timer after a successful action. [Wallpaper 03](03-processing-cache.md) supplies processed paths.

## Acceptance Scenarios

1. **Normal set.** Given a resolvable path argument, when set runs, then the manager applies it and the CLI prints the success line.
2. **Boundary selector.** Given `--screen 1`, when parsed, then it maps to internal screen zero; `all` is the default.
3. **Failure.** Given both PATH and `--random`, when parsed, then invalid arguments are returned before setting a wallpaper.
4. **Lifecycle.** Given an initialized manager and interval greater than zero, when a setter succeeds, then the global manager wrapper resets its timer.

## Testing Seam

`WallpaperCommands` parser tests isolate action/selector grammar; manager and macOS adapter seams separately cover effects.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                     | Implementation evidence                                  | Test evidence                                                                                                                           | Intended documentation        | Disposition  |
| -------------------------------------------------------------------- | -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | ------------ |
| Set grammar, selectors, success text, and error mapping              | `cli/commands/wallpaper.rs:17-61,95-160`                 | `cli/commands/wallpaper.rs::tests::test_wallpaper_set_path_parse`, `test_wallpaper_set_random_parse`, `test_wallpaper_set_screen_index` | `cli.md`; `wallpapers.md#CLI` | Aligned      |
| Aggregate `set_from_path` adapter and manager random-per-screen loop | `modules/wallpaper/macos.rs:51-60`; `manager.rs:215-230` | None — source-only evidence                                                                                                             | `wallpapers.md#Rotation`      | Current-only |
