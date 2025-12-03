//! CLI command definitions using Clap.
//!
//! This module defines all CLI commands and their arguments.

use std::str::FromStr;

use clap::{Parser, Subcommand};

use crate::error::CliError;
use crate::ipc;

/// Application version from Cargo.toml.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Barba Shell CLI - Dispatches events to the running Barba instance.
#[derive(Parser, Debug)]
#[command(name = "barba")]
#[command(author, version = APP_VERSION, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available CLI commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Notify Barba that the focused window changed.
    ///
    /// Use when your automation detects a window-focus change.
    /// Triggers Hyprspace queries to refresh current workspace and app state.
    #[command(name = "focus-changed")]
    FocusChanged,

    /// Notify Barba that the active workspace changed.
    ///
    /// Requires the new workspace name so Barba can update its Hyprspace view
    /// and trigger a window refresh.
    #[command(name = "workspace-changed")]
    WorkspaceChanged {
        /// Workspace identifier reported by hyprspace (e.g. coding).
        name: String,
    },

    /// Wallpaper management commands.
    #[command(subcommand)]
    Wallpaper(WallpaperCommands),

    /// Reload Barba configuration.
    ///
    /// Reloads the configuration file and applies changes without restarting
    /// the application.
    Reload,

    /// Generate JSON schema for the configuration file.
    ///
    /// Outputs a JSON Schema to stdout that describes the structure of the
    /// Barba configuration file. Can be redirected to a file for use with
    /// editors that support JSON Schema validation.
    #[command(name = "generate-schema")]
    GenerateSchema,
}

/// Screen target for wallpaper commands.
///
/// Specifies which screen(s) should receive the wallpaper.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenTarget {
    /// Apply to all screens.
    #[default]
    All,
    /// Apply to the main screen only.
    Main,
    /// Apply to a specific screen by 1-based index.
    Index(usize),
}

impl FromStr for ScreenTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(Self::All),
            "main" => Ok(Self::Main),
            _ => s.parse::<usize>().map(Self::Index).map_err(|_| {
                format!(
                    "Invalid screen value '{s}'. Expected 'all', 'main', or a positive integer."
                )
            }),
        }
    }
}

impl std::fmt::Display for ScreenTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Main => write!(f, "main"),
            Self::Index(idx) => write!(f, "{idx}"),
        }
    }
}

/// Data sent for the wallpaper set command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WallpaperSetData {
    /// The path to the image file to set as wallpaper, if specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether to set a random wallpaper.
    pub random: bool,
    /// The target screen(s) for the wallpaper.
    pub screen: ScreenTarget,
}

/// Wallpaper subcommands.
#[derive(Subcommand, Debug)]
pub enum WallpaperCommands {
    /// Set the desktop wallpaper.
    ///
    /// Set a specific wallpaper by providing a path, or use --random to set
    /// a random wallpaper from the configured wallpaper directory.
    #[command(
        verbatim_doc_comment,
        after_long_help = r#"Examples:
  barba wallpaper set /path/to/image.jpg               # Specific wallpaper for all screens
  barba wallpaper set /path/to/image.jpg --screen main # Specific wallpaper for main screen
  barba wallpaper set /path/to/image.jpg --screen 2    # Specific wallpaper for screen 2
  barba wallpaper set --random                         # Random wallpaper for all screens
  barba wallpaper set --random --screen main           # Random wallpaper for main screen
  barba wallpaper set --random --screen 2              # Random wallpaper for screen 2"#
    )]
    Set {
        /// The path to the image to use as wallpaper.
        #[arg(value_name = "PATH")]
        path: Option<String>,

        /// Set a random wallpaper from the configured wallpaper directory.
        #[arg(long, short)]
        random: bool,

        /// Specify which screen(s) to set the wallpaper on.
        /// Values: all, main, <index> (1-based).
        /// Default: all
        #[arg(long, short, default_value = "all")]
        screen: ScreenTarget,
    },

    /// Pre-generate all wallpapers.
    ///
    /// Processes all wallpapers from the configuration and stores them in the
    /// cache directory. Useful for pre-caching wallpapers to avoid delays
    /// when switching.
    #[command(name = "generate-all")]
    GenerateAll,
}

/// Payload for CLI events sent to the desktop app.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliEventPayload {
    /// The name of the CLI command/event.
    pub name: String,
    /// Optional data associated with the command.
    pub data: Option<String>,
}

