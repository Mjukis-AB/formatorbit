//! UniFFI bindings for Formatorbit.
//!
//! This crate exposes Formatorbit functionality via UniFFI for integration
//! with Swift, Kotlin, Python, and other languages.

mod types;

pub use types::*;

use formatorbit_core::Formatorbit;
use std::sync::OnceLock;

uniffi::setup_scaffolding!();

// =============================================================================
// Error Types
// =============================================================================

/// Error type for FFI operations that can fail.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    #[error("{message}")]
    FileError { message: String },
}

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

/// Convert raw bytes and return all possible interpretations.
///
/// This is useful for binary data like images, archives, etc.
/// The bytes are base64-encoded internally for format detection.
#[uniffi::export]
pub fn convert_bytes(data: Vec<u8>) -> Vec<FfiConversionResult> {
    let results = get_instance().convert_bytes(&data);
    results.into_iter().map(Into::into).collect()
}

/// Convert raw bytes, forcing interpretation as a specific format.
#[uniffi::export]
pub fn convert_bytes_from(data: Vec<u8>, from_format: String) -> Vec<FfiConversionResult> {
    let results = if from_format.is_empty() {
        get_instance().convert_bytes(&data)
    } else {
        get_instance().convert_bytes_filtered(&data, &[from_format])
    };
    results.into_iter().map(Into::into).collect()
}

/// Read a file and return all possible interpretations.
///
/// For text files, the content is parsed directly.
/// For binary files, the bytes are base64-encoded for format detection.
#[uniffi::export]
pub fn convert_file(path: String) -> Result<Vec<FfiConversionResult>, FfiError> {
    use base64::Engine;
    use std::fs;
    use std::path::Path;

    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err(FfiError::FileError {
            message: format!("File not found: {}", path),
        });
    }

    let data = fs::read(file_path).map_err(|e| FfiError::FileError {
        message: format!("Failed to read file: {}", e),
    })?;

    // Try to interpret as UTF-8 text first
    let input = if let Ok(text) = String::from_utf8(data.clone()) {
        // If it's valid UTF-8 and doesn't look like binary, treat as text
        if !text.contains('\0') {
            text
        } else {
            base64::engine::general_purpose::STANDARD.encode(&data)
        }
    } else {
        // Binary data - encode as base64
        base64::engine::general_purpose::STANDARD.encode(&data)
    };

    let results = get_instance().convert_all(&input);
    Ok(results.into_iter().map(Into::into).collect())
}

/// Read a file, forcing interpretation as a specific format.
#[uniffi::export]
pub fn convert_file_from(
    path: String,
    from_format: String,
) -> Result<Vec<FfiConversionResult>, FfiError> {
    use base64::Engine;
    use std::fs;
    use std::path::Path;

    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err(FfiError::FileError {
            message: format!("File not found: {}", path),
        });
    }

    let data = fs::read(file_path).map_err(|e| FfiError::FileError {
        message: format!("Failed to read file: {}", e),
    })?;

    // Try to interpret as UTF-8 text first
    let input = if let Ok(text) = String::from_utf8(data.clone()) {
        if !text.contains('\0') {
            text
        } else {
            base64::engine::general_purpose::STANDARD.encode(&data)
        }
    } else {
        base64::engine::general_purpose::STANDARD.encode(&data)
    };

    let results = if from_format.is_empty() {
        get_instance().convert_all(&input)
    } else {
        get_instance().convert_all_filtered(&input, &[from_format])
    };
    Ok(results.into_iter().map(Into::into).collect())
}

// =============================================================================
// Currency Functions
// =============================================================================

/// Result of getting the current target currency.
#[derive(uniffi::Record)]
pub struct TargetCurrency {
    /// The ISO 4217 currency code (e.g., "USD", "EUR", "SEK").
    pub code: String,
    /// How the currency was determined: "config", "environment (FORB_TARGET_CURRENCY)",
    /// "locale (...)", or "default".
    pub source: String,
}

/// Get the currency code for a given ISO 3166-1 alpha-2 country code.
///
/// For macOS apps, you can get the country code from:
/// `Locale.current.region?.identifier` (Swift)
///
/// Returns nil if the country code is not recognized.
#[uniffi::export]
pub fn currency_for_country(country_code: String) -> Option<String> {
    formatorbit_core::formats::currency_expr::currency_for_country(&country_code)
        .map(|s| s.to_string())
}

/// Get the currency code from a locale string (e.g., "en_US", "sv_SE.UTF-8").
///
/// Parses the country code from the locale and returns the corresponding currency.
/// Returns nil if the locale doesn't contain a recognizable country code.
#[uniffi::export]
pub fn currency_for_locale(locale: String) -> Option<String> {
    formatorbit_core::formats::currency_expr::currency_for_locale(&locale).map(|s| s.to_string())
}

/// Set the target currency for currency conversions.
///
/// Pass nil to clear and use locale/default detection.
#[uniffi::export]
pub fn set_target_currency(code: Option<String>) {
    formatorbit_core::formats::currency_expr::set_target_currency(code);
}

/// Get the current target currency and its source.
///
/// Source is one of: "config", "environment (FORB_TARGET_CURRENCY)", "locale (...)", "default".
#[uniffi::export]
pub fn get_target_currency() -> TargetCurrency {
    let (code, source) =
        formatorbit_core::formats::currency_expr::get_target_currency_with_source();
    TargetCurrency { code, source }
}

/// Get all available currency codes.
///
/// Returns both built-in ECB currencies and any plugin-provided currencies.
#[uniffi::export]
pub fn all_currency_codes() -> Vec<String> {
    formatorbit_core::formats::currency_expr::all_currency_codes()
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

    #[test]
    fn test_convert_bytes() {
        // PNG magic bytes
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let results = convert_bytes(png_header);
        assert!(!results.is_empty());
        // Should detect as image (since it has PNG magic bytes)
        // Note: short header may not be enough for full image detection,
        // so it may fall back to generic "bytes" interpretation
        let source_format = &results[0].interpretation.source_format;
        assert!(
            source_format == "image" || source_format == "bytes",
            "Expected 'image' or 'bytes', got '{}'",
            source_format
        );
    }

    #[test]
    fn test_convert_bytes_from() {
        // When forcing a specific format, it should use that format
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let results = convert_bytes_from(data, "image".to_string());
        // With no matching specialized format, should get "bytes" fallback
        assert!(!results.is_empty());
    }

    #[test]
    fn test_convert_file_not_found() {
        let result = convert_file("/nonexistent/path/to/file.txt".to_string());
        assert!(result.is_err());
        match result.unwrap_err() {
            FfiError::FileError { message } => {
                assert!(message.contains("File not found"));
            }
        }
    }
}
