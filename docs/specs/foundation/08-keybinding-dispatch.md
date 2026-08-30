# Foundation 08 — Keybinding Dispatch

> Status: ✅ Normative

## Purpose

Register configured standard, Caps Lock, and ISO section-key bindings and detach ordered command execution from input callbacks.

## Scope

- Binding classification/normalization, OS registration, Caps Lock remapping, section-key registration, command parsing/execution, and shutdown restoration hook.

### Out of Scope

| Excluded concern               | Owner                                                  | Boundary note                                           |
| ------------------------------ | ------------------------------------------------------ | ------------------------------------------------------- |
| Keybinding config root parsing | [configuration contract](01-configuration-contract.md) | This spec consumes `keybindings`.                       |
| Process terminal cleanup order | [shutdown and reload](04-shutdown-reload.md)           | It calls this module’s shutdown hook.                   |
| Tiling command semantics       | [tiling specifications](../index.md#specs-tiling)      | A binding may launch a CLI command but does not own it. |

## Terminology

- **Standard binding**: a `tauri-plugin-global-shortcut` `Shortcut`.
- **Caps binding**: `CapsLock+<single key>` handled as an F18 pseudo-modifier.
- **Section binding**: a `§` shortcut registered directly through Carbon.

## Data Contract

`ShortcutCommands` is an untagged single string or ordered string array; empty values capture/block without command execution. `collect_planned_shortcuts` sorts map entries, separates Caps/section/standard bindings, normalizes input, and logs invalid/duplicate normalized bindings. Caps accepts only one key after `CapsLock`; `CapsLock+Command+S` is ignored. Section resolution probes the keyboard layout and falls back to ISO keycode `0x0A` when inspection cannot resolve it.

## Configuration Contract

`keybindings` maps shortcut strings to `ShortcutCommands`; its field defaults are owned by the root configuration type. A configured Caps binding globally remaps physical Caps Lock to F18 while Stache owns it; tapping it alone does not toggle capitalization.

## Inputs

Configuration snapshot during Tauri setup; press events; keyboard layout; Carbon/CoreGraphics/HID APIs; strings beginning `stache` or external commands.

## State Transitions

| From                                | Input            | To                | Effect                                                       |
| ----------------------------------- | ---------------- | ----------------- | ------------------------------------------------------------ |
| unregistered                        | config binding   | registered/failed | Individual standard registration logs failure and continues. |
| no Caps tap                         | Caps bindings    | remap attempted   | Event tap must start before remap applies.                   |
| pressed standard/Caps/section match | callback         | dispatch detached | Only pressed standard events execute commands.               |
| command list                        | command succeeds | next command      | Run sequence in detached thread.                             |
| command list                        | command fails    | stopped           | Do not run remaining commands.                               |
| shutdown                            | orderly cleanup  | restored          | Restore Caps mapping if current mapping is still Stache’s.   |

## Outputs

OS shortcut registrations, detached external/Stache CLI execution, and logs. No frontend event or joinable command-completion result is published.

## Derived Effects

Standard bindings use global shortcut plugin; section bindings use Carbon `RegisterEventHotKey`; Caps uses a CoreGraphics event tap and `hidutil` mapping. `execute_shortcut_commands` clones commands and starts a thread; execution is sequential inside that thread, not on the UI callback.

## Failure & Recovery

Invalid or duplicate forms are logged. Individual standard registration failure does not abort registration. Caps event-tap/remap/layout failures log and leave that facility unavailable; the code does not expose a typed adapter status. If Caps mapping changed externally, shutdown leaves it untouched. Command parse/execution failure stops its sequence but has no caller-visible completion result.

## Cross-Module Contracts

[Startup orchestration](03-startup-orchestration.md) invokes `register_configured_hotkeys`; [shutdown](04-shutdown-reload.md) invokes `hotkey::shutdown`; feature owners receive only the launched command/effect.

## Acceptance Scenarios

1. Given empty keybindings, when setup runs, then no registration work is performed.
2. Given standard bindings, when callbacks receive Released then Pressed, then only Pressed dispatches.
3. Given `CapsLock+S`, when Caps setup succeeds, then physical Caps Lock is remapped and its binding can dispatch.
4. Given `CapsLock+Command+S`, when planned, then it is logged/ignored.
5. Given a section layout mapping failure, when section bindings register, then ISO keycode fallback is used.
6. Given multiple commands, when the first fails, then later commands do not execute.
7. Given orderly shutdown after an untouched Stache mapping, when cleanup runs, then Caps remapping is restored.

## Testing Seam

`modules/hotkey/mod.rs::{collect_planned_shortcuts,split_command}` plus the exact Caps/section parser/state tests are deterministic seams; OS event tap/Carbon registration is source-only integration evidence.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface            | Implementation evidence                                                                                                    | Test evidence                                                                                                                                                                                                     | Intended documentation         | Disposition  |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ | ------------ |
| Binding data/classification | `config/types/root.rs::ShortcutCommands`; `modules/hotkey/mod.rs::{register_configured_hotkeys,collect_planned_shortcuts}` | `modules/hotkey/mod.rs::tests::{test_normalize_shortcut_ctrl,test_collect_planned_shortcuts_separates_caps_bindings,test_collect_planned_shortcuts_rejects_invalid_caps_binding}`                                 | `keybindings.md#Configuration` | Aligned      |
| Caps Lock lifecycle         | `modules/hotkey/caps_lock/{mod,parser,remap,state}.rs`                                                                     | `caps_lock/mod.rs::tests::{parse_caps_letter_binding,reject_unsupported_caps_shapes,state_machine_executes_configured_chord_once}`; `caps_lock/remap.rs::tests::restored_mappings_preserves_external_caps_change` | `keybindings.md#CapsLock`      | Aligned      |
| Section key lifecycle       | `modules/hotkey/section.rs::{start,parse_section_shortcut,resolve_section_key}`                                            | `section.rs::tests::{parse_section_shortcut_with_modifiers,select_section_candidate_prefers_fewest_modifiers,foreign_carbon_hotkeys_are_forwarded}`                                                               | `keybindings.md#Section Key`   | Aligned      |
| Detached ordered execution  | `modules/hotkey/mod.rs::execute_shortcut_commands`                                                                         | None — source-only evidence                                                                                                                                                                                       | `keybindings.md#Commands`      | Current-only |
