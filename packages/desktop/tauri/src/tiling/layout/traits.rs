//! Layout trait and common types.
//!
//! This module defines the interface that all layout algorithms must implement.

use barba_shared::{GapsConfig, InnerGaps, OuterGaps, ScreenGaps};

use crate::tiling::error::TilingError;
use crate::tiling::state::{ScreenFrame, WindowFrame};

/// Result type for layout operations.
pub type LayoutResult<T> = Result<T, TilingError>;

/// Information about a window for layout purposes.
#[derive(Debug, Clone)]
pub struct LayoutWindow {
    /// Window ID.
    pub id: u64,

    /// Whether this window is floating (should be skipped).
    pub is_floating: bool,

    /// Whether this window is minimized (should be skipped).
    pub is_minimized: bool,

    /// Whether this window is fullscreen (takes entire screen).
    pub is_fullscreen: bool,
}

/// The computed layout for a window.
#[derive(Debug, Clone)]
pub struct WindowLayout {
    /// Window ID.
    pub id: u64,

    /// Target frame.
    pub frame: WindowFrame,
}

/// Layout context passed to layout algorithms.
#[derive(Debug, Clone)]
pub struct LayoutContext {
    /// Usable screen area.
    pub screen_frame: ScreenFrame,

    /// Gap configuration.
    pub gaps: ResolvedGaps,

    /// Split ratios for each split point in the layout.
    /// Each ratio is a value between 0.0 and 1.0 representing the portion
    /// of space allocated to the first part of the split.
    /// Default is 0.5 (equal split).
    pub split_ratios: Vec<f64>,
}

/// Resolved gap values in pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResolvedGaps {
    /// Gap from screen edges (top).
    pub outer_top: u32,

    /// Gap from screen edges (bottom).
    pub outer_bottom: u32,

    /// Gap from screen edges (left).
    pub outer_left: u32,

    /// Gap from screen edges (right).
    pub outer_right: u32,

    /// Gap between windows.
    pub inner: u32,
}

impl ResolvedGaps {
    /// Creates gaps from the shared config type.
    ///
    /// This resolves "main" and "secondary" screen names from the config
    /// to match against the actual screen properties.
    #[must_use]
    pub fn from_config(
        config: &GapsConfig,
        screen: &crate::tiling::state::Screen,
        screen_count: usize,
    ) -> Self {
        // Determine which config name to look up:
        // - "main" if this is the main screen
        // - "secondary" if there are exactly 2 screens and this is not main
        // - Otherwise, try the screen ID and name
        let screen_gaps = Self::find_matching_gaps(config, screen, screen_count);

        // For inner gaps, use the horizontal value as the primary
        // (most layouts use equal horizontal/vertical inner gaps)
        let inner = screen_gaps.inner.horizontal();
        let outer_top = screen_gaps.outer.top();
        let outer_bottom = screen_gaps.outer.bottom();
        let outer_left = screen_gaps.outer.left();
        let outer_right = screen_gaps.outer.right();

        Self {
            outer_top,
            outer_bottom,
            outer_left,
            outer_right,
            inner,
        }
    }

    /// Finds the matching gaps configuration for a screen.
    fn find_matching_gaps<'a>(
        config: &'a GapsConfig,
        screen: &crate::tiling::state::Screen,
        screen_count: usize,
    ) -> &'a ScreenGaps {
        match config {
            GapsConfig::Global(gaps) => gaps,
            GapsConfig::PerScreen(screens) => {
                // Try to find a matching config in order of priority:
                // 1. "main" if this is the main screen
                // 2. "secondary" if there are exactly 2 screens and this is not main
                // 3. Screen ID match
                // 4. Screen name match
                // 5. Default (no screen specified)

                if screen.is_main {
                    if let Some(gaps) = screens.iter().find(|g| g.screen.as_deref() == Some("main"))
                    {
                        return gaps;
                    }
                } else if screen_count == 2
                    && let Some(gaps) =
                        screens.iter().find(|g| g.screen.as_deref() == Some("secondary"))
                {
                    return gaps;
                }

                // Try matching by screen ID
                if let Some(gaps) = screens.iter().find(|g| g.screen.as_deref() == Some(&screen.id))
                {
                    return gaps;
                }

                // Try matching by screen name
                if let Some(gaps) =
                    screens.iter().find(|g| g.screen.as_deref() == Some(&screen.name))
                {
                    return gaps;
                }

                // Fall back to default (no screen specified)
                screens.iter().find(|g| g.screen.is_none()).unwrap_or_else(|| {
                    static DEFAULT: ScreenGaps = ScreenGaps {
                        screen: None,
                        inner: InnerGaps::Uniform(0),
                        outer: OuterGaps::Uniform(0),
                    };
                    &DEFAULT
                })
            }
        }
    }

    /// Calculates the usable area after applying outer gaps.
    #[must_use]
    pub const fn apply_to_screen(&self, screen: &ScreenFrame) -> WindowFrame {
        WindowFrame {
            x: screen.x + self.outer_left as i32,
            y: screen.y + self.outer_top as i32,
            width: screen.width.saturating_sub(self.outer_left + self.outer_right),
            height: screen.height.saturating_sub(self.outer_top + self.outer_bottom),
        }
    }
}

/// Trait that all layout algorithms must implement.
pub trait Layout: Send + Sync {
    /// Computes window layouts for the given windows.
    ///
    /// # Arguments
    /// * `windows` - The windows to lay out (in order)
    /// * `context` - Layout context with screen info and gaps
    ///
    /// # Returns
    /// A vector of `WindowLayout` for each non-floating, non-minimized window.
    fn layout(
        &self,
        windows: &[LayoutWindow],
        context: &LayoutContext,
    ) -> LayoutResult<Vec<WindowLayout>>;
}
