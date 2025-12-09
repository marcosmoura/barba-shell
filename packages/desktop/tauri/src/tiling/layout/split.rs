//! Split layout implementation.
//!
//! The split layout divides the screen between two windows. It supports three modes:
//! - **Split**: Automatically determines orientation based on screen aspect ratio
//!   (vertical split for landscape, horizontal split for portrait)
//! - **`SplitVertical`**: Windows are placed side by side (left/right)
//! - **`SplitHorizontal`**: Windows are stacked top/bottom
//!
//! When there are more than 2 windows, only the first two are visible in split mode.
//! When there is only 1 window, it takes the full screen area.
//!
//! Example (vertical split):
//! ```text
//! +--------+--------+
//! |        |        |
//! |   1    |   2    |
//! |        |        |
//! +--------+--------+
//! ```
//!

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
//! Example (horizontal split):
//! ```text
//! +----------------+
//! |       1        |
//! +----------------+
//! |       2        |
//! +----------------+
//! ```

use super::traits::{Layout, LayoutContext, LayoutResult, LayoutWindow, WindowLayout};
use crate::tiling::state::WindowFrame;

/// Split orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrientation {
    /// Windows are placed side by side (left/right).
    Vertical,
    /// Windows are stacked top/bottom.
    Horizontal,
    /// Automatically determine based on screen dimensions.
    Auto,
}

/// Split layout - divides the screen between two windows.
#[derive(Debug)]
pub struct SplitLayout {
    orientation: SplitOrientation,
}

impl SplitLayout {
    /// Creates a split layout that auto-detects orientation based on screen dimensions.
    #[must_use]
    pub const fn auto() -> Self {
        Self {
            orientation: SplitOrientation::Auto,
        }
    }

    /// Creates a vertical split layout (windows side by side).
    #[must_use]
    pub const fn vertical() -> Self {
        Self {
            orientation: SplitOrientation::Vertical,
        }
    }

    /// Creates a horizontal split layout (windows stacked).
    #[must_use]
    pub const fn horizontal() -> Self {
        Self {
            orientation: SplitOrientation::Horizontal,
        }
    }

    /// Determines the effective orientation based on screen dimensions.
    const fn effective_orientation(&self, frame: &WindowFrame) -> SplitOrientation {
        match self.orientation {
            SplitOrientation::Auto => {
                // Use vertical split (side by side) for landscape screens (wider than tall)
                // Use horizontal split (stacked) for portrait screens (taller than wide)
                if frame.width >= frame.height {
                    SplitOrientation::Vertical
                } else {
                    SplitOrientation::Horizontal
                }
            }
            other => other,
        }
    }

    /// Computes layouts for a vertical split (windows arranged in columns, side by side).
    /// All windows share the width equally.
    #[allow(clippy::unused_self)]
    fn layout_vertical(
        &self,
        windows: &[&LayoutWindow],
        frame: WindowFrame,
        inner_gap: u32,
    ) -> Vec<WindowLayout> {
        if windows.is_empty() {
            return Vec::new();
        }

        if windows.len() == 1 {
            return vec![WindowLayout { id: windows[0].id, frame }];
        }

        let window_count = windows.len() as u32;
        let total_gaps = inner_gap * (window_count - 1);
        let available_width = frame.width.saturating_sub(total_gaps);
        let base_width = available_width / window_count;
        let extra_pixels = available_width % window_count;

        let mut layouts = Vec::with_capacity(windows.len());
        let mut current_x = frame.x;

        for (i, window) in windows.iter().enumerate() {
            // Distribute extra pixels to the first windows
            let window_width = if (i as u32) < extra_pixels {
                base_width + 1
            } else {
                base_width
            };

            layouts.push(WindowLayout {
                id: window.id,
                frame: WindowFrame {
                    x: current_x,
                    y: frame.y,
                    width: window_width,
                    height: frame.height,
                },
            });

            current_x += window_width as i32 + inner_gap as i32;
        }

        layouts
    }

