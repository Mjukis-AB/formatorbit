//! Base64 format.

use base64::{engine::general_purpose::STANDARD, Engine};
use tracing::{debug, trace};

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct Base64Format;

impl Base64Format {
    /// Check if a string looks like valid base64.
    fn is_valid_base64_chars(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    }

    /// Check if the string looks like a word or identifier rather than base64 data.
    /// These are almost never base64-encoded data.
    fn looks_like_word_or_identifier(s: &str) -> bool {
        // Must be letters only (no digits, +, /, =) to be a word/identifier
        let letters_only = s.chars().all(|c| c.is_ascii_alphabetic());
        if !letters_only {
            return false;
        }

        // All-lowercase letters-only strings are likely words or tool names
        // e.g., "xcodegen", "rustfmt", "prettier"
        // Real base64 data almost always has mixed case, digits, or special chars
        let all_lowercase = s.chars().all(|c| c.is_ascii_lowercase());
        if all_lowercase && s.len() >= 4 && s.len() <= 20 {
            return true;
        }

        // camelCase: lowercase followed by uppercase somewhere
        // e.g., "aspectButton", "getElementById"
        let has_camel_case = s
            .chars()
            .zip(s.chars().skip(1))
            .any(|(a, b)| a.is_ascii_lowercase() && b.is_ascii_uppercase());

        // Starts with common identifier patterns
        let common_prefixes = ["get", "set", "is", "has", "on", "do", "my", "the", "new"];
        let starts_with_prefix = common_prefixes
            .iter()
            .any(|p| s.to_lowercase().starts_with(p) && s.len() > p.len() + 2);

        let reasonable_length = s.len() >= 4 && s.len() <= 30;

        // If it's camelCase or has common prefixes, likely an identifier
        has_camel_case || (starts_with_prefix && reasonable_length)
    }
}

impl Format for Base64Format {
    fn id(&self) -> &'static str {
        "base64"
    }

    fn name(&self) -> &'static str {
        "Base64"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Encoding",
            description: "Base64 encoded binary data",
            examples: &["SGVsbG8gV29ybGQ=", "aR4BuA=="],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        trace!(input_len = input.len(), "base64: checking input");

        // Quick validation of characters
        if !Self::is_valid_base64_chars(input) {
            trace!("base64: rejected - invalid characters");
            return vec![];
        }

        // Skip things that look like code identifiers (camelCase, etc.)
        if Self::looks_like_word_or_identifier(input) {
            debug!(input, "base64: rejected - looks like word/identifier");
            return vec![];
        }

        // Try to decode
        let Ok(bytes) = STANDARD.decode(input) else {
            trace!("base64: rejected - decode failed");
            return vec![];
        };

        // Empty decode is not useful
        if bytes.is_empty() {
            trace!("base64: rejected - empty decode");
            return vec![];
        }

        // Determine confidence
        let confidence = if input.ends_with("==") {
            0.9 // Padding is a strong indicator
        } else if input.ends_with('=') {
            0.85
        } else if input.len() >= 4 && input.len().is_multiple_of(4) {
            0.7 // Valid length, no padding needed
        } else {
            0.5
        };

        debug!(bytes_len = bytes.len(), confidence, "base64: matched");

        vec![Interpretation {
            value: CoreValue::Bytes(bytes.clone()),
            source_format: "base64".to_string(),
            confidence,
            description: format!("{} bytes", bytes.len()),
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) => Some(STANDARD.encode(bytes)),
            _ => None,
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["b64"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_base64_with_padding() {
        let format = Base64Format;
        let results = format.parse("aR4BuA==");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "base64");
        assert!(results[0].confidence >= 0.9);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_base64_hello() {
        let format = Base64Format;
        let results = format.parse("SGVsbG8=");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, b"Hello");
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_format_bytes_to_base64() {
        let format = Base64Format;
        let value = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        assert_eq!(format.format(&value), Some("aR4BuA==".to_string()));
    }

    #[test]
    fn test_invalid_base64() {
        let format = Base64Format;
        assert!(format.parse("!!!").is_empty());
        assert!(format.parse("abc!def").is_empty());
    }
}
