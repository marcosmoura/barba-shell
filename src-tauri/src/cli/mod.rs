//! CLI module for Barba Shell.
//!
//! Handles command-line argument parsing and dispatches CLI events
//! to the running Tauri application.

mod args;
mod commands;
mod error;
mod types;

use args::{is_version_request, normalize_cli_args};
use commands::build_cli_event;
pub use error::CliParseError;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tauri_plugin_cli::CliExt;
pub use types::CliEventPayload;
use types::{APP_VERSION, CLI_EVENT_CHANNEL, SYNTHETIC_BIN_NAME};

/// Handles a CLI invocation from the given arguments.
///
/// This is the main entry point for CLI processing. It handles:
/// - Version flag requests
/// - Help text display
/// - Command parsing and event emission
/// - Error reporting
pub fn handle_cli_invocation(app_handle: &AppHandle, args: &[String]) {
    if consume_version_flag(args) {
        return;
    }

    // Check if a subcommand was provided (used to determine if help should be shown on Ok(None))
    #[cfg(not(debug_assertions))]
    let has_subcommand = args.len() > 1 && !args[1].starts_with('-');

    match parse_cli_event(app_handle, args) {
        Ok(Some(event)) => {
            app_handle.emit(CLI_EVENT_CHANNEL, event).unwrap_or_else(|err| {
                eprintln!("barba: failed to emit CLI event: {err}");
            });
        }

        Ok(None) => {
            // Only show help if no subcommand was provided
            // Commands like generate-schema and wallpaper generate-all return None
            // but have already executed their action
            #[cfg(not(debug_assertions))]
            if !has_subcommand && !print_default_help(app_handle) {
                eprintln!("barba: {}", CliParseError::MissingArguments);
            }
        }

        Err(CliParseError::Help(help)) => {
            println!("{help}");
        }

        Err(CliParseError::MissingArguments) => {
            if !print_default_help(app_handle) {
                eprintln!("barba: {}", CliParseError::MissingArguments);
            }
        }

        Err(err) => {
            eprintln!("barba: {err}");
        }
    }
}

/// Parses CLI arguments and returns an event payload if applicable.
pub fn parse_cli_event(
    app_handle: &AppHandle,
    args: &[String],
) -> Result<Option<CliEventPayload>, CliParseError> {
    let Some(normalized_args) = normalize_cli_args(args) else {
        return Ok(None);
    };

    let matches = app_handle
        .cli()
        .matches_from(normalized_args)
        .map_err(|err| CliParseError::InvalidInvocation(err.to_string()))?;

    build_cli_event(&matches)
}

/// Previews a CLI event without requiring a Tauri app handle.
///
/// Useful for determining launch mode before the app is fully initialized.
#[allow(dead_code)]
pub fn preview_cli_event(args: &[String]) -> Result<Option<CliEventPayload>, CliParseError> {
    let Some(normalized_args) = normalize_cli_args(args) else {
        return Ok(None);
    };

    preview_cli_event_with(&normalized_args, should_render_help_on_empty_invocation())
}

/// Returns whether help should be shown when no command is provided.
pub const fn should_render_help_on_empty_invocation() -> bool { cfg!(not(debug_assertions)) }

fn consume_version_flag(args: &[String]) -> bool {
    let Some(normalized_args) = normalize_cli_args(args) else {
        return false;
    };

    if is_version_request(&normalized_args) {
        println!("{SYNTHETIC_BIN_NAME} {APP_VERSION}");
        return true;
    }

    false
}

fn preview_cli_event_with(
    normalized_args: &[String],
    require_command: bool,
) -> Result<Option<CliEventPayload>, CliParseError> {
    if normalized_args.len() <= 1 {
        return if require_command {
            Err(CliParseError::MissingArguments)
        } else {
            Ok(None)
        };
    }

    let event_candidate = normalized_args[1].trim().to_string();
    if event_candidate.is_empty() {
        return Err(CliParseError::UnknownCommand);
    }

    let data = if normalized_args.len() > 2 {
        Some(normalized_args[2..].join(" "))
    } else {
        None
    };

    Ok(Some(CliEventPayload { name: event_candidate, data }))
}

fn print_default_help(app_handle: &AppHandle) -> bool {
    resolve_help_text(app_handle, None).is_some_and(|help| {
        println!("{help}");
        true
    })
}

fn resolve_help_text(app_handle: &AppHandle, subcommand: Option<&str>) -> Option<String> {
    let mut args = vec![SYNTHETIC_BIN_NAME.to_string()];
    if let Some(name) = subcommand {
        args.push(name.to_string());
    }
    args.push("--help".to_string());

    let matches = app_handle.cli().matches_from(args).ok()?;
    let help = matches.args.get("help")?;

    if let Value::String(help_text) = &help.value {
        Some(help_text.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_cli_event_detects_command() {
        let args = vec!["barba".to_string(), "focus-changed".to_string()];
        let preview = preview_cli_event_with(&args, false).unwrap().unwrap();

        assert_eq!(preview.name, "focus-changed");
    }

    #[test]
    fn preview_cli_event_requires_command_when_enforced() {
        let args = vec!["barba".to_string()];
        let err = preview_cli_event_with(&args, true).unwrap_err();

        assert_eq!(err, CliParseError::MissingArguments);
    }
}
