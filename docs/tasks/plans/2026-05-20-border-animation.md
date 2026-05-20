# Focused Border Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix JankyBorders runtime updates and add focused-window-only animated gradient border colors.

**Architecture:** Border updates will be built as argv-style arguments and encoded as NUL-separated Mach messages so the running `borders` process receives every setting. Focused border configuration will be selected from the active window state: floating override first, then the focused workspace layout override, then the generic `focused` fallback. A cancellable animation loop will interpolate the selected gradient colors only when the selected focused-state config is a gradient with `animation`.

**Tech Stack:** Rust, Tauri, JankyBorders 1.9 Mach IPC, serde, schemars, existing tiling easing utilities.

---

### Task 1: Fix JankyBorders command encoding

**Files:**

- Modify: `app/native/src/modules/tiling/borders.rs:15-263`
- Test: `app/native/src/modules/tiling/borders.rs:419-442`

- [ ] **Step 1: Write failing tests for argv and Mach payload encoding**

Add these tests to the `#[cfg(test)] mod tests` block in `app/native/src/modules/tiling/borders.rs`:

```rust
#[test]
fn test_encode_mach_args_uses_nul_separated_argv() {
    let args = vec![
        "width=6".to_string(),
        "active_color=0xFFFF0000".to_string(),
        "inactive_color=0x00000000".to_string(),
    ];

    let payload = encode_mach_args(&args);

    assert_eq!(
        payload,
        b"width=6\0active_color=0xFFFF0000\0inactive_color=0x00000000\0\0".to_vec()
    );
}

#[test]
fn test_command_key_distinguishes_argument_boundaries() {
    let args = vec!["width=6".to_string(), "active_color=0xFFFF0000".to_string()];

    assert_eq!(command_key(&args), "width=6\0active_color=0xFFFF0000");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p stache --lib modules::tiling::borders::tests::test_encode_mach_args_uses_nul_separated_argv modules::tiling::borders::tests::test_command_key_distinguishes_argument_boundaries -- --show-output`

Expected: compile failure naming missing `encode_mach_args` and `command_key`.

- [ ] **Step 3: Replace string command cache with argv command cache**

Change the cache type in `app/native/src/modules/tiling/borders.rs`:

```rust
/// Last command sent to `JankyBorders` (for deduplication).
static LAST_COMMAND: OnceLock<Mutex<String>> = OnceLock::new();

fn get_last_command() -> &'static Mutex<String> {
    LAST_COMMAND.get_or_init(|| Mutex::new(String::new()))
}
```

Keep the stored value as `String`, but store a NUL-delimited command key instead of a shell-style command line.

- [ ] **Step 4: Add argv helpers**

Add these helpers near the Mach IPC section in `app/native/src/modules/tiling/borders.rs`:

```rust
fn command_key(args: &[String]) -> String { args.join("\0") }

fn encode_mach_args(args: &[String]) -> Vec<u8> {
    let mut payload = Vec::new();
    for arg in args {
        payload.extend_from_slice(arg.as_bytes());
        payload.push(0);
    }
    payload.push(0);
    payload
}
```

- [ ] **Step 5: Change `send_mach` to accept argv args**

Replace `fn send_mach(command: &str) -> bool` with:

```rust
fn send_mach(args: &[String]) -> bool {
    const MACH_MSGH_BITS_COMPLEX: u32 = 0x8000_0000;
    const MACH_MSGH_BITS_COPY_SEND: u32 = 19;
    const MACH_MSG_OOL_DESCRIPTOR: u8 = 1;
    const MACH_MSG_VIRTUAL_COPY: u8 = 1;

    let Some(port) = *get_mach_port().lock() else {
        return false;
    };

    let data = encode_mach_args(args);

    let mut msg = MachMessage {
        header: MachMsgHeader {
            bits: MACH_MSGH_BITS_COMPLEX | MACH_MSGH_BITS_COPY_SEND,
            #[allow(clippy::cast_possible_truncation)]
            size: std::mem::size_of::<MachMessage>() as u32,
            remote_port: port,
            local_port: 0,
            voucher_port: 0,
            id: 0,
        },
        body: MachMsgBody { descriptor_count: 1 },
        descriptor: MachMsgOolDescriptor {
            address: data.as_ptr(),
            deallocate: 0,
            copy: MACH_MSG_VIRTUAL_COPY,
            pad1: 0,
            type_: MACH_MSG_OOL_DESCRIPTOR,
            #[allow(clippy::cast_possible_truncation)]
            size: data.len() as u32,
        },
    };

    let result = unsafe {
        mach_msg(
            &raw mut msg,
            MACH_SEND_MSG,
            msg.header.size,
            0,
            MACH_PORT_NULL,
            MACH_MSG_TIMEOUT_NONE,
            MACH_PORT_NULL,
        )
    };

    result == 0
}
```

