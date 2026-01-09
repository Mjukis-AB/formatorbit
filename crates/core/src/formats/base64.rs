//! Base64 format.

use base64::{engine::general_purpose::STANDARD, Engine};
use tracing::{debug, trace};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct Base64Format;

impl Base64Format {
    /// Check if a string looks like valid base64.
    fn is_valid_base64_chars(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    }

    /// Check if a string looks like pure hex (only 0-9, A-F).
    /// Hex strings like "DEADBEEF" or "cafebabe" should not be interpreted as base64.
    fn looks_like_hex(s: &str) -> bool {
        // Must be at least 2 chars and even length (hex bytes)
        if s.len() < 2 || !s.len().is_multiple_of(2) {
            return false;
        }
        // All characters must be hex digits
        s.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Check if string has a hex prefix (0x or 0X).
    fn has_hex_prefix(s: &str) -> bool {
        s.starts_with("0x") || s.starts_with("0X")
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
        // Exception: if it decodes to valid UTF-8, it's probably intentional base64
        let all_lowercase = s.chars().all(|c| c.is_ascii_lowercase());
        if all_lowercase && s.len() >= 4 && s.len() <= 20 {
            // Check if it decodes to valid UTF-8 - that's a strong signal
            // (random words almost never decode to valid UTF-8)
            if let Ok(bytes) = STANDARD.decode(s) {
                if std::str::from_utf8(&bytes).is_ok() && !bytes.is_empty() {
                    // Decoded to valid UTF-8, probably intentional base64
                    return false;
                }
            }
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
        // But check if it decodes to valid UTF-8 - that's a strong base64 signal
        // (random identifiers almost never decode to valid UTF-8)
        if has_camel_case || (starts_with_prefix && reasonable_length) {
            if let Ok(bytes) = STANDARD.decode(s) {
                if std::str::from_utf8(&bytes).is_ok() && !bytes.is_empty() {
                    // Decoded to valid UTF-8, probably intentional base64
                    return false;
                }
            }
            return true;
        }

        false
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

        // Skip pure hex strings - they're hex, not base64
        // e.g., "DEADBEEF", "cafebabe", "0123456789abcdef"
        if Self::looks_like_hex(input) {
            debug!(input, "base64: rejected - looks like hex");
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
        let base_confidence = if input.ends_with("==") {
            0.9 // Padding is a strong indicator
        } else if input.ends_with('=') {
            0.85
        } else if input.len() >= 4 && input.len().is_multiple_of(4) {
            0.7 // Valid length, no padding needed
        } else {
            0.5
        };

        // Penalize if it looks like hex (0x prefix) - valid base64 but probably hex
        let confidence = if Self::has_hex_prefix(input) {
            base_confidence * 0.3 // Significantly lower confidence for hex-prefixed strings
        } else {
            base_confidence
        };

        debug!(bytes_len = bytes.len(), confidence, "base64: matched");

        vec![Interpretation {
            value: CoreValue::Bytes(bytes.clone()),
            source_format: "base64".to_string(),
            confidence,
            description: format!("{} bytes", bytes.len()),
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Use conversions() instead to support truncation for large data
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        // Use conversions() instead to support truncation for large data
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["b64"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        // Check for invalid characters
        for c in input.chars() {
            if !c.is_ascii_alphanumeric() && c != '+' && c != '/' && c != '=' {
                return Some(format!("invalid base64 character: '{}'", c));
            }
        }

        // Check for proper padding
        let padding_count = input.chars().filter(|&c| c == '=').count();
        if padding_count > 2 {
            return Some(format!(
                "too many padding characters ({}), max is 2",
                padding_count
            ));
        }

        // Check that padding is at the end
        if input.contains('=') && !input.ends_with('=') {
            return Some("padding '=' must be at the end".to_string());
        }

        // Try to decode
        match STANDARD.decode(input) {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        let full_b64 = STANDARD.encode(bytes);

        // Truncate display for large data (max ~100 chars of base64)
        let max_chars = 100;
        let display = if full_b64.len() <= max_chars {
            full_b64.clone()
        } else {
            let remaining = full_b64.len() - max_chars;
            format!("{}... ({} more chars)", &full_b64[..max_chars], remaining)
        };

        vec![Conversion {
            value: CoreValue::String(full_b64),
            target_format: "base64".to_string(),
            display: display.clone(),
            path: vec!["base64".to_string()],
            steps: vec![ConversionStep {
                format: "base64".to_string(),
                value: CoreValue::Bytes(bytes.clone()),
                display,
            }],
            is_lossy: false,
            priority: ConversionPriority::Encoding,
            display_only: true, // Don't explore further from base64 string (avoids codepoints noise)
            kind: ConversionKind::default(),
            hidden: false,
            rich_display: vec![],
        }]
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
    fn test_conversions_bytes_to_base64() {
        let format = Base64Format;
        let value = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = format.conversions(&value);
        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "base64");
        if let CoreValue::String(s) = &conversions[0].value {
            assert_eq!(s, "aR4BuA==");
        } else {
            panic!("Expected String value");
        }
    }

    #[test]
    fn test_invalid_base64() {
        let format = Base64Format;
        assert!(format.parse("!!!").is_empty());
        assert!(format.parse("abc!def").is_empty());
    }

    #[test]
    fn test_hex_not_base64() {
        // Pure hex strings should not be interpreted as base64
        let format = Base64Format;
        assert!(format.parse("DEADBEEF").is_empty());
        assert!(format.parse("cafebabe").is_empty());
        assert!(format.parse("0123456789abcdef").is_empty());
        assert!(format.parse("CAFED00D").is_empty());
    }

    #[test]
    fn test_hex_prefix_low_confidence() {
        // Strings with 0x prefix should have very low confidence as base64
        let format = Base64Format;

        // 0x87 is valid base64 but should have low confidence
        let results = format.parse("0x87");
        assert_eq!(results.len(), 1);
        assert!(
            results[0].confidence < 0.3,
            "0x87 should have low base64 confidence"
        );

        // 0xDEADBEEF - longer hex-prefixed string
        let results = format.parse("0xDEADBEEF");
        if !results.is_empty() {
            assert!(
                results[0].confidence < 0.3,
                "0xDEADBEEF should have low base64 confidence"
            );
        }
    }
}
