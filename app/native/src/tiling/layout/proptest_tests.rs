//! Property-based tests for layout algorithms.
//!
//! These tests use proptest to verify invariants that should hold for all
//! layout algorithms regardless of input values.

#![cfg(test)]

use proptest::prelude::*;

use super::{Gaps, dwindle, grid, master, monocle, split};
use crate::config::MasterPosition;
use crate::tiling::constants::layout::MAX_GRID_WINDOWS;
use crate::tiling::state::Rect;

// ============================================================================
// Strategies
// ============================================================================

/// Strategy for generating realistic screen frames.
fn screen_frame_strategy() -> impl Strategy<Value = Rect> {
    // Width: 800-7680 (from small laptop to 8K)
    // Height: 600-4320 (from small to 8K)
    (800.0f64..7680.0, 600.0f64..4320.0)
        .prop_map(|(width, height)| Rect::new(0.0, 0.0, width, height))
}

/// Strategy for generating gap configurations.
///
/// Uses realistic gap values that won't cause layout issues with many windows.
/// Real-world configs rarely exceed 20px gaps.
fn gaps_strategy() -> impl Strategy<Value = Gaps> {
    (
        0.0f64..20.0, // inner_h
        0.0f64..20.0, // inner_v
        0.0f64..50.0, // outer_top
        0.0f64..50.0, // outer_right
        0.0f64..50.0, // outer_bottom
        0.0f64..50.0, // outer_left
    )
        .prop_map(
            |(inner_h, inner_v, outer_top, outer_right, outer_bottom, outer_left)| Gaps {
                inner_h,
                inner_v,
                outer_top,
                outer_right,
                outer_bottom,
                outer_left,
            },
        )
}

/// Strategy for generating small gap configurations for stress tests.
fn small_gaps_strategy() -> impl Strategy<Value = Gaps> {
    (0.0f64..8.0, 0.0f64..8.0).prop_map(|(inner, outer)| Gaps::uniform(inner, outer))
}

/// Strategy for generating valid cumulative ratios for split layout.
///
/// Split layout expects N-1 ratios for N windows, where ratios are cumulative
/// positions in 0.0-1.0 range, monotonically increasing.
fn cumulative_ratios_strategy(max_count: usize) -> impl Strategy<Value = Vec<f64>> {
    prop::collection::vec(0.1f64..0.9, 0..max_count).prop_map(|mut ratios| {
        // Sort to make monotonically increasing
        ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Ensure proper spacing by scaling
        let len = ratios.len();
        if len > 0 {
            for (i, r) in ratios.iter_mut().enumerate() {
                // Spread ratios evenly between 0.1 and 0.9
                *r = 0.1 + (0.8 * (i as f64 + *r * 0.5) / (len as f64 + 0.5));
            }
        }
        ratios
    })
}

/// Strategy for generating window ID lists.
fn window_ids_strategy(max_count: usize) -> impl Strategy<Value = Vec<u32>> {
    prop::collection::vec(1u32..10000, 0..max_count).prop_map(|mut ids| {
        // Ensure unique IDs
        ids.sort_unstable();
        ids.dedup();
        ids
    })
}

/// Strategy for generating master position.
fn master_position_strategy() -> impl Strategy<Value = MasterPosition> {
    prop_oneof![
        Just(MasterPosition::Left),
        Just(MasterPosition::Right),
        Just(MasterPosition::Top),
        Just(MasterPosition::Bottom),
    ]
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Checks if two rectangles overlap (share interior area).
fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    // Two rects overlap if they share interior area
    // They don't overlap if one is completely to the left, right, above, or below the other
    let no_overlap = a.x + a.width <= b.x
        || b.x + b.width <= a.x
        || a.y + a.height <= b.y
        || b.y + b.height <= a.y;

    !no_overlap
}

/// Checks if a rect has valid (positive) dimensions.
fn rect_has_valid_dimensions(rect: &Rect) -> bool { rect.width > 0.0 && rect.height > 0.0 }

// ============================================================================
// Dwindle Layout Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn dwindle_returns_correct_window_count(
        window_ids in window_ids_strategy(50),
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
    ) {
        let result = dwindle::layout(&window_ids, &screen, &gaps);
        prop_assert_eq!(result.len(), window_ids.len());
    }

    #[test]
    fn dwindle_returns_all_window_ids(
        window_ids in window_ids_strategy(50),
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
    ) {
        let result = dwindle::layout(&window_ids, &screen, &gaps);
        let result_ids: Vec<u32> = result.iter().map(|(id, _)| *id).collect();

        for id in &window_ids {
            prop_assert!(result_ids.contains(id), "Missing window ID: {}", id);
        }
    }

    #[test]
    fn dwindle_windows_have_valid_dimensions(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
    ) {
        let result = dwindle::layout(&window_ids, &screen, &gaps);

        for (id, frame) in &result {
            prop_assert!(
                rect_has_valid_dimensions(frame),
                "Window {} has invalid dimensions: {:?}", id, frame
            );
        }
    }

    #[test]
    fn dwindle_no_overlapping_windows(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
    ) {
        let result = dwindle::layout(&window_ids, &screen, &gaps);

        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                let (id_a, frame_a) = &result[i];
                let (id_b, frame_b) = &result[j];

                prop_assert!(
                    !rects_overlap(frame_a, frame_b),
                    "Windows {} and {} overlap: {:?} vs {:?}",
                    id_a, id_b, frame_a, frame_b
                );
            }
        }
    }
}

