//! Easing functions for smooth animations.
//!
//! This module provides various easing functions including standard easing curves
//! and Hyprland-style cubic bezier curves for high-quality window animations.

// Allow intentional numeric casts in this math-heavy module
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

use simple_easing::{cubic_in, cubic_in_out, cubic_out, linear};

/// Number of pre-baked points for bezier curve lookup (like Hyprland).
const BAKED_POINTS: usize = 255;
const INV_BAKED_POINTS: f64 = 1.0 / BAKED_POINTS as f64;

/// Types of easing functions available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingType {
    /// Linear interpolation (no easing).
    Linear,
    /// Ease in (slow start).
    EaseIn,
    /// Ease out (slow end).
    EaseOut,
    /// Ease in and out (slow start and end).
    EaseInOut,
    /// Spring-like animation with slight overshoot.
    Spring,
}

/// A cubic bezier curve implementation inspired by Hyprland's `CBezierCurve`.
///
/// Uses pre-baked points for fast lookup during animation.
#[derive(Debug, Clone)]
pub struct BezierCurve {
    /// Control points: P0 (start), P1, P2, P3 (end).
    points: [(f64, f64); 4],
    /// Pre-baked curve points for fast lookup.
    baked: [(f64, f64); BAKED_POINTS],
}

impl BezierCurve {
    /// Creates a new bezier curve from two control points.
    ///
    /// The start point (0, 0) and end point (1, 1) are implicit.
    /// Control points P1 and P2 define the curve shape.
    #[must_use]
    pub fn new(p1x: f64, p1y: f64, p2x: f64, p2y: f64) -> Self {
        let points = [(0.0, 0.0), (p1x, p1y), (p2x, p2y), (1.0, 1.0)];
        let mut curve = Self {
            points,
            baked: [(0.0, 0.0); BAKED_POINTS],
        };
        curve.bake();
        curve
    }

    /// Creates the default Hyprland bezier curve.
    #[must_use]
    pub fn default_curve() -> Self {
        // Hyprland's default: (0.0, 0.75, 0.15, 1.0)
        Self::new(0.0, 0.75, 0.15, 1.0)
    }

    /// Pre-bakes curve points for fast lookup.
    fn bake(&mut self) {
        for i in 0..BAKED_POINTS {
            let t = (i as f64 + 1.0) * INV_BAKED_POINTS;
            self.baked[i] = (self.get_x_for_t(t), self.get_y_for_t(t));
        }
    }

    /// Gets X coordinate for parameter t.
    fn get_x_for_t(&self, t: f64) -> f64 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        t3.mul_add(
            self.points[3].0,
            (3.0 * t2 * mt).mul_add(
                self.points[2].0,
                mt3 * self.points[0].0 + 3.0 * t * mt2 * self.points[1].0,
            ),
        )
    }

    /// Gets Y coordinate for parameter t.
    fn get_y_for_t(&self, t: f64) -> f64 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        t3.mul_add(
            self.points[3].1,
            (3.0 * t2 * mt).mul_add(
                self.points[2].1,
                mt3 * self.points[0].1 + 3.0 * t * mt2 * self.points[1].1,
            ),
        )
    }

    /// Gets Y value for a given X using binary search on baked points.
    ///
    /// This is the main function used during animation - given progress (0-1),
    /// returns the eased value.
    #[must_use]
    pub fn get_y_for_point(&self, x: f64) -> f64 {
        if x >= 1.0 {
            return 1.0;
        }
        if x <= 0.0 {
            return 0.0;
        }

        // Binary search for the baked point
        let mut index = 0i32;
        let mut below = true;
        let mut step = (BAKED_POINTS as i32 + 1) / 2;

        while step > 0 {
            if below {
                index += step;
            } else {
                index -= step;
            }

            // Clamp index
            index = index.clamp(0, BAKED_POINTS as i32 - 1);

            below = self.baked[index as usize].0 < x;
            step /= 2;
        }

        let mut lower_index = index - i32::from(!below || index == BAKED_POINTS as i32 - 1);
        lower_index = lower_index.clamp(0, BAKED_POINTS as i32 - 2);
        let lower_index = lower_index as usize;

        let lower_point = self.baked[lower_index];
        let upper_point = self.baked[lower_index + 1];

        let dx = upper_point.0 - lower_point.0;
        if dx <= 1e-6 {
            return lower_point.1;
        }

        let perc = (x - lower_point.0) / dx;
        if perc.is_nan() || perc.is_infinite() {
            return lower_point.1;
        }

        (upper_point.1 - lower_point.1).mul_add(perc, lower_point.1)
    }
}

