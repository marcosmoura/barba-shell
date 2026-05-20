# Focused Border Animation

## Problem

Stache's tiling border integration currently supports JankyBorders gradients, but focused window borders do not reliably change when switching apps or workspaces. The current border module builds one space-delimited command string and sends it over Mach IPC. JankyBorders 1.9 expects argv-style, NUL-separated arguments, so runtime color updates can be parsed incorrectly.

Users also need focused-window border animations for gradient colors. The animation should work around JankyBorders' limited gradient direction support by animating the two configured gradient colors instead of rotating the gradient angle.

## Goals

- Fix runtime border updates for app focus, workspace focus, layout changes, and floating changes.
- Add focused-window-only color animation for gradient border states.
- Preserve existing static border behavior for solid colors, glow colors, and gradients without animation.
- Keep unfocused borders static.
- Preserve existing `focused`, `unfocused`, `monocle`, and `floating` configuration behavior.
- Allow optional focused layout overrides for layouts beyond the existing `monocle` and `floating` fields.

## Non-Goals

- Do not implement a custom Stache border renderer.
- Do not animate unfocused borders.
- Do not animate solid or glow border states.
- Do not require users to configure animation for gradients.
- Do not change JankyBorders itself.

## Border Command Protocol

Border commands should be represented internally as argv-style argument lists, not shell command strings.

Example arguments:

```text
width=6
active_color=gradient(top_left=0xFFFF0000,bottom_right=0xFF0000FF)
inactive_color=0x00000000
```

Mach IPC payloads must encode those arguments as NUL-separated strings with a final NUL terminator:

```text
width=6\0active_color=...\0inactive_color=...\0\0
```

The CLI fallback should pass the same arguments via `Command::args(args)`. It should not split a shell-style string with whitespace.

## Configuration

Gradient border states may include an optional `animation` property:

```jsonc
{
  "width": 6,
  "gradient": {
    "from": "#cba6f7",
    "to": "#a6e3a1",
    "angle": 180,
  },
  "animation": {
    "duration": 350,
    "easing": "ease-out-expo",
  },
}
```

`animation.duration` is the duration in milliseconds for one color-swap leg. With `duration: 350`, color A to color B takes 350ms, then color B to color A takes another 350ms.

`animation.easing` should reuse the existing tiling `EasingType` values: `linear`, `ease-in`, `ease-out`, `ease-in-out`, `ease-out-expo`, and `spring`. If `spring` is used for border color animation, it should be treated as linear because the existing spring implementation is tied to window geometry physics.

Passing `animation` to a solid or glow border state has no runtime effect. Serde may ignore unknown fields for those variants; the generated schema should only advertise `animation` for gradient states.

## Focused Border Selection

Only the focused window's active border color can animate.

The focused border state is selected in this order:

1. If the focused window is floating or the focused workspace layout is floating, use `floating` when enabled.
2. If the focused workspace layout has an enabled layout-specific border config, use it.
3. Otherwise, use `focused`.

Existing behavior must remain valid:

```jsonc
{
  "borders": {
    "enabled": true,
    "style": "round",
    "hidpi": true,
    "unfocused": false,
    "focused": {
      "width": 6,
      "gradient": {
        "from": "#cba6f7",
        "to": "#a6e3a1",
        "angle": 180,
      },
    },
    "monocle": {
      "width": 6,
      "gradient": {
        "from": "#f38ba8",
        "to": "#fab387",
        "angle": 180,
      },
    },
  },
}
```

In this example, focused monocle windows use the red/orange `monocle` gradient. Any other focused window uses the purple/green `focused` gradient.

Optional focused layout override fields should be added for layouts that are not currently configurable at the top level: `dwindle`, `master`, `grid`, `split`, `splitVertical`, and `splitHorizontal`. These fields should default to `None` so existing configs keep their current behavior.

## Animation Behavior

Animated gradients interpolate both gradient colors in opposite directions:

```text
animatedFrom = lerp(from, to, easedProgress)
animatedTo = lerp(to, from, easedProgress)
```

Each animation frame sends a new active border color:

```text
active_color=gradient(...animatedFrom..., ...animatedTo...)
```

The animation loop should run continuously while the selected focused border state remains the active state. It must cancel and restart when focus, workspace, layout mode, floating state, or border configuration changes.

Static behavior:

- Gradient without `animation` sends one static gradient update.
- Solid color sends one static active color update.
- Glow sends one static active glow update.
- Unfocused color is included in focus updates but never animated.

## Data Flow

Focus and workspace handlers already notify the effect subscriber through `notify_focus_changed()`. Layout changes notify `notify_workspace_layout_changed()`, and floating changes notify `notify_floating_changed()`.

The subscriber should refresh active border colors for:

- Focus changes.
- Initial focused state after startup.
- Workspace layout changes.
- Floating state changes for the focused window.
- Workspace switches that alter the focused window or active layout.

The border module owns the animation generation counter. Starting any focused border update cancels the previous animation before sending the new static base color and optionally starting a new gradient animation loop.

## Error Handling

- Invalid hex colors should fall back to transparent or no animation, matching the current border color conversion behavior.
- Failed Mach sends should retry after reconnecting to the Mach service once.
- Failed Mach sends should fall back to the `borders` CLI when available.
- Missing `borders` CLI should not prevent Mach communication with an already-running JankyBorders instance.
- Animation threads should exit when their generation is no longer current.

## Tests

- Unit test Mach payload encoding as NUL-separated argv strings.
- Unit test command deduplication key generation.
- Unit test gradient animation config deserialization.
- Unit test solid and glow states return no animation.
- Unit test focused border resolution for monocle vs generic focused fallback.
- Unit test RGBA interpolation.
- Unit test animated gradient color string generation.
- Run native border and config tests.
- Run `pnpm test:native` and `pnpm lint:native` after implementation.

## Documentation

- Update `docs/sample-config.jsonc` with an animated focused gradient example.
- Update the monocle example to show a layout-specific focused gradient override.
- Regenerate `stache.schema.json` with `./scripts/generate-schema.sh` after config type changes.

## Implementation Plan

The detailed implementation plan is saved at:

`docs/tasks/plans/2026-05-20-border-animation.md`
