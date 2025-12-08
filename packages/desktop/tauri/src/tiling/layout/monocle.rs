//! Monocle layout implementation.
//!
//! The monocle layout displays one window at a time, filling the entire
//! usable screen area. All other windows are hidden behind the focused one.

use super::traits::{Layout, LayoutContext, LayoutResult, LayoutWindow, WindowLayout};

/// Monocle layout - one window fills the screen.
#[derive(Debug, Default)]
pub struct MonocleLayout;

impl MonocleLayout {
    /// Creates a new monocle layout instance.
    #[must_use]
    pub const fn new() -> Self { Self }
}

impl Layout for MonocleLayout {
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

        // All windows get the same frame - they'll be stacked
        let layouts: Vec<WindowLayout> =
            layoutable.iter().map(|w| WindowLayout { id: w.id, frame }).collect();

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
                y: 25, // Menu bar offset
                width: 1920,
                height: 1055,
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
    fn test_monocle_empty() {
        let layout = MonocleLayout::new();
        let context = create_test_context();

        let result = layout.layout(&[], &context).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_monocle_single_window() {
        let layout = MonocleLayout::new();
        let context = create_test_context();

        let windows = vec![LayoutWindow {
            id: 1,
            is_floating: false,
            is_minimized: false,
            is_fullscreen: false,
        }];

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 1);

        let window_layout = &result[0];
        assert_eq!(window_layout.id, 1);
        assert_eq!(window_layout.frame.x, 10);
        assert_eq!(window_layout.frame.y, 35); // 25 + 10
        assert_eq!(window_layout.frame.width, 1900); // 1920 - 10 - 10
        assert_eq!(window_layout.frame.height, 1035); // 1055 - 10 - 10
    }

    #[test]
    fn test_monocle_multiple_windows() {
        let layout = MonocleLayout::new();
        let context = create_test_context();

        let windows = vec![
            LayoutWindow {
                id: 1,
                is_floating: false,
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
        assert_eq!(result.len(), 3);

        // All windows should have the same frame
        for (i, window_layout) in result.iter().enumerate() {
            assert_eq!(window_layout.id, (i + 1) as u64);
            assert_eq!(window_layout.frame.x, 10);
            assert_eq!(window_layout.frame.y, 35);
            assert_eq!(window_layout.frame.width, 1900);
            assert_eq!(window_layout.frame.height, 1035);
        }
    }

    #[test]
    fn test_monocle_skips_floating() {
        let layout = MonocleLayout::new();
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
        ];

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 2);
    }

    #[test]
    fn test_monocle_skips_minimized() {
        let layout = MonocleLayout::new();
        let context = create_test_context();

        let windows = vec![
            LayoutWindow {
                id: 1,
                is_floating: false,
                is_minimized: true,
                is_fullscreen: false,
            },
            LayoutWindow {
                id: 2,
                is_floating: false,
                is_minimized: false,
                is_fullscreen: false,
            },
        ];

        let result = layout.layout(&windows, &context).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 2);
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
    fn test_monocle_with_outer_gaps() {
        let layout = MonocleLayout::new();
        let context = create_context_with_gaps();

        let windows = vec![LayoutWindow {
            id: 1,
            is_floating: false,
            is_minimized: false,
            is_fullscreen: false,
        }];

        let result = layout.layout(&windows, &context).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
        // With 10px outer gaps on all sides
        assert_eq!(result[0].frame.x, 10);
        assert_eq!(result[0].frame.y, 10);
        assert_eq!(result[0].frame.width, 1900); // 1920 - 10 - 10
        assert_eq!(result[0].frame.height, 1060); // 1080 - 10 - 10
    }
}
