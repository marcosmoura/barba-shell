//! Weather configuration component.
//!
//! Exposes the weather configuration from the config file to the frontend.

use serde::Serialize;

use crate::config::{WeatherConfig, get_config};

/// Weather configuration payload for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherConfigInfo {
    /// API key for Visual Crossing Weather API.
    pub visual_crossing_api_key: String,
    /// Default location for weather data when geolocation fails.
    pub default_location: String,
}

impl From<&WeatherConfig> for WeatherConfigInfo {
    fn from(config: &WeatherConfig) -> Self {
        Self {
            visual_crossing_api_key: config.visual_crossing_api_key.clone(),
            default_location: config.default_location.clone(),
        }
    }
}

/// Get the weather configuration from the config file.
#[tauri::command]
pub fn get_weather_config() -> WeatherConfigInfo {
    WeatherConfigInfo::from(&get_config().bar.weather)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_config_info_from_weather_config() {
        let config = WeatherConfig::default();
        let info = WeatherConfigInfo::from(&config);

        assert!(info.visual_crossing_api_key.is_empty());
        assert!(info.default_location.is_empty());
    }
}