- [ ] **Step 6: Change `send_command` to use argv args and direct CLI args**

Replace `fn send_command(command: &str) -> bool` with:

```rust
fn send_command(args: &[String]) -> bool {
    let key = command_key(args);
    {
        let mut last = get_last_command().lock();
        if *last == key {
            return true;
        }
        *last = key;
    }

    if send_mach(args) {
        return true;
    }

    if connect_mach() && send_mach(args) {
        return true;
    }

    Command::new("borders").args(args).output().is_ok_and(|output| output.status.success())
}
```

- [ ] **Step 7: Run tests to verify argv helpers pass**

Run: `cargo test -p stache --lib modules::tiling::borders::tests::test_encode_mach_args_uses_nul_separated_argv modules::tiling::borders::tests::test_command_key_distinguishes_argument_boundaries -- --show-output`

Expected: both tests pass.

- [ ] **Step 8: Commit the IPC fix**

```bash
git add app/native/src/modules/tiling/borders.rs
git commit -m "fix: send borders updates as argv messages"
```

---

### Task 2: Add gradient animation config

**Files:**

- Modify: `app/native/src/config/types/borders.rs:5-180`
- Test: `app/native/src/config/types/borders.rs:360-392`

- [ ] **Step 1: Write failing tests for gradient animation deserialization**

Add these tests to `app/native/src/config/types/borders.rs`:

```rust
#[test]
fn test_gradient_border_deserializes_animation() {
    let json = r##"{
        "width": 6,
        "gradient": {
            "from": "#cba6f7",
            "to": "#a6e3a1",
            "angle": 180
        },
        "animation": {
            "duration": 350,
            "easing": "ease-out-expo"
        }
    }"##;

    let config: BorderStateConfig = serde_json::from_str(json).unwrap();

    let Some(animation) = config.animation() else {
        panic!("expected gradient animation config");
    };
    assert_eq!(animation.duration, 350);
    assert_eq!(animation.easing, super::tiling::EasingType::EaseOutExpo);
}

#[test]
fn test_solid_border_ignores_animation() {
    let json = r##"{
        "width": 6,
        "color": "#cba6f7",
        "animation": {
            "duration": 350,
            "easing": "ease-out-expo"
        }
    }"##;

    let config: BorderStateConfig = serde_json::from_str(json).unwrap();

    assert!(matches!(config, BorderStateConfig::SolidColor { .. }));
    assert!(config.animation().is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p stache --lib config::types::borders::tests::test_gradient_border_deserializes_animation config::types::borders::tests::test_solid_border_ignores_animation -- --show-output`

Expected: compile failure naming missing `animation` method and missing `BorderAnimationConfig` support.

- [ ] **Step 3: Import `EasingType`**

Add this import to `app/native/src/config/types/borders.rs`:

```rust
use super::tiling::EasingType;
```

- [ ] **Step 4: Add `BorderAnimationConfig`**

Add after `GradientConfig`:

```rust
/// Animation settings for gradient border colors.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BorderAnimationConfig {
    /// Duration in milliseconds for one color-swap leg.
    pub duration: u32,
    /// Easing function used for each color-swap leg.
    pub easing: EasingType,
}
```

- [ ] **Step 5: Add optional animation to gradient border state**

Change the `GradientColor` variant:

```rust
GradientColor {
    /// Border width in pixels.
    width: u32,
    /// Gradient configuration.
    gradient: GradientConfig,
    /// Optional color animation for gradient borders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    animation: Option<BorderAnimationConfig>,
},
```

- [ ] **Step 6: Update match arms for the changed variant**

Update every `GradientColor` pattern in `borders.rs` config type code to include `..` where needed:

