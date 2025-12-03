//! CLI error types.

/// A help message to display to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpMessage(pub String);

impl std::fmt::Display for HelpMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}

/// Errors that can occur during CLI parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum CliParseError {
    /// User requested help text (global or subcommand).
    Help(HelpMessage),
    /// Help should be displayed because no args were provided in release builds.
    MissingArguments,
    /// `workspace-changed` missing required workspace name.
    MissingWorkspaceName,
    /// `set-wallpaper` missing required action argument.
    MissingWallpaperAction,
    /// `set-wallpaper` has an invalid action argument.
    InvalidWallpaperAction(String),
    /// The CLI invocation did not match a known subcommand.
    UnknownCommand,
    /// Internal clap error surfaced through plugin config.
    InvalidInvocation(String),
}

impl std::fmt::Display for CliParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help(message) => f.write_str(&message.0),
            Self::MissingArguments => write!(
                f,
                "No CLI arguments provided. Run `barba --help` to discover available commands."
            ),
            Self::MissingWorkspaceName => {
                write!(
                    f,
                    "Missing workspace name. Usage: `barba workspace-changed <name>`"
                )
            }
            Self::MissingWallpaperAction => {
                write!(
                    f,
                    "Missing wallpaper action. Usage: `barba set-wallpaper <next|previous|random|index>`"
                )
            }
            Self::InvalidWallpaperAction(action) => {
                write!(
                    f,
                    "Invalid wallpaper action: '{action}'. Expected: next, previous, random, or an index number"
                )
            }
            Self::UnknownCommand => write!(
                f,
                "Unknown command. Run `barba --help` to list all supported commands."
            ),
            Self::InvalidInvocation(message) => {
                write!(f, "CLI invocation could not be parsed: {message}")
            }
        }
    }
}

impl std::error::Error for CliParseError {}
