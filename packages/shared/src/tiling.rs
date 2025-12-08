//! Tiling window manager configuration types.
//!
//! This module provides configuration types for the tiling window manager feature,
//! including layouts, gaps, workspaces, window rules, and animations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// Layout Types
// ============================================================================

/// Available layout modes for workspaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    /// Windows are arranged in a binary space partitioning pattern (dwindle algorithm).
    #[default]
    Tiling,
    /// All windows are maximized and stacked, only the focused one is visible.
    Monocle,
    /// One master window on the left, remaining windows stacked on the right.
    Master,
    /// Two windows split based on screen orientation.
    Split,
    /// Two windows split vertically (side by side).
    #[serde(rename = "split-vertical")]
    SplitVertical,
    /// Two windows split horizontally (stacked top/bottom).
    #[serde(rename = "split-horizontal")]
    SplitHorizontal,
    /// Windows can be freely positioned and resized.
    Floating,
    /// Niri-style scrolling workspace layout.
    Scrolling,
}

impl std::fmt::Display for LayoutMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiling => write!(f, "tiling"),
            Self::Monocle => write!(f, "monocle"),
            Self::Master => write!(f, "master"),
            Self::Split => write!(f, "split"),
            Self::SplitVertical => write!(f, "split-vertical"),
            Self::SplitHorizontal => write!(f, "split-horizontal"),
            Self::Floating => write!(f, "floating"),
            Self::Scrolling => write!(f, "scrolling"),
        }
    }
}

impl std::str::FromStr for LayoutMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tiling" => Ok(Self::Tiling),
            "monocle" => Ok(Self::Monocle),
            "master" => Ok(Self::Master),
            "split" => Ok(Self::Split),
            "split-vertical" => Ok(Self::SplitVertical),
            "split-horizontal" => Ok(Self::SplitHorizontal),
            "floating" => Ok(Self::Floating),
            "scrolling" => Ok(Self::Scrolling),
            _ => Err(format!(
                "Invalid layout mode '{s}'. Expected one of: tiling, monocle, master, split, split-vertical, split-horizontal, floating, scrolling"
            )),
        }
    }
}

// ============================================================================
// Dimension Value Types
// ============================================================================

/// A dimension value that can be either pixels or a percentage.
///
/// Examples:
/// - `800` - 800 pixels
/// - `"50%"` - 50% of the available space
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum DimensionValue {
    /// Absolute value in pixels.
    Pixels(u32),
    /// Percentage of the available space (e.g., "50%").
    Percentage(String),
}

impl Default for DimensionValue {
    fn default() -> Self { Self::Pixels(0) }
}

impl DimensionValue {
    /// Resolves the dimension value to pixels given the available space.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn resolve(&self, available: u32) -> u32 {
        match self {
            Self::Pixels(px) => *px,
            Self::Percentage(pct) => {
                let pct_value =
                    pct.trim_end_matches('%').parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0);
                ((f64::from(available) * pct_value) / 100.0).round() as u32
            }
        }
    }
}

// ============================================================================
// Gap Configuration Types
// ============================================================================

/// Inner gap configuration (between windows).
///
/// Can be a uniform value or separate horizontal/vertical values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum InnerGaps {
    /// Same gap for both horizontal and vertical.
    Uniform(u32),
    /// Different gaps for horizontal and vertical.
    PerAxis {
        /// Horizontal gap between windows (left/right neighbors).
        horizontal: u32,
        /// Vertical gap between windows (top/bottom neighbors).
        vertical: u32,
    },
}

impl Default for InnerGaps {
    fn default() -> Self { Self::Uniform(0) }
}

impl InnerGaps {
    /// Returns the horizontal gap value.
    #[must_use]
    pub const fn horizontal(&self) -> u32 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerAxis { horizontal, .. } => *horizontal,
        }
    }

    /// Returns the vertical gap value.
    #[must_use]
    pub const fn vertical(&self) -> u32 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerAxis { vertical, .. } => *vertical,
        }
    }
}

/// Outer gap configuration (between windows and screen edges).
///
/// Can be a uniform value or separate values per side.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum OuterGaps {
    /// Same gap for all sides.
    Uniform(u32),
    /// Different gaps for each side.
    PerSide {
        /// Gap from the top edge of the screen.
        top: u32,
        /// Gap from the right edge of the screen.
        right: u32,
        /// Gap from the bottom edge of the screen.
        bottom: u32,
        /// Gap from the left edge of the screen.
        left: u32,
    },
}

impl Default for OuterGaps {
    fn default() -> Self { Self::Uniform(0) }
}

