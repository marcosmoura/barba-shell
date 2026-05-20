//! Simple border management via `JankyBorders`.
//!
//! This module handles window borders by communicating with `JankyBorders`.
//! It's intentionally simple:
//!
//! 1. On init: Configure `JankyBorders` with style settings and blacklist
//! 2. On focus change: Send a single batched command with all border colors
//!
//! # Architecture
//!
//! - Uses Mach IPC for fast communication (falls back to CLI)
//! - Caches the last command to avoid duplicate sends
//! - Batches all settings into a single call

use std::ffi::CString;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::config::types::tiling::LayoutType as ConfigLayoutType;
use crate::config::{
    BorderAnimationConfig, BorderColor, BorderStateConfig, GradientConfig, Rgba, get_config,
    parse_hex_color,
};
use crate::modules::tiling::effects::animation::{apply_easing, lerp};
use crate::modules::tiling::rules::{SKIP_TILING_APP_NAMES, SKIP_TILING_BUNDLE_IDS};
use crate::modules::tiling::state::LayoutType;

// ============================================================================
// Constants
// ============================================================================

/// Mach service name for `JankyBorders`.
const JANKY_BORDERS_SERVICE: &str = "git.felix.borders";

// ============================================================================
// State
// ============================================================================

/// Last command sent to `JankyBorders` (for deduplication).
static LAST_COMMAND: OnceLock<Mutex<String>> = OnceLock::new();

/// Mach port for IPC communication.
static MACH_PORT: OnceLock<Mutex<Option<u32>>> = OnceLock::new();

static ANIMATION_GENERATION: AtomicU64 = AtomicU64::new(0);
static ANIMATION_SEND_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[allow(dead_code)]
fn stop_animation() {
    let _guard = get_animation_send_lock().lock();
    ANIMATION_GENERATION.fetch_add(1, Ordering::SeqCst);
}

fn get_last_command() -> &'static Mutex<String> {
    LAST_COMMAND.get_or_init(|| Mutex::new(String::new()))
}

fn get_mach_port() -> &'static Mutex<Option<u32>> { MACH_PORT.get_or_init(|| Mutex::new(None)) }

fn get_animation_send_lock() -> &'static Mutex<()> {
    ANIMATION_SEND_LOCK.get_or_init(|| Mutex::new(()))
}

// ============================================================================
// Mach IPC
// ============================================================================

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

#[link(name = "System", kind = "dylib")]
unsafe extern "C" {
    fn bootstrap_look_up(bp: u32, service_name: *const i8, sp: *mut u32) -> i32;
    fn mach_msg(
        msg: *mut MachMessage,
        option: i32,
        send_size: u32,
        rcv_size: u32,
        rcv_name: u32,
        timeout: u32,
        notify: u32,
    ) -> i32;
}

const BOOTSTRAP_PORT: u32 = 0;
const MACH_SEND_MSG: i32 = 1;
const MACH_MSG_TIMEOUT_NONE: u32 = 0;
const MACH_PORT_NULL: u32 = 0;

#[repr(C)]
struct MachMessage {
    header: MachMsgHeader,
    body: MachMsgBody,
    descriptor: MachMsgOolDescriptor,
}

#[repr(C)]
struct MachMsgHeader {
    bits: u32,
    size: u32,
    remote_port: u32,
    local_port: u32,
    voucher_port: u32,
    id: i32,
}

#[repr(C)]
struct MachMsgBody {
    descriptor_count: u32,
}

#[repr(C)]
struct MachMsgOolDescriptor {
    address: *const u8,
    deallocate: u8,
    copy: u8,
    pad1: u8,
    type_: u8,
    size: u32,
}

/// Connects to `JankyBorders` via Mach IPC.
fn connect_mach() -> bool {
    let Ok(service) = CString::new(JANKY_BORDERS_SERVICE) else {
        return false;
    };

    let mut port: u32 = 0;
    let result = unsafe { bootstrap_look_up(BOOTSTRAP_PORT, service.as_ptr(), &raw mut port) };

    if result == 0 && port != 0 {
        *get_mach_port().lock() = Some(port);
        true
    } else {
        false
    }
}

/// Sends arguments via Mach IPC.
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

// ============================================================================
// Color Conversion
// ============================================================================

/// Converts RGBA to `JankyBorders` hex format (`0xAARRGGBB`).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rgba_to_hex(rgba: &Rgba) -> String {
    let a = (rgba.a * 255.0).round() as u8;
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    format!("0x{a:02X}{r:02X}{g:02X}{b:02X}")
}

fn lerp_rgba(from: &Rgba, to: &Rgba, progress: f64) -> Rgba {
    Rgba {
        r: lerp(from.r, to.r, progress),
        g: lerp(from.g, to.g, progress),
        b: lerp(from.b, to.b, progress),
        a: lerp(from.a, to.a, progress),
    }
}

/// Converts a hex color string to `JankyBorders` format.
fn hex_to_janky(hex: &str) -> Option<String> {
    let rgba = parse_hex_color(hex).ok()?;
    Some(rgba_to_hex(&rgba))
}

