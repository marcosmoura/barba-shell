# Audio 04 — Queries and Commands

> Status: ✅ Normative · Decides: AU5

## Purpose

List current CoreAudio device information through the read-only audio CLI.

## Scope

- `stache audio list` flags and human/JSON projections.

### Out of Scope

| Excluded concern           | Owner                               | Boundary note                |
| -------------------------- | ----------------------------------- | ---------------------------- |
| Inventory failure fidelity | [Audio 01](01-device-inventory.md)  | Owns observations.           |
| Routing selection          | [Audio 02](02-routing-policy.md)    | Not user-controlled here.    |
| Topology reactions         | [Audio 03](03-topology-reaction.md) | Not exposed as CLI commands. |

## Terminology

- **All filter** — the device filter selected when both or neither direction flags are supplied.

## Data Contract

Each JSON entry has `name: string`, `type: string`, `input: bool`, and `output: bool`. Type strings are the device transport labels from `AudioDeviceType`.

## Configuration Contract

Consumes no CLI-specific config.

## Inputs

`audio list` accepts `--json`/`-j`, `--input`/`-i`, and `--output`/`-o`. `--input` alone selects input-only, `--output` alone selects output-only, and both/neither select all.

## State Transitions

No owned state. Each command collects a fresh list and renders it.

## Outputs

`--json` prints pretty JSON. Without it, the command prints a formatted device table. JSON serialization failure returns `AudioError` and therefore a nonzero CLI result. The CLI has no default-device query or switch operation.

## Derived Effects

Read-only CoreAudio list queries occur through `audio::list::list_devices`.

## Failure & Recovery

The command does not distinguish a genuine empty list from inventory failures collapsed by the underlying list helper. Running it again repeats observation.

## Cross-Module Contracts

The output shape comes from [Audio 01](01-device-inventory.md); it does not invoke [Audio 02](02-routing-policy.md) or [Audio 03](03-topology-reaction.md).

## Acceptance Scenarios

1. **Normal JSON.** Given `--json`, when devices are listed, then pretty JSON entries contain name, type, input, and output.
2. **Boundary filters.** Given both `--input` and `--output`, when listed, then all devices are selected.
3. **Failure.** Given JSON serialization failure, when rendering, then `AudioError` is returned.
4. **Lifecycle.** Given a later invocation after a device change, when list runs, then it performs a fresh query rather than retaining prior output.

## Testing Seam

Clap parsing is covered by command tests; `format_devices_table` accepts plain entries and is a stable output seam.

## Open Decisions

None.

## Resolved Decisions

- **AU5 — Manual audio controls.** Outcome: the current audio CLI is list-only. Basis: no query/switch subcommand is defined. Evidence: `cli/commands/audio.rs:13-63`; `cli.md`.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                         | Implementation evidence                               | Test evidence                                                                                                                                      | Intended documentation      | Disposition  |
| -------------------------------------------------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | ------------ |
| Flag grammar and filter precedence                       | `cli/commands/audio.rs:13-49`                         | `cli/commands/audio.rs::tests::test_audio_list_parse`, `test_audio_list_json_parse`, `test_audio_list_input_parse`, `test_audio_list_output_parse` | `cli.md`; `audio.md#CLI`    | Aligned      |
| JSON/table output and serialization error                | `cli/commands/audio.rs:49-63`; `audio/list.rs:15-116` | None — source-only evidence                                                                                                                        | `audio.md#CLI`              | Aligned      |
| Inventory failure can project as empty successful output | `audio/list.rs:40-73`                                 | None — source-only evidence                                                                                                                        | None — source-only evidence | Current-only |