```rust
Self::GradientColor { width, .. } => Some(*width),
Self::GradientColor { gradient, .. } => Some(gradient.from.clone()),
Self::GradientColor { gradient, .. } => parse_hex_color(&gradient.from),
Self::GradientColor { gradient, .. } => {
    let from = parse_hex_color(&gradient.from)?;
    let to = parse_hex_color(&gradient.to)?;
    Ok((from, to, gradient.angle))
}
```

- [ ] **Step 7: Add `animation()` accessor**

Add to `impl BorderStateConfig`:

```rust
#[must_use]
pub const fn animation(&self) -> Option<&BorderAnimationConfig> {
    match self {
        Self::GradientColor { animation, .. } => animation.as_ref(),
        Self::Disabled(_) | Self::SolidColor { .. } | Self::GlowColor { .. } => None,
    }
}
```

- [ ] **Step 8: Update `BorderColor::from_state_config` pattern**

Change the gradient arm:

```rust
BorderStateConfig::GradientColor { gradient, .. } => Some(Self::Gradient {
    from: gradient.from.clone(),
    to: gradient.to.clone(),
    angle: Some(gradient.angle),
}),
```

- [ ] **Step 9: Re-export `BorderAnimationConfig`**

Update `app/native/src/config/types/mod.rs`:

```rust
pub use borders::{BorderAnimationConfig, BorderColor, BorderStateConfig, BordersConfig, GradientConfig};
```

Update `app/native/src/config/mod.rs`:

```rust
BorderAnimationConfig, BorderColor, BorderStateConfig, BordersConfig,
```

- [ ] **Step 10: Run config tests**

Run: `cargo test -p stache --lib config::types::borders -- --show-output`

Expected: all border config tests pass.

- [ ] **Step 11: Commit the config change**

```bash
git add app/native/src/config/types/borders.rs app/native/src/config/types/mod.rs app/native/src/config/mod.rs
git commit -m "feat: add gradient border animation config"
```

---

### Task 3: Select focused border config by window mode

**Files:**

- Modify: `app/native/src/config/types/borders.rs:207-275`
- Modify: `app/native/src/modules/tiling/borders.rs:374-400`
- Test: `app/native/src/config/types/borders.rs:360-392`
- Test: `app/native/src/modules/tiling/borders.rs:419-442`

- [ ] **Step 1: Write failing tests for focused mode resolution**

Add tests in `app/native/src/config/types/borders.rs`:

```rust
#[test]
fn test_borders_config_selects_monocle_for_focused_monocle_workspace() {
    let config = BordersConfig {
        enabled: true,
        focused: BorderStateConfig::SolidColor { width: 6, color: "#cba6f7".to_string() },
        monocle: BorderStateConfig::SolidColor { width: 6, color: "#f38ba8".to_string() },
        ..BordersConfig::default()
    };

    let selected = config.focused_state_config(super::tiling::LayoutType::Monocle, false);

    assert_eq!(selected.color().as_deref(), Some("#f38ba8"));
}

#[test]
fn test_borders_config_falls_back_to_focused_for_dwindle() {
    let config = BordersConfig {
        enabled: true,
        focused: BorderStateConfig::SolidColor { width: 6, color: "#cba6f7".to_string() },
        monocle: BorderStateConfig::SolidColor { width: 6, color: "#f38ba8".to_string() },
        ..BordersConfig::default()
    };

    let selected = config.focused_state_config(super::tiling::LayoutType::Dwindle, false);

    assert_eq!(selected.color().as_deref(), Some("#cba6f7"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p stache --lib config::types::borders::tests::test_borders_config_selects_monocle_for_focused_monocle_workspace config::types::borders::tests::test_borders_config_falls_back_to_focused_for_dwindle -- --show-output`

Expected: compile failure naming missing `focused_state_config`.

- [ ] **Step 3: Add layout override fields for layouts not currently covered**

Add optional fields to `BordersConfig` after `floating`:

```rust
/// Border configuration for windows in dwindle layout.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub dwindle: Option<BorderStateConfig>,

/// Border configuration for windows in master layout.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub master: Option<BorderStateConfig>,

/// Border configuration for windows in grid layout.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub grid: Option<BorderStateConfig>,

/// Border configuration for windows in split layout.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub split: Option<BorderStateConfig>,

/// Border configuration for windows in vertical split layout.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub split_vertical: Option<BorderStateConfig>,

/// Border configuration for windows in horizontal split layout.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub split_horizontal: Option<BorderStateConfig>,
```

