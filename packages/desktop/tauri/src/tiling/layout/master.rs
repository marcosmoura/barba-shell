//! Master layout implementation.
//!
//! The master layout dedicates a configurable portion of the screen to one or more
//! "master" windows, while the remaining windows are stacked in the remaining space.
//!
//! The layout automatically adapts based on screen aspect ratio:
//! - **Landscape (width >= height)**: Master on the left, stack on the right (stacked vertically)
//! - **Portrait (height > width)**: Master on the top, stack on the bottom (stacked horizontally)
//!
//! Configuration options:
//! - `ratio`: Percentage of screen for the master area (0-100, default 50)
//! - `max_masters`: Maximum number of windows in the master area (default 1)
//!
//! Example with 1 master and 3 stack windows (ratio=50, landscape):
//! ```text
//! +----------+----------+
//! |          |    2     |
//! |          +----------+
//! |    1     |    3     |
//! |          +----------+
//! |          |    4     |
//! +----------+----------+
//! ```
//!
//! Example with 1 master and 3 stack windows (ratio=50, portrait):
//! ```text
//! +--------------------+
//! |         1          |
//! +------+------+------+
//! |  2   |  3   |  4   |
//! +------+------+------+
//! ```

use barba_shared::MasterConfig;

use super::traits::{Layout, LayoutContext, LayoutResult, LayoutWindow, WindowLayout};
use crate::tiling::state::WindowFrame;

/// Master layout - adapts orientation based on screen aspect ratio.
///
/// - Landscape screens: master on left, stack on right (stacked vertically)
/// - Portrait screens: master on top, stack on bottom (stacked horizontally)
#[derive(Debug)]
pub struct MasterLayout {
    config: MasterConfig,
}

impl MasterLayout {
    /// Creates a new master layout with the given configuration.
    #[must_use]
    pub const fn new(config: MasterConfig) -> Self { Self { config } }

