# Status Bar 09 — CPU Item

> Status: ✅ Normative

## Purpose

The CPU status item reports rounded global usage and an optional temperature, signals hot CPU at 85°C or above, and opens Activity Monitor when clicked.

## Scope

- `get_cpu_info` computation/result and the CPU status item's poll, presentation, and click.

### Out of Scope

| Excluded concern                                             | Owner                                                                    | Boundary note                                      |
| ------------------------------------------------------------ | ------------------------------------------------------------------------ | -------------------------------------------------- |
| Application-launch allowlist, URL targets, and launch errors | [foundation/10 application shell](../foundation/10-application-shell.md) | CPU only requests the Activity Monitor target.     |
| Bar composition                                              | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)                | Status owns CPU's position.                        |
| Generic command registration                                 | [foundation/06 frontend events](../foundation/06-frontend-events.md)     | This spec owns the command's result semantics.     |
| Hardware thermal control                                     | Not a current capability                                                 | CPU monitoring does not alter fan or power policy. |

## Terminology

- **Global usage** — the `sysinfo::System` aggregate CPU use, not a per-core value.
- **Available temperature** — a reading strictly greater than 0°C and strictly less than 150°C from the fallback sequence.
- **Hot** — temperature `>= 85°C`.

## Data Contract

`get_cpu_info` takes no frontend argument and serializes:

```text
{ usage: number, temperature: number | null }
```

`usage` is global CPU percentage from one process-global `LazyLock<Mutex<System>>`, refreshed on each blocking computation and rounded to the nearest `f32`. `temperature` is likewise rounded, or null when every source fails/has only invalid readings. A blocking-pool join failure produces the default `{ usage: 0, temperature: null }`; this command has no observable `Result` error channel.

Temperature source order is exact:

1. `SMC::cpus_temperature()` valid-reading average.
2. Valid average of CPU-related SMC sensor keys containing `TC`, `Tp`, `Te`, or `Tf`.
3. `ismc temp -o json`, averaging CPU-related valid readings.
4. `smctemp -c`, if its one reading is valid.

Each temperature is valid only when `0 < °C < 150`.

The frontend polls every 2 seconds. It always shows usage, omits temperature when null (or zero), and renders hot data with red text and `CpuChargeIcon`; other data uses text color and `CpuIcon`.

## Configuration Contract

None. SMC availability and optional installed `ismc`/`smctemp` tools affect only temperature availability; the poll interval and threshold are fixed.

## Inputs

- `sysinfo` aggregate CPU refresh.
- macOS SMC calls; optional `ismc` and `smctemp` executable output.
- CPU status-item click.

## State Transitions

| From                       | Input                      | To         | Effect                                             |
| -------------------------- | -------------------------- | ---------- | -------------------------------------------------- |
| Unqueried                  | mount                      | Snapshot   | Invoke CPU command and schedule 2-second refetch.  |
| Snapshot                   | next interval              | Snapshot   | Replace usage/temperature with rounded new result. |
| Snapshot                   | no valid temperature       | Usage-only | Hide the temperature span.                         |
| Snapshot                   | temperature `>= 85`        | Hot        | Use red color and charge icon.                     |
| Snapshot                   | temperature `< 85` or null | Normal     | Use regular icon/text color.                       |
| Any rendered state         | click                      | Same       | Invoke `open_app({ name: 'Activity Monitor' })`.   |
| Blocking task join failure | poll result                | Fallback   | Render zero usage and hidden temperature.          |

## Outputs

The Tauri command returns the snapshot; it emits no event. The frontend produces a button surface containing usage and optional Celsius. Its click is a request to the application-shell launch policy.

## Derived Effects

`get_cpu_info` uses `tauri::async_runtime::spawn_blocking` so SMC, command execution, and sysinfo refresh do not occupy the async runtime worker. External tools are resolved from the current path and known Homebrew prefixes before invocation.

## Failure & Recovery

SMC connection/sensor failures, unavailable external commands, JSON parse failure, and out-of-range readings fall through to the next source and finally null temperature. A `spawn_blocking` join failure uses `CpuInfo::default`. The next 2-second poll retries all reads; there is no separate error view or circuit breaker.

## Cross-Module Contracts

[bar/01](01-bar-window-lifecycle.md) composes CPU after Weather. [foundation/06](../foundation/06-frontend-events.md) registers public invocation transport. [foundation/10](../foundation/10-application-shell.md) authorizes the exact Activity Monitor launch target.

## Acceptance Scenarios

1. Given the CPU item mounts, when its query starts, then it invokes `get_cpu_info` and refetches every 2 seconds.
2. Given a refreshed global usage of 30.4, when returned, then the command returns rounded usage 30.
3. Given valid aggregate SMC readings, when available, then their average is used before sensor-key or shell fallbacks.
4. Given absent aggregate readings and valid `Tp`/`Te`/`Tf`/`TC` sensor readings, when sampled, then their valid average is used.
5. Given no valid SMC readings but valid `ismc` JSON, when queried, then that result is used before `smctemp`.
6. Given a reading of 0, 150, or an unavailable tool, when evaluating temperature, then it is rejected and the next fallback is tried; all failures yield null.
7. Given a blocking-task join failure, when the command completes, then it returns `{ usage: 0, temperature: null }`.
8. Given null temperature, when rendered, then usage remains visible and Celsius is hidden.
9. Given temperature 85°C, when rendered, then the CPU is hot/red; given 84°C, then it is normal.
10. Given a click, when the item is visible, then it invokes `open_app` with exactly `{ name: 'Activity Monitor' }`.
11. Given process lifetime, when a later poll occurs after a transient source failure, then it retries on the same fixed cadence without an additional listener lifecycle.

## Testing Seam

The source-selection helpers (`is_valid_temp`, SMC/shell parsing), `get_cpu_info_blocking`, and `useCpu` are stable seams. Existing tests are `app/native/src/modules/bar/components/cpu.rs:259-488` and `app/ui/renderer/bar/Status/Cpu/Cpu.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                      | Implementation evidence                                             | Test evidence                                                                                                                                                                                        | Intended documentation                                    | Disposition |
| --------------------------------------------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | ----------- |
| Result, blocking fallback, global usage, and rounding                 | `app/native/src/modules/bar/components/cpu.rs:16-59`                | `cpu.rs` — `test_cpu_info_creation`; `test_cpu_info_default`; `test_get_cpu_info_command_awaits_blocking_computation`; `test_get_cpu_usage`                                                          | `status-bar.md:105-109`                                   | Aligned     |
| Temperature ordering and strict range                                 | `app/native/src/modules/bar/components/cpu.rs:61-184,231-257`       | `cpu.rs` — `test_get_smc_cpu_temperature`; `test_parse_ismc_cpu_temps`; `test_parse_ismc_cpu_temps_no_cpu_sensors`; `test_is_valid_temp`; `test_is_valid_temp_boundary_values`; `test_average_temps` | `status-bar.md:105-109`; `getting-started.md:21-22,95-96` | Aligned     |
| Two-second UI polling, null/hot rendering, and Activity Monitor click | `app/ui/renderer/bar/Status/Cpu/Cpu.state.ts:11-51`; `Cpu.tsx:7-17` | `Cpu.test.tsx` — `renders cpu usage`; `renders cpu usage with temperature`; `renders zero usage when no data`; `renders high temperature cpu info`                                                   | `status-bar.md:105-109,136-151`                           | Aligned     |