impl OuterGaps {
    /// Returns the top gap value.
    #[must_use]
    pub const fn top(&self) -> u32 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerSide { top, .. } => *top,
        }
    }

    /// Returns the right gap value.
    #[must_use]
    pub const fn right(&self) -> u32 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerSide { right, .. } => *right,
        }
    }

    /// Returns the bottom gap value.
    #[must_use]
    pub const fn bottom(&self) -> u32 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerSide { bottom, .. } => *bottom,
        }
    }

    /// Returns the left gap value.
    #[must_use]
    pub const fn left(&self) -> u32 {
        match self {
            Self::Uniform(v) => *v,
            Self::PerSide { left, .. } => *left,
        }
    }
}

/// Gap settings for a specific screen or all screens.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default)]
pub struct ScreenGaps {
    /// The screen this configuration applies to.
    /// Use "main" for the primary screen, or the screen name/identifier.
    /// If not specified, applies to all screens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,

    /// Gap between windows.
    pub inner: InnerGaps,

    /// Gap between windows and screen edges.
    pub outer: OuterGaps,
}

/// Gap configuration that supports both global and per-screen settings.
///
/// Examples:
/// ```json
/// // Global settings
/// { "inner": 10, "outer": 15 }
///
/// // Per-screen settings
/// [
///   { "screen": "main", "inner": 10, "outer": 15 },
///   { "screen": "DP-1", "inner": 8, "outer": 12 }
/// ]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum GapsConfig {
    /// Global gap settings applied to all screens.
    Global(ScreenGaps),
    /// Per-screen gap settings.
    PerScreen(Vec<ScreenGaps>),
}

impl Default for GapsConfig {
    fn default() -> Self { Self::Global(ScreenGaps::default()) }
}

impl GapsConfig {
    /// Gets the gap settings for a specific screen.
    ///
    /// Returns the screen-specific settings if available, otherwise the global settings.
    #[must_use]
    pub fn for_screen(&self, screen_name: &str) -> &ScreenGaps {
        match self {
            Self::Global(gaps) => gaps,
            Self::PerScreen(screens) => screens
                .iter()
                .find(|g| g.screen.as_deref() == Some(screen_name))
                .or_else(|| screens.iter().find(|g| g.screen.is_none()))
                .unwrap_or_else(|| {
                    // This shouldn't happen in practice, but return a safe default
                    static DEFAULT: ScreenGaps = ScreenGaps {
                        screen: None,
                        inner: InnerGaps::Uniform(0),
                        outer: OuterGaps::Uniform(0),
                    };
                    &DEFAULT
                }),
        }
    }
}

// ============================================================================
// Window Rule Types
// ============================================================================

/// A rule for matching windows to workspaces.
///
/// At least one matching criterion should be specified.
/// Multiple criteria are combined with AND logic.
///
/// When used as a global rule (in `TilingConfig.rules`), the `workspace` field
/// specifies which workspace matched windows should be assigned to.
/// When used as a per-workspace rule (in `WorkspaceConfig.rules`), the `workspace`
/// field is ignored and the window is assigned to the containing workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct WindowRule {
    /// Match by window title (substring match, case-insensitive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Match by window class name (substring match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,

    /// Match by application bundle identifier (e.g., "com.apple.Safari").
    /// Supports both exact match and substring match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,

    /// Match by application name (e.g., "Safari").
    /// Supports both exact match and substring match (case-insensitive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Target workspace for global rules.
    /// This field is only used when the rule is defined in `TilingConfig.rules`.
    /// For per-workspace rules, this field is ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

impl WindowRule {
    /// Returns whether this rule has any matching criteria.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.title.is_none() && self.class.is_none() && self.app_id.is_none() && self.name.is_none()
    }
}

// ============================================================================
// Workspace Configuration
// ============================================================================

/// Screen target for workspace assignment.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScreenTarget {
    /// The primary/main display.
    #[default]
    Main,
    /// The secondary display (if available).
    Secondary,
    /// A specific display by name or identifier.
    #[serde(untagged)]
    Named(String),
}

impl std::fmt::Display for ScreenTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Secondary => write!(f, "secondary"),
            Self::Named(name) => write!(f, "{name}"),
        }
    }
}

/// Configuration for a workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct WorkspaceConfig {
    /// Unique name/identifier for the workspace.
    pub name: String,

    /// The layout mode for this workspace.
    pub layout: LayoutMode,

    /// The screen this workspace is assigned to.
    pub screen: ScreenTarget,

    /// Window rules for automatically assigning windows to this workspace.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<WindowRule>,

    /// Name of a floating preset to automatically apply when a window is opened in this workspace.
    /// Only applies when the workspace layout is set to floating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_on_open: Option<String>,
}

// ============================================================================
// Floating Window Configuration
// ============================================================================

