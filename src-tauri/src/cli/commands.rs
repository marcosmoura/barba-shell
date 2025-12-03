//! CLI command parsing and matching.

use std::collections::HashMap;

use serde_json::Value;
use tauri_plugin_cli::{ArgData, Matches, SubcommandMatches};

use super::error::{CliParseError, HelpMessage};
use super::should_render_help_on_empty_invocation;
use super::types::CliEventPayload;
use crate::wallpaper;

/// Represents a matched CLI command.
#[derive(Debug)]
pub enum CommandMatch {
    /// No command matched.
    None,
    /// Focus changed notification.
    FocusChanged,
    /// Workspace changed notification with workspace name.
    WorkspaceChanged(String),
    /// Set wallpaper action.
    SetWallpaper(wallpaper::WallpaperAction),
    /// Generate all wallpapers.
    GenerateAllWallpapers,
    /// Reload configuration.
    Reload,
    /// Generate JSON schema for configuration.
    GenerateSchema,
}

/// Builds a CLI event from parsed matches.
pub fn build_cli_event(matches: &Matches) -> Result<Option<CliEventPayload>, CliParseError> {
    build_cli_event_with(matches, should_render_help_on_empty_invocation())
}

/// Builds a CLI event from parsed matches with configurable command requirement.
pub fn build_cli_event_with(
    matches: &Matches,
    require_command: bool,
) -> Result<Option<CliEventPayload>, CliParseError> {
    // Check for help flag
    if let Some(help) = matches.args.get("help")
        && let Value::String(help_text) = &help.value
    {
        return Err(CliParseError::Help(HelpMessage(help_text.clone())));
    }

    // Check if command is required but missing
    if require_command && matches.subcommand.is_none() {
        return Err(CliParseError::MissingArguments);
    }

    // Match and execute the command
    match extract_subcommand(matches.subcommand.as_deref())? {
        CommandMatch::None => Ok(None),
        CommandMatch::FocusChanged => Ok(Some(CliEventPayload {
            name: "focus-changed".to_string(),
            data: None,
        })),
        CommandMatch::WorkspaceChanged(workspace) => Ok(Some(CliEventPayload {
            name: "workspace-changed".to_string(),
            data: Some(workspace),
        })),
        CommandMatch::SetWallpaper(action) => {
            // Execute wallpaper action immediately (no event needed)
            if let Err(err) = wallpaper::perform_action(action) {
                eprintln!("barba: wallpaper error: {err}");
            }
            Ok(None)
        }
        CommandMatch::GenerateAllWallpapers => {
            // Generate all wallpapers immediately (no event needed)
            if let Err(err) = wallpaper::generate_all() {
                eprintln!("barba: wallpaper generation error: {err}");
            }
            Ok(None)
        }
        CommandMatch::Reload => Ok(Some(CliEventPayload {
            name: "reload".to_string(),
            data: None,
        })),
        CommandMatch::GenerateSchema => {
            // Generate and print JSON schema to stdout
            let schema = crate::config::generate_schema_json();
            println!("{schema}");
            Ok(None)
        }
    }
}

/// Extracts a command match from a subcommand.
fn extract_subcommand(
    subcommand: Option<&SubcommandMatches>,
) -> Result<CommandMatch, CliParseError> {
    let Some(matches) = subcommand else {
        return Ok(CommandMatch::None);
    };

    parse_command(
        matches.name.as_str(),
        &matches.matches.args,
        matches.matches.subcommand.as_deref(),
    )
}

