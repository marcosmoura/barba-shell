# Status Bar 01 — Bar Window Lifecycle

> Status: ✅ Normative

## Purpose

The `bar` webview hosts the visible status-bar composition when `bar.enabled` is true; it is otherwise left hidden.

## Scope

- Bar configuration, initialization order, and hosting the renderer.
- The visible `Spaces → Media → Status` composition and the status-item order.

### Out of Scope

| Excluded concern                                   | Owner                                                                                                                                                                               | Boundary note                                             |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| Frame calculation and menu-visibility presentation | [bar/02 geometry and menu visibility](02-geometry-menubar-visibility.md)                                                                                                            | This spec starts the watcher but does not own its result. |
| Spaces state and interactions                      | [bar/03 spaces presentation](03-spaces-presentation.md)                                                                                                                             | Spaces is the left region.                                |
| Media state                                        | [bar/04 media item](04-media-item.md)                                                                                                                                               | Media is the centre region.                               |
| Individual status-item contracts                   | [bar/05 battery](05-battery-item.md), [bar/06 Wi-Fi](06-wifi-item.md), [bar/07 weather](07-weather-data.md), [bar/08 clock](08-clock-calendar.md), and [bar/09 CPU](09-cpu-item.md) | Status composes their views.                              |
| Keep Awake state                                   | [capabilities/04 keep awake](../capabilities/04-keep-awake.md)                                                                                                                      | It owns the Keep Awake status item.                       |
| Application-launch authorization                   | [foundation/10 application shell](../foundation/10-application-shell.md)                                                                                                            | Bar controls are command consumers, never an Apps item.   |
| Widget overlay                                     | [widgets/01 overlay lifecycle](../widgets/01-overlay-lifecycle.md)                                                                                                                  | Widgets use the bar frame.                                |

## Terminology

- **Bar window** — the Tauri webview labelled `bar`.
- **Status composition** — the ordered right-side group rendered by `Status`.

## Data Contract

`BarConfig` supplies `enabled: bool`, `height: u32`, `padding: u32`, and `weather`. Its derived defaults are `false`, `0`, `0`, and the default weather configuration. The frontend composition is exactly:

```text
Spaces → Media → Status
Status: Weather → CPU → Battery → Keep Awake → Wi-Fi → Clock
```

## Configuration Contract

`bar.enabled` controls whether initialization runs. `bar.height` and `bar.padding` are passed unchanged to the frame owner; there is no validated reference-value fallback in the current initializer.

## Inputs

- Immutable process configuration.
- The predeclared `bar` webview window.
- Screen/menu watcher and component initialization requests.

## State Transitions

| From               | Input                            | To              | Effect                                                               |
| ------------------ | -------------------------------- | --------------- | -------------------------------------------------------------------- |
| Predeclared hidden | `enabled = false`                | Hidden          | Return without bar setup.                                            |
| Predeclared hidden | `enabled = true`, window missing | Unavailable     | Log and return.                                                      |
| Predeclared hidden | `enabled = true`, window present | Starting        | Apply sticky/layer/frame setup; start watchers, components, and IPC. |
| Starting           | setup calls return               | Visible attempt | Call `show`.                                                         |
| Visible attempt    | show failure                     | Hidden          | Log; process continues.                                              |
| Visible            | process exit                     | Terminated      | No bar-owned in-process stop operation exists.                       |

The current call order is sticky setup, layer setup, position, screen watcher, menu watcher, components, bar IPC, then `show`.

## Outputs

- The shown bar window when setup reaches `show` successfully.
- A renderer containing the exact composition above.
- No dedicated bar-ready event.

## Derived Effects

The native setup marks the window sticky and layers it below the system menu; screen and menu watchers are started before components and the IPC listener.

## Failure & Recovery

A missing window and `show` failure are logged. `set_window_position` returns early when its screen lookup fails, but initialization continues. Recovery is a later process initialization; there is no bar-specific retry or shutdown API.

## Cross-Module Contracts

The renderer consumes the boolean menu state from [bar/02](02-geometry-menubar-visibility.md), requests Spaces/Media/status contracts from their owners, and passes only the bar-frame boundary to [widgets/01](../widgets/01-overlay-lifecycle.md). Any `open_app` call is authorized by [foundation/10](../foundation/10-application-shell.md).

## Acceptance Scenarios

1. Given `bar.enabled: false`, when initialization runs, then the bar remains hidden and no bar setup starts.
2. Given enabled configuration and no `bar` window, when initialization runs, then it logs and returns without a panic.
3. Given an enabled existing window, when initialization runs, then native setup precedes watchers, components, IPC, and `show`.
4. Given the renderer, when it mounts, then regions appear as Spaces, Media, Status and status children appear in the specified order.
5. Given a frame lookup failure, when initialization continues, then no invented geometry fallback is applied.
6. Given `show` rejects, when setup finishes, then the process remains running and the failure is logged.
7. Given process termination, when resources end, then no unsupported in-process bar restart/teardown guarantee is assumed.

## Testing Seam

`modules::bar::init` is the startup seam; `Bar.tsx` and `Status.tsx` are the stable composition seam. Existing tests include `app/ui/renderer/bar/Bar.test.tsx` and `app/ui/renderer/bar/Status/Status.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                      | Implementation evidence                                                                | Test evidence                                                                                                                                               | Intended documentation | Disposition  |
| ------------------------------------- | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- | ------------ |
| Config defaults and enabled gate      | `app/native/src/config/types/bar.rs:88-112`; `app/native/src/modules/bar/mod.rs:12-17` | None — source-only evidence                                                                                                                                 | `status-bar.md:24-37`  | Aligned      |
| Initialization order and show failure | `app/native/src/modules/bar/mod.rs:19-46`                                              | None — source-only evidence                                                                                                                                 | `status-bar.md:40-48`  | Current-only |
| Visible composition and status order  | `app/ui/renderer/bar/Bar.tsx:20-30`; `app/ui/renderer/bar/Status/Status.tsx:12-22`     | `Bar.test.tsx` — `renders main bar container`; `renders Spaces and Status containers`; `Status.test.tsx` — `renders status container with child components` | `status-bar.md:50-57`  | Aligned      |
