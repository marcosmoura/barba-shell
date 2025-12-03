//! CLI argument normalization utilities.

use super::types::SYNTHETIC_BIN_NAME;

/// Normalizes CLI arguments to ensure they start with the binary name.
///
/// Returns `None` if the arguments are empty or only contain the binary name.
pub fn normalize_cli_args(args: &[String]) -> Option<Vec<String>> {
    if args.is_empty() {
        return None;
    }

    if looks_like_binary(&args[0]) {
        if args.len() == 1 {
            return None;
        }

        return Some(args.to_vec());
    }

    let mut normalized = Vec::with_capacity(args.len() + 1);
    normalized.push(SYNTHETIC_BIN_NAME.to_string());
    normalized.extend_from_slice(args);
    Some(normalized)
}

/// Checks if the given path looks like the barba binary.
fn looks_like_binary(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    let normalized = path.trim_matches('"');
    let lowered = normalized.to_ascii_lowercase();
    let mentions_binary = lowered.contains(SYNTHETIC_BIN_NAME);
    let has_separator = normalized.contains('/') || normalized.contains('\\');

    mentions_binary && (normalized == SYNTHETIC_BIN_NAME || has_separator)
}

/// Checks if the arguments represent a version request.
pub fn is_version_request(normalized_args: &[String]) -> bool {
    normalized_args.len() == 2 && matches!(normalized_args[1].as_str(), "--version" | "-V")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_binary_prefix() {
        assert!(normalize_cli_args(&[]).is_none());
        assert!(normalize_cli_args(&["barba".to_string()]).is_none());

        let args = vec!["focus-changed".to_string()];
        let normalized = normalize_cli_args(&args).unwrap();

        assert_eq!(normalized[0], SYNTHETIC_BIN_NAME);
        assert_eq!(&normalized[1..], args.as_slice());
    }

    #[test]
    fn detects_version_request_flags() {
        let args = vec!["barba".to_string(), "--version".to_string()];
        assert!(is_version_request(&args));

        let short = vec!["barba".to_string(), "-V".to_string()];
        assert!(is_version_request(&short));
    }

    #[test]
    fn rejects_version_request_with_extra_args() {
        let args = vec![
            "barba".to_string(),
            "--version".to_string(),
            "extra".to_string(),
        ];
        assert!(!is_version_request(&args));
    }

    #[test]
    fn looks_like_binary_detects_paths() {
        assert!(looks_like_binary("barba"));
        assert!(looks_like_binary("/usr/bin/barba"));
        assert!(looks_like_binary("C:\\Program Files\\barba.exe"));
        assert!(looks_like_binary("\"barba\""));

        assert!(!looks_like_binary(""));
        assert!(!looks_like_binary("other"));
        assert!(!looks_like_binary("/usr/bin/other"));
    }
}
