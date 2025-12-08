//! Wallpaper command handlers.
//!
//! This module handles all wallpaper-related IPC commands including
//! setting wallpapers from files, random selection, and generation.

use std::io::Write;
use std::os::unix::net::UnixStream;

use serde::Deserialize;

use crate::ipc::types::ScreenTarget;
use crate::wallpaper::{self, WallpaperAction, WallpaperManagerError};

/// Data for wallpaper set commands.
#[derive(Deserialize)]
pub struct WallpaperSetData {
    /// Path to the wallpaper file.
    pub path: Option<String>,
    /// Whether to select a random wallpaper.
    pub random: bool,
    /// Which screen(s) to apply the wallpaper to.
    pub screen: ScreenTarget,
}

/// Handles the wallpaper-set command.
///
/// Sets a wallpaper either from a specific file path or randomly,
/// targeting all screens, the main screen, or a specific screen index.
pub fn handle_set(data: &str) {
    let set_data: WallpaperSetData = match serde_json::from_str(data) {
        Ok(d) => d,
        Err(err) => {
            eprintln!("barba: failed to parse wallpaper data: {err}");
            return;
        }
    };

    let result = if set_data.random {
        handle_random(&set_data.screen)
    } else if let Some(path) = set_data.path {
        handle_file(path, &set_data.screen)
    } else {
        Err(WallpaperManagerError::InvalidAction(
            "Either path or random must be specified".to_string(),
        ))
    };

    if let Err(err) = result {
        eprintln!("barba: wallpaper error: {err}");
    }
}

/// Handles setting a random wallpaper for the specified screen target.
fn handle_random(screen: &ScreenTarget) -> Result<(), WallpaperManagerError> {
    match screen {
        ScreenTarget::All => {
            let action = WallpaperAction::Random;
            wallpaper::perform_action(&action)
        }
        ScreenTarget::Main => {
            let action = WallpaperAction::RandomForScreen(0);
            wallpaper::perform_action(&action)
        }
        ScreenTarget::Index(idx) => {
            if *idx == 0 {
                Err(WallpaperManagerError::InvalidScreen(
                    "Screen index must be 1 or greater".to_string(),
                ))
            } else {
                let action = WallpaperAction::RandomForScreen(idx - 1);
                wallpaper::perform_action(&action)
            }
        }
    }
}

/// Handles setting a specific wallpaper file for the specified screen target.
fn handle_file(path: String, screen: &ScreenTarget) -> Result<(), WallpaperManagerError> {
    match screen {
        ScreenTarget::All => {
            let action = WallpaperAction::File(path);
            wallpaper::perform_action(&action)
        }
        ScreenTarget::Main => {
            let action = WallpaperAction::FileForScreen(0, path);
            wallpaper::perform_action(&action)
        }
        ScreenTarget::Index(idx) => {
            if *idx == 0 {
                Err(WallpaperManagerError::InvalidScreen(
                    "Screen index must be 1 or greater".to_string(),
                ))
            } else {
                let action = WallpaperAction::FileForScreen(idx - 1, path);
                wallpaper::perform_action(&action)
            }
        }
    }
}

/// Handles the wallpaper-generate-all command.
///
/// Generates processed versions of all wallpapers, streaming progress
/// back to the client.
pub fn handle_generate_all(stream: &mut UnixStream) -> bool {
    // Write the 'O' prefix to indicate streaming output
    if stream.write_all(b"O").is_err() {
        return false;
    }
    stream.flush().ok();

    if let Err(err) = wallpaper::generate_all_streaming(stream) {
        eprintln!("barba: wallpaper generation error: {err}");
    }

    true
}

/// Handles the wallpaper-list command.
///
/// Returns a JSON array of available wallpaper paths.
pub fn handle_list(stream: &mut UnixStream) {
    let result = match wallpaper::list_wallpapers() {
        Ok(paths) => serde_json::to_string(&paths).unwrap_or_else(|_| "[]".to_string()),
        Err(err) => {
            eprintln!("barba: failed to list wallpapers: {err}");
            "[]".to_string()
        }
    };

    // Write response with 'R' prefix for response data
    let _ = stream.write_all(format!("R{result}").as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallpaper_set_data_deserialization() {
        let json = r#"{"path":"/path/to/image.jpg","random":false,"screen":"all"}"#;
        let data: WallpaperSetData = serde_json::from_str(json).unwrap();
        assert_eq!(data.path, Some("/path/to/image.jpg".to_string()));
        assert!(!data.random);
        assert!(matches!(data.screen, ScreenTarget::All));
    }

    #[test]
    fn test_wallpaper_set_data_random_deserialization() {
        let json = r#"{"random":true,"screen":"main"}"#;
        let data: WallpaperSetData = serde_json::from_str(json).unwrap();
        assert!(data.path.is_none());
        assert!(data.random);
        assert!(matches!(data.screen, ScreenTarget::Main));
    }
}
