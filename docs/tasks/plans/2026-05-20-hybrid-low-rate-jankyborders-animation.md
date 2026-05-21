# Hybrid Low-Rate JankyBorders Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep JankyBorders-rendered animated gradient borders while reducing animation queue pressure so focus and layout color changes are processed quickly.

**Architecture:** Keep semantic focus/layout updates on the existing reliable JankyBorders command path. Keep animation frames on the existing Mach-only best-effort path, but send them at 8fps and pause them for 150ms after each semantic update. The animation runner remains the single writer so newer focus/layout updates interrupt active animation immediately.

**Tech Stack:** Rust, Tauri native module, macOS Mach IPC, JankyBorders, `cargo test`, `cargo clippy`.

---

## File Structure

- Modify: `app/native/src/modules/tiling/borders.rs`
  - Owns JankyBorders IPC, animation runner state, animation frame sending, and unit tests.
  - Add low-rate timing constants and helper functions near the existing constants/animation runner.
  - Update `run_animation()` to wait for the focus-priority pause before sending the first animation frame and to use the low-rate frame interval.
  - Add unit tests in the existing `#[cfg(test)] mod tests` section.

No other files are needed for this implementation.

---

### Task 1: Add Low-Rate Animation Timing Constants

**Files:**

- Modify: `app/native/src/modules/tiling/borders.rs`
- Test: `app/native/src/modules/tiling/borders.rs`

- [ ] **Step 1: Write the failing test for the low-rate interval**

Add this test near the other border animation tests in `app/native/src/modules/tiling/borders.rs`:

```rust
#[test]
fn test_animation_frame_duration_is_low_rate() {
    assert_eq!(BORDER_ANIMATION_FPS, 8);
    assert_eq!(animation_frame_duration(), Duration::from_millis(125));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests::test_animation_frame_duration_is_low_rate
```

Expected: FAIL to compile because `BORDER_ANIMATION_FPS` and `animation_frame_duration()` do not exist.

- [ ] **Step 3: Add the timing constants and helper**

In `app/native/src/modules/tiling/borders.rs`, add these constants near the existing top-level constants after `JANKY_BORDERS_SERVICE`:

```rust
/// Low-rate animation avoids flooding JankyBorders' FIFO Mach queue.
const BORDER_ANIMATION_FPS: u64 = 8;
const BORDER_ANIMATION_FRAME_DURATION_MS: u64 = 1_000 / BORDER_ANIMATION_FPS;

fn animation_frame_duration() -> Duration {
    Duration::from_millis(BORDER_ANIMATION_FRAME_DURATION_MS)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests::test_animation_frame_duration_is_low_rate
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add app/native/src/modules/tiling/borders.rs
git commit -m "fix(borders): define low-rate animation timing"
```

---

### Task 2: Add Focus/Layout Priority Pause

**Files:**

- Modify: `app/native/src/modules/tiling/borders.rs`
- Test: `app/native/src/modules/tiling/borders.rs`

- [ ] **Step 1: Write the failing tests for the priority pause**

Add these tests near `test_animation_wait_returns_queued_command_before_next_frame`:

```rust
#[test]
fn test_focus_priority_pause_duration_is_short() {
    assert_eq!(focus_priority_pause_duration(), Duration::from_millis(150));
}

#[test]
fn test_focus_priority_pause_returns_queued_command() {
    let (tx, rx) = mpsc::channel();
    tx.send(AnimationCommand::Update {
        args: vec!["active_color=0xFFFF0000".to_string()],
        animation: None,
    })
    .unwrap();

    let command = wait_for_focus_priority_pause(&rx).expect("queued command should interrupt pause");

    let AnimationCommand::Update { args, animation } = command;
    assert_eq!(args, vec!["active_color=0xFFFF0000".to_string()]);
    assert!(animation.is_none());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests::test_focus_priority_pause
```

Expected: FAIL to compile because `focus_priority_pause_duration()` and `wait_for_focus_priority_pause()` do not exist.

- [ ] **Step 3: Add the priority pause constants and helper**

Add this constant near the low-rate constants:

```rust
const BORDER_FOCUS_PRIORITY_PAUSE_MS: u64 = 150;
```

Add this helper near `animation_frame_duration()`:

```rust
fn focus_priority_pause_duration() -> Duration {
    Duration::from_millis(BORDER_FOCUS_PRIORITY_PAUSE_MS)
}
```

Add this helper below `wait_for_animation_command()`:

```rust
fn wait_for_focus_priority_pause(rx: &mpsc::Receiver<AnimationCommand>) -> Option<AnimationCommand> {
    wait_for_animation_command(rx, focus_priority_pause_duration())
        .ok()
        .flatten()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests::test_focus_priority_pause
```

