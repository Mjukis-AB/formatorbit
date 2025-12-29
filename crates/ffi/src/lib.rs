//! UniFFI bindings for Formatorbit.
//!
//! This crate exposes Formatorbit functionality via UniFFI for integration
//! with Swift, Kotlin, Python, and other languages.

mod types;

pub use types::*;

use formatorbit_core::Formatorbit;
use std::sync::OnceLock;

uniffi::setup_scaffolding!();

static INSTANCE: OnceLock<Formatorbit> = OnceLock::new();

fn get_instance() -> &'static Formatorbit {
    INSTANCE.get_or_init(Formatorbit::new)
}

// =============================================================================
// Exported Functions
// =============================================================================

/// Get the library version string.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get information about all supported formats as JSON.
///
/// Returns a JSON array of format info objects. Using JSON here since
/// FormatInfo contains static str references which UniFFI cannot handle.
#[uniffi::export]
pub fn list_formats() -> String {
    let infos = get_instance().format_infos();
    serde_json::to_string(&infos).unwrap_or_else(|_| "[]".to_string())
}

/// Convert input and return all possible interpretations and conversions.
///
/// Returns all possible interpretations sorted by confidence (highest first).
/// Each interpretation includes all possible conversions sorted by priority.
#[uniffi::export]
pub fn convert_all(input: String) -> Vec<FfiConversionResult> {
    let results = get_instance().convert_all(&input);
    results.into_iter().map(Into::into).collect()
}

/// Convert input using only specific formats.
///
/// `formats` is a list of format IDs or aliases (e.g., ["hex", "uuid", "ts"]).
#[uniffi::export]
pub fn convert_filtered(input: String, formats: Vec<String>) -> Vec<FfiConversionResult> {
    let format_refs: Vec<String> = formats;
    let results = get_instance().convert_all_filtered(&input, &format_refs);
    results.into_iter().map(Into::into).collect()
}

/// Convert input and return only the highest-confidence result.
///
/// Returns None if no interpretation found with confidence > 0.2.
#[uniffi::export]
pub fn convert_first(input: String) -> Option<FfiConversionResult> {
    let results = get_instance().convert_all(&input);
    results
        .into_iter()
        .find(|r| r.interpretation.confidence > 0.2)
        .map(Into::into)
}

/// Convert input, forcing interpretation as a specific format.
///
/// Skips auto-detection and treats the input as the specified format.
#[uniffi::export]
pub fn convert_from(input: String, from_format: String) -> Vec<FfiConversionResult> {
    let results = if from_format.is_empty() {
        get_instance().convert_all(&input)
    } else {
        get_instance().convert_all_filtered(&input, &[from_format])
    };
    results.into_iter().map(Into::into).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let v = version();
        assert!(v.starts_with("0."));
    }

    #[test]
    fn test_list_formats() {
        let formats = list_formats();
        assert!(formats.contains("hex"));
        assert!(formats.contains("uuid"));
    }

    #[test]
    fn test_convert_all() {
        let results = convert_all("691E01B8".to_string());
        assert!(!results.is_empty());
        // Should find hex interpretation
        let has_hex = results
            .iter()
            .any(|r| r.interpretation.source_format == "hex");
        assert!(has_hex);
    }

    #[test]
    fn test_convert_filtered() {
        let results = convert_filtered("691E01B8".to_string(), vec!["hex".to_string()]);
        assert!(!results.is_empty());
        // All should be hex
        for r in &results {
            assert_eq!(r.interpretation.source_format, "hex");
        }
    }

    #[test]
    fn test_convert_first() {
        let result = convert_first("691E01B8".to_string());
        assert!(result.is_some());
    }
}
