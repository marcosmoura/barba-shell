# Status Bar 08 — Clock and Calendar

> Status: 🟡 Draft · Open: CL2

## Purpose

The Clock item formats local time once per second and toggles the Calendar; Calendar renders a Sunday-first local month grid with navigation and a midnight-refreshed today marker.

## Scope

- Clock formatting/timer and Calendar content/navigation state.

### Out of Scope

| Excluded concern                      | Owner                                                                    | Boundary note                                   |
| ------------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------- |
| Overlay lifecycle and transition race | [widgets/01 overlay lifecycle](../widgets/01-overlay-lifecycle.md)       | This spec supplies Calendar content only.       |
| Bar composition                       | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)                | Status owns Clock placement.                    |
| Clock app launch                      | [foundation/10 application shell](../foundation/10-application-shell.md) | The Clock item toggles Calendar, not Clock.app. |

## Terminology

- **Displayed month** — the `currentDate` month used for the grid.
- **Today snapshot** — local `Date` refreshed at the next local midnight.

## Data Contract

The Clock formats a local `Date` through `Intl.DateTimeFormat('en-US', { hour12: false, weekday: 'short', month: 'short', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })` as `weekday month day hour:minute:second`.

Calendar state contains displayed date, today, and animation direction (`left` previous / `right` next). Its grid fills leading and trailing outside-month cells to complete Sunday-first weeks; `weekCount = ceil(days.length / 7)`. Outside-month cells are never today. Month label uses `en-US` long month and numeric year.

## Configuration Contract

None. Locale, local time zone, seconds, and Sunday-first grid are fixed by the code.

## Inputs

- Browser local time and one-second refetch interval.
- Previous, next, and Today controls.
- Clock click/trigger rectangle.

## State Transitions

| From                | Input                  | To                      | Effect                                                     |
| ------------------- | ---------------------- | ----------------------- | ---------------------------------------------------------- |
| Clock mount         | current Date           | Clock ready             | Format now; refetch every second.                          |
| Calendar month      | Previous               | Earlier request         | Set left then call `Date.setMonth(month - 1)`.             |
| Calendar month      | Next                   | Later request           | Set right then call `Date.setMonth(month + 1)`.            |
| Any displayed month | Today                  | Current local month     | Set direction from year/month comparison and replace date. |
| Mounted calendar    | local midnight timeout | Same display, new today | Replace today and schedule next timeout.                   |
| Clock visible       | click                  | Same                    | Toggle Calendar overlay.                                   |

## Outputs

The Clock renders one string or nothing when it is falsy. Calendar emits no backend event itself; its trigger uses the widgets toggle contract.

## Derived Effects

Clock uses React Query's 1000 ms `refetchInterval`. Calendar uses one `setTimeout` to the computed next local midnight and clears it on dependency change/unmount.

## Failure & Recovery

Missing `Intl` parts are concatenated as empty segments. Browser timer suspension delays a refresh until the next callback. `Date.setMonth` is not clamped: dates 29–31 can overflow a shorter target month and skip the expected month.

## Cross-Module Contracts

[widgets/01](../widgets/01-overlay-lifecycle.md) receives the Clock toggle and renders lazily loaded Calendar content. [foundation/06](../foundation/06-frontend-events.md) owns transport. [bar/01](01-bar-window-lifecycle.md) owns placement.

## Acceptance Scenarios

1. Given local Wednesday 2026-08-23 14:05:09, when Clock formats, then it uses `Wed Aug 23 14:05:09`.
2. Given a mounted Clock, when one second elapses, then it reads and formats current time again.
3. Given a Clock click, when it occurs, then Calendar receives the widget toggle.
4. Given a month beginning Wednesday, when Calendar builds its grid, then Sunday–Tuesday leading cells and a complete final week are present.
5. Given normal day-of-month navigation, when Previous/Next is clicked, then direction is left/right and the selected month changes.
6. Given a 29th–31st crossing into a shorter month, when `setMonth` overflows, then the current behavior is not described as clamped.
7. Given local midnight while mounted, when the timeout fires, then the today marker refreshes.
8. Given an outside-month matching day number, when rendered, then it is not today.
9. Given overlay close/reopen, then transition behavior remains owned by widgets/01.

## Testing Seam

Clock formatter, month-grid generation, `calculateMonthHeight`, and Calendar hook state are stable seams. Existing tests: `app/ui/renderer/bar/Status/Clock/Clock.test.tsx` and `app/ui/renderer/widgets/components/Calendar/Calendar.test.tsx`.

## Open Decisions

| ID  | Current behavior                                                                                                 | Documented intent                                                                             | Rewrite consequence                                                                                               | Evidence                                                                                  |
| --- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| CL2 | `goToPreviousMonth` and `goToNextMonth` copy the current date and call `setMonth` directly; no day clamp exists. | `widgets.md:31-36` promises month navigation but does not specify its end-of-month semantics. | Do not claim exact one-month navigation at dates 29–31 until a clamped rule is implemented or explicitly adopted. | `app/ui/renderer/widgets/components/Calendar/Calendar.state.ts:79-95`; `widgets.md:31-36` |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                  | Implementation evidence                                                       | Test evidence                                                                                                                                                                        | Intended documentation  | Disposition |
| --------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------- | ----------- |
| Clock format, cadence, and toggle | `app/ui/renderer/bar/Status/Clock/Clock.state.ts:7-45`; `Clock.tsx:9-22`      | `Clock.test.tsx` — `renders clock time`; `renders formatted date and time`                                                                                                           | `status-bar.md:132-134` | Aligned     |
| Grid, today refresh, and height   | `app/ui/renderer/widgets/components/Calendar/Calendar.state.ts:15-77,113-146` | `Calendar.test.tsx` — `renders the current month with weekday headers`; `calculateMonthHeight accounts for day height and row gaps`; `refreshes the today highlight across midnight` | `widgets.md:31-36`      | Aligned     |
| End-of-month navigation           | `app/ui/renderer/widgets/components/Calendar/Calendar.state.ts:79-95`         | `Calendar.test.tsx` — `navigates to the next month`; `navigates to the previous month`                                                                                               | `widgets.md:31-36`      | Conflict    |
