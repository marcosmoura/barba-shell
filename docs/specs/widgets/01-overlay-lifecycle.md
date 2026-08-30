# Widgets 01 — Overlay Lifecycle

> Status: 🟡 Draft · Open: WG2

## Purpose

Calendar, Battery, and Weather render one at a time in the non-focusable `widgets` overlay positioned beneath their bar trigger; toggling while any widget remains active closes it first.

## Scope

- Widget toggle/click-outside handling, overlay state, animation, frame placement, and native click monitoring.

### Out of Scope

| Excluded concern                   | Owner                                                                           | Boundary note                              |
| ---------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------ |
| Calendar content and navigation    | [bar/08 clock and calendar](../bar/08-clock-calendar.md)                        | Overlay selects lazily loaded content.     |
| Battery data and bar trigger       | [bar/05 battery item](../bar/05-battery-item.md)                                | Overlay only receives `battery`.           |
| Weather data and bar trigger       | [bar/07 weather data](../bar/07-weather-data.md)                                | Overlay only receives `weather`.           |
| Bar frame semantics                | [bar/02 geometry and menu visibility](../bar/02-geometry-menubar-visibility.md) | Overlay queries its current frame.         |
| Widget event transport declaration | [foundation/06 frontend events](../foundation/06-frontend-events.md)            | This spec owns event handling and effects. |

## Terminology

- **Active widget** — `calendar`, `battery`, `weather`, or null.
- **Trigger rectangle** — `{ x, y, width, height }` captured from the bar control.
- **Close generation** — the one monotonic `closeGenerationRef` used to invalidate stale open/close continuations.
- **Direct switch** — replacing one active widget with another in a single toggle.

## Data Contract

`stache://widgets/toggle` is declared with `{ name: 'calendar' | 'battery' | 'weather', rect: { x, y, width, height } }`; no current source emitter was found. `stache://widgets/click-outside` has a unit payload and is emitted by the native monitor when a visible widgets window receives an outside left/right/other mouse-down.

The hook owns `activeWidget`, `triggerRect`, `isAnimatingIn`, one content ref, and exactly one `closeGenerationRef`. It displays at most one widget. `activeWidget` and `triggerRect` clear only after a current close hides the window.

After a non-zero content ResizeObserver size, width/height are set to the border-box size; X is `clamp(triggerRect.x, 0, barWidth - contentWidth) + barX`; Y is `barY + barHeight + 4`; size is requested before position. Resize handling is debounced 4.17 ms.

## Configuration Contract

None. Native initialization obtains the `widgets` window, makes it sticky and always on top, starts click monitoring, and initializes widget components. The overlay depends on the bar frame query rather than an independent geometry setting.

## Inputs

- Targeted widgets toggle and click-outside events.
- `get_bar_window_frame` result.
- ResizeObserver dimensions.
- Global left/right/other mouse events plus widget-window visibility/frame.
- React unmount.

## State Transitions

| From           | Input                               | To             | Effect                                                                 |
| -------------- | ----------------------------------- | -------------- | ---------------------------------------------------------------------- |
| Closed         | toggle(config)                      | Opening        | Increment generation; set active widget/rect; call `show`.             |
| Opening        | current `show` resolves             | Open           | Set enter animation true.                                              |
| Opening/Open   | toggle(any)                         | Closing        | Increment generation; set enter false; delay the slow spring duration. |
| Opening/Open   | click outside                       | Closing        | Same close flow.                                                       |
| Closing        | current delay resolves              | Hidden attempt | Call `hide`.                                                           |
| Hidden attempt | current hide resolves               | Closed         | Clear active widget/rect.                                              |
| Any            | newer open/close/unmount generation | Superseded     | Older continuation returns without state mutation.                     |

A toggle received while `activeWidget` is non-null invokes `closeWidget` regardless of requested name. Thus selecting a different widget is a two-action workflow: first toggle closes; only a later toggle after close has cleared state opens the requested widget. A toggle during a close ordinarily sees the still-active widget and starts another close; it does not directly replace the content.

## Outputs

- `show`, `hide`, `setSize`, and `setPosition` requests to the `widgets` webview.
- Lazy Calendar/Battery/Weather content and enter/exit animation state.
- Native `stache://widgets/click-outside` emission when its current conditions hold.

## Derived Effects

The overlay uses its own React Query client/error boundary and lazily imports widget content. The native monitor runs a listen-only Core Graphics event tap on a background thread/run loop. There is no native toggle emitter in the current source; frontend bar controls are the consumer-side event boundary.

