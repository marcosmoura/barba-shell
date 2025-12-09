//! Tiling layout implementation using the dwindle algorithm.
//!
//! The dwindle algorithm recursively splits the available space in alternating
//! directions (horizontal/vertical). Each new window takes half of the remaining
//! space, creating a fibonacci-like spiral pattern.
//!
//! Special cases:
//! - 4 windows: Uses a 2x2 grid for more balanced layout
//!
//! Example with 5 windows:
//! ```text
//! +-------+-------+
//! |       |   2   |
//! |   1   +---+---+
//! |       | 3 | 4 |
//! |       |   +---+
//! |       |   | 5 |
//! +-------+---+---+
//! ```

#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]

use super::traits::{Layout, LayoutContext, LayoutResult, LayoutWindow, WindowLayout};
use crate::tiling::state::WindowFrame;

/// Represents the direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitDirection {
    /// Split horizontally (windows side by side).
    Horizontal,
    /// Split vertically (windows stacked).
    Vertical,
}

impl SplitDirection {
    /// Returns the opposite direction.
    const fn toggle(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// Tiling layout using the dwindle algorithm.
#[derive(Debug, Default)]
pub struct TilingLayout;

impl TilingLayout {
    /// Creates a new tiling layout instance.
    #[must_use]
    pub const fn new() -> Self { Self }

    /// Lays out exactly 4 windows in a 2x2 grid.
    /// Order: top-left, top-right, bottom-left, bottom-right
    fn grid_2x2(
        windows: &[&LayoutWindow],
        frame: WindowFrame,
        inner_gap: u32,
    ) -> Vec<WindowLayout> {
        debug_assert!(windows.len() == 4, "grid_2x2 requires exactly 4 windows");

        let half_gap = inner_gap / 2;
        let left_width = frame.width / 2 - half_gap;
        let right_width = frame.width - left_width - inner_gap;
        let top_height = frame.height / 2 - half_gap;
        let bottom_height = frame.height - top_height - inner_gap;

        vec![
            // Top-left
            WindowLayout {
                id: windows[0].id,
                frame: WindowFrame {
                    x: frame.x,
                    y: frame.y,
                    width: left_width,
                    height: top_height,
                },
            },
            // Top-right
            WindowLayout {
                id: windows[1].id,
                frame: WindowFrame {
                    x: frame.x + left_width as i32 + inner_gap as i32,
                    y: frame.y,
                    width: right_width,
                    height: top_height,
                },
            },
            // Bottom-left
            WindowLayout {
                id: windows[2].id,
                frame: WindowFrame {
                    x: frame.x,
                    y: frame.y + top_height as i32 + inner_gap as i32,
                    width: left_width,
                    height: bottom_height,
                },
            },
            // Bottom-right
            WindowLayout {
                id: windows[3].id,
                frame: WindowFrame {
                    x: frame.x + left_width as i32 + inner_gap as i32,
                    y: frame.y + top_height as i32 + inner_gap as i32,
                    width: right_width,
                    height: bottom_height,
                },
            },
        ]
    }

    /// Recursively computes layouts for windows using dwindle algorithm.
    ///
    /// # Arguments
    /// * `windows` - Windows to layout
    /// * `frame` - Available frame for this subtree
    /// * `direction` - Current split direction
    /// * `inner_gap` - Gap between windows in pixels
    /// * `split_ratios` - Slice of ratios for each split level
    /// * `depth` - Current recursion depth (for indexing into `split_ratios`)
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    fn dwindle_layout_with_ratios(
        windows: &[&LayoutWindow],
        frame: WindowFrame,
        direction: SplitDirection,
        inner_gap: u32,
        split_ratios: &[f64],
        depth: usize,
    ) -> Vec<WindowLayout> {
        match windows.len() {
            0 => Vec::new(),
            1 => {
                // Single window takes the entire frame
                vec![WindowLayout { id: windows[0].id, frame }]
            }
            _ => {
                // Get the ratio for this split level (default to 0.5 if not specified)
                let ratio = split_ratios.get(depth).copied().unwrap_or(0.5);

                // Split the space for the first window and recursively handle the rest
                let (first_frame, rest_frame) =
                    Self::split_frame_with_ratio(&frame, direction, inner_gap, ratio);

                let mut layouts = vec![WindowLayout {
                    id: windows[0].id,
                    frame: first_frame,
                }];

                // Recursively layout remaining windows in the opposite direction
                let rest_layouts = Self::dwindle_layout_with_ratios(
                    &windows[1..],
                    rest_frame,
                    direction.toggle(),
                    inner_gap,
                    split_ratios,
                    depth + 1,
                );

                layouts.extend(rest_layouts);
                layouts
            }
        }
    }

    /// Splits a frame based on the direction and ratio.
    ///
    /// # Arguments
    /// * `frame` - The frame to split
    /// * `direction` - Split direction (horizontal or vertical)
    /// * `gap` - Gap between the two parts
    /// * `ratio` - Portion of space for the first part (0.0-1.0)
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    fn split_frame_with_ratio(
        frame: &WindowFrame,
        direction: SplitDirection,
        gap: u32,
        ratio: f64,
    ) -> (WindowFrame, WindowFrame) {
        // Clamp ratio to valid range
        let ratio = ratio.clamp(0.1, 0.9);

        match direction {
            SplitDirection::Horizontal => {
                // Split horizontally: first window on left, rest on right
                let total_width = frame.width.saturating_sub(gap);
                let left_width = (f64::from(total_width) * ratio) as u32;
                let right_width = total_width - left_width;

                let left = WindowFrame {
                    x: frame.x,
                    y: frame.y,
                    width: left_width,
                    height: frame.height,
                };

                let right = WindowFrame {
                    x: frame.x + left_width as i32 + gap as i32,
                    y: frame.y,
                    width: right_width,
                    height: frame.height,
                };

                (left, right)
            }
            SplitDirection::Vertical => {
                // Split vertically: first window on top, rest on bottom
                let total_height = frame.height.saturating_sub(gap);
                let top_height = (f64::from(total_height) * ratio) as u32;
                let bottom_height = total_height - top_height;

                let top = WindowFrame {
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: top_height,
                };

                let bottom = WindowFrame {
                    x: frame.x,
                    y: frame.y + top_height as i32 + gap as i32,
                    width: frame.width,
                    height: bottom_height,
                };

                (top, bottom)
            }
        }
    }
}

impl Layout for TilingLayout {
    fn layout(
        &self,
        windows: &[LayoutWindow],
        context: &LayoutContext,
    ) -> LayoutResult<Vec<WindowLayout>> {
        // Filter to only layoutable windows
        let layoutable: Vec<_> = windows
            .iter()
            .filter(|w| !w.is_floating && !w.is_minimized && !w.is_fullscreen)
            .collect();

        if layoutable.is_empty() {
            return Ok(Vec::new());
        }

        // Apply outer gaps to get the usable frame
        let frame = context.gaps.apply_to_screen(&context.screen_frame);

        // Get inner gap for spacing between windows
        let inner_gap = context.gaps.inner;

        // Choose initial split direction based on screen aspect ratio
        // For portrait/vertical monitors (height > width), start with vertical split
        // For landscape monitors (width >= height), start with horizontal split
        let initial_direction = if context.screen_frame.height > context.screen_frame.width {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };

        // Special case: 4 windows use a 2x2 grid for more balanced layout
        let layouts = if layoutable.len() == 4 {
            Self::grid_2x2(&layoutable, frame, inner_gap)
        } else {
            Self::dwindle_layout_with_ratios(
                &layoutable,
                frame,
                initial_direction,
                inner_gap,
                &context.split_ratios,
                0,
            )
        };

        Ok(layouts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiling::state::ScreenFrame;

    fn create_test_context() -> LayoutContext {
        LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            gaps: super::super::traits::ResolvedGaps::default(),
            split_ratios: Vec::new(),
        }
    }

    fn create_window(id: u64) -> LayoutWindow {
        LayoutWindow {
            id,
            is_floating: false,
            is_minimized: false,
            is_fullscreen: false,
        }
    }

    #[test]
    fn test_tiling_empty() {
        let layout = TilingLayout::new();
        let context = create_test_context();

        let result = layout.layout(&[], &context).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_tiling_single_window() {
        let layout = TilingLayout::new();
        let context = create_test_context();

        let windows = vec![create_window(1)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 0);
        assert_eq!(result[0].frame.y, 0);
        assert_eq!(result[0].frame.width, 1920);
        assert_eq!(result[0].frame.height, 1080);
    }

    #[test]
    fn test_tiling_two_windows() {
        let layout = TilingLayout::new();
        let context = create_test_context();

        let windows = vec![create_window(1), create_window(2)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 2);

        // First window takes left half
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 0);
        assert_eq!(result[0].frame.y, 0);
        assert_eq!(result[0].frame.width, 960);
        assert_eq!(result[0].frame.height, 1080);

        // Second window takes right half
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 960);
        assert_eq!(result[1].frame.y, 0);
        assert_eq!(result[1].frame.width, 960);
        assert_eq!(result[1].frame.height, 1080);
    }

    #[test]
    fn test_tiling_three_windows() {
        let layout = TilingLayout::new();
        let context = create_test_context();

        let windows = vec![create_window(1), create_window(2), create_window(3)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 3);

        // First window takes left half
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 0);
        assert_eq!(result[0].frame.width, 960);
        assert_eq!(result[0].frame.height, 1080);

        // Second window takes top-right quarter
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 960);
        assert_eq!(result[1].frame.y, 0);
        assert_eq!(result[1].frame.width, 960);
        assert_eq!(result[1].frame.height, 540);

        // Third window takes bottom-right quarter
        assert_eq!(result[2].id, 3);
        assert_eq!(result[2].frame.x, 960);
        assert_eq!(result[2].frame.y, 540);
        assert_eq!(result[2].frame.width, 960);
        assert_eq!(result[2].frame.height, 540);
    }

