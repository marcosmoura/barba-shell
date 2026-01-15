//! Integration tests for Stache.
//!
//! These tests create real windows on screen and require accessibility permissions.
//! They must run sequentially to avoid interference between tests.
//!
//! ## Requirements
//!
//! - macOS with accessibility permissions granted to the terminal/test runner
//! - Grant in: System Settings > Privacy & Security > Accessibility
//! - The `integration-tests` feature must be enabled
//! - Dictionary and TextEdit must NOT be running before tests start
//!
//! ## Running Integration Tests
//!
//! ```bash
//! # Run all integration tests (sequentially via nextest config)
//! cargo nextest run -p stache --features integration-tests
//!
//! # Run specific test module
//! cargo nextest run -p stache --features integration-tests -E 'test(/tiling__layout_dwindle/)'
//!
//! # Run with output
//! cargo nextest run -p stache --features integration-tests --no-capture
//! ```
//!
//! ## Test Organization
//!
//! Tests follow the naming convention `<module>__<test_name>` to allow filtering by module:
//! - `tiling__*` - Window tiling tests
//! - `audio__*` - Audio device tests (future)
//! - `bar__*` - Status bar tests (future)

#![cfg(feature = "integration-tests")]
// Allow double-underscore naming for test modules (e.g., tiling__layout_dwindle)
#![allow(non_snake_case)]
// Relax clippy lints for integration tests - these are test utilities, not production code
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::doc_markdown,
    clippy::map_unwrap_or,
    clippy::manual_assert,
    clippy::missing_const_for_fn,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::redundant_clone,
    clippy::redundant_closure_for_method_calls,
    clippy::similar_names,
    clippy::uninlined_format_args,
    clippy::use_self,
    clippy::wildcard_imports
)]

mod common;
mod framework;

// Suite-level precondition check - runs before any test
#[ctor::ctor]
fn check_preconditions() { framework::check_suite_preconditions(); }

// Tiling layout tests
mod tiling__layout_dwindle;
mod tiling__layout_floating;
mod tiling__layout_grid;
mod tiling__layout_master;
mod tiling__layout_monocle;
mod tiling__layout_split;

// Tiling operation tests
mod tiling__window_focus;
mod tiling__window_operations;
mod tiling__window_rules;
mod tiling__workspace_operations;

// Multi-screen tests
mod tiling__multi_screen;