## Failure & Recovery

Without a bar-frame result, open/close/frame updates return without effect; the renderer error boundary owns query retry. Zero content dimensions cause no frame operation. A stale close cannot clear a newer open because generation checks guard after the delay and hide. Click-monitor creation/run-loop failures log and leave bar toggles usable; the monitor has no stop/unregister operation and is process-lifetime.

## Cross-Module Contracts

[bar/02](../bar/02-geometry-menubar-visibility.md) returns bar geometry. [bar/05](../bar/05-battery-item.md), [bar/07](../bar/07-weather-data.md), and [bar/08](../bar/08-clock-calendar.md) supply named toggle consumers. [foundation/06](../foundation/06-frontend-events.md) owns declaration/transport.

## Acceptance Scenarios

1. Given Closed and a Calendar toggle, when the bar frame exists, then Calendar becomes active, the window shows, and enter animation begins only after the current `show` resolves.
2. Given any active widget and a same or different toggle, when it arrives, then the active widget starts closing and the payload does not replace it in that action.
3. Given close completion and a later different-widget toggle, when it arrives after state clears, then that widget opens.
4. Given close begins while show awaits, when show later resolves, then the stale open does not enable enter animation.
5. Given repeated close requests, when generations advance, then only the current close may hide and clear state.
6. Given trigger geometry near the right edge and non-zero content size, when resized, then X clamps within bar width, Y is 4 px below the bar, and size precedes position.
7. Given zero dimensions or no bar frame, when resize/open/close work runs, then no overlay-frame effect is requested.
8. Given a visible widgets window and an outside left/right/other click, when the native monitor receives it, then click-outside is emitted and the hook closes.
9. Given hidden window or inside click, when the monitor receives it, then no click-outside emission is expected.
10. Given process exit, when the monitor remains process-lifetime, then this spec does not promise stop, join, or in-process restart.

## Testing Seam

`useWidgets` transition/generation logic and frame calculation are frontend seams; native `WindowFrame::contains` and the event-tap callback are native seams. Existing tests: `app/ui/renderer/widgets/Widgets.state.test.tsx`, `Widgets.test.tsx`, and `app/native/src/modules/widgets/window.rs:225-304`.

## Open Decisions

| ID  | Current behavior                                                                                                                                                                                                                     | Documented intent                                                                                                                                                | Rewrite consequence                                                                                                               | Evidence                                                                    |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| WG2 | While `activeWidget` remains set, every toggle calls `closeWidget`; a different name opens only on a later toggle after close completes. `closeGenerationRef` cancels stale continuations but does not implement direct replacement. | `widgets.md:21-22` says a new open during close takes precedence, while `widgets.md:65-69` says direct switching is not implemented and requires two selections. | Retain the two-action current contract; resolve the contradictory intended wording before specifying one-action direct switching. | `app/ui/renderer/widgets/Widgets.state.ts:90-163`; `widgets.md:15-22,65-69` |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                                         | Implementation evidence                                          | Test evidence                                                                                                                                                                                                                                                                                                                                            | Intended documentation   | Disposition |
| ------------------------------------------------------------------------ | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ | ----------- |
| Window initialization and click-outside monitor                          | `app/native/src/modules/widgets/mod.rs:9-27`; `window.rs:88-223` | `window.rs` — `window_frame_contains_point_inside`; `window_frame_does_not_contain_point_outside`; `event_constants_are_valid`; `tap_constants_are_valid`                                                                                                                                                                                                | `widgets.md:15-20,65-72` | Aligned     |
| Toggle state, one generation counter, placement, and two-action behavior | `app/ui/renderer/widgets/Widgets.state.ts:32-205`                | `Widgets.state.test.tsx` — `opens a widget and shows the window on toggle`; `closes the widget after the exit animation`; `rapid double close collapses into a single hide`; `reopens a widget after a full close cycle`; `a close superseding a pending open does not trigger the enter animation`; `passes a stable resize callback across re-renders` | `widgets.md:17-22,65-69` | Conflict    |
| Lazy content/error boundary                                              | `app/ui/renderer/widgets/Widgets.tsx:1-55`                       | `Widgets.test.tsx` — `renders a retry fallback on transient query error and recovers`; `renders the calendar widget when opened`; `renders the battery widget when opened`; `renders the weather widget when opened`                                                                                                                                     | `widgets.md:65-69`       | Aligned     |