- [ ] **Step 4: Update `Default for BordersConfig`**

Add these fields to the default initializer:

```rust
dwindle: None,
master: None,
grid: None,
split: None,
split_vertical: None,
split_horizontal: None,
```

- [ ] **Step 5: Add focused-state resolution helper**

Add to `impl BordersConfig`:

```rust
#[must_use]
pub fn focused_state_config(
    &self,
    layout: super::tiling::LayoutType,
    is_window_floating: bool,
) -> &BorderStateConfig {
    if (layout == super::tiling::LayoutType::Floating || is_window_floating)
        && self.floating.is_enabled()
    {
        return &self.floating;
    }

    let layout_config = match layout {
        super::tiling::LayoutType::Monocle => Some(&self.monocle),
        super::tiling::LayoutType::Dwindle => self.dwindle.as_ref(),
        super::tiling::LayoutType::Master => self.master.as_ref(),
        super::tiling::LayoutType::Grid => self.grid.as_ref(),
        super::tiling::LayoutType::Split => self.split.as_ref(),
        super::tiling::LayoutType::SplitVertical => self.split_vertical.as_ref(),
        super::tiling::LayoutType::SplitHorizontal => self.split_horizontal.as_ref(),
        super::tiling::LayoutType::Floating => Some(&self.floating),
    };

    layout_config.filter(|config| config.is_enabled()).unwrap_or(&self.focused)
}
```

- [ ] **Step 6: Use helper in `borders::on_focus_changed`**

Replace the active-config selection in `app/native/src/modules/tiling/borders.rs` with:

```rust
let active_config = borders.focused_state_config(layout, is_window_floating);
```

- [ ] **Step 7: Run focused mode tests**

Run: `cargo test -p stache --lib config::types::borders -- --show-output`

Expected: all border config tests pass.

- [ ] **Step 8: Commit focused mode resolution**

```bash
git add app/native/src/config/types/borders.rs app/native/src/modules/tiling/borders.rs
git commit -m "feat: resolve focused border color by layout mode"
```

---

### Task 4: Add color interpolation and gradient animation loop

**Files:**

- Modify: `app/native/src/modules/tiling/borders.rs:15-400`
- Modify: `app/native/src/modules/tiling/effects/animation/mod.rs:37-39`
- Test: `app/native/src/modules/tiling/borders.rs:419-442`

- [ ] **Step 1: Write failing tests for color interpolation and animated gradient strings**

Add these tests to `app/native/src/modules/tiling/borders.rs`:

```rust
#[test]
fn test_lerp_rgba_midpoint() {
    let from = Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let to = Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

    let color = lerp_rgba(&from, &to, 0.5);

    assert!((color.r - 0.5).abs() < f64::EPSILON);
    assert!((color.g - 0.0).abs() < f64::EPSILON);
    assert!((color.b - 0.5).abs() < f64::EPSILON);
    assert!((color.a - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_gradient_color_string_uses_interpolated_from_and_to() {
    let from = Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    let to = Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

    let color = animated_gradient_color(&from, &to, 180.0, 0.5);

    assert_eq!(
        color,
        Some("gradient(top_left=0xFF800080,bottom_right=0xFF800080)".to_string())
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p stache --lib modules::tiling::borders::tests::test_lerp_rgba_midpoint modules::tiling::borders::tests::test_gradient_color_string_uses_interpolated_from_and_to -- --show-output`

Expected: compile failure naming missing `lerp_rgba` and `animated_gradient_color`.

- [ ] **Step 3: Re-export easing helpers for border animation**

Update `app/native/src/modules/tiling/effects/animation/mod.rs`:

```rust
pub use easing::{apply_easing, lerp};
```

This already exports both helpers; keep the export available to `borders.rs`.

- [ ] **Step 4: Add animation state imports**

Add to `app/native/src/modules/tiling/borders.rs`:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{BorderAnimationConfig, GradientConfig};
use crate::modules::tiling::effects::animation::{apply_easing, lerp};
```

- [ ] **Step 5: Add animation generation state**

Add near the other statics:

```rust
static ANIMATION_GENERATION: AtomicU64 = AtomicU64::new(0);

