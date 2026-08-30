# Capabilities 02 — Menu Anywhere

> Status: ✅ Normative

## Purpose

Show the frontmost application's accessibility menu where a configured mouse/modifier chord occurs.

## Scope

- Trigger configuration, event-tap/menu behavior, and status lifecycle.

### Out of Scope

| Excluded concern                          | Owner                                                      | Boundary note                                |
| ----------------------------------------- | ---------------------------------------------------------- | -------------------------------------------- |
| Accessibility permission acquisition      | [Foundation 09](../foundation/09-platform-capabilities.md) | This module only observes cached permission. |
| Tray rendering                            | [Tray Controls](05-tray-controls.md)                       | Projects lifecycle state.                    |
| Application-menu reconstruction internals | Not a current capability                                   | No separately owned public contract.         |

## Terminology

- **Chord** — selected mouse-down event plus the exact configured modifier mask.
- **Tap state** — `Some(enabled)` for a retained tap, otherwise `None`.

## Data Contract

`menuAnywhere.enabled` defaults false; modifiers default `[control, command]`; mouse button defaults `rightClick`. Modifiers are from control, option, command, shift. Buttons are `rightClick` and `middleClick`.

## Configuration Contract

The monitor translates `rightClick` to Core Graphics right-mouse-down and `middleClick` to other-mouse-down. Required modifier flags are combined and compared against all supported modifier flags.

## Inputs

A matching mouse-down with exactly the configured modifier mask queues its Core Graphics location, schedules a main-thread menu build, and suppresses the original event. A nonmatching event or extra/missing modifier passes through.

## State Transitions

| From                        | Input                   | To       | Result                                       |
| --------------------------- | ----------------------- | -------- | -------------------------------------------- |
| disabled config             | start                   | inactive | Returns without a tap.                       |
| no accessibility permission | start                   | inactive | Returns without a tap.                       |
| inactive                    | successful tap creation | running  | Retains/enables tap and enters its run loop. |
| running                     | tray pause              | paused   | Disables retained tap.                       |
| paused                      | tray resume             | running  | Enables retained tap.                        |

## Outputs

The frontend receives no event. The current app's menu is rebuilt and shown through `NSMenu` at converted coordinates; if menu construction fails/panics, it is not shown.

## Derived Effects

The trigger crosses to the main thread, converts y using main-screen height (falling back to 1080), builds a frontmost-app menu, and calls `popUpMenuPositioningItem`.

## Failure & Recovery

Failed tap or run-loop-source creation returns from monitor startup without a diagnostic result. Status reports `ConfiguredOff`, `Unavailable("Accessibility permission required")`, `Unavailable` when running with no tap, `Running` for an enabled tap, or `Paused`. No confirmed-registration generation, rollback/quarantine state, or retryable construction contract exists.

## Cross-Module Contracts

Accessibility permission comes from the cached foundation check. Tray toggle directly calls `set_enabled`; it does not receive an externally confirmed callback-registration acknowledgement.

## Acceptance Scenarios

1. **Normal chord.** Given control-command/right click, when the event tap receives it, then it queues menu display and suppresses that event.
2. **Boundary modifiers.** Given an extra Shift key, when the same click occurs, then it passes through.
3. **Failure.** Given tap creation failure, when start runs, then no tap exists and status can be unavailable.
4. **Lifecycle.** Given a retained tap, when pause then resume runs, then it disables then enables that tap.

## Testing Seam

`menu_anywhere_status` and modifier-mask selection are stable local seams; event-tap and accessibility menu actions require macOS evidence.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                               | Implementation evidence                                         | Test evidence                 | Intended documentation           | Disposition  |
| ------------------------------------------------------------------------------ | --------------------------------------------------------------- | ----------------------------- | -------------------------------- | ------------ |
| Defaults, button mapping, and exact modifier matching                          | `config/types/menu_anywhere.rs`; `event_monitor.rs:87-178`      | `menu_anywhere/mod.rs::tests` | `menu-anywhere.md#Configuration` | Aligned      |
| Menu construction/display and status projection                                | `event_monitor.rs:191-305`; `menu_anywhere/mod.rs:63-120`       | None — source-only evidence   | `menu-anywhere.md`               | Current-only |
| Startup state, main-screen coordinate conversion, and frontmost-app menu build | `menu_anywhere/mod.rs:35-59`; `event_monitor.rs:87-124,228-305` | None — source-only evidence   | None — source-only evidence      | Current-only |