fn gradient_to_janky(from_hex: &str, to_hex: &str, angle: f64) -> String {
    let angle = ((angle % 360.0) + 360.0) % 360.0;
    if (0.0..90.0).contains(&angle) || (180.0 < angle && angle < 270.0) {
        format!("gradient(top_right={from_hex},bottom_left={to_hex})")
    } else {
        format!("gradient(top_left={from_hex},bottom_right={to_hex})")
    }
}

fn animated_gradient_color(from: &Rgba, to: &Rgba, angle: f64, progress: f64) -> String {
    let animated_from = lerp_rgba(from, to, progress);
    let animated_to = lerp_rgba(to, from, progress);
    let from_hex = rgba_to_hex(&animated_from);
    let to_hex = rgba_to_hex(&animated_to);
    gradient_to_janky(&from_hex, &to_hex, angle)
}

fn send_animation_frame(generation: u64, active_color: &str) -> bool {
    let _guard = get_animation_send_lock().lock();

    if ANIMATION_GENERATION.load(Ordering::SeqCst) != generation {
        return false;
    }

    let args = vec![format!("active_color={active_color}")];
    send_command(&args)
}

/// Converts a `BorderColor` to `JankyBorders` color string.
fn border_color_to_janky(color: &BorderColor) -> Option<String> {
    match color {
        BorderColor::Solid(hex) => hex_to_janky(hex),
        BorderColor::Gradient { from, to, angle } => {
            let from_hex = hex_to_janky(from)?;
            let to_hex = hex_to_janky(to)?;
            let angle = angle.unwrap_or(135.0);
            Some(gradient_to_janky(&from_hex, &to_hex, angle))
        }
        BorderColor::Glow(hex) => {
            let janky_hex = hex_to_janky(hex)?;
            Some(format!("glow({janky_hex})"))
        }
    }
}

/// Gets the `JankyBorders` color string for a border state config.
/// Returns transparent (0x00000000) and width 0 if disabled.
fn get_border_settings(config: &BorderStateConfig) -> (String, u32) {
    if !config.is_enabled() {
        return ("0x00000000".to_string(), 0);
    }

    let color = BorderColor::from_state_config(config)
        .and_then(|c| border_color_to_janky(&c))
        .unwrap_or_else(|| "0x00000000".to_string());

    let width = config.width().unwrap_or(0);

    (color, width)
}

const fn animated_gradient_parts(
    config: &BorderStateConfig,
) -> Option<(&GradientConfig, &BorderAnimationConfig)> {
    match config {
        BorderStateConfig::GradientColor {
            gradient,
            animation: Some(animation),
            ..
        } => Some((gradient, animation)),
        BorderStateConfig::Disabled(_)
        | BorderStateConfig::SolidColor { .. }
        | BorderStateConfig::GradientColor { animation: None, .. }
        | BorderStateConfig::GlowColor { .. } => None,
    }
}

// ============================================================================
// JankyBorders Communication
// ============================================================================

/// Checks if `JankyBorders` is available.
fn is_available() -> bool {
    Command::new("which")
        .arg("borders")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Sends arguments to `JankyBorders` (with deduplication).
fn send_command(args: &[String]) -> bool {
    let key = command_key(args);

    // Check if command is the same as last time
    {
        let mut last = get_last_command().lock();
        if *last == key {
            return true; // Already sent this exact command
        }
        *last = key;
    }

    // Try Mach IPC first
    if send_mach(args) {
        return true;
    }

    if connect_mach() && send_mach(args) {
        return true;
    }

    // Fall back to CLI
    Command::new("borders")
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

#[allow(dead_code)]
fn start_gradient_animation(
    gradient: &GradientConfig,
    animation: &BorderAnimationConfig,
    generation: u64,
) {
    let Ok(from) = parse_hex_color(&gradient.from) else {
        return;
    };
    let Ok(to) = parse_hex_color(&gradient.to) else {
        return;
    };

    let duration = Duration::from_millis(u64::from(animation.duration.max(16)));
    let easing = animation.easing;
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
            let eased = apply_easing(raw_progress, easing);
            let progress = if forward { eased } else { 1.0 - eased };

            let active_color = animated_gradient_color(&from, &to, angle, progress);
            if !send_animation_frame(generation, &active_color) {
                break;
            }

            if raw_progress >= 1.0 {
                forward = !forward;
                start = Instant::now();
            }

            thread::sleep(frame_duration);
        }
    });
}

/// Builds the blacklist string for `JankyBorders`.
fn build_blacklist() -> String {
    let config = get_config();
    let mut apps: Vec<String> = Vec::new();

    // Add built-in skip lists from rules module
    apps.extend(SKIP_TILING_BUNDLE_IDS.iter().map(|s| (*s).to_string()));
    apps.extend(SKIP_TILING_APP_NAMES.iter().map(|s| (*s).to_string()));

    // Add user-configured ignore rules (app names and bundle IDs)
    for rule in &config.tiling.ignore {
        if let Some(app_id) = &rule.app_id {
            apps.push(app_id.clone());
        }
        if let Some(app_name) = &rule.app_name {
            apps.push(app_name.clone());
        }
    }

    // Add border-specific ignore rules
    for rule in &config.tiling.borders.ignore {
        if let Some(app_id) = &rule.app_id {
            apps.push(app_id.clone());
        }
        if let Some(app_name) = &rule.app_name {
            apps.push(app_name.clone());
        }
    }

    apps.join(",")
}