// ============================================================================
// Master Layout Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn master_returns_correct_window_count(
        window_ids in window_ids_strategy(50),
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
        master_ratio in 0.1f64..0.9,
        position in master_position_strategy(),
    ) {
        let result = master::layout(&window_ids, &screen, master_ratio, &gaps, position);
        prop_assert_eq!(result.len(), window_ids.len());
    }

    #[test]
    fn master_windows_have_valid_dimensions(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
        master_ratio in 0.1f64..0.9,
        position in master_position_strategy(),
    ) {
        let result = master::layout(&window_ids, &screen, master_ratio, &gaps, position);

        for (id, frame) in &result {
            prop_assert!(
                rect_has_valid_dimensions(frame),
                "Window {} has invalid dimensions: {:?}", id, frame
            );
        }
    }

    #[test]
    fn master_no_overlapping_windows(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
        master_ratio in 0.1f64..0.9,
        position in master_position_strategy(),
    ) {
        let result = master::layout(&window_ids, &screen, master_ratio, &gaps, position);

        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                let (id_a, frame_a) = &result[i];
                let (id_b, frame_b) = &result[j];

                prop_assert!(
                    !rects_overlap(frame_a, frame_b),
                    "Windows {} and {} overlap: {:?} vs {:?}",
                    id_a, id_b, frame_a, frame_b
                );
            }
        }
    }
}

// ============================================================================
// Grid Layout Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn grid_returns_correct_window_count(
        window_ids in window_ids_strategy(50),
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
    ) {
        let result = grid::layout(&window_ids, &screen, &gaps);
        // Grid layout caps at MAX_GRID_WINDOWS (12)
        let expected = window_ids.len().min(MAX_GRID_WINDOWS);
        prop_assert_eq!(result.len(), expected);
    }

    #[test]
    fn grid_windows_have_valid_dimensions(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
    ) {
        let result = grid::layout(&window_ids, &screen, &gaps);

        for (id, frame) in &result {
            prop_assert!(
                rect_has_valid_dimensions(frame),
                "Window {} has invalid dimensions: {:?}", id, frame
            );
        }
    }

    #[test]
    fn grid_no_overlapping_windows(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
    ) {
        let result = grid::layout(&window_ids, &screen, &gaps);

        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                let (id_a, frame_a) = &result[i];
                let (id_b, frame_b) = &result[j];

                prop_assert!(
                    !rects_overlap(frame_a, frame_b),
                    "Windows {} and {} overlap: {:?} vs {:?}",
                    id_a, id_b, frame_a, frame_b
                );
            }
        }
    }
}

// ============================================================================
// Split Layout Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn split_returns_correct_window_count(
        window_ids in window_ids_strategy(50),
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
        ratios in cumulative_ratios_strategy(10),
    ) {
        let result = split::layout_auto(&window_ids, &screen, &gaps, &ratios);
        prop_assert_eq!(result.len(), window_ids.len());
    }

    #[test]
    fn split_windows_have_valid_dimensions(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
        ratios in cumulative_ratios_strategy(10),
    ) {
        let result = split::layout_auto(&window_ids, &screen, &gaps, &ratios);

        for (id, frame) in &result {
            prop_assert!(
                rect_has_valid_dimensions(frame),
                "Window {} has invalid dimensions: {:?}", id, frame
            );
        }
    }

    #[test]
    fn split_no_overlapping_windows(
        window_ids in window_ids_strategy(15),
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
        ratios in cumulative_ratios_strategy(10),
    ) {
        let result = split::layout_auto(&window_ids, &screen, &gaps, &ratios);

        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                let (id_a, frame_a) = &result[i];
                let (id_b, frame_b) = &result[j];

                prop_assert!(
                    !rects_overlap(frame_a, frame_b),
                    "Windows {} and {} overlap: {:?} vs {:?}",
                    id_a, id_b, frame_a, frame_b
                );
            }
        }
    }
}

// ============================================================================
// Monocle Layout Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn monocle_returns_correct_window_count(
        window_ids in window_ids_strategy(50),
        screen in screen_frame_strategy(),
    ) {
        let result = monocle::layout(&window_ids, &screen);
        prop_assert_eq!(result.len(), window_ids.len());
    }

    #[test]
    fn monocle_all_windows_same_size(
        window_ids in window_ids_strategy(30),
        screen in screen_frame_strategy(),
    ) {
        let result = monocle::layout(&window_ids, &screen);

        if result.len() > 1 {
            let (_, first_frame) = &result[0];

            for (id, frame) in result.iter().skip(1) {
                prop_assert!(
                    (frame.width - first_frame.width).abs() < 0.1
                        && (frame.height - first_frame.height).abs() < 0.1
                        && (frame.x - first_frame.x).abs() < 0.1
                        && (frame.y - first_frame.y).abs() < 0.1,
                    "Window {} has different frame than first window: {:?} vs {:?}",
                    id, frame, first_frame
                );
            }
        }
    }

    #[test]
    fn monocle_windows_have_valid_dimensions(
        window_ids in window_ids_strategy(30),
        screen in screen_frame_strategy(),
    ) {
        let result = monocle::layout(&window_ids, &screen);

        for (id, frame) in &result {
            prop_assert!(
                rect_has_valid_dimensions(frame),
                "Window {} has invalid dimensions: {:?}", id, frame
            );
        }
    }
}

