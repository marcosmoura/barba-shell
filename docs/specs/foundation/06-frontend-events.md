# Foundation 06 — Frontend Events and Invokes

> Status: ✅ Normative

## Purpose

Record the declared cross-process event names and registered Tauri commands, including their actual emitters and known consumers, without inventing a generated manifest.

## Scope

- `events.rs`/`tauri-events.ts` declaration parity, registered handler names, literal frontend invokes, and emitter/consumer evidence.

### Out of Scope

| Excluded concern              | Owner                                                                    | Boundary note                                    |
| ----------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------ |
| Feature event payload meaning | [owning feature specifications](../index.md#specs-capabilities)          | This inventory does not redefine feature state.  |
| Socket IPC                    | [CLI control surface](05-cli-control-surface.md)                         | Tauri events and invokes are distinct transport. |
| Tiling actor event production | [tiling runtime lifecycle](../tiling/27-runtime-lifecycle-quarantine.md) | This file records its public names only.         |

## Terminology

- **Declared-only**: present in Rust/TypeScript constants but no audited emitter was found.
- **Literal invoke**: a frontend `invoke("name")` whose backend registration is in `lib.rs`.

## Data Contract

The Rust and TypeScript declarations match these names: `stache://menubar/visibility-changed`, `stache://keepawake/state-changed`, `stache://media/playback-changed`, `stache://spaces/window-focus-changed`, `stache://spaces/workspace-changed`, `stache://widgets/toggle`, `stache://widgets/click-outside`, `stache://cmd-q/alert`, `stache://app/reload`, and tiling `workspace-changed`, `workspace-windows-changed`, `layout-changed`, `window-tracked`, `window-untracked`, `screens-changed`, `initialized`, `window-focus-changed`, and `window-title-changed` under `stache://tiling/`.

`generate_handler!` registers: `open_app`, `get_battery_info`, `get_cpu_info`, `is_system_awake`, `toggle_system_awake`, `get_current_media_info`, `focus_tiling_window`, `focus_tiling_workspace`, `get_tiling_current_workspace_windows`, `get_tiling_focused_window`, `get_tiling_focused_workspace`, `get_tiling_windows`, `get_tiling_workspaces`, `is_tiling_enabled`, `get_weather_config`, `get_wifi_info`, and `get_bar_window_frame`.

## Configuration Contract

No owned configuration keys.

## Inputs

Feature emitters and frontend listeners/invokes. Event delivery is Tauri emission; no current source establishes replay, queueing, or manifest generation.

## State Transitions

No owned state. A declaration becomes delivered only when an emitter invokes Tauri `emit`; a declaration without an emitter remains declared-only.

## Outputs

| Event group                                | Emitter                                                                                      | Consumer evidence                                                                                                                         |
| ------------------------------------------ | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| menubar visibility                         | `modules/bar/menubar.rs::set_menubar_visible`                                                | `app/ui/renderer/bar/Bar.state.ts`                                                                                                        |
| keep-awake state                           | `modules/bar/components/keepawake.rs::emit_state_changed`                                    | `app/ui/renderer/bar/Status/KeepAwake/KeepAwake.state.ts`; the consumer defect is owned by [Keep Awake](../capabilities/04-keep-awake.md) |
| media playback                             | `modules/bar/components/media.rs::emit_media_update`                                         | `app/ui/renderer/bar/Media/Media.state.ts`                                                                                                |
| spaces focus/workspace                     | `modules/bar/ipc_listener.rs::handle_command`                                                | no consumer found for legacy spaces names                                                                                                 |
| widgets toggle/click-outside               | no current emitter for declared-only Toggle; `modules/widgets/window.rs` emits click-outside | `app/ui/renderer/widgets/Widgets.state.ts` listens to both declarations                                                                   |
| Cmd-Q alert                                | `modules/cmd_q/mod.rs`                                                                       | no visible renderer identified                                                                                                            |
| app reload                                 | `modules/bar/ipc_listener.rs::handle_command`                                                | `app/ui/renderer/Renderer.state.ts`                                                                                                       |
| tiling initialized/workspace/window events | `modules/tiling/init.rs`; focus adapters in `components/tiling.rs`/IPC listener              | `app/ui/renderer/bar/Spaces/Spaces.state.ts` listens to initialized/workspace/window events                                               |
| tiling layout-changed                      | dormant `modules/tiling/init.rs::emit_layout_applied` helper; no in-tree caller              | no frontend subscriber; [Tiling 12](../tiling/12-layout-selection.md) owns the open delivery decision                                     |
| `stache://tiling/screens-changed`          | no emitter found                                                                             | declaration only                                                                                                                          |

## Derived Effects

Emitters notify Tauri windows. `tauri-events.ts` mirrors constants manually; no shared event-manifest or generation seam is implemented.

## Failure & Recovery

Emit failures are logged or ignored by their feature emitter. Consumers must obtain fresh command/query state after missed events; no generic retry/replay contract exists. `screens-changed` must not be used as proof of screen notifications until an emitter exists.

## Cross-Module Contracts

The application shell ([foundation 10](10-application-shell.md)) owns registration; feature owners own payload interpretation; tiling owns actor/runtime event facts.

## Acceptance Scenarios

1. Given each declared name, when comparing Rust and TypeScript, then spelling is identical.
2. Given a menubar visibility change, when emitted, then payload is boolean.
3. Given keep-awake state, when emitted, then payload is `{ locked, desired_awake }`.
4. Given Cmd-Q early release, when alert is emitted, then no visible renderer is assumed.
5. Given `screens-changed`, when reviewing source, then it is declared-only rather than claimed as delivered.
6. Given a literal UI invoke, when it is used, then its name is one of the listed handlers.

## Testing Seam

`events.rs::tests::test_event_naming_convention` tests declaration spelling only. Registration is statically auditable at `lib.rs::run`; it has no cited runtime coverage test.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                   | Implementation evidence                                                                                              | Test evidence                                    | Intended documentation        | Disposition  |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | ----------------------------- | ------------ |
| Rust/TypeScript event declarations | `app/native/src/events.rs`; `app/ui/types/tauri-events.ts`                                                           | `events.rs::tests::test_event_naming_convention` | `events-and-ipc.md#Events`    | Aligned      |
| Emitters and consumers             | `modules/bar/{menubar,ipc_listener}.rs`; `components/{keepawake,media,tiling}.rs`; `modules/{cmd_q,widgets,tiling}/` | None — source-only evidence                      | `events-and-ipc.md#Events`    | Current-only |
| Declared `screens-changed`         | `events.rs::tiling::SCREENS_CHANGED`; `tauri-events.ts::TilingEvents`                                                | None — source-only evidence                      | `events-and-ipc.md#Events`    | Current-only |
| Handler and invoke surface         | `app/native/src/lib.rs:189-207::generate_handler!`; literal `invoke` calls under `app/ui`                            | None — source-only evidence                      | `architecture.md#Application` | Current-only |