/// Default position for floating windows.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FloatingDefaultPosition {
    /// Center the window on the screen.
    #[default]
    Center,
    /// Use the last known position (or system default for new windows).
    Default,
}

/// A preset configuration for floating window position and size.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct FloatingPreset {
    /// Unique name for this preset (used in CLI commands).
    pub name: String,

    /// Width of the window (pixels or percentage).
    pub width: DimensionValue,

    /// Height of the window (pixels or percentage).
    pub height: DimensionValue,

    /// X position from the left edge (pixels or percentage).
    /// Ignored if `center` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<DimensionValue>,

    /// Y position from the top edge (pixels or percentage).
    /// Ignored if `center` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<DimensionValue>,

    /// If true, center the window on the screen (x and y are ignored).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub center: bool,
}

/// Configuration for floating window behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct FloatingConfig {
    /// Default position for new floating windows.
    pub default_position: FloatingDefaultPosition,

    /// Predefined position/size presets for floating windows.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<FloatingPreset>,
}

// ============================================================================
// Master Layout Configuration
// ============================================================================

/// Configuration for the master layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct MasterConfig {
    /// The percentage of screen width the master window takes (0-100).
    pub ratio: u32,

    /// Maximum number of windows in the master area.
    pub max_masters: u32,
}

impl Default for MasterConfig {
    fn default() -> Self { Self { ratio: 60, max_masters: 1 } }
}

// ============================================================================
// Animation Configuration
// ============================================================================

/// Easing functions for animations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EasingFunction {
    /// Linear interpolation (no easing).
    Linear,
    /// Slow start, fast end.
    EaseIn,
    /// Fast start, slow end.
    #[default]
    EaseOut,
    /// Slow start and end, fast middle.
    EaseInOut,
    /// Spring physics-based animation.
    Spring,
}

/// Animation settings when enabled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct AnimationSettings {
    /// Duration of animations in milliseconds.
    pub duration: u32,

    /// Easing function to use for animations.
    pub easing: EasingFunction,
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self {
            duration: 200,
            easing: EasingFunction::default(),
        }
    }
}

/// Animation configuration that can be enabled with settings or disabled.
///
/// Examples:
/// ```json
/// // Disabled (default)
/// { "animations": false }
///
/// // Enabled with default settings
/// { "animations": true }
///
/// // Enabled with custom settings
/// { "animations": { "duration": 200, "easing": "spring" } }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum AnimationConfig {
    /// Simple boolean to enable/disable with defaults.
    Enabled(bool),
    /// Custom animation settings (implies enabled).
    Settings(AnimationSettings),
}

impl Default for AnimationConfig {
    fn default() -> Self { Self::Enabled(false) }
}

impl AnimationConfig {
    /// Returns whether animations are enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        match self {
            Self::Enabled(enabled) => *enabled,
            Self::Settings(_) => true,
        }
    }

    /// Returns the animation settings, or defaults if disabled.
    #[must_use]
    pub fn settings(&self) -> AnimationSettings {
        match self {
            Self::Enabled(true) | Self::Settings(_) => {
                if let Self::Settings(settings) = self {
                    settings.clone()
                } else {
                    AnimationSettings::default()
                }
            }
            Self::Enabled(false) => AnimationSettings::default(),
        }
    }
}

// ============================================================================
// Root Tiling Configuration
// ============================================================================

/// Root configuration for the tiling window manager.
///
/// Example:
/// ```json
/// {
///   "tiling": {
///     "enabled": true,
///     "defaultLayout": "tiling",
///     "gaps": { "inner": 10, "outer": 15 },
///     "workspaces": [
///       { "name": "1", "layout": "tiling", "screen": "main" }
///     ],
///     "animations": false
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct TilingConfig {
    /// Whether the tiling window manager is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Default layout mode for new workspaces.
    pub default_layout: LayoutMode,

    /// Gap configuration between windows and screen edges.
    pub gaps: GapsConfig,

    /// Workspace configurations.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub workspaces: Vec<WorkspaceConfig>,

    /// Floating window behavior configuration.
    pub floating: FloatingConfig,

    /// Master layout configuration.
    pub master: MasterConfig,

    /// Window rules for all workspaces (in addition to per-workspace rules).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<WindowRule>,

    /// Animation configuration.
    pub animations: AnimationConfig,
}

/// Default for enabled - true when workspaces are configured.
const fn default_enabled() -> bool { true }

impl Default for TilingConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            default_layout: LayoutMode::default(),
            gaps: GapsConfig::default(),
            workspaces: Vec::new(),
            floating: FloatingConfig::default(),
            master: MasterConfig::default(),
            rules: Vec::new(),
            animations: AnimationConfig::default(),
        }
    }
}