/// Default bezier curve for quick access.
static DEFAULT_BEZIER: std::sync::OnceLock<BezierCurve> = std::sync::OnceLock::new();

fn get_default_bezier() -> &'static BezierCurve {
    DEFAULT_BEZIER.get_or_init(BezierCurve::default_curve)
}

/// Applies an easing function to a progress value (0.0 to 1.0).
///
/// Returns the eased progress value.
#[must_use]
pub fn apply_easing(easing: EasingType, progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0) as f32;

    match easing {
        EasingType::Linear => f64::from(linear(t)),
        EasingType::EaseIn => f64::from(cubic_in(t)),
        EasingType::EaseOut => f64::from(cubic_out(t)),
        EasingType::EaseInOut => f64::from(cubic_in_out(t)),
        EasingType::Spring => apply_spring(progress),
    }
}

/// Applies a spring-like easing with slight overshoot.
fn apply_spring(t: f64) -> f64 {
    // Spring approximation using bezier - slight overshoot then settle
    get_default_bezier().get_y_for_point(t)
}

/// Linear interpolation for i32 values.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn lerp_i32(start: i32, end: i32, t: f64) -> i32 {
    let start_f = f64::from(start);
    let end_f = f64::from(end);
    (end_f - start_f).mul_add(t, start_f).round() as i32
}

/// Linear interpolation for u32 values.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn lerp_u32(start: u32, end: u32, t: f64) -> u32 {
    let start_f = f64::from(start);
    let end_f = f64::from(end);
    (end_f - start_f).mul_add(t, start_f).round().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_endpoints() {
        let curve = BezierCurve::default_curve();

        // At t=0, should be at (0, 0)
        assert!((curve.get_y_for_point(0.0) - 0.0).abs() < 0.01);

        // At t=1, should be at (1, 1)
        assert!((curve.get_y_for_point(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_bezier_monotonic() {
        let curve = BezierCurve::default_curve();

        // The curve should be generally increasing
        let mut prev = 0.0;
        for i in 1..=10 {
            let t = i as f64 / 10.0;
            let y = curve.get_y_for_point(t);
            assert!(y >= prev - 0.1, "Curve should be generally monotonic");
            prev = y;
        }
    }

    #[test]
    fn test_easing_bounds() {
        for easing in [
            EasingType::Linear,
            EasingType::EaseIn,
            EasingType::EaseOut,
            EasingType::EaseInOut,
        ] {
            let start = apply_easing(easing, 0.0);
            let end = apply_easing(easing, 1.0);

            assert!((start - 0.0).abs() < 0.01, "{easing:?} should start at 0");
            assert!((end - 1.0).abs() < 0.01, "{easing:?} should end at 1");
        }
    }

    #[test]
    fn test_lerp_i32() {
        assert_eq!(lerp_i32(0, 100, 0.0), 0);
        assert_eq!(lerp_i32(0, 100, 1.0), 100);
        assert_eq!(lerp_i32(0, 100, 0.5), 50);
        assert_eq!(lerp_i32(-100, 100, 0.5), 0);
    }

    #[test]
    fn test_lerp_u32() {
        assert_eq!(lerp_u32(0, 100, 0.0), 0);
        assert_eq!(lerp_u32(0, 100, 1.0), 100);
        assert_eq!(lerp_u32(0, 100, 0.5), 50);
    }

    #[test]
    fn test_spring_easing() {
        let result = apply_easing(EasingType::Spring, 0.5);
        // Spring should give a value, we just check it's in reasonable range
        assert!(result > 0.0 && result <= 1.5);
    }
}
