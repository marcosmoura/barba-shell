//! Screen representation for integration tests.

use super::{Frame, native};

/// Represents a display screen.
#[derive(Debug, Clone)]
pub struct Screen {
    /// The frame of this screen (position and size).
    frame: Frame,
    /// Whether this is the main screen.
    is_main: bool,
    /// Display ID from Core Graphics.
    display_id: u32,
}

impl Screen {
    /// Creates a new Screen from ScreenInfo.
    pub(crate) fn from_info(info: native::ScreenInfo) -> Self {
        Self {
            frame: info.frame,
            is_main: info.is_main,
            display_id: info.display_id,
        }
    }

    /// Gets the main screen.
    pub fn main() -> Self {
        let screens = native::get_all_screens();
        screens
            .into_iter()
            .find(|s| s.is_main)
            .map(Self::from_info)
            .expect("No main screen found")
    }

    /// Gets all active screens.
    pub fn all() -> Vec<Self> {
        native::get_all_screens().into_iter().map(Self::from_info).collect()
    }

    /// Finds the screen containing a given frame (based on center point).
    pub fn containing_frame(frame: &Frame) -> Option<Self> {
        native::screen_containing_frame(frame).map(Self::from_info)
    }

    /// Returns the frame of this screen.
    pub fn frame(&self) -> Frame { self.frame }

    /// Returns true if this is the main screen.
    pub fn is_main(&self) -> bool { self.is_main }

    /// Returns the display ID.
    pub fn display_id(&self) -> u32 { self.display_id }

    /// Returns the width of this screen.
    pub fn width(&self) -> i32 { self.frame.width }

    /// Returns the height of this screen.
    pub fn height(&self) -> i32 { self.frame.height }

    /// Calculates the expected tiling area after accounting for gaps and menu bar.
    ///
    /// This represents the area where windows should be tiled.
    /// - `outer_gap`: Gap from screen edges
    /// - `menu_bar_height`: Height of macOS menu bar (typically ~40px, only on main screen)
    pub fn tiling_area(&self, outer_gap: i32, menu_bar_height: i32) -> Frame {
        // Menu bar is only on the main screen (y=0 in global coords)
        let top_offset = if self.frame.y == 0 {
            menu_bar_height
        } else {
            0
        };

        Frame {
            x: self.frame.x + outer_gap,
            y: self.frame.y + top_offset + outer_gap,
            width: self.frame.width - (outer_gap * 2),
            height: self.frame.height - top_offset - (outer_gap * 2),
        }
    }
}