impl TilingConfig {
    /// Returns whether tiling functionality is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool { self.enabled }

    /// Gets the default workspaces if none are configured.
    #[must_use]
    pub fn effective_workspaces(&self) -> Vec<WorkspaceConfig> {
        if self.workspaces.is_empty() {
            // Create default workspaces 1-9 on main screen
            (1..=9)
                .map(|i| WorkspaceConfig {
                    name: i.to_string(),
                    layout: self.default_layout.clone(),
                    screen: ScreenTarget::Main,
                    rules: Vec::new(),
                    preset_on_open: None,
                })
                .collect()
        } else {
            self.workspaces.clone()
        }
    }

    /// Validates the configuration and returns a list of warnings and errors.
    ///
    /// Warnings indicate potential issues but don't prevent tiling from working.
    /// Errors indicate configuration problems that should be fixed.
    ///
    /// # Returns
    ///
    /// A tuple of (warnings, errors) where each is a vector of human-readable messages.
    #[must_use]
    pub fn validate(&self) -> (Vec<String>, Vec<String>) {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Check for duplicate workspace names
        let mut seen_names = std::collections::HashSet::new();
        for ws in &self.workspaces {
            if !seen_names.insert(&ws.name) {
                errors.push(format!(
                    "Duplicate workspace name '{}'. Each workspace must have a unique name.",
                    ws.name
                ));
            }
        }

        // Check for empty workspace names
        for ws in &self.workspaces {
            if ws.name.trim().is_empty() {
                errors.push(
                    "Empty workspace name found. Workspace names cannot be empty.".to_string(),
                );
            }
        }

        // Check master ratio is within valid range
        if self.master.ratio > 100 {
            errors.push(format!(
                "Master ratio {} is invalid. Must be between 0 and 100. Fix: set 'master.ratio' to a value like 55.",
                self.master.ratio
            ));
        }

        // Check master max_masters is reasonable
        if self.master.max_masters == 0 {
            warnings.push(
                "Master max-masters is 0, which means no master windows. Consider setting it to at least 1.".to_string()
            );
        }

        // Check for global rules without workspace targets
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.workspace.is_none() && !rule.is_empty() {
                warnings.push(format!(
                    "Global rule {} has no 'workspace' target. Global rules should specify which workspace to assign windows to. \
                     Fix: add 'workspace: \"workspace-name\"' to the rule.",
                    i + 1
                ));
            }
        }

        // Check for rules with no matching criteria
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.is_empty() {
                warnings.push(format!(
                    "Global rule {} has no matching criteria. It should have at least one of: 'title', 'class', or 'app-id'.",
                    i + 1
                ));
            }
        }

        // Check per-workspace rules
        for ws in &self.workspaces {
            for (i, rule) in ws.rules.iter().enumerate() {
                if rule.is_empty() {
                    warnings.push(format!(
                        "Workspace '{}' rule {} has no matching criteria. It should have at least one of: 'title', 'class', or 'app-id'.",
                        ws.name, i + 1
                    ));
                }
            }
        }

        // Check floating presets for duplicate names
        let mut seen_preset_names = std::collections::HashSet::new();
        for preset in &self.floating.presets {
            if !seen_preset_names.insert(&preset.name) {
                errors.push(format!(
                    "Duplicate floating preset name '{}'. Each preset must have a unique name.",
                    preset.name
                ));
            }
        }

        // Check floating presets for empty names
        for preset in &self.floating.presets {
            if preset.name.trim().is_empty() {
                errors.push(
                    "Floating preset with empty name found. Preset names cannot be empty."
                        .to_string(),
                );
            }
        }

        // Check preset_on_open references valid presets
        let preset_names: std::collections::HashSet<_> =
            self.floating.presets.iter().map(|p| &p.name).collect();
        for ws in &self.workspaces {
            if let Some(ref preset_name) = ws.preset_on_open {
                if !preset_names.contains(preset_name) {
                    errors.push(format!(
                        "Workspace '{}' references unknown floating preset '{}'. \
                         Fix: define the preset in 'floating.presets' or remove 'preset-on-open'.",
                        ws.name, preset_name
                    ));
                }
            }
        }

        // Check animation duration is reasonable
        let anim_settings = self.animations.settings();
        if self.animations.is_enabled() && anim_settings.duration == 0 {
            warnings.push(
                "Animation duration is 0ms but animations are enabled. This will cause instant transitions. \
                 Fix: set 'animations.duration' to a value like 150.".to_string()
            );
        }

        if anim_settings.duration > 1000 {
            warnings.push(format!(
                "Animation duration of {}ms is very long (>1 second). This may feel sluggish. \
                 Consider reducing to 150-300ms.",
                anim_settings.duration
            ));
        }

        (warnings, errors)
    }

    /// Validates the configuration and logs any warnings or errors.
    ///
    /// # Returns
    ///
    /// `true` if no errors were found, `false` otherwise.
    pub fn validate_and_log(&self) -> bool {
        let (warnings, errors) = self.validate();

        for warning in &warnings {
            eprintln!("barba: config warning: {warning}");
        }

        for error in &errors {
            eprintln!("barba: config error: {error}");
        }

        if !errors.is_empty() {
            eprintln!(
                "barba: {} config error(s) found. Tiling may not work correctly.",
                errors.len()
            );
        }

        errors.is_empty()
    }
}

