//! Application constants for Stache.
//!
//! This module contains global constants used throughout the application,
//! including bundle identifiers, application names, and other static values.

/// The application bundle identifier.
pub const BUNDLE_ID: &str = "co.anomaly.stache";

/// The application name.
pub const APP_NAME: &str = "Stache";

/// Application version from Cargo.toml.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bundle identifiers for Apple Music and iTunes (blocked by noTunes).
pub mod apple_music {
    /// Apple Music bundle identifier.
    pub const MUSIC_BUNDLE_ID: &str = "com.apple.Music";

    /// iTunes bundle identifier (legacy).
    pub const ITUNES_BUNDLE_ID: &str = "com.apple.iTunes";
}

/// Cache directory names.
pub mod cache {
    /// Subdirectory for cached wallpapers.
    pub const WALLPAPERS_DIR: &str = "wallpapers";

    /// Subdirectory for cached media artwork.
    pub const MEDIA_DIR: &str = "media";
}

/// Default configuration file names.
pub mod config {
    /// Primary config file name.
    pub const CONFIG_FILE: &str = "config.jsonc";

    /// Alternative config file name (JSON without comments).
    pub const CONFIG_FILE_ALT: &str = "config.json";

    /// Legacy config file name in home directory.
    pub const CONFIG_FILE_LEGACY: &str = ".stache.json";
}
