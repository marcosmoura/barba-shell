//! Window rule matching for workspace assignment.
//!
//! This module provides functionality to match windows against rules
//! defined in the configuration to determine which workspace they belong to.
//!
//! # Rule Matching
//!
//! Rules use AND logic - all specified properties must match for a rule to match.
//! At least one property must be specified for a rule to be valid.
//!
//! # Examples
//!
//! ```text
//! // Rule: app-id = "com.apple.finder"
//! // Matches: Any Finder window
//!
//! // Rule: app-id = "com.apple.Safari", title = "Settings"
//! // Matches: Safari windows with "Settings" in title (AND logic)
//! ```

use super::window::WindowInfo;
use crate::config::WindowRule;

/// Checks if a window matches a rule.
///
/// All specified properties in the rule must match (AND logic).
/// Returns `false` if the rule has no matching criteria.
///
/// # Matching Behavior
///
/// - `app_id`: Exact match against bundle identifier (case-insensitive)
/// - `app_name`: Case-insensitive substring match
/// - `title`: Case-insensitive substring match
///
/// # Performance
///
/// If rules have been prepared with [`WindowRule::prepare()`], this function
/// uses cached lowercase strings. Otherwise, it falls back to computing
/// lowercase on each call.
///
/// # Arguments
///
/// * `rule` - The rule to match against
/// * `window` - The window to check
///
/// # Returns
///
/// `true` if all specified rule criteria match the window
#[must_use]
pub fn matches_window(rule: &WindowRule, window: &WindowInfo) -> bool {
    // Rule must have at least one criterion
    if !rule.is_valid() {
        return false;
    }

    // Check app_id (bundle identifier) - case-insensitive exact match
    // Use cached lowercase if available, otherwise use eq_ignore_ascii_case
    if rule.app_id.is_some() {
        if let Some(app_id_lower) = &rule.app_id_lower {
            // Fast path: use pre-computed lowercase
            if !window.bundle_id.to_ascii_lowercase().eq(app_id_lower) {
                return false;
            }
        } else if let Some(app_id) = &rule.app_id {
            // Fallback: case-insensitive comparison
            if !window.bundle_id.eq_ignore_ascii_case(app_id) {
                return false;
            }
        }
    }

    // Check app_name - case-insensitive substring match
    if rule.app_name.is_some() {
        let window_app_lower = window.app_name.to_lowercase();
        if let Some(app_name_lower) = &rule.app_name_lower {
            // Fast path: use pre-computed lowercase
            if !window_app_lower.contains(app_name_lower.as_str()) {
                return false;
            }
        } else if let Some(app_name) = &rule.app_name {
            // Fallback: compute lowercase
            if !window_app_lower.contains(&app_name.to_lowercase()) {
                return false;
            }
        }
    }

    // Check title - case-insensitive substring match
    if rule.title.is_some() {
        let window_title_lower = window.title.to_lowercase();
        if let Some(title_lower) = &rule.title_lower {
            // Fast path: use pre-computed lowercase
            if !window_title_lower.contains(title_lower.as_str()) {
                return false;
            }
        } else if let Some(title) = &rule.title {
            // Fallback: compute lowercase
            if !window_title_lower.contains(&title.to_lowercase()) {
                return false;
            }
        }
    }

    true
}

/// Result of finding a workspace match for a window.
#[derive(Debug, Clone)]
pub struct WorkspaceMatch {
    /// Name of the matching workspace.
    pub workspace_name: String,
    /// Index of the matching rule within the workspace's rules.
    pub rule_index: usize,
}

/// Finds the first workspace that has a rule matching the given window.
///
/// Workspaces are checked in order, and rules within each workspace are
/// checked in order. The first matching rule wins.
///
/// # Arguments
///
/// * `window` - The window to find a workspace for
/// * `workspaces` - Iterator of (`workspace_name`, rules) pairs
///
/// # Returns
///
/// `Some(WorkspaceMatch)` if a matching rule was found, `None` otherwise
pub fn find_matching_workspace<'a, I>(
    window: &WindowInfo,
    workspaces: I,
) -> Option<WorkspaceMatch>
where
    I: IntoIterator<Item = (&'a str, &'a [WindowRule])>,
{
    for (workspace_name, rules) in workspaces {
        for (rule_index, rule) in rules.iter().enumerate() {
            if matches_window(rule, window) {
                return Some(WorkspaceMatch {
                    workspace_name: workspace_name.to_string(),
                    rule_index,
                });
            }
        }
    }
    None
}

/// Checks if any rule in the list matches the window.
///
/// # Arguments
///
/// * `rules` - The rules to check against
/// * `window` - The window to check
///
/// # Returns
///
/// `true` if any rule matches the window
#[must_use]
pub fn any_rule_matches(rules: &[WindowRule], window: &WindowInfo) -> bool {
    rules.iter().any(|rule| matches_window(rule, window))
}