/// Parses a command name and its arguments into a `CommandMatch`.
fn parse_command(
    name: &str,
    args: &HashMap<String, ArgData>,
    nested_subcommand: Option<&SubcommandMatches>,
) -> Result<CommandMatch, CliParseError> {
    match name {
        "focus-changed" => Ok(CommandMatch::FocusChanged),

        "workspace-changed" => {
            let workspace =
                extract_string_arg(args, "name").ok_or(CliParseError::MissingWorkspaceName)?;
            Ok(CommandMatch::WorkspaceChanged(workspace))
        }

        "wallpaper" => parse_wallpaper_subcommand(nested_subcommand),

        "reload" => Ok(CommandMatch::Reload),

        "generate-schema" => Ok(CommandMatch::GenerateSchema),

        _ => Err(CliParseError::UnknownCommand),
    }
}

/// Parses wallpaper subcommands (e.g., `wallpaper set next`).
fn parse_wallpaper_subcommand(
    subcommand: Option<&SubcommandMatches>,
) -> Result<CommandMatch, CliParseError> {
    let Some(matches) = subcommand else {
        return Err(CliParseError::MissingArguments);
    };

    match matches.name.as_str() {
        "set" => {
            let action_str = extract_string_arg(&matches.matches.args, "action")
                .ok_or(CliParseError::MissingWallpaperAction)?;
            let action = wallpaper::parse_action(&action_str)
                .ok_or(CliParseError::InvalidWallpaperAction(action_str))?;
            Ok(CommandMatch::SetWallpaper(action))
        }
        "generate-all" => Ok(CommandMatch::GenerateAllWallpapers),
        _ => Err(CliParseError::UnknownCommand),
    }
}

/// Extracts a non-empty string argument by name.
fn extract_string_arg(args: &HashMap<String, ArgData>, name: &str) -> Option<String> {
    args.get(name).and_then(|arg| match &arg.value {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tauri_plugin_cli::{ArgData, Matches};

    use super::*;

    fn arg_map(pairs: &[(&str, Value)]) -> HashMap<String, ArgData> {
        let mut map = HashMap::new();
        for (key, value) in pairs {
            let mut arg = ArgData::default();
            arg.value = value.clone();
            arg.occurrences = 1;
            map.insert((*key).to_string(), arg);
        }
        map
    }

    fn matches_with_help(text: &str) -> Matches {
        let mut matches = Matches::default();
        let mut data = ArgData::default();
        data.value = Value::String(text.to_string());
        matches.args.insert("help".to_string(), data);
        matches
    }

    #[test]
    fn parse_command_focus_changed() {
        let args = HashMap::new();
        let command = parse_command("focus-changed", &args, None).unwrap();

        assert!(matches!(command, CommandMatch::FocusChanged));
    }

    #[test]
    fn parse_command_workspace_extracts_name() {
        let args = arg_map(&[("name", Value::String("coding".to_string()))]);
        let command = parse_command("workspace-changed", &args, None).unwrap();

        match command {
            CommandMatch::WorkspaceChanged(name) => assert_eq!(name, "coding"),
            _ => panic!("expected workspace command"),
        }
    }

    #[test]
    fn parse_command_requires_workspace_name() {
        let err = parse_command("workspace-changed", &HashMap::new(), None).unwrap_err();

        assert_eq!(err, CliParseError::MissingWorkspaceName);
    }

    #[test]
    fn build_cli_event_returns_help_error() {
        let matches = matches_with_help("Usage");
        let err = build_cli_event_with(&matches, false).unwrap_err();

        assert!(matches!(err, CliParseError::Help(_)));
    }

    #[test]
    fn build_cli_event_requires_command_when_enforced() {
        let matches = Matches::default();
        let err = build_cli_event_with(&matches, true).unwrap_err();

        assert_eq!(err, CliParseError::MissingArguments);
    }

    #[test]
    fn parse_wallpaper_requires_subcommand() {
        let err = parse_wallpaper_subcommand(None).unwrap_err();

        assert_eq!(err, CliParseError::MissingArguments);
    }

    #[test]
    fn parse_command_unknown_returns_error() {
        let args = HashMap::new();
        let err = parse_command("unknown-command", &args, None).unwrap_err();

        assert_eq!(err, CliParseError::UnknownCommand);
    }
}
