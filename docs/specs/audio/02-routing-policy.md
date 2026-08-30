# Audio 02 — Routing Policy

> Status: ✅ Normative · Decides: AU2

## Purpose

Choose the preferred current audio input and output devices from configured priorities and observed topology.

## Scope

- `proxyAudio` configuration, matching, dependencies, and deterministic target selection.

### Out of Scope

| Excluded concern                     | Owner                               | Boundary note                  |
| ------------------------------------ | ----------------------------------- | ------------------------------ |
| CoreAudio observation                | [Audio 01](01-device-inventory.md)  | Supplies devices and defaults. |
| Listener registration/default writes | [Audio 03](03-topology-reaction.md) | Applies policy after changes.  |
| Terminal list output                 | [Audio 04](04-queries-commands.md)  | Is read-only presentation.     |

## Terminology

- **Priority entry** — configured name, strategy, and optional dependency.
- **AirPlay preservation** — retaining a current AirPlay target when mirroring does not supersede it.

## Data Contract

`proxyAudio.enabled` defaults false and `input`/`output` default empty arrays. An `AudioDevicePriority` has `name` default `""`, `strategy` default `exact`, and optional `dependsOn`; a dependency has `name` default `""` and strategy default `exact`. Strategies are `exact`, `contains`, `startsWith`, and `regex`.

## Configuration Contract

`proxyAudio.output` and `.input` are ordered priority lists. `dependsOn` must be satisfied against all available devices before the parent is eligible. Root config owns parsing the `proxyAudio` key.

## Inputs

Selection takes current default, available direction-specific devices, config, and screen-mirroring state. It ignores unavailable priority targets and unsatisfied dependencies.

## State Transitions

No owned state. Selection returns an existing device reference or `None` for output when no candidate exists. Input retains the current device only if that ID remains in the supplied input-device list; otherwise it returns `None`.

## Outputs

The target is an in-process `Option<&AudioDevice>`; this policy publishes no event.

## Derived Effects

None. [Audio 03](03-topology-reaction.md) performs CoreAudio default-device writes.

## Failure & Recovery

No priority match is a normal selection outcome. Configuration/matching does not retry or mutate inventory.

## Cross-Module Contracts

[Audio 03](03-topology-reaction.md) gives this policy fresh current/default inventory. AirPlay and HDMI decisions are policy only; actual writes remain adapter effects.

## Acceptance Scenarios

1. **Normal priority.** Given an available first configured output device, when not superseded, then it is selected.
2. **Boundary dependency.** Given a target with an absent dependency, when selected, then it is skipped.
3. **Failure/no match.** Given no output candidate, when resolved, then the result is `None` rather than an invented device.
4. **Boundary missing current input.** Given no configured input replacement and a current input absent from the supplied input list, when resolved, then the result is `None`.
5. **Lifecycle.** Given a topology callback, when routing is reevaluated, then policy has no retained state from an earlier callback.

## Testing Seam

`resolve_output_device` and `resolve_input_device` take collections and mirroring state directly; focused unit tests pin priority ordering.

## Open Decisions

None.

## Resolved Decisions

- **AU2 — Dependency inventory.** Outcome: dependencies are checked against the supplied all-device collection and only gate the parent. Basis: current implementation and configuration agree. Evidence: `audio/device.rs:211-262`; `audio.md#Configuration`.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                   | Implementation evidence                                                                   | Test evidence                                           | Intended documentation                                          | Disposition |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------- | --------------------------------------------------------------- | ----------- |
| Proxy audio defaults and priority/dependency types | `config/types/audio.rs::ProxyAudioConfig`, `AudioDevicePriority`, `AudioDeviceDependency` | `config/mod.rs::tests::test_shared_types_are_available` | `audio.md#Configuration`; `configuration.md#Top-Level Sections` | Aligned     |
| Output AirPlay/HDMI/priority/fallback order        | `audio/priority.rs:10-70`                                                                 | `audio/priority.rs::tests`                              | `audio.md#Output Selection Order`                               | Aligned     |
| Input AirPlay/priority/fallback order              | `audio/priority.rs:72-130`                                                                | `audio/priority.rs::tests`                              | `audio.md#Input Selection Order`                                | Aligned     |