/// Counts how many rules match a window.
///
/// Useful for debugging and testing rule configurations.
///
/// # Arguments
///
/// * `rules` - The rules to check against
/// * `window` - The window to check
///
/// # Returns
///
/// The number of rules that match the window
#[must_use]
pub fn count_matching_rules(rules: &[WindowRule], window: &WindowInfo) -> usize {
    rules.iter().filter(|rule| matches_window(rule, window)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiling::state::Rect;

    /// Creates a test window with the given properties.
    fn make_window(bundle_id: &str, app_name: &str, title: &str) -> WindowInfo {
        WindowInfo::new(
            1,
            1234,
            bundle_id.to_string(),
            app_name.to_string(),
            title.to_string(),
            Rect::new(0.0, 0.0, 800.0, 600.0),
            false,
            false,
            true,
            false,
        )
    }

    /// Creates a rule with optional properties (with pre-computed lowercase).
    fn make_rule(app_id: Option<&str>, app_name: Option<&str>, title: Option<&str>) -> WindowRule {
        let mut rule = WindowRule {
            app_id: app_id.map(String::from),
            app_name: app_name.map(String::from),
            title: title.map(String::from),
            app_id_lower: None,
            app_name_lower: None,
            title_lower: None,
        };
        rule.prepare();
        rule
    }

    // ========================================================================
    // Basic matching tests
    // ========================================================================

    #[test]
    fn test_matches_window_empty_rule() {
        let window = make_window("com.apple.finder", "Finder", "Documents");
        let rule = make_rule(None, None, None);

        assert!(!rule.is_valid());
        assert!(!matches_window(&rule, &window));
    }

    #[test]
    fn test_matches_window_app_id_only() {
        let window = make_window("com.apple.finder", "Finder", "Documents");

        // Exact match
        let rule = make_rule(Some("com.apple.finder"), None, None);
        assert!(matches_window(&rule, &window));

        // Case-insensitive match
        let rule = make_rule(Some("COM.APPLE.FINDER"), None, None);
        assert!(matches_window(&rule, &window));

        // Non-match
        let rule = make_rule(Some("com.apple.safari"), None, None);
        assert!(!matches_window(&rule, &window));
    }

    #[test]
    fn test_matches_window_app_name_only() {
        let window = make_window("com.apple.finder", "Finder", "Documents");

        // Exact match
        let rule = make_rule(None, Some("Finder"), None);
        assert!(matches_window(&rule, &window));

        // Case-insensitive match
        let rule = make_rule(None, Some("finder"), None);
        assert!(matches_window(&rule, &window));

        // Substring match
        let rule = make_rule(None, Some("ind"), None);
        assert!(matches_window(&rule, &window));

        // Non-match
        let rule = make_rule(None, Some("Safari"), None);
        assert!(!matches_window(&rule, &window));
    }

    #[test]
    fn test_matches_window_title_only() {
        let window = make_window("com.apple.finder", "Finder", "Documents - MyFolder");

        // Exact match
        let rule = make_rule(None, None, Some("Documents - MyFolder"));
        assert!(matches_window(&rule, &window));

        // Case-insensitive match
        let rule = make_rule(None, None, Some("documents"));
        assert!(matches_window(&rule, &window));

        // Substring match
        let rule = make_rule(None, None, Some("MyFolder"));
        assert!(matches_window(&rule, &window));

        // Non-match
        let rule = make_rule(None, None, Some("Downloads"));
        assert!(!matches_window(&rule, &window));
    }

    // ========================================================================
    // AND logic tests
    // ========================================================================

    #[test]
    fn test_matches_window_and_logic_app_id_and_title() {
        let window = make_window("com.apple.safari", "Safari", "Settings");

        // Both match
        let rule = make_rule(Some("com.apple.safari"), None, Some("Settings"));
        assert!(matches_window(&rule, &window));

        // app_id matches, title doesn't
        let rule = make_rule(Some("com.apple.safari"), None, Some("History"));
        assert!(!matches_window(&rule, &window));

        // title matches, app_id doesn't
        let rule = make_rule(Some("com.apple.finder"), None, Some("Settings"));
        assert!(!matches_window(&rule, &window));
    }

    #[test]
    fn test_matches_window_and_logic_all_three() {
        let window = make_window("com.apple.safari", "Safari", "Settings");

        // All three match
        let rule = make_rule(Some("com.apple.safari"), Some("Safari"), Some("Settings"));
        assert!(matches_window(&rule, &window));

        // Two match, one doesn't (app_name fails)
        let rule = make_rule(Some("com.apple.safari"), Some("Chrome"), Some("Settings"));
        assert!(!matches_window(&rule, &window));

        // Only one matches
        let rule = make_rule(Some("com.google.chrome"), Some("Chrome"), Some("Settings"));
        assert!(!matches_window(&rule, &window));
    }

    // ========================================================================
    // find_matching_workspace tests
    // ========================================================================

    #[test]
    fn test_find_matching_workspace_no_match() {
        let window = make_window("com.apple.finder", "Finder", "Documents");

        let browser_rules = [make_rule(Some("com.apple.safari"), None, None)];
        let code_rules = [make_rule(Some("com.microsoft.vscode"), None, None)];

        let workspaces: Vec<(&str, &[WindowRule])> =
            vec![("browser", &browser_rules), ("code", &code_rules)];

        let result = find_matching_workspace(&window, workspaces);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_workspace_single_match() {
        let window = make_window("com.apple.safari", "Safari", "Google");

        let browser_rules = [make_rule(Some("com.apple.safari"), None, None)];
        let code_rules = [make_rule(Some("com.microsoft.vscode"), None, None)];

        let workspaces: Vec<(&str, &[WindowRule])> =
            vec![("browser", &browser_rules), ("code", &code_rules)];

        let result = find_matching_workspace(&window, workspaces);
        assert!(result.is_some());
        let match_result = result.unwrap();
        assert_eq!(match_result.workspace_name, "browser");
        assert_eq!(match_result.rule_index, 0);
    }

    #[test]
    fn test_find_matching_workspace_first_match_wins() {
        let window = make_window("com.apple.safari", "Safari", "Google");

        // Both workspaces have rules that would match
        let ws1_rules = [make_rule(Some("com.apple.safari"), None, None)];
        let ws2_rules = [make_rule(None, Some("Safari"), None)];

        let workspaces: Vec<(&str, &[WindowRule])> =
            vec![("workspace-1", &ws1_rules), ("workspace-2", &ws2_rules)];

        let result = find_matching_workspace(&window, workspaces);
        assert!(result.is_some());
        let match_result = result.unwrap();
        // First workspace should win
        assert_eq!(match_result.workspace_name, "workspace-1");
    }

    #[test]
    fn test_find_matching_workspace_multiple_rules() {
        let window = make_window("com.apple.finder", "Finder", "Documents");

        // Workspace has multiple rules, second one matches
        let rules = [
            make_rule(Some("com.apple.safari"), None, None),
            make_rule(Some("com.apple.finder"), None, None),
            make_rule(Some("com.microsoft.vscode"), None, None),
        ];

        let workspaces: Vec<(&str, &[WindowRule])> = vec![("files", &rules)];

        let result = find_matching_workspace(&window, workspaces);
        assert!(result.is_some());
        let match_result = result.unwrap();
        assert_eq!(match_result.workspace_name, "files");
        assert_eq!(match_result.rule_index, 1);
    }

    // ========================================================================
    // Helper function tests
    // ========================================================================

    #[test]
    fn test_any_rule_matches() {
        let window = make_window("com.apple.finder", "Finder", "Documents");

        let rules = [
            make_rule(Some("com.apple.safari"), None, None),
            make_rule(Some("com.apple.finder"), None, None),
        ];

        assert!(any_rule_matches(&rules, &window));

        let rules = [
            make_rule(Some("com.apple.safari"), None, None),
            make_rule(Some("com.google.chrome"), None, None),
        ];

        assert!(!any_rule_matches(&rules, &window));

        // Empty rules
        let rules: [WindowRule; 0] = [];
        assert!(!any_rule_matches(&rules, &window));
    }

    #[test]
    fn test_count_matching_rules() {
        let window = make_window("com.apple.finder", "Finder", "Documents");

        let rules = [
            make_rule(Some("com.apple.finder"), None, None), // Matches
            make_rule(None, Some("Finder"), None),           // Matches
            make_rule(None, None, Some("Documents")),        // Matches
            make_rule(Some("com.apple.safari"), None, None), // Doesn't match
            make_rule(Some("com.apple.finder"), None, Some("X")), // Doesn't match (AND fails)
        ];

        assert_eq!(count_matching_rules(&rules, &window), 3);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_matches_window_empty_strings() {
        let window = make_window("com.apple.finder", "Finder", "");

        // Empty title in window, rule requires title
        let rule = make_rule(None, None, Some("Documents"));
        assert!(!matches_window(&rule, &window));

        // Empty title in rule should still match (empty string is substring of anything)
        let rule = make_rule(Some("com.apple.finder"), None, Some(""));
        assert!(matches_window(&rule, &window));
    }

    #[test]
    fn test_matches_window_special_characters() {
        let window = make_window("com.app.test", "App", "File: test.txt (modified)");

        let rule = make_rule(None, None, Some("test.txt"));
        assert!(matches_window(&rule, &window));

        let rule = make_rule(None, None, Some("(modified)"));
        assert!(matches_window(&rule, &window));

        let rule = make_rule(None, None, Some(":"));
        assert!(matches_window(&rule, &window));
    }

    #[test]
    fn test_matches_window_unicode() {
        let window = make_window("com.app.test", "文件管理器", "我的文档");

        let rule = make_rule(None, Some("文件"), None);
        assert!(matches_window(&rule, &window));

        let rule = make_rule(None, None, Some("文档"));
        assert!(matches_window(&rule, &window));
    }
}