Expected: PASS for both priority pause tests.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add app/native/src/modules/tiling/borders.rs
git commit -m "fix(borders): add focus priority animation pause"
```

---

### Task 3: Apply Low-Rate Animation and Pre-Send Interruption in the Runner

**Files:**

- Modify: `app/native/src/modules/tiling/borders.rs`
- Test: `app/native/src/modules/tiling/borders.rs`

- [ ] **Step 1: Write the failing regression test for pre-send interruption**

Add this test near the other animation runner tests:

```rust
#[test]
fn test_take_queued_animation_command_returns_pending_update() {
    let (tx, rx) = mpsc::channel();
    tx.send(AnimationCommand::Update {
        args: vec!["active_color=0xFFFF0000".to_string()],
        animation: None,
    })
    .unwrap();

    let command = take_queued_animation_command(&rx).expect("queued command should be returned");

    let AnimationCommand::Update { args, animation } = command;
    assert_eq!(args, vec!["active_color=0xFFFF0000".to_string()]);
    assert!(animation.is_none());
}
```

This test should fail to compile because `take_queued_animation_command()` does not exist yet.

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests::test_take_queued_animation_command_returns_pending_update
```

Expected: FAIL to compile because `take_queued_animation_command()` does not exist.

- [ ] **Step 3: Add the pre-send interruption helper**

Add this helper near `wait_for_focus_priority_pause()`:

```rust
fn take_queued_animation_command(
    rx: &mpsc::Receiver<AnimationCommand>,
) -> Option<AnimationCommand> {
    rx.try_recv().ok()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests::test_take_queued_animation_command_returns_pending_update
```

Expected: PASS.

- [ ] **Step 5: Update `run_animation()` to use the low-rate interval, priority pause, and pre-send interruption**

In `app/native/src/modules/tiling/borders.rs`, replace this line in `run_animation()`:

```rust
let frame_duration = Duration::from_millis(16);
```

with:

```rust
let frame_duration = animation_frame_duration();
```

Then insert this block after `let mut start = Instant::now();` and before `loop {`:

```rust
if let Some(cmd) = wait_for_focus_priority_pause(rx) {
    return Some(cmd);
}
```

Replace the existing top-of-loop check:

```rust
if let Ok(cmd) = rx.try_recv() {
    return Some(cmd);
}
```

with:

```rust
if let Some(cmd) = take_queued_animation_command(rx) {
    return Some(cmd);
}
```

Finally, move the command check as close as possible to frame sending. The loop body should contain this ordering around the frame send:

```rust
let active_color = animated_gradient_color(&from, &to, angle, progress);
if let Some(cmd) = take_queued_animation_command(rx) {
    return Some(cmd);
}
let _ = send_animation_frame(&[format!("active_color={active_color}")]);
```

The pre-send `try_recv()` prevents Stache from enqueueing one more stale animation frame if a focus/layout command arrived during frame computation.

- [ ] **Step 6: Run border tests**

Run:

```bash
cargo test -p stache --lib modules::tiling::borders::tests
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
git add app/native/src/modules/tiling/borders.rs
git commit -m "fix(borders): throttle jankyborders animation frames"
```

---

### Task 4: Full Verification

**Files:**

- Verify: full repo native checks

- [ ] **Step 1: Format Rust code**

Run:

```bash
cargo fmt -p stache
```

Expected: exits successfully with no output or only normal formatter output.

- [ ] **Step 2: Run clippy**

Run:

```bash
cargo clippy -p stache --lib --tests -- -D warnings
```

Expected: exits successfully with `Finished` and no warnings or errors.

- [ ] **Step 3: Run native tests**

Run:

```bash
pnpm test:native
```

Expected: all tests pass. The expected count should be at least the current `1016` tests plus any new tests added by this plan.

- [ ] **Step 4: Inspect the final diff**

Run:

```bash
git status --short --branch
git diff -- app/native/src/modules/tiling/borders.rs
```

Expected: only intentional changes in `app/native/src/modules/tiling/borders.rs` remain uncommitted.

- [ ] **Step 5: Commit verification cleanup if needed**

If formatting changed files after the previous task commits, run:

```bash
git add app/native/src/modules/tiling/borders.rs
git commit -m "chore(borders): format hybrid animation changes"
```

If there are no formatting changes, do not create an empty commit.

---

## Manual Verification Checklist

- [ ] Run `pnpm tauri:dev`.
- [ ] Use a config with focused animated gradient borders.
- [ ] Switch focus repeatedly between windows for at least one minute.
- [ ] Switch between focused, floating, and monocle border states.
- [ ] Leave the app running for several minutes and confirm focus/layout switching does not get progressively slower.
- [ ] Run with `RUST_LOG=stache=debug` and confirm there are no repeated `FAILED to send border command` warnings for semantic focus/layout updates.

---

## Self-Review Notes

- Spec coverage: low-rate frames are Task 1 and Task 3; best-effort animation sends are already implemented and protected by existing tests; focus/layout priority pause is Task 2 and Task 3; full verification is Task 4.
- Placeholder scan: no `TBD`, `TODO`, or open-ended implementation steps remain.
- Type consistency: all new helpers use existing `Duration`, `mpsc::Receiver<AnimationCommand>`, and existing `wait_for_animation_command()` return types.