    #[test]
    fn test_tiling_four_windows() {
        let layout = TilingLayout::new();
        let context = create_test_context();

        let windows = vec![
            create_window(1),
            create_window(2),
            create_window(3),
            create_window(4),
        ];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 4);

        // 2x2 grid layout:
        // +-------+-------+
        // |   1   |   2   |
        // +-------+-------+
        // |   3   |   4   |
        // +-------+-------+

        // Top-left (window 1)
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 0);
        assert_eq!(result[0].frame.y, 0);
        assert_eq!(result[0].frame.width, 960);
        assert_eq!(result[0].frame.height, 540);

        // Top-right (window 2)
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 960);
        assert_eq!(result[1].frame.y, 0);
        assert_eq!(result[1].frame.width, 960);
        assert_eq!(result[1].frame.height, 540);

        // Bottom-left (window 3)
        assert_eq!(result[2].id, 3);
        assert_eq!(result[2].frame.x, 0);
        assert_eq!(result[2].frame.y, 540);
        assert_eq!(result[2].frame.width, 960);
        assert_eq!(result[2].frame.height, 540);

        // Bottom-right (window 4)
        assert_eq!(result[3].id, 4);
        assert_eq!(result[3].frame.x, 960);
        assert_eq!(result[3].frame.y, 540);
        assert_eq!(result[3].frame.width, 960);
        assert_eq!(result[3].frame.height, 540);
    }

    #[test]
    fn test_tiling_skips_floating() {
        let layout = TilingLayout::new();
        let context = create_test_context();

        let windows = vec![
            LayoutWindow {
                id: 1,
                is_floating: true,
                is_minimized: false,
                is_fullscreen: false,
            },
            create_window(2),
        ];

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 2);
        // Single window should take full screen
        assert_eq!(result[0].frame.width, 1920);
        assert_eq!(result[0].frame.height, 1080);
    }

    fn create_vertical_context() -> LayoutContext {
        LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1080,
                height: 1920,
            },
            gaps: super::super::traits::ResolvedGaps::default(),
            split_ratios: Vec::new(),
        }
    }

    #[test]
    fn test_tiling_two_windows_vertical_monitor() {
        let layout = TilingLayout::new();
        let context = create_vertical_context();

        let windows = vec![create_window(1), create_window(2)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 2);

        // On vertical monitor, first window takes top half
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 0);
        assert_eq!(result[0].frame.y, 0);
        assert_eq!(result[0].frame.width, 1080);
        assert_eq!(result[0].frame.height, 960);

        // Second window takes bottom half
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 0);
        assert_eq!(result[1].frame.y, 960);
        assert_eq!(result[1].frame.width, 1080);
        assert_eq!(result[1].frame.height, 960);
    }

    #[test]
    fn test_tiling_three_windows_vertical_monitor() {
        let layout = TilingLayout::new();
        let context = create_vertical_context();

        let windows = vec![create_window(1), create_window(2), create_window(3)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 3);

        // First window takes top half
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.y, 0);
        assert_eq!(result[0].frame.width, 1080);
        assert_eq!(result[0].frame.height, 960);

        // Second window takes bottom-left quarter
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 0);
        assert_eq!(result[1].frame.y, 960);
        assert_eq!(result[1].frame.width, 540);
        assert_eq!(result[1].frame.height, 960);

        // Third window takes bottom-right quarter
        assert_eq!(result[2].id, 3);
        assert_eq!(result[2].frame.x, 540);
        assert_eq!(result[2].frame.y, 960);
        assert_eq!(result[2].frame.width, 540);
        assert_eq!(result[2].frame.height, 960);
    }

    fn create_context_with_gaps() -> LayoutContext {
        LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            gaps: super::super::traits::ResolvedGaps {
                outer_top: 10,
                outer_bottom: 10,
                outer_left: 10,
                outer_right: 10,
                inner: 10,
            },
            split_ratios: Vec::new(),
        }
    }

    #[test]
    fn test_tiling_with_outer_gaps() {
        let layout = TilingLayout::new();
        let context = create_context_with_gaps();

        let windows = vec![create_window(1)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
        // With 10px outer gaps on all sides
        assert_eq!(result[0].frame.x, 10);
        assert_eq!(result[0].frame.y, 10);
        assert_eq!(result[0].frame.width, 1900); // 1920 - 10 - 10
        assert_eq!(result[0].frame.height, 1060); // 1080 - 10 - 10
    }

    #[test]
    fn test_tiling_two_windows_with_gaps() {
        let layout = TilingLayout::new();
        let context = create_context_with_gaps();

        let windows = vec![create_window(1), create_window(2)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 2);

        // Usable width: 1920 - 10 - 10 = 1900
        // With 10px inner gap: left = 1900/2 - 5 = 945, right = 1900 - 945 - 10 = 945

        // First window takes left half with outer gaps
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 10);
        assert_eq!(result[0].frame.y, 10);
        assert_eq!(result[0].frame.width, 945);
        assert_eq!(result[0].frame.height, 1060);

        // Second window takes right half
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 965); // 10 + 945 + 10
        assert_eq!(result[1].frame.y, 10);
        assert_eq!(result[1].frame.width, 945);
        assert_eq!(result[1].frame.height, 1060);
    }

    #[test]
    fn test_tiling_four_windows_with_gaps() {
        let layout = TilingLayout::new();
        let context = create_context_with_gaps();

        let windows = vec![
            create_window(1),
            create_window(2),
            create_window(3),
            create_window(4),
        ];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 4);

        // Usable area: 1900x1060 (after 10px outer gaps)
        // With 10px inner gaps for 2x2 grid:
        // Each cell: (1900-10)/2 = 945 width, (1060-10)/2 = 525 height

        // Top-left (window 1)
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.x, 10);
        assert_eq!(result[0].frame.y, 10);
        assert_eq!(result[0].frame.width, 945);
        assert_eq!(result[0].frame.height, 525);

        // Top-right (window 2)
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.x, 965);
        assert_eq!(result[1].frame.y, 10);
        assert_eq!(result[1].frame.width, 945);
        assert_eq!(result[1].frame.height, 525);

        // Bottom-left (window 3)
        assert_eq!(result[2].id, 3);
        assert_eq!(result[2].frame.x, 10);
        assert_eq!(result[2].frame.y, 545);
        assert_eq!(result[2].frame.width, 945);
        assert_eq!(result[2].frame.height, 525);

        // Bottom-right (window 4)
        assert_eq!(result[3].id, 4);
        assert_eq!(result[3].frame.x, 965);
        assert_eq!(result[3].frame.y, 545);
        assert_eq!(result[3].frame.width, 945);
        assert_eq!(result[3].frame.height, 525);
    }

    #[test]
    fn test_tiling_with_custom_split_ratio() {
        let layout = TilingLayout::new();

        // Create context with a 70/30 split ratio
        let context = LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1000,
                height: 1000,
            },
            gaps: super::super::traits::ResolvedGaps::default(),
            split_ratios: vec![0.7], // 70% for first split
        };

        let windows = vec![create_window(1), create_window(2)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 2);

        // First window should take 70% of width
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.width, 700);

        // Second window should take 30% of width
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.width, 300);
    }

    #[test]
    fn test_tiling_with_multiple_split_ratios() {
        let layout = TilingLayout::new();

        // Create context with multiple split ratios for dwindle
        let context = LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1000,
                height: 1000,
            },
            gaps: super::super::traits::ResolvedGaps::default(),
            split_ratios: vec![0.6, 0.5], // 60% first split, 50% second split
        };

        let windows = vec![create_window(1), create_window(2), create_window(3)];
        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 3);

        // First window should take 60% of width
        assert_eq!(result[0].id, 1);
        assert_eq!(result[0].frame.width, 600);

        // Second and third split the remaining 40% (400px) with 50/50
        assert_eq!(result[1].id, 2);
        assert_eq!(result[1].frame.width, 400); // Full remaining width
        assert_eq!(result[1].frame.height, 500); // 50% of height

        assert_eq!(result[2].id, 3);
        assert_eq!(result[2].frame.width, 400);
        assert_eq!(result[2].frame.height, 500);
    }
}