// ============================================================================
// Query Result Types (for CLI output)
// ============================================================================

/// Information about a connected screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScreenInfo {
    /// Unique identifier for the screen.
    pub id: String,

    /// Human-readable name of the screen.
    pub name: String,

    /// Whether this is the main/primary display.
    pub is_main: bool,

    /// Screen position (x coordinate).
    pub x: i32,

    /// Screen position (y coordinate).
    pub y: i32,

    /// Screen width in pixels.
    pub width: u32,

    /// Screen height in pixels.
    pub height: u32,

    /// Usable area X position (accounts for dock/menu bar).
    pub usable_x: i32,

    /// Usable area Y position (accounts for dock/menu bar).
    pub usable_y: i32,

    /// Usable area width (accounts for dock/menu bar).
    pub usable_width: u32,

    /// Usable area height (accounts for dock/menu bar).
    pub usable_height: u32,
}

/// Information about a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    /// Workspace name/identifier.
    pub name: String,

    /// Current layout mode.
    pub layout: LayoutMode,

    /// Screen this workspace is on.
    pub screen: String,

    /// Whether this workspace is currently focused.
    pub is_focused: bool,

    /// Number of windows in this workspace.
    pub window_count: usize,
}

/// Information about a managed window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    /// Unique window identifier.
    pub id: u64,

    /// Window title.
    pub title: String,

    /// Application name.
    pub app_name: String,

    /// Application bundle identifier (macOS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,

    /// Window class name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,

    /// Workspace this window belongs to.
    pub workspace: String,

    /// Whether this window is currently focused.
    pub is_focused: bool,

    /// Whether this window is floating.
    pub is_floating: bool,

    /// Window position (x coordinate).
    pub x: i32,

    /// Window position (y coordinate).
    pub y: i32,

    /// Window width in pixels.
    pub width: u32,

    /// Window height in pixels.
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_mode_default() {
        assert_eq!(LayoutMode::default(), LayoutMode::Tiling);
    }

    #[test]
    fn test_layout_mode_from_str() {
        assert_eq!("tiling".parse::<LayoutMode>().unwrap(), LayoutMode::Tiling);
        assert_eq!("monocle".parse::<LayoutMode>().unwrap(), LayoutMode::Monocle);
        assert_eq!("master".parse::<LayoutMode>().unwrap(), LayoutMode::Master);
        assert_eq!("split".parse::<LayoutMode>().unwrap(), LayoutMode::Split);
        assert_eq!(
            "split-vertical".parse::<LayoutMode>().unwrap(),
            LayoutMode::SplitVertical
        );
        assert_eq!(
            "split-horizontal".parse::<LayoutMode>().unwrap(),
            LayoutMode::SplitHorizontal
        );
        assert_eq!("floating".parse::<LayoutMode>().unwrap(), LayoutMode::Floating);
        assert_eq!("scrolling".parse::<LayoutMode>().unwrap(), LayoutMode::Scrolling);
        assert!("invalid".parse::<LayoutMode>().is_err());
    }

    #[test]
    fn test_dimension_value_resolve() {
        let px = DimensionValue::Pixels(800);
        assert_eq!(px.resolve(1920), 800);

        let pct = DimensionValue::Percentage("50%".to_string());
        assert_eq!(pct.resolve(1920), 960);

        let pct_zero = DimensionValue::Percentage("0%".to_string());
        assert_eq!(pct_zero.resolve(1920), 0);

        let pct_full = DimensionValue::Percentage("100%".to_string());
        assert_eq!(pct_full.resolve(1920), 1920);
    }

    #[test]
    fn test_inner_gaps() {
        let uniform = InnerGaps::Uniform(10);
        assert_eq!(uniform.horizontal(), 10);
        assert_eq!(uniform.vertical(), 10);

        let per_axis = InnerGaps::PerAxis { horizontal: 8, vertical: 12 };
        assert_eq!(per_axis.horizontal(), 8);
        assert_eq!(per_axis.vertical(), 12);
    }

    #[test]
    fn test_outer_gaps() {
        let uniform = OuterGaps::Uniform(15);
        assert_eq!(uniform.top(), 15);
        assert_eq!(uniform.right(), 15);
        assert_eq!(uniform.bottom(), 15);
        assert_eq!(uniform.left(), 15);

        let per_side = OuterGaps::PerSide {
            top: 10,
            right: 20,
            bottom: 10,
            left: 20,
        };
        assert_eq!(per_side.top(), 10);
        assert_eq!(per_side.right(), 20);
        assert_eq!(per_side.bottom(), 10);
        assert_eq!(per_side.left(), 20);
    }

    #[test]
    fn test_gaps_config_for_screen() {
        let global = GapsConfig::Global(ScreenGaps {
            screen: None,
            inner: InnerGaps::Uniform(10),
            outer: OuterGaps::Uniform(15),
        });
        assert_eq!(global.for_screen("any").inner.horizontal(), 10);

        let per_screen = GapsConfig::PerScreen(vec![
            ScreenGaps {
                screen: Some("main".to_string()),
                inner: InnerGaps::Uniform(8),
                outer: OuterGaps::Uniform(12),
            },
            ScreenGaps {
                screen: Some("DP-1".to_string()),
                inner: InnerGaps::Uniform(10),
                outer: OuterGaps::Uniform(15),
            },
        ]);
        assert_eq!(per_screen.for_screen("main").inner.horizontal(), 8);
        assert_eq!(per_screen.for_screen("DP-1").inner.horizontal(), 10);
    }

    #[test]
    fn test_animation_config() {
        let disabled = AnimationConfig::Enabled(false);
        assert!(!disabled.is_enabled());

        let enabled = AnimationConfig::Enabled(true);
        assert!(enabled.is_enabled());

        let with_settings = AnimationConfig::Settings(AnimationSettings {
            duration: 300,
            easing: EasingFunction::Spring,
        });
        assert!(with_settings.is_enabled());
        assert_eq!(with_settings.settings().duration, 300);
    }

    #[test]
    fn test_tiling_config_default() {
        let config = TilingConfig::default();
        assert!(config.is_enabled());
        assert_eq!(config.default_layout, LayoutMode::Tiling);
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn test_tiling_config_effective_workspaces() {
        let empty_config = TilingConfig::default();
        let default_workspaces = empty_config.effective_workspaces();
        assert_eq!(default_workspaces.len(), 9);
        assert_eq!(default_workspaces[0].name, "1");
        assert_eq!(default_workspaces[8].name, "9");

        let config_with_workspaces = TilingConfig {
            workspaces: vec![WorkspaceConfig {
                name: "custom".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let custom_workspaces = config_with_workspaces.effective_workspaces();
        assert_eq!(custom_workspaces.len(), 1);
        assert_eq!(custom_workspaces[0].name, "custom");
    }

    #[test]
    fn test_window_rule_is_empty() {
        let empty = WindowRule::default();
        assert!(empty.is_empty());

        let with_title = WindowRule {
            title: Some("Test".to_string()),
            ..Default::default()
        };
        assert!(!with_title.is_empty());

        let with_class = WindowRule {
            class: Some("NSWindow".to_string()),
            ..Default::default()
        };
        assert!(!with_class.is_empty());

        let with_app_id = WindowRule {
            app_id: Some("com.apple.Safari".to_string()),
            ..Default::default()
        };
        assert!(!with_app_id.is_empty());

        // workspace field alone doesn't make a rule non-empty (it's just a target)
        let with_only_workspace = WindowRule {
            workspace: Some("browser".to_string()),
            ..Default::default()
        };
        assert!(with_only_workspace.is_empty());

        // Multiple criteria
        let with_multiple = WindowRule {
            title: Some("Test".to_string()),
            app_id: Some("com.test.app".to_string()),
            ..Default::default()
        };
        assert!(!with_multiple.is_empty());
    }

    #[test]
    fn test_window_rule_with_workspace_target() {
        let rule = WindowRule {
            app_id: Some("com.apple.Safari".to_string()),
            workspace: Some("browser".to_string()),
            ..Default::default()
        };

        assert!(!rule.is_empty());
        assert_eq!(rule.workspace, Some("browser".to_string()));
    }

    #[test]
    fn test_window_rule_serialization() {
        // Per-workspace rule (no workspace field)
        let per_workspace_rule = WindowRule {
            app_id: Some("com.apple.Safari".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&per_workspace_rule).unwrap();
        assert!(json.contains("app-id"));
        assert!(!json.contains("workspace")); // Should be skipped when None

        // Global rule (with workspace field)
        let global_rule = WindowRule {
            app_id: Some("com.apple.Safari".to_string()),
            workspace: Some("browser".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&global_rule).unwrap();
        assert!(json.contains("app-id"));
        assert!(json.contains("workspace"));
        assert!(json.contains("browser"));
    }

    #[test]
    fn test_window_rule_deserialization() {
        let json = r#"{"app-id": "com.apple.Safari", "workspace": "browser"}"#;
        let rule: WindowRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.app_id, Some("com.apple.Safari".to_string()));
        assert_eq!(rule.workspace, Some("browser".to_string()));

        // Without workspace
        let json = r#"{"title": "Firefox"}"#;
        let rule: WindowRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.title, Some("Firefox".to_string()));
        assert_eq!(rule.workspace, None);
    }

    #[test]
    fn test_tiling_config_global_rules() {
        let config = TilingConfig {
            rules: vec![
                WindowRule {
                    app_id: Some("com.apple.Safari".to_string()),
                    workspace: Some("browser".to_string()),
                    ..Default::default()
                },
                WindowRule {
                    app_id: Some("com.microsoft.VSCode".to_string()),
                    workspace: Some("code".to_string()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(config.rules.len(), 2);
        assert_eq!(config.rules[0].workspace, Some("browser".to_string()));
        assert_eq!(config.rules[1].workspace, Some("code".to_string()));
    }

    #[test]
    fn test_workspace_config_with_rules() {
        let ws = WorkspaceConfig {
            name: "browser".to_string(),
            layout: LayoutMode::Tiling,
            screen: ScreenTarget::Main,
            rules: vec![
                WindowRule {
                    app_id: Some("com.apple.Safari".to_string()),
                    ..Default::default()
                },
                WindowRule {
                    title: Some("Firefox".to_string()),
                    ..Default::default()
                },
            ],
            preset_on_open: None,
        };

        assert_eq!(ws.rules.len(), 2);
        // Per-workspace rules shouldn't have workspace field set
        assert_eq!(ws.rules[0].workspace, None);
        assert_eq!(ws.rules[1].workspace, None);
    }

    #[test]
    fn test_tiling_config_serialization() {
        let config = TilingConfig {
            enabled: true,
            default_layout: LayoutMode::Tiling,
            gaps: GapsConfig::Global(ScreenGaps {
                screen: None,
                inner: InnerGaps::Uniform(10),
                outer: OuterGaps::Uniform(15),
            }),
            workspaces: vec![WorkspaceConfig {
                name: "1".to_string(),
                layout: LayoutMode::Tiling,
                screen: ScreenTarget::Main,
                rules: vec![WindowRule {
                    app_id: Some("com.apple.Safari".to_string()),
                    ..Default::default()
                }],
                preset_on_open: None,
            }],
            floating: FloatingConfig::default(),
            master: MasterConfig::default(),
            rules: Vec::new(),
            animations: AnimationConfig::Enabled(false),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"enabled\": true"));
        assert!(json.contains("\"defaultLayout\": \"tiling\""));

        // Deserialize back
        let parsed: TilingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.enabled, config.enabled);
        assert_eq!(parsed.workspaces.len(), 1);
    }

    #[test]
    fn test_floating_preset_centered() {
        let preset = FloatingPreset {
            name: "centered".to_string(),
            width: DimensionValue::Percentage("80%".to_string()),
            height: DimensionValue::Percentage("80%".to_string()),
            x: None,
            y: None,
            center: true,
        };

        // Width/height resolve correctly
        assert_eq!(preset.width.resolve(1920), 1536); // 80% of 1920
        assert_eq!(preset.height.resolve(1080), 864); // 80% of 1080

        // Center flag is set
        assert!(preset.center);
    }

    #[test]
    fn test_floating_preset_positioned() {
        let preset = FloatingPreset {
            name: "top-left".to_string(),
            width: DimensionValue::Pixels(800),
            height: DimensionValue::Pixels(600),
            x: Some(DimensionValue::Pixels(100)),
            y: Some(DimensionValue::Pixels(50)),
            center: false,
        };

        assert_eq!(preset.width.resolve(1920), 800);
        assert_eq!(preset.height.resolve(1080), 600);
        assert_eq!(preset.x.as_ref().unwrap().resolve(1920), 100);
        assert_eq!(preset.y.as_ref().unwrap().resolve(1080), 50);
        assert!(!preset.center);
    }

    #[test]
    fn test_floating_preset_percentage_position() {
        let preset = FloatingPreset {
            name: "right-half".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Percentage("100%".to_string()),
            x: Some(DimensionValue::Percentage("50%".to_string())),
            y: Some(DimensionValue::Pixels(0)),
            center: false,
        };

        // Right half of 1920x1080 screen
        assert_eq!(preset.width.resolve(1920), 960); // 50%
        assert_eq!(preset.height.resolve(1080), 1080); // 100%
        assert_eq!(preset.x.as_ref().unwrap().resolve(1920), 960); // starts at 50%
        assert_eq!(preset.y.as_ref().unwrap().resolve(1080), 0);
    }

    #[test]
    fn test_floating_config_with_presets() {
        let config = FloatingConfig {
            default_position: FloatingDefaultPosition::Center,
            presets: vec![
                FloatingPreset {
                    name: "small".to_string(),
                    width: DimensionValue::Pixels(400),
                    height: DimensionValue::Pixels(300),
                    center: true,
                    ..Default::default()
                },
                FloatingPreset {
                    name: "large".to_string(),
                    width: DimensionValue::Percentage("90%".to_string()),
                    height: DimensionValue::Percentage("90%".to_string()),
                    center: true,
                    ..Default::default()
                },
            ],
        };

        assert_eq!(config.presets.len(), 2);
        assert_eq!(config.presets[0].name, "small");
        assert_eq!(config.presets[1].name, "large");
    }

    #[test]
    fn test_workspace_config_with_preset_on_open() {
        let ws = WorkspaceConfig {
            name: "floating-ws".to_string(),
            layout: LayoutMode::Floating,
            screen: ScreenTarget::Main,
            rules: vec![],
            preset_on_open: Some("centered".to_string()),
        };

        assert_eq!(ws.preset_on_open, Some("centered".to_string()));
    }

    #[test]
    fn test_floating_preset_serialization() {
        let preset = FloatingPreset {
            name: "test".to_string(),
            width: DimensionValue::Percentage("50%".to_string()),
            height: DimensionValue::Pixels(600),
            x: Some(DimensionValue::Pixels(100)),
            y: None,
            center: false,
        };

        let json = serde_json::to_string(&preset).unwrap();
        // center=false should be skipped
        assert!(!json.contains("center"));
        // x is present
        assert!(json.contains("\"x\""));
        // y=None should be skipped
        assert!(!json.contains("\"y\""));

        // Round-trip
        let parsed: FloatingPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.width, DimensionValue::Percentage("50%".to_string()));
        assert_eq!(parsed.height, DimensionValue::Pixels(600));
        assert_eq!(parsed.x, Some(DimensionValue::Pixels(100)));
        assert_eq!(parsed.y, None);
        assert!(!parsed.center);
    }

    #[test]
    fn test_config_validation_duplicate_workspaces() {
        let config = TilingConfig {
            workspaces: vec![
                WorkspaceConfig {
                    name: "ws1".to_string(),
                    ..Default::default()
                },
                WorkspaceConfig {
                    name: "ws1".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (warnings, errors) = config.validate();
        assert!(errors.iter().any(|e| e.contains("Duplicate workspace name")));
        assert!(warnings.is_empty() || !warnings.iter().any(|w| w.contains("Duplicate")));
    }

    #[test]
    fn test_config_validation_invalid_master_ratio() {
        let config = TilingConfig {
            master: MasterConfig { ratio: 150, max_masters: 1 },
            ..Default::default()
        };
        let (_, errors) = config.validate();
        assert!(errors.iter().any(|e| e.contains("Master ratio")));
    }

    #[test]
    fn test_config_validation_global_rule_without_workspace() {
        let config = TilingConfig {
            rules: vec![WindowRule {
                app_id: Some("com.test.app".to_string()),
                workspace: None,
                ..Default::default()
            }],
            ..Default::default()
        };
        let (warnings, _) = config.validate();
        assert!(warnings.iter().any(|w| w.contains("no 'workspace' target")));
    }

    #[test]
    fn test_config_validation_unknown_preset_reference() {
        let config = TilingConfig {
            workspaces: vec![WorkspaceConfig {
                name: "ws1".to_string(),
                preset_on_open: Some("nonexistent".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let (_, errors) = config.validate();
        assert!(errors.iter().any(|e| e.contains("unknown floating preset")));
    }

    #[test]
    fn test_config_validation_valid_config() {
        let config = TilingConfig {
            workspaces: vec![
                WorkspaceConfig {
                    name: "ws1".to_string(),
                    ..Default::default()
                },
                WorkspaceConfig {
                    name: "ws2".to_string(),
                    ..Default::default()
                },
            ],
            floating: FloatingConfig {
                presets: vec![FloatingPreset {
                    name: "centered".to_string(),
                    width: DimensionValue::Percentage("50%".to_string()),
                    height: DimensionValue::Percentage("50%".to_string()),
                    center: true,
                    ..Default::default()
                }],
                ..Default::default()
            },
            rules: vec![WindowRule {
                app_id: Some("com.test.app".to_string()),
                workspace: Some("ws1".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let (warnings, errors) = config.validate();
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }
}
