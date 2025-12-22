//! NanoID format.
//!
//! NanoID is a tiny, secure, URL-friendly unique string ID.
//! Default: 21 characters using alphabet A-Za-z0-9_-

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct NanoIdFormat;

/// Default NanoID alphabet: A-Za-z0-9_-
const NANOID_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";

impl NanoIdFormat {
    /// Check if all characters are in the NanoID alphabet.
    fn is_valid_nanoid(s: &str) -> bool {
        s.chars().all(|c| NANOID_ALPHABET.contains(c))
    }

    /// Calculate confidence based on various heuristics.
    fn calculate_confidence(s: &str) -> f32 {
        let len = s.len();

        // Must use underscore or hyphen (distinguishes from plain base64)
        let has_special = s.contains('_') || s.contains('-');

        // Length heuristics
        let length_score = match len {
            21 => 0.85,      // Default NanoID length
            10..=32 => 0.60, // Common custom lengths
            _ => 0.30,
        };

        // Boost if has special chars (underscore/hyphen)
        if has_special {
            (length_score + 0.10_f32).min(0.90)
        } else {
            // Without special chars, could be anything - lower confidence
            length_score * 0.5
        }
    }
}

impl Format for NanoIdFormat {
    fn id(&self) -> &'static str {
        "nanoid"
    }

    fn name(&self) -> &'static str {
        "NanoID"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Identifiers",
            description: "Tiny, secure, URL-friendly unique string ID",
            examples: &["V1StGXR8_Z5jdHi6B-myT", "FwKo-QKdZ3Lg_8cCrH9kJ"],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Reasonable length range for NanoID
        if trimmed.len() < 10 || trimmed.len() > 36 {
            return vec![];
        }

        // Must be valid NanoID alphabet
        if !Self::is_valid_nanoid(trimmed) {
            return vec![];
        }

        let confidence = Self::calculate_confidence(trimmed);

        // Skip if confidence is too low
        if confidence < 0.30 {
            return vec![];
        }

        let description = format!("NanoID ({} chars)", trimmed.len());

        vec![Interpretation {
            value: CoreValue::String(trimmed.to_string()),
            source_format: "nanoid".to_string(),
            confidence,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // We don't generate NanoIDs
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["nano"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nanoid_default_length() {
        let format = NanoIdFormat;
        // 21 chars with underscore
        let results = format.parse("V1StGXR8_Z5jdHi6B-myT");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "nanoid");
        assert!(results[0].confidence > 0.8);
    }

    #[test]
    fn test_parse_nanoid_with_hyphen() {
        let format = NanoIdFormat;
        let results = format.parse("FwKo-QKdZ3Lg_8cCrH9kJ");

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_no_special_chars_lower_confidence() {
        let format = NanoIdFormat;
        // No underscore or hyphen - could be anything
        let results = format.parse("V1StGXR8aZ5jdHi6BamyT");

        // Should still parse but with lower confidence
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence < 0.5);
    }

    #[test]
    fn test_invalid_chars() {
        let format = NanoIdFormat;
        // Contains invalid characters
        assert!(format.parse("V1StGXR8@Z5jdHi6B-myT").is_empty());
        assert!(format.parse("V1StGXR8 Z5jdHi6B-myT").is_empty());
    }

    #[test]
    fn test_wrong_length() {
        let format = NanoIdFormat;
        // Too short
        assert!(format.parse("abc").is_empty());
        // Too long
        assert!(format
            .parse("V1StGXR8_Z5jdHi6B-myTV1StGXR8_Z5jdHi6B-myT")
            .is_empty());
    }
}
