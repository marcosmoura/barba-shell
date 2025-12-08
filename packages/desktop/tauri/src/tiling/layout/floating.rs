//! Floating layout implementation.
//!
//! The floating layout allows windows to be freely positioned and resized
//! by the user. Windows maintain their current positions and sizes.
//! The window manager does not automatically arrange windows in this layout.

use super::traits::{Layout, LayoutContext, LayoutResult, LayoutWindow, WindowLayout};

/// Floating layout - windows maintain their current positions.
#[derive(Debug, Default)]
pub struct FloatingLayout;

impl FloatingLayout {
    /// Creates a new floating layout instance.
    #[must_use]
    pub const fn new() -> Self { Self }
}

impl Layout for FloatingLayout {
    fn layout(
        &self,
        _windows: &[LayoutWindow],
        _context: &LayoutContext,
    ) -> LayoutResult<Vec<WindowLayout>> {
        // Floating layout does not arrange windows - they keep their current positions.
        // Return an empty layout list to indicate no position changes should be applied.
        Ok(Vec::new())
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
                y: 25,
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

    #[test]
    fn test_floating_returns_empty_layout() {
        let layout = FloatingLayout::new();
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
        ];

        let result = layout.layout(&windows, &context).unwrap();

        // Floating layout should return empty - no automatic positioning
        assert!(result.is_empty(), "Floating layout should not arrange windows");
    }
}
