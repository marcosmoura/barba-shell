//! Floating layout and window presets.
//!
//! This module provides:
//! - Floating layout (windows keep their current positions)
//! - Floating window presets for quick positioning (half-screen, quarter-screen, centered, etc.)
//!
//! # Example Configuration
//!
//! ```jsonc
//! {
//!   "tiling": {
//!     "floating": {
//!       "presets": [
//!         { "name": "center-large", "width": "80%", "height": "80%", "center": true },
//!         { "name": "half-left", "width": "50%", "height": "100%", "x": 0, "y": 0 },
//!         { "name": "half-right", "width": "50%", "height": "100%", "x": "50%", "y": 0 }
//!       ]
//!     }
//!   }
//! }
//! ```
//!
//! # Usage
//!
//! ```bash
//! stache tiling window --preset center-large
//! stache tiling window --preset half-left
//! ```

use smallvec::SmallVec;

use super::{Gaps, LayoutResult};
use crate::config::{DimensionValue, FloatingPreset, get_config};
use crate::tiling::state::Rect;

// ============================================================================
// Floating Layout
// ============================================================================

/// Floating layout - windows keep their current positions.
///
/// Returns an empty result since no repositioning is needed.
/// The tiling manager will skip applying positions for floating windows.
#[must_use]
pub fn layout(_window_ids: &[u32]) -> LayoutResult {
    // Floating windows don't get repositioned by the layout engine
    // Return empty to indicate no changes needed
    SmallVec::new()
}

// ============================================================================
// Floating Presets
// ============================================================================

/// Calculates the frame for a preset on a given screen.
///
/// This resolves percentage-based dimensions to absolute pixels,
/// handles centering, and respects both outer and inner gaps.
///
/// Inner gaps are applied when using 50% dimensions so that two
/// adjacent windows (e.g., half-left and half-right) have proper
/// spacing between them.
///
/// # Arguments
///
/// * `preset` - The preset configuration to apply.
/// * `screen_frame` - The available screen area (already adjusted for menu bar, dock, etc.).
/// * `gaps` - Gap configuration for outer and inner margins.
///
/// # Returns
///
/// The calculated window frame as a `Rect`.
#[must_use]
pub fn calculate_preset_frame(preset: &FloatingPreset, screen_frame: &Rect, gaps: &Gaps) -> Rect {
    // Apply outer gaps to get the usable area
    let usable = gaps.apply_outer(screen_frame);

    // Check if dimensions are 50% (for inner gap handling)
    let width_is_half = is_half_percentage(&preset.width);
    let height_is_half = is_half_percentage(&preset.height);

    // Resolve width, accounting for inner gap if 50%
    let width = if width_is_half {
        // Two 50% windows side by side need an inner gap between them
        (usable.width - gaps.inner_h) / 2.0
    } else {
        preset.width.resolve(usable.width)
    };

    // Resolve height, accounting for inner gap if 50%
    let height = if height_is_half {
        // Two 50% windows stacked need an inner gap between them
        (usable.height - gaps.inner_v) / 2.0
    } else {
        preset.height.resolve(usable.height)
    };

    // Clamp dimensions to usable area
    let width = width.min(usable.width).max(1.0);
    let height = height.min(usable.height).max(1.0);

    // Calculate position
    let (x, y) = if preset.center {
        // Center the window in the usable area
        let center_x = usable.x + (usable.width - width) / 2.0;
        let center_y = usable.y + (usable.height - height) / 2.0;
        (center_x, center_y)
    } else {
        // Calculate x position, accounting for inner gap if width is 50%
        let x = preset.x.as_ref().map_or(usable.x, |dim| {
            if width_is_half && is_half_percentage(dim) {
                // Right half: position after left half + inner gap
                usable.x + width + gaps.inner_h
            } else {
                usable.x + dim.resolve(usable.width)
            }
        });

        // Calculate y position, accounting for inner gap if height is 50%
        let y = preset.y.as_ref().map_or(usable.y, |dim| {
            if height_is_half && is_half_percentage(dim) {
                // Bottom half: position after top half + inner gap
                usable.y + height + gaps.inner_v
            } else {
                usable.y + dim.resolve(usable.height)
            }
        });

        (x, y)
    };

    // Clamp position to keep window within usable area
    let x = x.max(usable.x).min(usable.x + usable.width - width);
    let y = y.max(usable.y).min(usable.y + usable.height - height);

    Rect::new(x, y, width, height)
}

