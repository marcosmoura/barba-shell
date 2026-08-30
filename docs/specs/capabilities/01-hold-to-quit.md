# Capabilities 01 — Hold to Quit

> Status: 🟡 Draft · Open: CQ1

## Purpose

Intercept Command-Q so the frontmost app is terminated only after the configured hold duration.

## Scope

- Command-Q event tap, hold tracking, termination action, alert event, and tray lifecycle.

### Out of Scope

| Excluded concern        | Owner                                                    | Boundary note                              |
| ----------------------- | -------------------------------------------------------- | ------------------------------------------ |
| Visible alert rendering | Not a current capability                                 | No frontend consumer for the alert exists. |
| Tray menu rendering     | [Tray Controls](05-tray-controls.md)                     | Consumes lifecycle status.                 |
| Keybinding dispatch     | [Foundation 08](../foundation/08-keybinding-dispatch.md) | Does not own this event tap.               |

## Terminology

- **Early release** — Command-Q release before the timer marks `quit_triggered`.
- **Tap state** — Core Graphics event-tap enabled state, if an event tap was created.

## Data Contract

`commandQuit.enabled` defaults false and `holdDuration` defaults 1500 milliseconds. The module stores hold duration atomically, one press start time, `quit_triggered`, a check flag, retained event tap, running flag, and optional Tauri app handle.

## Configuration Contract

When config is disabled, `init` returns without starting threads. The current code does not validate a lower/upper hold-duration range.

## Inputs

A global event tap filters Command-Q down/up events. Key down starts timing only if no press is tracked; key up resets state. The timer polls every 100 ms while idle and 16 ms while checking a hold.

## State Transitions

| From                         | Input                            | To                           | Result                                                                        |
| ---------------------------- | -------------------------------- | ---------------------------- | ----------------------------------------------------------------------------- |
| idle                         | Command-Q down                   | holding                      | Records start and enables check.                                              |
| holding                      | hold reaches configured duration | quit-fired / awaiting key-up | Marks `quit_triggered`, requests termination, and clears only the check flag. |
| holding                      | early Command-Q up               | idle                         | Clears tracked press/state and emits/logs alert.                              |
| quit-fired / awaiting key-up | Command-Q up                     | idle                         | Clears retained press start and `quit_triggered` without an alert.            |
| tap enabled                  | tray pause                       | tap disabled                 | Core Graphics tap is disabled.                                                |
| tap disabled                 | tray resume                      | tap enabled                  | Existing tap is enabled.                                                      |

## Outputs

Early release emits `stache://cmd-q/alert` with a string `Hold ⌘Q to quit {app}` when an app handle exists, then logs the same message at debug level. No visible UI consumer was found. The event name is declared in `events.rs` and `tauri-events.ts`.

## Derived Effects

The timeout gets the frontmost `NSWorkspace` application, tries `terminate`, then `forceTerminate` if termination fails. OS lookup/termination failures are logged.

## Failure & Recovery

Missing Objective-C objects and event-tap failures are logged. Status is `ConfiguredOff` when disabled, `Unavailable` when marked running without a tap, `Running` for an enabled tap, and `Paused` for absent/disabled tap. Pause does not clear a currently tracked hold.

## Cross-Module Contracts

Tray status is resource-derived from configuration, running flag, and `CGEventTapIsEnabled`; it is not an acknowledgement that the timer thread stopped. The alert is declared for frontend consumption but no current renderer owns it.

## Acceptance Scenarios

1. **Normal hold.** Given a frontmost app and a held Command-Q, when the duration elapses, then the module requests terminate and may force terminate.
2. **Boundary repeat.** Given key repeat while a hold exists, when another down event arrives, then it does not replace the start time.
3. **Failure.** Given no event-tap handle, when pause runs, then it returns `event tap handle not available`.
4. **Lifecycle/early release.** Given a short Command-Q press, when released early, then it emits the alert string when possible and writes a debug log; it does not render a visible alert.

## Testing Seam

`cmd_q_status` is pure and the hold state/timer logic is localized to `cmd_q/mod.rs`; Objective-C termination and physical event taps require platform evidence.

## Open Decisions

| ID  | Current behavior                                                                           | Documented intent                                              | Rewrite consequence                                                                | Evidence                                                                                      |
| --- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| CQ1 | Early release emits `stache://cmd-q/alert` and debug logs, but no visible consumer exists. | Documentation explicitly identifies the missing visible alert. | Keep backend event and absence of rendering explicit; do not claim CQ1 removed it. | `cmd_q/mod.rs:425-439`; `events.rs::cmd_q::ALERT`; `cmd-q.md#Missing Feature: Visible Alert`. |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                      | Implementation evidence                                                       | Test evidence               | Intended documentation                                                | Disposition  |
| ----------------------------------------------------- | ----------------------------------------------------------------------------- | --------------------------- | --------------------------------------------------------------------- | ------------ |
| Config defaults and hold/event-tap behavior           | `config/types/command_quit.rs::CommandQuitConfig`; `cmd_q/mod.rs:126-339`     | `cmd_q/mod.rs::tests`       | `cmd-q.md#Configuration`                                              | Aligned      |
| Early-release alert event and absent visible consumer | `cmd_q/mod.rs:425-439`; `events.rs::cmd_q::ALERT`; `ui/types/tauri-events.ts` | None — source-only evidence | `cmd-q.md#Missing Feature: Visible Alert`; `events-and-ipc.md#Events` | Known defect |
