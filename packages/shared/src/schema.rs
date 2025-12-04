use crate::BarbaConfig;

/// Generates a JSON Schema for the Barba configuration.
///
/// The schema includes all configuration options with their types,
/// descriptions, and default values.
#[must_use]
pub fn generate_schema() -> schemars::Schema {
    let mut schema = schemars::schema_for!(BarbaConfig);

    // Add $id for proper schema identification
    if let Some(obj) = schema.as_object_mut() {
        obj.insert(
            "$id".to_string(),
            serde_json::json!(
                "https://raw.githubusercontent.com/marcosmoura/barba-shell/main/barba.schema.json"
            ),
        );
    }

    schema
}

/// Generates a JSON Schema string for the Barba configuration.
///
/// Returns a pretty-printed JSON string that can be saved to a file
/// or used for validation.
#[must_use]
pub fn generate_schema_json() -> String {
    let schema = generate_schema();
    serde_json::to_string_pretty(&schema).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_schema_produces_valid_json() {
        let schema_json = generate_schema_json();
        assert!(!schema_json.is_empty());

        let parsed: serde_json::Value = serde_json::from_str(&schema_json).unwrap();

        assert!(parsed["$id"].as_str().unwrap().contains("barba.schema.json"));
        assert_eq!(parsed["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(parsed["title"], "BarbaConfig");
        assert!(parsed["properties"]["shortcuts"].is_object());
        assert!(parsed["properties"]["wallpapers"].is_object());
    }

    #[test]
    fn test_generate_schema_returns_schema_object() {
        let schema = generate_schema();
        // Schema should be a valid schema object
        assert!(schema.as_object().is_some());
    }

    #[test]
    fn test_schema_has_id_field() {
        let schema = generate_schema();
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("$id"));
    }

    #[test]
    fn test_schema_id_is_correct_url() {
        let schema_json = generate_schema_json();
        let parsed: serde_json::Value = serde_json::from_str(&schema_json).unwrap();
        let id = parsed["$id"].as_str().unwrap();

        assert!(id.starts_with("https://"));
        assert!(id.contains("githubusercontent.com"));
        assert!(id.ends_with("barba.schema.json"));
    }

    #[test]
    fn test_schema_contains_wallpapers_config() {
        let schema_json = generate_schema_json();
        let parsed: serde_json::Value = serde_json::from_str(&schema_json).unwrap();

        let wallpapers = &parsed["properties"]["wallpapers"];
        assert!(wallpapers.is_object());
    }

    #[test]
    fn test_schema_contains_weather_config() {
        let schema_json = generate_schema_json();
        let parsed: serde_json::Value = serde_json::from_str(&schema_json).unwrap();

        let weather = &parsed["properties"]["weather"];
        assert!(weather.is_object());
    }

    #[test]
    fn test_schema_json_is_pretty_printed() {
        let schema_json = generate_schema_json();
        // Pretty-printed JSON should have newlines
        assert!(schema_json.contains('\n'));
    }
}