// ============================================================================
// Public API
// ============================================================================

/// Initializes the border system.
///
/// Sets up `JankyBorders` with:
/// - Style settings (width, style, hidpi)
/// - Blacklist of ignored apps
/// - Initial colors (unfocused always, active based on initial layout)
#[must_use]
pub fn init() -> bool {
    let config = get_config();

    if !config.tiling.borders.is_enabled() {
        tracing::debug!("tiling: borders disabled in config");
        return true;
    }

    if !is_available() {
        tracing::warn!("tiling: JankyBorders not found");
        return false;
    }

    // Connect via Mach IPC
    if connect_mach() {
        tracing::debug!("tiling: connected to JankyBorders via Mach IPC");
    }

    // Build initial command with all settings
    let borders = &config.tiling.borders;
    let blacklist = build_blacklist();

    // Get style settings
    let width = borders.focused.width().unwrap_or(4);
    let style = borders.style.as_deref().unwrap_or("round");
    let style_char = if style == "square" { 's' } else { 'r' };
    let hidpi = if borders.hidpi.unwrap_or(true) {
        "on"
    } else {
        "off"
    };

    // Get unfocused color (always needed)
    let (inactive_color, _) = get_border_settings(&borders.unfocused);

    // Get initial active color (default to focused)
    let (active_color, _) = get_border_settings(&borders.focused);

    // Build and send the initial command
    let args = vec![
        format!("width={width}"),
        format!("style={style_char}"),
        format!("hidpi={hidpi}"),
        format!("active_color={active_color}"),
        format!("inactive_color={inactive_color}"),
        format!("blacklist={blacklist}"),
    ];

    // Clear the cache so first command always sends
    *get_last_command().lock() = String::new();

    if send_command(&args) {
        tracing::debug!("tiling: borders initialized");
        true
    } else {
        tracing::warn!("tiling: failed to initialize borders");
        false
    }
}

/// Updates borders based on workspace layout.
///
/// Called when focus changes. Determines the correct active color based on:
/// - Monocle layout → monocle config (if enabled)
/// - Floating layout → floating config (if enabled)
/// - Otherwise → focused config
///
/// Always sends unfocused color as `inactive_color`.
/// All settings are batched into a single `JankyBorders` call.
pub fn on_focus_changed(layout: LayoutType, is_window_floating: bool) {
    let config = get_config();
    let borders = &config.tiling.borders;

    if !borders.is_enabled() {
        return;
    }

    let config_layout = match layout {
        LayoutType::Floating => ConfigLayoutType::Floating,
        LayoutType::Dwindle => ConfigLayoutType::Dwindle,
        LayoutType::Monocle => ConfigLayoutType::Monocle,
        LayoutType::Master => ConfigLayoutType::Master,
        LayoutType::Split => ConfigLayoutType::Split,
        LayoutType::SplitVertical => ConfigLayoutType::SplitVertical,
        LayoutType::SplitHorizontal => ConfigLayoutType::SplitHorizontal,
        LayoutType::Grid => ConfigLayoutType::Grid,
    };

    // Determine which config to use for active color
    let active_config = borders.focused_state_config(config_layout, is_window_floating);

    // Get colors and width
    let (active_color, width) = get_border_settings(active_config);
    let (inactive_color, _) = get_border_settings(&borders.unfocused);

    // Build and send command
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
}

/// Refreshes border configuration.
///
/// Call this when configuration is reloaded.
pub fn refresh() {
    // Clear cache to force re-send
    *get_last_command().lock() = String::new();

    // Re-initialize
    let _ = init();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_to_hex() {
        let rgba = Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
        assert_eq!(rgba_to_hex(&rgba), "0xFFFF0000");
    }

    #[test]
    fn test_hex_to_janky() {
        assert_eq!(hex_to_janky("#FF0000"), Some("0xFFFF0000".to_string()));
        assert_eq!(hex_to_janky("#00FF00"), Some("0xFF00FF00".to_string()));
    }

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
            "gradient(top_left=0xFF800080,bottom_right=0xFF800080)".to_string()
        );
    }

    #[test]
    fn test_animation_frame_is_not_sent_after_cancellation() {
        let from = Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
        let to = Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
        let generation = ANIMATION_GENERATION.load(Ordering::SeqCst);
        let active_color = animated_gradient_color(&from, &to, 180.0, 0.5);

        stop_animation();

        assert!(!send_animation_frame(generation, &active_color));
    }

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
        let config = BorderStateConfig::SolidColor {
            width: 6,
            color: "#cba6f7".to_string(),
        };

        assert!(animated_gradient_parts(&config).is_none());
    }

    #[test]
    fn test_get_border_settings_disabled() {
        let config = BorderStateConfig::Disabled(false);
        let (color, width) = get_border_settings(&config);
        assert_eq!(color, "0x00000000");
        assert_eq!(width, 0);
    }

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
}
