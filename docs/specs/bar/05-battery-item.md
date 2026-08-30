# Status Bar 05 — Battery Item

> Status: ✅ Normative

## Purpose

The Battery item presents the current system battery when one is available and opens the Battery widget on click.

## Scope

- `get_battery_info` payload/error contract and bar projection.

### Out of Scope

| Excluded concern                 | Owner                                                                    | Boundary note                          |
| -------------------------------- | ------------------------------------------------------------------------ | -------------------------------------- |
| Battery detail panel and overlay | [widgets/01 overlay lifecycle](../widgets/01-overlay-lifecycle.md)       | The item only toggles `battery`.       |
| Bar composition                  | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)                | Status owns placement.                 |
| Battery settings launch          | [foundation/10 application shell](../foundation/10-application-shell.md) | The current bar item does not open it. |

## Terminology

- **Absent battery** — no `BatteryInfo` snapshot is in the frontend store.

## Data Contract

`get_battery_info` takes no frontend argument and returns `Result<BatteryInfo, StacheError>`. `BatteryInfo` includes percentage (0–100), state, health, technology, energy, full/design capacities, rate, voltage, optional temperature/cycle/time fields, and optional vendor/model/serial. States are `Unknown`, `Charging`, `Discharging`, `Empty`, and `Full`.

The bar hides when `percentage == null`; it does not render a no-battery loading state. Labels are `Loading...` for incomplete state, `100%` for Full, percentage-only for Unknown or compact mode, otherwise `percentage% (state)`. Charging is green, Discharging yellow, Empty red, otherwise text color.

## Configuration Contract

None.

## Inputs

- Battery store snapshots supplied by the application store.
- A click with a captured trigger rectangle.

## State Transitions

| From          | Input                | To      | Effect                            |
| ------------- | -------------------- | ------- | --------------------------------- |
| Absent        | no percentage        | Hidden  | Render nothing.                   |
| Snapshot      | state/percentage     | Visible | Derive icon, label, and color.    |
| Visible       | click                | Visible | Emit/toggle the Battery widget.   |
| Query failure | store remains absent | Hidden  | Preserve no-battery presentation. |

## Outputs

The command serializes `BatteryInfo` or an observable `BatteryError`; the bar produces a widget-toggle request and no direct Battery settings launch.

## Derived Effects

The backend constructs a `starship_battery::Manager`, reads its first battery, and converts units to the public payload.

## Failure & Recovery

Manager initialization, enumeration, first-entry, and read failures return `BatteryError`. The item remains hidden without a store percentage. The store cadence is owned by `app/ui/stores/BatteryStore`, not this renderer; no item-specific retry is declared.

## Cross-Module Contracts

[widgets/01](../widgets/01-overlay-lifecycle.md) owns opening/closing. [bar/01](01-bar-window-lifecycle.md) owns composition. [foundation/06](../foundation/06-frontend-events.md) owns widget-event transport.

## Acceptance Scenarios

1. Given a valid charging battery, when projected, then its label/color reflect percentage and Charging.
2. Given Full, when projected, then the label is `100%`.
3. Given laptop mode, when a percentage exists, then the label is percentage-only.
4. Given no battery, when the backend reports its documented error, then the bar item is hidden.
5. Given a click, when the item is visible, then it toggles the Battery widget.
6. Given store update cadence changes, when this component renders, then it consumes the supplied snapshot rather than scheduling another poll.
7. Given process exit, then no independent battery listener teardown is required by this item.

## Testing Seam

The Rust conversion and label/color functions are stable seams. Existing tests: `app/native/src/modules/bar/components/battery.rs:152-259`, `app/ui/renderer/bar/Status/Battery/Battery.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                       | Implementation evidence                                                        | Test evidence                                                                                                                                                                                                         | Intended documentation                      | Disposition |
| -------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | ----------- |
| Command fields and errors              | `app/native/src/modules/bar/components/battery.rs:12-146`                      | `battery.rs` — `percentage_from_ratio_clamps_and_rounds`; `battery_state_from_state_matches_variants`; `battery_info_default`                                                                                         | `status-bar.md:111-119`                     | Aligned     |
| Absence, label/color, and widget click | `app/ui/renderer/bar/Status/Battery/Battery.state.ts:8-51`; `Battery.tsx:9-22` | `Battery.test.tsx` — `renders battery info when available`; `renders nothing when battery percentage is not available`; `renders full battery label`; `renders charging battery label`; `renders empty battery state` | `status-bar.md:111-119`; `widgets.md:38-49` | Aligned     |