    /// Creates a master layout with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            config: MasterConfig::default(),
        }
    }

    /// Determines if the screen is portrait orientation (height > width).
    const fn is_portrait(frame: &WindowFrame) -> bool { frame.height > frame.width }

    /// Computes the master area size (width for landscape, height for portrait).
    /// If a `split_ratio` is provided, it overrides the config ratio.
    fn master_size(&self, total_size: u32, inner_gap: u32, split_ratio: Option<f64>) -> u32 {
        let ratio = if let Some(r) = split_ratio {
            // Convert 0.0-1.0 ratio to 0-100 percentage
            (r * 100.0).clamp(10.0, 90.0) as u32
        } else {
            self.config.ratio.min(100)
        };
        let available_size = total_size.saturating_sub(inner_gap);
        (available_size * ratio) / 100
    }

    /// Lays out windows stacked vertically within a frame.
    fn layout_vertical_stack(
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

    /// Lays out windows stacked horizontally within a frame.
    fn layout_horizontal_stack(
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

    /// Lays out windows in landscape mode (master left, stack right).
    fn layout_landscape(
        &self,
        master_windows: &[&LayoutWindow],
        stack_windows: &[&LayoutWindow],
        frame: WindowFrame,
        inner_gap: u32,
        split_ratio: Option<f64>,
    ) -> Vec<WindowLayout> {
        let master_width = self.master_size(frame.width, inner_gap, split_ratio);
        let stack_width = frame.width.saturating_sub(master_width + inner_gap);

        // Master area frame (left side)
        let master_frame = WindowFrame {
            x: frame.x,
            y: frame.y,
            width: master_width,
            height: frame.height,
        };

        // Stack area frame (right side)
        let stack_frame = WindowFrame {
            x: frame.x + master_width as i32 + inner_gap as i32,
            y: frame.y,
            width: stack_width,
            height: frame.height,
        };

        // Layout master area (stacked vertically)
        let mut layouts = Self::layout_vertical_stack(master_windows, master_frame, inner_gap);
        // Layout stack area (stacked vertically)
        layouts.extend(Self::layout_vertical_stack(
            stack_windows,
            stack_frame,
            inner_gap,
        ));

        layouts
    }

    /// Lays out windows in portrait mode (master top, stack bottom).
    fn layout_portrait(
        &self,
        master_windows: &[&LayoutWindow],
        stack_windows: &[&LayoutWindow],
        frame: WindowFrame,
        inner_gap: u32,
        split_ratio: Option<f64>,
    ) -> Vec<WindowLayout> {
        let master_height = self.master_size(frame.height, inner_gap, split_ratio);
        let stack_height = frame.height.saturating_sub(master_height + inner_gap);

        // Master area frame (top)
        let master_frame = WindowFrame {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: master_height,
        };

        // Stack area frame (bottom)
        let stack_frame = WindowFrame {
            x: frame.x,
            y: frame.y + master_height as i32 + inner_gap as i32,
            width: frame.width,
            height: stack_height,
        };

        // Layout master area (stacked horizontally for portrait)
        let mut layouts = Self::layout_horizontal_stack(master_windows, master_frame, inner_gap);
        // Layout stack area (stacked horizontally for portrait)
        layouts.extend(Self::layout_horizontal_stack(
            stack_windows,
            stack_frame,
            inner_gap,
        ));

        layouts
    }
}

impl Default for MasterLayout {
    fn default() -> Self { Self::with_defaults() }
}

impl Layout for MasterLayout {
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
        let inner_gap = context.gaps.inner;

        // If only one window, it takes the full frame
        if layoutable.len() == 1 {
            return Ok(vec![WindowLayout { id: layoutable[0].id, frame }]);
        }

        // Determine how many windows go in the master area
        let max_masters = self.config.max_masters.max(1) as usize;
        let master_count = layoutable.len().min(max_masters);

        // Split windows into master and stack
        let (master_windows, stack_windows) = layoutable.split_at(master_count);

        // If all windows fit in master area, just stack them appropriately
        if stack_windows.is_empty() {
            // Stack based on orientation
            return Ok(if Self::is_portrait(&frame) {
                Self::layout_horizontal_stack(master_windows, frame, inner_gap)
            } else {
                Self::layout_vertical_stack(master_windows, frame, inner_gap)
            });
        }

        // Get the master/stack split ratio from context (first ratio controls master size)
        // This allows dynamic resizing of the master area
        let master_split_ratio = context.split_ratios.first().copied();

        // Layout based on screen aspect ratio
        Ok(if Self::is_portrait(&frame) {
            // Portrait: master on top, stack on bottom (stacked horizontally)
            self.layout_portrait(
                master_windows,
                stack_windows,
                frame,
                inner_gap,
                master_split_ratio,
            )
        } else {
            // Landscape: master on left, stack on right (stacked vertically)
            self.layout_landscape(
                master_windows,
                stack_windows,
                frame,
                inner_gap,
                master_split_ratio,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiling::layout::traits::ResolvedGaps;
    use crate::tiling::state::ScreenFrame;

    fn create_test_context() -> LayoutContext {
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
    fn test_master_empty() {
        let layout = MasterLayout::with_defaults();
        let context = create_test_context();

        let result = layout.layout(&[], &context).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_master_single_window() {
        let layout = MasterLayout::with_defaults();
        let context = create_test_context();
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
    fn test_master_two_windows_default_ratio() {
        let layout = MasterLayout::with_defaults(); // 50% ratio
        let context = create_test_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        let master = &result[0];
        let stack = &result[1];

        // Master window on left
        assert_eq!(master.id, 1);
        assert_eq!(master.frame.x, 10);
        assert_eq!(master.frame.y, 35);

        // Stack window on right
        assert_eq!(stack.id, 2);
        assert!(stack.frame.x > master.frame.x);
        assert_eq!(stack.frame.y, 35);

        // Both should have full height
        assert_eq!(master.frame.height, 1035);
        assert_eq!(stack.frame.height, 1035);
    }

    #[test]
    fn test_master_three_windows() {
        let layout = MasterLayout::with_defaults();
        let context = create_test_context();
        let windows = create_test_windows(3);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 3);

        // First window is master (left, full height)
        assert_eq!(result[0].id, 1);

        // Other two are stacked on right
        assert_eq!(result[1].id, 2);
        assert_eq!(result[2].id, 3);

        // Stack windows should have same x (right side)
        assert_eq!(result[1].frame.x, result[2].frame.x);

        // Stack windows should be vertically stacked
        assert!(result[1].frame.y < result[2].frame.y);
    }

    #[test]
    fn test_master_custom_ratio() {
        let config = MasterConfig { ratio: 70, max_masters: 1 };
        let layout = MasterLayout::new(config);
        let context = create_test_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        let master = &result[0];
        let stack = &result[1];

        // Master should be larger than stack with 70% ratio
        // Available width after outer gaps = 1900, minus inner gap = 1890
        // Master = 1890 * 0.70 = 1323
        assert!(master.frame.width > stack.frame.width);
    }

    #[test]
    fn test_master_multiple_masters() {
        let config = MasterConfig { ratio: 50, max_masters: 2 };
        let layout = MasterLayout::new(config);
        let context = create_test_context();
        let windows = create_test_windows(4);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 4);

        // First two windows are masters (left side, stacked)
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
        assert_eq!(result[0].frame.x, result[1].frame.x);
        assert!(result[0].frame.y < result[1].frame.y);

        // Last two windows are stack (right side, stacked)
        assert_eq!(result[2].id, 3);
        assert_eq!(result[3].id, 4);
        assert_eq!(result[2].frame.x, result[3].frame.x);
        assert!(result[2].frame.y < result[3].frame.y);

        // Stack should be to the right of master
        assert!(result[2].frame.x > result[0].frame.x);
    }

    #[test]
    fn test_master_skips_floating_windows() {
        let layout = MasterLayout::with_defaults();
        let context = create_test_context();

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

    #[test]
    fn test_master_all_windows_fit_in_master() {
        let config = MasterConfig { ratio: 50, max_masters: 5 };
        let layout = MasterLayout::new(config);
        let context = create_test_context();
        let windows = create_test_windows(3);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 3);

        // All windows should be stacked vertically (no stack area)
        // All should have the same x position
        assert_eq!(result[0].frame.x, result[1].frame.x);
        assert_eq!(result[1].frame.x, result[2].frame.x);

        // All should have the full width
        assert_eq!(result[0].frame.width, 1900);
    }

    // ========================================================================
    // Portrait Mode Tests
    // ========================================================================

    fn create_portrait_context() -> LayoutContext {
        LayoutContext {
            screen_frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1080, // Portrait: width < height
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

    #[test]
    fn test_master_portrait_single_window() {
        let layout = MasterLayout::with_defaults();
        let context = create_portrait_context();
        let windows = create_test_windows(1);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 1);

        let window_layout = &result[0];
        assert_eq!(window_layout.id, 1);
        // Full usable area after outer gaps
        assert_eq!(window_layout.frame.x, 10);
        assert_eq!(window_layout.frame.y, 10);
        assert_eq!(window_layout.frame.width, 1060); // 1080 - 10 - 10
        assert_eq!(window_layout.frame.height, 1900); // 1920 - 10 - 10
    }

    #[test]
    fn test_master_portrait_two_windows() {
        let layout = MasterLayout::with_defaults(); // 50% ratio
        let context = create_portrait_context();
        let windows = create_test_windows(2);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 2);

        let master = &result[0];
        let stack = &result[1];

        // Master window on top
        assert_eq!(master.id, 1);
        assert_eq!(master.frame.x, 10);
        assert_eq!(master.frame.y, 10);

        // Stack window on bottom
        assert_eq!(stack.id, 2);
        assert!(stack.frame.y > master.frame.y);
        assert_eq!(stack.frame.x, 10);

        // Both should have full width
        assert_eq!(master.frame.width, 1060);
        assert_eq!(stack.frame.width, 1060);
    }

    #[test]
    fn test_master_portrait_three_windows() {
        let layout = MasterLayout::with_defaults();
        let context = create_portrait_context();
        let windows = create_test_windows(3);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 3);

        // First window is master (top, full width)
        assert_eq!(result[0].id, 1);

        // Other two are stacked on bottom (side by side, horizontally)
        assert_eq!(result[1].id, 2);
        assert_eq!(result[2].id, 3);

        // Stack windows should have same y (bottom)
        assert_eq!(result[1].frame.y, result[2].frame.y);

        // Stack windows should be horizontally stacked
        assert!(result[1].frame.x < result[2].frame.x);

        // Master should be above the stack
        assert!(result[0].frame.y < result[1].frame.y);
    }

    #[test]
    fn test_master_portrait_multiple_masters() {
        let config = MasterConfig { ratio: 50, max_masters: 2 };
        let layout = MasterLayout::new(config);
        let context = create_portrait_context();
        let windows = create_test_windows(4);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 4);

        // First two windows are masters (top, side by side)
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
        assert_eq!(result[0].frame.y, result[1].frame.y);
        assert!(result[0].frame.x < result[1].frame.x);

        // Last two windows are stack (bottom, side by side)
        assert_eq!(result[2].id, 3);
        assert_eq!(result[3].id, 4);
        assert_eq!(result[2].frame.y, result[3].frame.y);
        assert!(result[2].frame.x < result[3].frame.x);

        // Stack should be below master
        assert!(result[2].frame.y > result[0].frame.y);
    }

    #[test]
    fn test_master_portrait_all_windows_fit_in_master() {
        let config = MasterConfig { ratio: 50, max_masters: 5 };
        let layout = MasterLayout::new(config);
        let context = create_portrait_context();
        let windows = create_test_windows(3);

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 3);

        // All windows should be stacked horizontally (no stack area)
        // All should have the same y position
        assert_eq!(result[0].frame.y, result[1].frame.y);
        assert_eq!(result[1].frame.y, result[2].frame.y);

        // All should have the full height
        assert_eq!(result[0].frame.height, 1900);
    }

    #[test]
    fn test_is_portrait() {
        let landscape = WindowFrame {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert!(!MasterLayout::is_portrait(&landscape));

        let portrait = WindowFrame {
            x: 0,
            y: 0,
            width: 1080,
            height: 1920,
        };
        assert!(MasterLayout::is_portrait(&portrait));

        let square = WindowFrame {
            x: 0,
            y: 0,
            width: 1000,
            height: 1000,
        };
        assert!(!MasterLayout::is_portrait(&square)); // Square is treated as landscape
    }
}