    /// Computes layouts for a horizontal split (windows arranged in rows, stacked).
    /// All windows share the height equally.
    #[allow(clippy::unused_self)]
    fn layout_horizontal(
        &self,
        windows: &[&LayoutWindow],
        frame: WindowFrame,
        inner_gap: u32,
    ) -> Vec<WindowLayout> {
        if windows.is_empty() {
            return Vec::new();
        }

        if windows.len() == 1 {
            return vec![WindowLayout { id: windows[0].id, frame }];
        }

        let window_count = windows.len() as u32;
        let total_gaps = inner_gap * (window_count - 1);
        let available_height = frame.height.saturating_sub(total_gaps);
        let base_height = available_height / window_count;
        let extra_pixels = available_height % window_count;

        let mut layouts = Vec::with_capacity(windows.len());
        let mut current_y = frame.y;

        for (i, window) in windows.iter().enumerate() {
            // Distribute extra pixels to the first windows
            let window_height = if (i as u32) < extra_pixels {
                base_height + 1
            } else {
                base_height
            };

            layouts.push(WindowLayout {
                id: window.id,
                frame: WindowFrame {
                    x: frame.x,
                    y: current_y,
                    width: frame.width,
                    height: window_height,
                },
            });

            current_y += window_height as i32 + inner_gap as i32;
        }

        layouts
    }
}

impl Default for SplitLayout {
    fn default() -> Self { Self::auto() }
}

impl Layout for SplitLayout {
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

        // Determine effective orientation and compute layout
        let orientation = self.effective_orientation(&frame);
        let layouts = match orientation {
            SplitOrientation::Vertical | SplitOrientation::Auto => {
                self.layout_vertical(&layoutable, frame, context.gaps.inner)
            }
            SplitOrientation::Horizontal => {
                self.layout_horizontal(&layoutable, frame, context.gaps.inner)
            }
        };