/// Checks if a dimension value is exactly 50%.
fn is_half_percentage(dim: &DimensionValue) -> bool {
    match dim {
        DimensionValue::Percentage(s) => {
            let trimmed = s.trim().trim_end_matches('%');
            trimmed.parse::<f64>().is_ok_and(|pct| (pct - 50.0).abs() < 0.001)
        }
        DimensionValue::Pixels(_) => false,
    }
}

/// Finds a preset by name from the configuration.
///
/// # Arguments
///
/// * `name` - The name of the preset to find.
///
/// # Returns
///
/// The preset if found, or `None` if no preset with that name exists.
#[must_use]
pub fn find_preset(name: &str) -> Option<FloatingPreset> {
    let config = get_config();
    config
        .tiling
        .floating
        .presets
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .cloned()
}

/// Returns a list of all available preset names.
#[must_use]
pub fn list_preset_names() -> Vec<String> {
    let config = get_config();
    config.tiling.floating.presets.iter().map(|p| p.name.clone()).collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Floating Layout Tests
    // ========================================================================

    #[test]
    fn test_floating_returns_empty() {
        let result = layout(&[1, 2, 3]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_floating_empty_input() {
        let result = layout(&[]);
        assert!(result.is_empty());
    }

    // ========================================================================
    // Preset Tests - Helpers
    // ========================================================================

    fn make_screen() -> Rect { Rect::new(0.0, 0.0, 1920.0, 1080.0) }

    fn make_gaps() -> Gaps {
        Gaps {
            inner_h: 8.0,
            inner_v: 8.0,
            outer_top: 8.0,
            outer_right: 8.0,
            outer_bottom: 8.0,
            outer_left: 8.0,
        }
    }

    fn make_zero_gaps() -> Gaps { Gaps::default() }

    // ========================================================================
    // Preset Tests - Centered presets
    // ========================================================================

    #[test]
    fn test_preset_centered_percentage() {
        let preset = FloatingPreset {
            name: "center".to_string(),
            width: DimensionValue::Percentage("80%".to_string()),
            height: DimensionValue::Percentage("80%".to_string()),
            x: None,
            y: None,
            center: true,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        // 80% of 1920 = 1536, 80% of 1080 = 864
        assert!((frame.width - 1536.0).abs() < 0.01);
        assert!((frame.height - 864.0).abs() < 0.01);

        // Centered: x = (1920 - 1536) / 2 = 192, y = (1080 - 864) / 2 = 108
        assert!((frame.x - 192.0).abs() < 0.01);
        assert!((frame.y - 108.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_centered_pixels() {
        let preset = FloatingPreset {
            name: "center".to_string(),
            width: DimensionValue::Pixels(800),
            height: DimensionValue::Pixels(600),
            x: None,
            y: None,
            center: true,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        assert!((frame.width - 800.0).abs() < 0.01);
        assert!((frame.height - 600.0).abs() < 0.01);

        // Centered: x = (1920 - 800) / 2 = 560, y = (1080 - 600) / 2 = 240
        assert!((frame.x - 560.0).abs() < 0.01);
        assert!((frame.y - 240.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_centered_with_gaps() {
        let preset = FloatingPreset {
            name: "center".to_string(),
            width: DimensionValue::Percentage("100%".to_string()),
            height: DimensionValue::Percentage("100%".to_string()),
            x: None,
            y: None,
            center: true,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable area: 1920 - 16 = 1904 width, 1080 - 16 = 1064 height
        // 100% of usable = 1904 x 1064
        assert!((frame.width - 1904.0).abs() < 0.01);
        assert!((frame.height - 1064.0).abs() < 0.01);

        // Position should be at gap offset
        assert!((frame.x - 8.0).abs() < 0.01);
        assert!((frame.y - 8.0).abs() < 0.01);
    }

    // ========================================================================
    // Preset Tests - Half-screen layouts (with inner gap handling)
    // ========================================================================

    #[test]
    fn test_preset_half_left() {
        let preset = FloatingPreset {
            name: "half-left".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("100%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        // With zero gaps, 50% of 1920 = 960
        assert!((frame.width - 960.0).abs() < 0.01);
        assert!((frame.height - 1080.0).abs() < 0.01);
        assert!((frame.x - 0.0).abs() < 0.01);
        assert!((frame.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_left_with_gaps() {
        let preset = FloatingPreset {
            name: "half-left".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("100%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable area: 1904 x 1064 (after outer gaps of 8px each side)
        // With 8px inner gap: width = (1904 - 8) / 2 = 948
        assert!((frame.width - 948.0).abs() < 0.01);
        assert!((frame.height - 1064.0).abs() < 0.01);
        // Position at outer gap offset
        assert!((frame.x - 8.0).abs() < 0.01);
        assert!((frame.y - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_right() {
        let preset = FloatingPreset {
            name: "half-right".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("100%".to_string()),
            x: Some(DimensionValue::Percentage("50%".to_string())),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        assert!((frame.width - 960.0).abs() < 0.01);
        assert!((frame.height - 1080.0).abs() < 0.01);
        assert!((frame.x - 960.0).abs() < 0.01);
        assert!((frame.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_right_with_gaps() {
        let preset = FloatingPreset {
            name: "half-right".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("100%".to_string()),
            x: Some(DimensionValue::Percentage("50%".to_string())),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable area: 1904 x 1064 (after outer gaps)
        // With 8px inner gap: width = (1904 - 8) / 2 = 948
        assert!((frame.width - 948.0).abs() < 0.01);
        assert!((frame.height - 1064.0).abs() < 0.01);
        // x = outer_left + width + inner_gap = 8 + 948 + 8 = 964
        assert!((frame.x - 964.0).abs() < 0.01);
        assert!((frame.y - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_top() {
        let preset = FloatingPreset {
            name: "half-top".to_string(),
            width: DimensionValue::Percentage("100%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        assert!((frame.width - 1920.0).abs() < 0.01);
        assert!((frame.height - 540.0).abs() < 0.01);
        assert!((frame.x - 0.0).abs() < 0.01);
        assert!((frame.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_top_with_gaps() {
        let preset = FloatingPreset {
            name: "half-top".to_string(),
            width: DimensionValue::Percentage("100%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable: 1904 x 1064
        // Height with inner gap: (1064 - 8) / 2 = 528
        assert!((frame.width - 1904.0).abs() < 0.01);
        assert!((frame.height - 528.0).abs() < 0.01);
        assert!((frame.x - 8.0).abs() < 0.01);
        assert!((frame.y - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_bottom() {
        let preset = FloatingPreset {
            name: "half-bottom".to_string(),
            width: DimensionValue::Percentage("100%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Percentage("50%".to_string())),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        assert!((frame.width - 1920.0).abs() < 0.01);
        assert!((frame.height - 540.0).abs() < 0.01);
        assert!((frame.x - 0.0).abs() < 0.01);
        assert!((frame.y - 540.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_half_bottom_with_gaps() {
        let preset = FloatingPreset {
            name: "half-bottom".to_string(),
            width: DimensionValue::Percentage("100%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Percentage("50%".to_string())),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable: 1904 x 1064
        // Height with inner gap: (1064 - 8) / 2 = 528
        // y = outer_top + height + inner_gap = 8 + 528 + 8 = 544
        assert!((frame.width - 1904.0).abs() < 0.01);
        assert!((frame.height - 528.0).abs() < 0.01);
        assert!((frame.x - 8.0).abs() < 0.01);
        assert!((frame.y - 544.0).abs() < 0.01);
    }

    // ========================================================================
    // Preset Tests - Quarter-screen presets
    // ========================================================================

    #[test]
    fn test_preset_quarter_top_left() {
        let preset = FloatingPreset {
            name: "quarter-tl".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        assert!((frame.width - 960.0).abs() < 0.01);
        assert!((frame.height - 540.0).abs() < 0.01);
        assert!((frame.x - 0.0).abs() < 0.01);
        assert!((frame.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_quarter_top_left_with_gaps() {
        let preset = FloatingPreset {
            name: "quarter-tl".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Pixels(0)),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable: 1904 x 1064
        // Width with inner gap: (1904 - 8) / 2 = 948
        // Height with inner gap: (1064 - 8) / 2 = 528
        assert!((frame.width - 948.0).abs() < 0.01);
        assert!((frame.height - 528.0).abs() < 0.01);
        assert!((frame.x - 8.0).abs() < 0.01);
        assert!((frame.y - 8.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_quarter_bottom_right() {
        let preset = FloatingPreset {
            name: "quarter-br".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Percentage("50%".to_string())),
            y: Some(DimensionValue::Percentage("50%".to_string())),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        assert!((frame.width - 960.0).abs() < 0.01);
        assert!((frame.height - 540.0).abs() < 0.01);
        assert!((frame.x - 960.0).abs() < 0.01);
        assert!((frame.y - 540.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_quarter_bottom_right_with_gaps() {
        let preset = FloatingPreset {
            name: "quarter-br".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: Some(DimensionValue::Percentage("50%".to_string())),
            y: Some(DimensionValue::Percentage("50%".to_string())),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_gaps());

        // Usable: 1904 x 1064
        // Width with inner gap: (1904 - 8) / 2 = 948
        // Height with inner gap: (1064 - 8) / 2 = 528
        // x = 8 + 948 + 8 = 964
        // y = 8 + 528 + 8 = 544
        assert!((frame.width - 948.0).abs() < 0.01);
        assert!((frame.height - 528.0).abs() < 0.01);
        assert!((frame.x - 964.0).abs() < 0.01);
        assert!((frame.y - 544.0).abs() < 0.01);
    }

    // ========================================================================
    // Preset Tests - Edge cases
    // ========================================================================

    #[test]
    fn test_preset_clamps_oversized_dimensions() {
        let preset = FloatingPreset {
            name: "oversized".to_string(),
            width: DimensionValue::Pixels(5000), // Larger than screen
            height: DimensionValue::Pixels(3000),
            x: None,
            y: None,
            center: true,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        // Should clamp to screen size
        assert!((frame.width - 1920.0).abs() < 0.01);
        assert!((frame.height - 1080.0).abs() < 0.01);
        assert!((frame.x - 0.0).abs() < 0.01);
        assert!((frame.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_clamps_position_to_screen() {
        let preset = FloatingPreset {
            name: "off-screen".to_string(),
            width: DimensionValue::Pixels(800),
            height: DimensionValue::Pixels(600),
            x: Some(DimensionValue::Pixels(2000)), // Beyond screen edge
            y: Some(DimensionValue::Pixels(1000)),
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        // Position should be clamped to keep window on screen
        // Max x = 1920 - 800 = 1120
        // Max y = 1080 - 600 = 480
        assert!((frame.x - 1120.0).abs() < 0.01);
        assert!((frame.y - 480.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_default_position_when_not_specified() {
        let preset = FloatingPreset {
            name: "no-pos".to_string(),
            width: DimensionValue::Pixels(800),
            height: DimensionValue::Pixels(600),
            x: None,
            y: None,
            center: false,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        // When not centered and no position specified, defaults to top-left (usable area origin)
        assert!((frame.x - 0.0).abs() < 0.01);
        assert!((frame.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_with_screen_offset() {
        // Simulate a secondary screen that's positioned to the right of the main screen
        let screen = Rect::new(1920.0, 0.0, 1920.0, 1080.0);

        let preset = FloatingPreset {
            name: "center".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("50%".to_string()),
            x: None,
            y: None,
            center: true,
        };

        let frame = calculate_preset_frame(&preset, &screen, &make_zero_gaps());

        // Width/height: 50% with inner gap = (1920 - 0) / 2 = 960, (1080 - 0) / 2 = 540
        assert!((frame.width - 960.0).abs() < 0.01);
        assert!((frame.height - 540.0).abs() < 0.01);

        // Centered on the secondary screen
        // x = 1920 + (1920 - 960) / 2 = 1920 + 480 = 2400
        // y = 0 + (1080 - 540) / 2 = 270
        assert!((frame.x - 2400.0).abs() < 0.01);
        assert!((frame.y - 270.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_percentage_over_100() {
        let preset = FloatingPreset {
            name: "over-100".to_string(),
            width: DimensionValue::Percentage("150%".to_string()),
            height: DimensionValue::Percentage("150%".to_string()),
            x: None,
            y: None,
            center: true,
        };

        let frame = calculate_preset_frame(&preset, &make_screen(), &make_zero_gaps());

        // Should clamp to screen size
        assert!((frame.width - 1920.0).abs() < 0.01);
        assert!((frame.height - 1080.0).abs() < 0.01);
    }

    #[test]
    fn test_is_half_percentage() {
        assert!(is_half_percentage(&DimensionValue::Percentage(
            "50%".to_string()
        )));
        assert!(is_half_percentage(&DimensionValue::Percentage("50".to_string())));
        assert!(is_half_percentage(&DimensionValue::Percentage(
            " 50% ".to_string()
        )));
        assert!(!is_half_percentage(&DimensionValue::Percentage(
            "49%".to_string()
        )));
        assert!(!is_half_percentage(&DimensionValue::Percentage(
            "51%".to_string()
        )));
        assert!(!is_half_percentage(&DimensionValue::Percentage(
            "100%".to_string()
        )));
        assert!(!is_half_percentage(&DimensionValue::Pixels(50)));
    }
}
