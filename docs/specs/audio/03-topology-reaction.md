# Audio 03 — Topology Reaction

> Status: 🟡 Draft · Open: AU3, AU4

## Purpose

Listen for CoreAudio topology/default changes and apply the audio routing policy.

## Scope

- CoreAudio listener generation, pause/resume, status, and default-device effects.

### Out of Scope

| Excluded concern    | Owner                              | Boundary note                     |
| ------------------- | ---------------------------------- | --------------------------------- |
| Inventory semantics | [Audio 01](01-device-inventory.md) | Supplies observations.            |
| Target policy       | [Audio 02](02-routing-policy.md)   | Decides desired devices.          |
| CLI listing         | [Audio 04](04-queries-commands.md) | Is not a watcher control surface. |

## Terminology

- **Generation** — one retained set of three CoreAudio listeners, sender, and worker.
- **Runtime** — the optional active retained listener generation.

## Data Contract

A runtime owns three property addresses, callback client data, a sender, and a worker join handle. The next generation counter is atomic. There is at most one retained runtime slot.

## Configuration Contract

Uses `proxyAudio.enabled`, input, and output priorities from [Audio 02](02-routing-policy.md).

## Inputs

The watcher responds to device-list, default-output, and default-input CoreAudio property callbacks. `start` is a no-op when a runtime exists. A lifecycle `pause` removes the runtime; `resume` starts using current configuration.

## State Transitions

| From    | Input                     | To                | Result                                                |
| ------- | ------------------------- | ----------------- | ----------------------------------------------------- |
| absent  | start with enabled config | runtime or absent | Attempts listener generation.                         |
| runtime | device callback           | runtime           | Worker reevaluates output and input.                  |
| runtime | pause                     | absent            | Removes listeners, disconnects channel, joins worker. |
| absent  | resume                    | runtime or absent | Attempts a new generation.                            |

## Outputs

No public event is emitted. The runtime may set a CoreAudio default output/input when policy returns a different target.

## Derived Effects

`set_default_output_device` and `set_default_input_device` call CoreAudio property writes and return booleans; failed writes are logged by change handlers. Listener setup and removal operate through CoreAudio APIs.

## Failure & Recovery

A start-generation error is logged by `start`; current status is `ConfiguredOff` when disabled, `Running` only when runtime is present, otherwise `Paused`. Listener setup rolls back any listeners registered before a later registration failure; listener removal logs removal or worker-join errors. No aggregate degraded publication, typed quarantine, or user-visible retry contract exists.

## Cross-Module Contracts

[Audio 02](02-routing-policy.md) supplies target decisions. Tray consumers use this module's resource-derived status; they do not receive a startup result event.

## Acceptance Scenarios

1. **Normal callback.** Given an active runtime and device change, when its worker receives the callback, then it reevaluates input and output policy.
2. **Boundary start.** Given an active runtime, when start is called, then a second generation is not created.
3. **Failure.** Given listener setup failure, when start runs, then failure is logged and status has no typed unavailable reason.
4. **Lifecycle.** Given pause, when removal completes, then listeners are removed, channel disconnects, and worker join is attempted before the slot is cleared.

## Testing Seam

`proxy_audio_status` is a pure status seam; listener address and generation lifetime are localized to `watcher.rs`. Existing tests cover status decision, not physical CoreAudio callbacks.

## Open Decisions

| ID  | Current behavior                                                                                                                               | Documented intent                                                                    | Rewrite consequence                                        | Evidence                                             |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ---------------------------------------------------------- | ---------------------------------------------------- |
| AU3 | Watcher startup failure is logged and an absent runtime maps to `Paused`; no typed unavailable/degraded startup publication is produced.       | Audio documentation's error contract requires an observable startup failure outcome. | Do not specify an unavailable/degraded startup result.     | `audio/watcher.rs:289-344`; `audio.md#Errors`.       |
| AU4 | Listener setup/removal owns callback resources internally but exposes no typed quarantine or externally confirmed partial-registration result. | Audio documentation's error contract requires listener-failure diagnostics.          | Do not promise quarantine or confirmed-registration state. | `audio/watcher.rs:27-42,194-287`; `audio.md#Errors`. |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                              | Implementation evidence                                 | Test evidence               | Intended documentation | Disposition |
| --------------------------------------------- | ------------------------------------------------------- | --------------------------- | ---------------------- | ----------- |
| Callback routing and CoreAudio default writes | `audio/watcher.rs:48-153`                               | None — source-only evidence | `audio.md#Monitoring`  | Aligned     |
| Listener setup/removal failure representation | `audio/watcher.rs:27-42,194-287`                        | `audio/watcher.rs::tests`   | `audio.md#Errors`      | Conflict    |
| Startup failure status                        | `audio/watcher.rs:289-344`; `lib.rs::lazy_load_modules` | None — source-only evidence | `audio.md#Errors`      | Conflict    |