        Ok(layouts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiling::layout::traits::ResolvedGaps;
    use crate::tiling::state::ScreenFrame;

    fn create_landscape_context() -> LayoutContext {
        LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 25, // Menu bar offset
                width: 1920,
                height: 1055,
            },
            gaps: ResolvedGaps {
                outer_top: 10,
                outer_bottom: 10,
                outer_left: 10,
                outer_right: 10,
                inner: 10,
            },
            split_ratios: Vec::new(),
        }
    }

    fn create_portrait_context() -> LayoutContext {
        LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 25,
                width: 1080,
                height: 1920,
            },
            gaps: ResolvedGaps {
                outer_top: 10,
                outer_bottom: 10,
                outer_left: 10,
                outer_right: 10,
                inner: 10,
            },
            split_ratios: Vec::new(),
        }
    }

    fn create_test_windows(count: usize) -> Vec<LayoutWindow> {
        (1..=count)
            .map(|i| LayoutWindow {
                id: i as u64,
                is_floating: false,
                is_minimized: false,
                is_fullscreen: false,
            })
            .collect()
    }

    #[test]
    fn test_split_empty() {
        let layout = SplitLayout::auto();
        let context = create_landscape_context();

        let result = layout.layout(&[], &context).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_split_single_window() {
        let layout = SplitLayout::auto();
        let context = create_landscape_context();
        let windows = create_test_windows(1);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 1);

        let window_layout = &result[0];
        assert_eq!(window_layout.id, 1);
        // Full usable area after outer gaps
        assert_eq!(window_layout.frame.x, 10);
        assert_eq!(window_layout.frame.y, 35); // 25 + 10
        assert_eq!(window_layout.frame.width, 1900); // 1920 - 10 - 10
        assert_eq!(window_layout.frame.height, 1035); // 1055 - 10 - 10
    }

    #[test]
    fn test_split_auto_landscape() {
        let layout = SplitLayout::auto();
        let context = create_landscape_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        // Should be vertical split (side by side) for landscape
        let left = &result[0];
        let right = &result[1];

        // Left window
        assert_eq!(left.id, 1);
        assert_eq!(left.frame.x, 10);
        assert_eq!(left.frame.y, 35);
        assert_eq!(left.frame.width, 945); // (1900 - 10) / 2 = 945
        assert_eq!(left.frame.height, 1035);

        // Right window
        assert_eq!(right.id, 2);
        assert_eq!(right.frame.x, 965); // 10 + 945 + 10
        assert_eq!(right.frame.y, 35);
        assert_eq!(right.frame.width, 945);
        assert_eq!(right.frame.height, 1035);
    }

    #[test]
    fn test_split_auto_portrait() {
        let layout = SplitLayout::auto();
        let context = create_portrait_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        // Should be horizontal split (stacked) for portrait
        let top = &result[0];
        let bottom = &result[1];

        // Top window
        assert_eq!(top.id, 1);
        assert_eq!(top.frame.x, 10);
        assert_eq!(top.frame.y, 35); // 25 + 10
        assert_eq!(top.frame.width, 1060); // 1080 - 10 - 10

        // Bottom window
        assert_eq!(bottom.id, 2);
        assert_eq!(bottom.frame.x, 10);
        assert_eq!(bottom.frame.width, 1060);
    }

    #[test]
    fn test_split_vertical_explicit() {
        let layout = SplitLayout::vertical();
        let context = create_landscape_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        // Should be side by side
        assert_eq!(result[0].frame.y, result[1].frame.y);
        assert!(result[0].frame.x < result[1].frame.x);
    }

    #[test]
    fn test_split_horizontal_explicit() {
        let layout = SplitLayout::horizontal();
        let context = create_landscape_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        // Should be stacked
        assert_eq!(result[0].frame.x, result[1].frame.x);
        assert!(result[0].frame.y < result[1].frame.y);
    }

    #[test]
    fn test_split_more_than_two_windows() {
        let layout = SplitLayout::vertical();
        let context = create_landscape_context();
        let windows = create_test_windows(3);

        let result = layout.layout(&windows, &context).unwrap();
        // All windows should be laid out
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
        assert_eq!(result[2].id, 3);

        // All windows should be side by side (same y, different x)
        assert_eq!(result[0].frame.y, result[1].frame.y);
        assert_eq!(result[1].frame.y, result[2].frame.y);
        assert!(result[0].frame.x < result[1].frame.x);
        assert!(result[1].frame.x < result[2].frame.x);

        // All windows should have same height
        assert_eq!(result[0].frame.height, result[1].frame.height);
        assert_eq!(result[1].frame.height, result[2].frame.height);
    }

    #[test]
    fn test_split_horizontal_three_windows() {
        let layout = SplitLayout::horizontal();
        let context = create_landscape_context();
        let windows = create_test_windows(3);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 3);

        // All windows should be stacked (same x, different y)
        assert_eq!(result[0].frame.x, result[1].frame.x);
        assert_eq!(result[1].frame.x, result[2].frame.x);
        assert!(result[0].frame.y < result[1].frame.y);
        assert!(result[1].frame.y < result[2].frame.y);

        // All windows should have same width
        assert_eq!(result[0].frame.width, result[1].frame.width);
        assert_eq!(result[1].frame.width, result[2].frame.width);
    }

    #[test]
    fn test_split_five_windows() {
        let layout = SplitLayout::vertical();
        let context = create_landscape_context();
        let windows = create_test_windows(5);

        let result = layout.layout(&windows, &context).unwrap();
        // All 5 windows should be laid out
        assert_eq!(result.len(), 5);

        // Verify all window IDs are present and in columns (same y, increasing x)
        for i in 1..=5 {
            assert!(result.iter().any(|w| w.id == i as u64));
        }

        // All windows should have same y (same row)
        let first_y = result[0].frame.y;
        for layout in &result {
            assert_eq!(layout.frame.y, first_y);
        }

        // Windows should be in increasing x order
        for i in 1..result.len() {
            assert!(result[i].frame.x > result[i - 1].frame.x);
        }
    }

    #[test]
    fn test_split_skips_floating_windows() {
        let layout = SplitLayout::auto();
        let context = create_landscape_context();

        let windows = vec![
            LayoutWindow {
                id: 1,
                is_floating: true,
                is_minimized: false,
                is_fullscreen: false,
            },
            LayoutWindow {
                id: 2,
                is_floating: false,
                is_minimized: false,
                is_fullscreen: false,
            },
            LayoutWindow {
                id: 3,
                is_floating: false,
                is_minimized: false,
                is_fullscreen: false,
            },
        ];

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, 2);
        assert_eq!(result[1].id, 3);
    }
}