fn stop_animation() {
    ANIMATION_GENERATION.fetch_add(1, Ordering::SeqCst);
}
```

- [ ] **Step 6: Add RGBA interpolation helper**

Add near color conversion helpers:

```rust
fn lerp_rgba(from: &Rgba, to: &Rgba, progress: f64) -> Rgba {
    Rgba {
        r: lerp(from.r, to.r, progress),
        g: lerp(from.g, to.g, progress),
        b: lerp(from.b, to.b, progress),
        a: lerp(from.a, to.a, progress),
    }
}
```

- [ ] **Step 7: Extract gradient formatting helper**

Add:

```rust
fn gradient_to_janky(from_hex: &str, to_hex: &str, angle: f64) -> String {
    let angle = ((angle % 360.0) + 360.0) % 360.0;
    if (0.0..90.0).contains(&angle) || (180.0..270.0).contains(&angle) {
        format!("gradient(top_right={from_hex},bottom_left={to_hex})")
    } else {
        format!("gradient(top_left={from_hex},bottom_right={to_hex})")
    }
}
```

Then update `border_color_to_janky` to call `gradient_to_janky(&from_hex, &to_hex, angle)`.

- [ ] **Step 8: Add animated gradient formatter**

Add:

```rust
fn animated_gradient_color(from: &Rgba, to: &Rgba, angle: f64, progress: f64) -> Option<String> {
    let animated_from = lerp_rgba(from, to, progress);
    let animated_to = lerp_rgba(to, from, progress);
    let from_hex = rgba_to_hex(&animated_from);
    let to_hex = rgba_to_hex(&animated_to);
    Some(gradient_to_janky(&from_hex, &to_hex, angle))
}
```

- [ ] **Step 9: Add animation starter**

Add:

```rust
fn start_gradient_animation(
    gradient: GradientConfig,
    animation: BorderAnimationConfig,
    generation: u64,
) {
    let Ok(from) = parse_hex_color(&gradient.from) else {
        return;
    };
    let Ok(to) = parse_hex_color(&gradient.to) else {
        return;
    };

    let duration = Duration::from_millis(u64::from(animation.duration.max(16)));
    let frame_duration = Duration::from_millis(16);
    let angle = gradient.angle;

    thread::spawn(move || {
        let mut forward = true;
        let mut start = Instant::now();

        loop {
            if ANIMATION_GENERATION.load(Ordering::SeqCst) != generation {
                break;
            }

            let raw_progress = (start.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
            let eased = apply_easing(raw_progress, animation.easing);
            let progress = if forward { eased } else { 1.0 - eased };

            if let Some(active_color) = animated_gradient_color(&from, &to, angle, progress) {
                let args = vec![format!("active_color={active_color}")];
                let _ = send_command(&args);
            }

            if raw_progress >= 1.0 {
                forward = !forward;
                start = Instant::now();
            }

            thread::sleep(frame_duration);
        }
    });
}
```

- [ ] **Step 10: Run animation helper tests**

Run: `cargo test -p stache --lib modules::tiling::borders::tests::test_lerp_rgba_midpoint modules::tiling::borders::tests::test_gradient_color_string_uses_interpolated_from_and_to -- --show-output`

Expected: both tests pass.

- [ ] **Step 11: Commit interpolation helpers**

```bash
git add app/native/src/modules/tiling/borders.rs app/native/src/modules/tiling/effects/animation/mod.rs
git commit -m "feat: add border gradient color interpolation"
```

---

### Task 5: Wire animation into focused border updates

**Files:**

- Modify: `app/native/src/modules/tiling/borders.rs:213-400`
- Modify: `app/native/src/modules/tiling/effects/subscriber.rs:443-628`
- Test: `app/native/src/modules/tiling/borders.rs:419-442`

- [ ] **Step 1: Write failing test for animation eligibility**

Add this test to `app/native/src/modules/tiling/borders.rs`:

```rust
#[test]
fn test_gradient_with_animation_is_animatable() {
    let config = BorderStateConfig::GradientColor {
        width: 6,
        gradient: GradientConfig {
            from: "#cba6f7".to_string(),
            to: "#a6e3a1".to_string(),
            angle: 180.0,
        },
        animation: Some(BorderAnimationConfig {
            duration: 350,
            easing: crate::config::EasingType::EaseOutExpo,
        }),
    };

    assert!(animated_gradient_parts(&config).is_some());
}

#[test]
fn test_solid_color_is_not_animatable() {
    let config = BorderStateConfig::SolidColor { width: 6, color: "#cba6f7".to_string() };

    assert!(animated_gradient_parts(&config).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p stache --lib modules::tiling::borders::tests::test_gradient_with_animation_is_animatable modules::tiling::borders::tests::test_solid_color_is_not_animatable -- --show-output`

Expected: compile failure naming missing `animated_gradient_parts`.

- [ ] **Step 3: Add `animated_gradient_parts` helper**

Add to `app/native/src/modules/tiling/borders.rs`:

```rust
fn animated_gradient_parts(
    config: &BorderStateConfig,
) -> Option<(GradientConfig, BorderAnimationConfig)> {
    match config {
        BorderStateConfig::GradientColor { gradient, animation: Some(animation), .. } => {
            Some((gradient.clone(), animation.clone()))
        }
        BorderStateConfig::Disabled(_)
        | BorderStateConfig::SolidColor { .. }
        | BorderStateConfig::GradientColor { animation: None, .. }
        | BorderStateConfig::GlowColor { .. } => None,
    }
}
```

- [ ] **Step 4: Update `init()` command building to argv args**

Replace the formatted command string in `init()` with:

```rust
let args = vec![
    format!("width={width}"),
    format!("style={style_char}"),
    format!("hidpi={hidpi}"),
    format!("active_color={active_color}"),
    format!("inactive_color={inactive_color}"),
    format!("blacklist={blacklist}"),
];
```

Then call:

```rust
if send_command(&args) {
```

- [ ] **Step 5: Update `on_focus_changed` to start or stop animation**

Replace the end of `on_focus_changed` with:

```rust
let (active_color, width) = get_border_settings(active_config);
let (inactive_color, _) = get_border_settings(&borders.unfocused);

let args = vec![
    format!("width={width}"),
    format!("active_color={active_color}"),
    format!("inactive_color={inactive_color}"),
];

stop_animation();
let generation = ANIMATION_GENERATION.load(Ordering::SeqCst);

if send_command(&args)
    && let Some((gradient, animation)) = animated_gradient_parts(active_config)
{
    start_gradient_animation(gradient, animation, generation);
}
```

- [ ] **Step 6: Add a subscriber helper for refreshing active borders**

In `app/native/src/modules/tiling/effects/subscriber.rs`, extract the repeated border refresh query logic into:

```rust
async fn refresh_active_border(&self, focused_window_id: Option<u32>) {
    let mut layout = LayoutType::Floating;
    let mut is_window_floating = false;

    if let Ok(QueryResult::Workspace(Some(workspace))) =
        self.actor_handle.query(StateQuery::GetFocusedWorkspace).await
    {
        layout = workspace.layout;
    }

    if let Some(window_id) = focused_window_id
        && let Ok(QueryResult::Window(Some(window))) =
            self.actor_handle.query(StateQuery::GetWindow { id: window_id }).await
    {
        is_window_floating = window.is_floating;
    }

    crate::modules::tiling::borders::on_focus_changed(layout, is_window_floating);
}
```

- [ ] **Step 7: Use `refresh_active_border` in focus and initial paths**

In `handle_focus_changed`, replace the inline border query block with:

```rust
self.refresh_active_border(new_focus.focused_window_id).await;
```

Keep the local `layout` and `is_window_floating` values for `effects_from_focus_change`; query them before calling `effects_from_focus_change` using the same values returned by the helper if you choose to return a tuple instead of `()`.

In `apply_initial_border_colors`, replace the inline block with:

```rust
self.refresh_active_border(self.state.focus.focused_window_id).await;
```

- [ ] **Step 8: Refresh active border on layout and floating changes**

Update the subscriber notification match arms:

```rust
SubscriberNotification::FloatingChanged { window_id, floating } => {
    self.handle_floating_changed(window_id, floating);
    let focused_window_id = self.state.focus.focused_window_id;
    self.refresh_active_border(focused_window_id).await;
    Vec::new()
}

SubscriberNotification::WorkspaceLayoutChanged { workspace_id, layout } => {
    self.handle_workspace_layout_changed(workspace_id, layout);
    let focused_window_id = self.state.focus.focused_window_id;
    self.refresh_active_border(focused_window_id).await;
    Vec::new()
}
```

- [ ] **Step 9: Run targeted tests**

Run: `cargo test -p stache --lib modules::tiling::borders -- --show-output`

Expected: all border module tests pass.

- [ ] **Step 10: Commit animation wiring**

```bash
git add app/native/src/modules/tiling/borders.rs app/native/src/modules/tiling/effects/subscriber.rs
git commit -m "feat: animate focused gradient borders"
```

---

### Task 6: Update sample config and generated schema

**Files:**

- Modify: `docs/sample-config.jsonc:318-344`
- Modify: `stache.schema.json`

- [ ] **Step 1: Update sample focused gradient config**

In `docs/sample-config.jsonc`, change the focused example to:

```jsonc
// Focused window border (animated gradient)
"focused": {
  "width": 6,
  "gradient": {
    "from": "#cba6f7", // Catppuccin Mauve
    "to": "#a6e3a1", // Catppuccin Green
    "angle": 180,
  },
  "animation": {
    "duration": 350,
    "easing": "ease-out-expo",
  },
},
```

- [ ] **Step 2: Update sample monocle gradient config**

Change the monocle example to:

```jsonc
// Monocle layout border (static gradient)
"monocle": {
  "width": 6,
  "gradient": {
    "from": "#f38ba8", // Catppuccin Red
    "to": "#fab387", // Catppuccin Peach
    "angle": 180,
  },
},
```

- [ ] **Step 3: Regenerate schema**

Run: `./scripts/generate-schema.sh`

Expected: `stache.schema.json` includes `BorderAnimationConfig`, optional `animation` on gradient border states, and optional layout override fields.

- [ ] **Step 4: Commit docs and schema**

```bash
git add docs/sample-config.jsonc stache.schema.json
git commit -m "docs: document animated border gradients"
```

---

### Task 7: Final verification

**Files:**

- Verify only: no planned source edits in this task

- [ ] **Step 1: Run border and config tests**

Run: `cargo test -p stache --lib modules::tiling::borders config::types::borders -- --show-output`

Expected: all selected tests pass.

- [ ] **Step 2: Run all native tests**

Run: `pnpm test:native`

Expected: all native tests pass.

- [ ] **Step 3: Run native lint**

Run: `pnpm lint:native`

Expected: clippy, cargo sort, and fixes complete without warnings. If the command modifies files, inspect the diff, run `pnpm test:native` again, then create a new commit for lint-generated changes.

- [ ] **Step 4: Manually verify installed borders version**

Run: `borders --version`

Expected: output includes `borders-v1.9.0` or newer.

- [ ] **Step 5: Manually verify app behavior**

Run: `pnpm tauri:dev`

Expected behavior:

- Focused non-monocle windows use the configured purple/green focused gradient.
- Focused monocle windows use the configured red/orange monocle gradient.
- Switching focused apps updates active border colors.
- Switching workspaces updates active border colors.
- Switching layouts updates active border colors without requiring a separate focus change.
- Floating a focused window switches to the floating config when configured.
- Gradient configs with `animation` continuously swap the two colors.
- Gradient configs without `animation` remain static.
- Solid and glow configs remain static even if the JSON includes an `animation` property.

- [ ] **Step 6: Commit verification-only fixes if needed**

If verification required source changes, commit them:

```bash
git add app/native/src/modules/tiling app/native/src/config docs/sample-config.jsonc stache.schema.json
git commit -m "fix: stabilize animated border updates"
```

If verification required no source changes, do not create an empty commit.

---

## Self-Review Notes

- Root cause coverage: Task 1 fixes the broken Mach payload format and CLI fallback argument splitting.
- Focused-only scope: Tasks 3 and 5 only call animation from `on_focus_changed`, which controls the active border color.
- Layout override scope: Task 3 resolves monocle/floating and optional layout-specific focused overrides before falling back to `focused`.
- Animation scope: Tasks 2, 4, and 5 only expose and execute animation for `GradientColor`.
- Schema/docs coverage: Task 6 updates sample config and generated schema.
- Verification coverage: Task 7 includes targeted tests, full native tests, lint, and manual app behavior checks.
