//! Animation system for smooth window transitions.
//!
//! This module provides display-synced animations using macOS `CVDisplayLink`
//! for window transitions including opening, closing, moving, and resizing.

mod display_link;
mod easing;
mod manager;

use std::sync::OnceLock;

use barba_shared::{AnimationConfig, EasingFunction};
pub use easing::EasingType;
use manager::AnimationManager;
use parking_lot::RwLock;

use crate::tiling::state::WindowFrame;

/// Global animation manager instance.
static ANIMATION_MANAGER: OnceLock<RwLock<AnimationManager>> = OnceLock::new();

/// Initializes the animation manager with the given configuration.
pub fn init(config: &AnimationConfig) {
    let manager = AnimationManager::new(config.clone());
    let _ = ANIMATION_MANAGER.set(RwLock::new(manager));
}

/// Returns whether animations are enabled.
#[must_use]
pub fn is_enabled() -> bool { ANIMATION_MANAGER.get().is_some_and(|m| m.read().is_enabled()) }

/// Animates a window to a target frame.
///
/// If animations are disabled, the window is moved immediately.
/// If animations are enabled, the window is smoothly transitioned.
pub fn animate_window(window_id: u64, target_frame: WindowFrame) {
    if let Some(manager) = ANIMATION_MANAGER.get() {
        manager.write().animate_window(window_id, target_frame);
    } else {
        // No animation manager, apply immediately
        let _ = crate::tiling::window::set_window_frame(window_id, &target_frame);
    }
}

/// Animates multiple windows simultaneously.
///
/// This is more efficient than calling `animate_window` for each window
/// as it can batch the animations together.
pub fn animate_windows(targets: Vec<(u64, WindowFrame)>) {
    if let Some(manager) = ANIMATION_MANAGER.get() {
        manager.write().animate_windows(targets);
    } else {
        // No animation manager, apply immediately
        for (window_id, frame) in targets {
            let _ = crate::tiling::window::set_window_frame(window_id, &frame);
        }
    }
}

/// Returns the configured animation duration in milliseconds.
/// Returns 0 if animations are disabled or not initialized.
#[must_use]
pub fn get_duration_ms() -> u64 {
    ANIMATION_MANAGER
        .get()
        .map(|m| u64::from(m.read().settings().duration))
        .unwrap_or(0)
}

/// Converts the shared easing function to the internal easing type.
#[must_use]
pub const fn easing_from_config(easing: &EasingFunction) -> EasingType {
    match easing {
        EasingFunction::Linear => EasingType::Linear,
        EasingFunction::EaseIn => EasingType::EaseIn,
        EasingFunction::EaseOut => EasingType::EaseOut,
        EasingFunction::EaseInOut => EasingType::EaseInOut,
        EasingFunction::Spring => EasingType::Spring,
    }
}

#[cfg(test)]
mod tests {
    use barba_shared::AnimationSettings;

    use super::*;

    #[test]
    fn test_easing_conversion() {
        assert!(matches!(
            easing_from_config(&EasingFunction::Linear),
            EasingType::Linear
        ));
        assert!(matches!(
            easing_from_config(&EasingFunction::EaseIn),
            EasingType::EaseIn
        ));
        assert!(matches!(
            easing_from_config(&EasingFunction::EaseOut),
            EasingType::EaseOut
        ));
        assert!(matches!(
            easing_from_config(&EasingFunction::EaseInOut),
            EasingType::EaseInOut
        ));
        assert!(matches!(
            easing_from_config(&EasingFunction::Spring),
            EasingType::Spring
        ));
    }

    #[test]
    fn test_animation_config_defaults() {
        let settings = AnimationSettings::default();
        assert_eq!(settings.duration, 200);
    }
}