impl Cli {
    /// Execute the CLI command.
    pub fn execute(&self) -> Result<(), CliError> {
        match &self.command {
            Commands::FocusChanged => {
                let payload = CliEventPayload {
                    name: "focus-changed".to_string(),
                    data: None,
                };
                ipc::send_to_desktop_app(&payload)?;
            }

            Commands::WorkspaceChanged { name } => {
                let payload = CliEventPayload {
                    name: "workspace-changed".to_string(),
                    data: Some(name.clone()),
                };
                ipc::send_to_desktop_app(&payload)?;
            }

            Commands::Wallpaper(wallpaper_cmd) => match wallpaper_cmd {
                WallpaperCommands::Set { path, random, screen } => {
                    // Validate: either path or random must be specified, but not both
                    if path.is_some() && *random {
                        return Err(CliError::InvalidArguments(
                            "Cannot specify both <path> and --random. Use one or the other."
                                .to_string(),
                        ));
                    }

                    if path.is_none() && !*random {
                        return Err(CliError::InvalidArguments(
                            "Either <path> or --random must be specified.".to_string(),
                        ));
                    }

                    let data = WallpaperSetData {
                        path: path.clone(),
                        random: *random,
                        screen: screen.clone(),
                    };
                    let payload = CliEventPayload {
                        name: "wallpaper-set".to_string(),
                        data: Some(serde_json::to_string(&data).unwrap()),
                    };
                    ipc::send_to_desktop_app(&payload)?;
                }
                WallpaperCommands::GenerateAll => {
                    let payload = CliEventPayload {
                        name: "wallpaper-generate-all".to_string(),
                        data: None,
                    };
                    ipc::send_to_desktop_app_extended(&payload)?;
                }
            },

            Commands::Reload => {
                let payload = CliEventPayload {
                    name: "reload".to_string(),
                    data: None,
                };
                ipc::send_to_desktop_app(&payload)?;
            }

            Commands::GenerateSchema => {
                // Generate schema locally using the shared config types
                // This doesn't require the desktop app to be running
                let schema = barba_shared::generate_schema_json();
                println!("{schema}");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_event_payload_serialization() {
        let payload = CliEventPayload {
            name: "test-event".to_string(),
            data: Some("test-data".to_string()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("test-event"));
        assert!(json.contains("test-data"));
    }

    #[test]
    fn test_cli_event_payload_serialization_without_data() {
        let payload = CliEventPayload {
            name: "test-event".to_string(),
            data: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("test-event"));
        assert!(json.contains("null"));
    }

    #[test]
    fn test_screen_target_from_str_all() {
        let target: ScreenTarget = "all".parse().unwrap();
        assert_eq!(target, ScreenTarget::All);
    }

    #[test]
    fn test_screen_target_from_str_main() {
        let target: ScreenTarget = "main".parse().unwrap();
        assert_eq!(target, ScreenTarget::Main);
    }

    #[test]
    fn test_screen_target_from_str_index() {
        let target: ScreenTarget = "2".parse().unwrap();
        assert_eq!(target, ScreenTarget::Index(2));
    }

    #[test]
    fn test_screen_target_from_str_case_insensitive() {
        let target: ScreenTarget = "ALL".parse().unwrap();
        assert_eq!(target, ScreenTarget::All);

        let target: ScreenTarget = "Main".parse().unwrap();
        assert_eq!(target, ScreenTarget::Main);
    }

    #[test]
    fn test_screen_target_from_str_invalid() {
        let result: Result<ScreenTarget, _> = "invalid".parse();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid screen value"));
    }

    #[test]
    fn test_screen_target_display() {
        assert_eq!(ScreenTarget::All.to_string(), "all");
        assert_eq!(ScreenTarget::Main.to_string(), "main");
        assert_eq!(ScreenTarget::Index(2).to_string(), "2");
    }

    #[test]
    fn test_screen_target_default() {
        let target = ScreenTarget::default();
        assert_eq!(target, ScreenTarget::All);
    }

    #[test]
    fn test_wallpaper_set_data_serialization_with_path() {
        let data = WallpaperSetData {
            path: Some("/path/to/image.jpg".to_string()),
            random: false,
            screen: ScreenTarget::All,
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("/path/to/image.jpg"));
        assert!(json.contains("\"random\":false"));
        assert!(json.contains("\"screen\":\"all\""));
    }

    #[test]
    fn test_wallpaper_set_data_serialization_with_random() {
        let data = WallpaperSetData {
            path: None,
            random: true,
            screen: ScreenTarget::Main,
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("path"));
        assert!(json.contains("\"random\":true"));
        assert!(json.contains("\"screen\":\"main\""));
    }

    #[test]
    fn test_wallpaper_set_data_serialization_with_screen_index() {
        let data = WallpaperSetData {
            path: Some("/path/to/image.jpg".to_string()),
            random: false,
            screen: ScreenTarget::Index(2),
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"screen\":{\"index\":2}"));
    }
}