// ============================================================================
// Cross-Layout Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn all_layouts_handle_empty_windows(
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
    ) {
        let empty: Vec<u32> = vec![];

        prop_assert!(dwindle::layout(&empty, &screen, &gaps).is_empty());
        prop_assert!(master::layout(&empty, &screen, 0.5, &gaps, MasterPosition::Left).is_empty());
        prop_assert!(grid::layout(&empty, &screen, &gaps).is_empty());
        prop_assert!(split::layout_auto(&empty, &screen, &gaps, &[]).is_empty());
        prop_assert!(monocle::layout(&empty, &screen).is_empty());
    }

    #[test]
    fn all_layouts_handle_single_window(
        screen in screen_frame_strategy(),
        gaps in gaps_strategy(),
        window_id in 1u32..10000,
    ) {
        let windows = vec![window_id];

        let dwindle_result = dwindle::layout(&windows, &screen, &gaps);
        let master_result = master::layout(&windows, &screen, 0.5, &gaps, MasterPosition::Left);
        let grid_result = grid::layout(&windows, &screen, &gaps);
        let split_result = split::layout_auto(&windows, &screen, &gaps, &[]);
        let monocle_result = monocle::layout(&windows, &screen);

        prop_assert_eq!(dwindle_result.len(), 1);
        prop_assert_eq!(master_result.len(), 1);
        prop_assert_eq!(grid_result.len(), 1);
        prop_assert_eq!(split_result.len(), 1);
        prop_assert_eq!(monocle_result.len(), 1);

        // All layouts should place the single window ID correctly
        prop_assert_eq!(dwindle_result[0].0, window_id);
        prop_assert_eq!(master_result[0].0, window_id);
        prop_assert_eq!(grid_result[0].0, window_id);
        prop_assert_eq!(split_result[0].0, window_id);
        prop_assert_eq!(monocle_result[0].0, window_id);
    }

    #[test]
    fn all_layouts_handle_many_windows(
        screen in screen_frame_strategy(),
        gaps in small_gaps_strategy(),
    ) {
        // Test with 100 windows - a stress test for layouts
        // Use small gaps to avoid negative dimensions with many windows
        let windows: Vec<u32> = (1..=100).collect();

        let dwindle_result = dwindle::layout(&windows, &screen, &gaps);
        let master_result = master::layout(&windows, &screen, 0.5, &gaps, MasterPosition::Left);
        let grid_result = grid::layout(&windows, &screen, &gaps);
        let split_result = split::layout_auto(&windows, &screen, &gaps, &[]);
        let monocle_result = monocle::layout(&windows, &screen);

        prop_assert_eq!(dwindle_result.len(), 100);
        prop_assert_eq!(master_result.len(), 100);
        // Grid layout caps at MAX_GRID_WINDOWS (12)
        prop_assert_eq!(grid_result.len(), MAX_GRID_WINDOWS);
        prop_assert_eq!(split_result.len(), 100);
        prop_assert_eq!(monocle_result.len(), 100);
    }

    #[test]
    fn all_layouts_handle_extreme_aspect_ratios(
        gaps in gaps_strategy(),
        window_ids in window_ids_strategy(10),
    ) {
        // Very wide screen (10:1 aspect ratio)
        let wide_screen = Rect::new(0.0, 0.0, 5000.0, 500.0);
        // Very tall screen (1:10 aspect ratio)
        let tall_screen = Rect::new(0.0, 0.0, 500.0, 5000.0);

        // All layouts should handle extreme aspect ratios without panicking
        let _ = dwindle::layout(&window_ids, &wide_screen, &gaps);
        let _ = dwindle::layout(&window_ids, &tall_screen, &gaps);
        let _ = master::layout(&window_ids, &wide_screen, 0.5, &gaps, MasterPosition::Left);
        let _ = master::layout(&window_ids, &tall_screen, 0.5, &gaps, MasterPosition::Left);
        let _ = grid::layout(&window_ids, &wide_screen, &gaps);
        let _ = grid::layout(&window_ids, &tall_screen, &gaps);
        let _ = split::layout_auto(&window_ids, &wide_screen, &gaps, &[]);
        let _ = split::layout_auto(&window_ids, &tall_screen, &gaps, &[]);
        let _ = monocle::layout(&window_ids, &wide_screen);
        let _ = monocle::layout(&window_ids, &tall_screen);
    }
}
