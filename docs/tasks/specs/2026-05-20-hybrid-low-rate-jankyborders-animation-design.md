# Hybrid Low-Rate JankyBorders Animation Design

## Context

Stache uses JankyBorders for window border rendering and sends runtime border updates through JankyBorders' Mach IPC service. Animated gradient borders currently work by streaming per-frame `active_color` updates to JankyBorders.

That creates a FIFO queue conflict: focus and layout color changes share the same JankyBorders Mach queue as animation frames. Once animation frames are accepted by JankyBorders, Stache cannot remove, replace, or reprioritize them. A focus or layout update can therefore wait behind stale animation frames.

JankyBorders must remain the renderer, and JankyBorders cannot be patched.

## Goal

Keep animated gradient borders while reducing queue pressure enough that focus and layout border changes feel immediate in normal use.

## Non-Goals

- Do not disable animated gradient borders.
- Do not patch or fork JankyBorders.
- Do not replace JankyBorders with a Stache-owned border renderer.
- Do not use the JankyBorders CLI for animation frames.
- Do not chase 60fps border animation through Mach IPC.

## Design

### Low-Rate Animation Frames

Animation frames will be sent at a low fixed rate instead of every 16ms. The initial target rate is 8fps, which means one frame every 125ms.

This lowers maximum animation enqueue pressure from about 62.5 messages per second to 8 messages per second.

### Best-Effort Animation Sends

Animation frame sends remain Mach-only and best-effort:

- Use a zero-millisecond Mach send timeout.
- If the frame cannot be queued immediately, drop it.
- Do not reconnect Mach for animation frames.
- Do not fall back to the JankyBorders CLI for animation frames.
- Do not cache failed animation frames as successfully sent.

The animation loop computes the next frame from elapsed time, not from frame count, so dropped frames skip visual samples without slowing animation progress.

### Focus/Layout Priority Window

After every focus or layout border update, animation will pause briefly before resuming. The initial pause is 150ms.

During this window:

- The focus/layout command is sent through the reliable command path.
- No animation frames are sent.
- Any newly queued focus/layout command replaces older pending animation work before animation resumes.

This gives JankyBorders time to process the semantic color switch before receiving more animated `active_color` updates.

### Reliable Semantic Commands

Focus and layout changes remain semantic one-shot commands containing width, active color, and inactive color.

These commands continue to use the reliable command path:

- Try existing Mach port.
- Reconnect and retry Mach if needed.
- Fall back to the CLI only for semantic commands, not animation frames.

### Animation Restart Behavior

When a focus/layout update selects an animated gradient:

- Send the static gradient immediately as the semantic command.
- Reset animation timing for that selected gradient.
- Pause for the priority window.
- Resume low-rate animation from the new gradient.

When a focus/layout update selects a non-animated state:

- Send the semantic command.
- Stop animation until another animated state is selected.

## Data Flow

1. `on_focus_changed()` determines the active border state for layout and floating state.
2. It queues an `AnimationCommand::Update` containing the semantic command and optional animation configuration.
3. The animation runner drains stale queued updates and processes only the latest update.
4. The runner sends the semantic command reliably.
5. If animation is present, the runner waits for the focus/layout priority pause.
6. The runner starts low-rate best-effort animation frames.
7. Any new command wakes the runner immediately and replaces the current animation state.

## Constants

Initial constants:

- `BORDER_ANIMATION_FPS = 8`
- `BORDER_ANIMATION_FRAME_MS = 125`
- `BORDER_FOCUS_PRIORITY_PAUSE_MS = 150`
- `MACH_FRAME_SEND_TIMEOUT_MS = 0`

These are implementation constants, not user-facing config, unless later tuning shows users need control.

## Expected Behavior

- Border gradients still animate continuously when the selected border state has animation enabled.
- Animation is less fluid than 60fps but should remain visibly alive.
- Window and layout switches should update border colors much faster because JankyBorders receives far fewer stale frames.
- The fix is a mitigation, not a hard real-time guarantee, because Stache cannot remove frames already accepted into JankyBorders' FIFO queue.

## Testing

Unit tests should cover:

- Animation frame interval uses the low-rate value instead of 16ms.
- Animation waits can be interrupted immediately by queued focus/layout commands.
- Animation frame failures are dropped and not cached.
- Semantic command processing runs before animation resumes after an update.
- The priority pause is applied before the first animation frame after a focus/layout update.

Manual verification should cover:

- Focus switching between windows while animation is active.
- Switching between focused, monocle, and floating border states.
- Long-running animation without progressively increasing delay.
- Debug logs should not show repeated failed semantic border commands.
