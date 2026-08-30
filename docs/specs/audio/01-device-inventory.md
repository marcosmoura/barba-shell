# Audio 01 — Device Inventory

> Status: 🟡 Draft · Open: AU1

## Purpose

Read CoreAudio devices and expose the currently observable inventory used by routing and CLI listing.

## Scope

- Device identity, transport classification, input/output enumeration, and default-device queries.

### Out of Scope

| Excluded concern   | Owner                               | Boundary note                 |
| ------------------ | ----------------------------------- | ----------------------------- |
| Selection priority | [Audio 02](02-routing-policy.md)    | Consumes inventory.           |
| Listener lifecycle | [Audio 03](03-topology-reaction.md) | Reacts to inventory changes.  |
| CLI formatting     | [Audio 04](04-queries-commands.md)  | Projects inventory to output. |

## Terminology

- **Inventory** — the current best-effort vector of `AudioDevice` values.
- **Transport type** — `AirPlay`, Bluetooth, USB, HDMI, built-in, virtual, or other classification.

## Data Contract

`AudioDevice` carries CoreAudio ID, name, and type. Output and input helpers return `Vec<AudioDevice>`; default helpers return `Option<AudioDevice>`. Name lookup is case-insensitive substring matching; configured strategy supports exact, contains, starts-with, and regex matching. A dependency only enables its parent priority entry and is never selected itself.

## Configuration Contract

No inventory-only key. Matching strategy and dependencies are consumed by [Audio 02](02-routing-policy.md).

## Inputs

CoreAudio enumeration and property queries provide input.

## State Transitions

No owned state; every helper takes a fresh CoreAudio observation and returns a vector or optional default.

## Outputs

Inventory is an in-process value. No device event or snapshot is emitted directly by this module.

## Derived Effects

Read-only CoreAudio property queries determine available and default devices.

## Failure & Recovery

The current error-shape remains open decision AU1. A later query re-observes CoreAudio. Watcher startup and listener lifecycle decisions are owned by [Audio 03](03-topology-reaction.md).

## Cross-Module Contracts

[Audio 02](02-routing-policy.md) selects only from supplied inventory. [Audio 04](04-queries-commands.md) independently projects CoreAudio device information for CLI output.

## Acceptance Scenarios

1. **Normal inventory.** Given CoreAudio output devices, when queried, then the helper returns discovered `AudioDevice` values.
2. **Boundary dependency.** Given a priority dependency absent from all devices, when evaluated, then its parent is not selected.
3. **Failure re-observation.** Given a failed CoreAudio observation, when a later query runs, then it takes a fresh inventory observation.
4. **Lifecycle.** Given a later query after topology changes, when invoked, then it reads a fresh inventory.

## Testing Seam

Device matching and dependency functions accept value collections and are unit-testable without CoreAudio hardware; CoreAudio enumeration is source-only behavior.

## Open Decisions

| ID  | Current behavior                                                                                                                                     | Documented intent                                                             | Rewrite consequence                                          | Evidence                                                             |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------- |
| AU1 | CoreAudio inventory/list helpers collapse failed observations into vectors or optional defaults without a typed complete/partial/unavailable result. | Audio documentation's error contract requires a diagnosable inventory result. | Keep typed snapshot/result shape out of normative contracts. | `audio/device.rs:158-190`; `audio/list.rs:40-73`; `audio.md#Errors`. |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                            | Implementation evidence                          | Test evidence               | Intended documentation   | Disposition |
| ----------------------------------------------------------- | ------------------------------------------------ | --------------------------- | ------------------------ | ----------- |
| Device shape, type detection, matching, and dependency rule | `audio/device.rs:16-262`                         | `audio/device.rs::tests`    | `audio.md#Configuration` | Aligned     |
| Inventory failure representation                            | `audio/device.rs:158-190`; `audio/list.rs:40-73` | None — source-only evidence | `audio.md#Errors`        | Conflict    |
