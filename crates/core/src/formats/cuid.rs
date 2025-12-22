//! CUID2 (Collision-resistant Unique Identifier) format.
//!
//! CUID2 is a secure, collision-resistant ID using base36 (lowercase a-z, 0-9).
//! Default length is 24 characters, starting with a letter.

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct CuidFormat;

impl CuidFormat {
    /// Check if a string matches CUID2 format.
    /// Must be lowercase alphanumeric, starting with a letter.
    fn is_valid_cuid2(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        let mut chars = s.chars();

        // First character must be a lowercase letter
        let first = chars.next().unwrap();
        if !first.is_ascii_lowercase() {
            return false;
        }

        // Rest must be lowercase alphanumeric
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    }

    /// Calculate confidence based on heuristics.
    fn calculate_confidence(s: &str) -> f32 {
        let len = s.len();

        // Length heuristics
        match len {
            24 => 0.85,  // Default CUID2 length
            25 => 0.80,  // Original CUID length
            20..=32 => 0.65,  // Common custom lengths
            10..=19 => 0.50,
            _ => 0.30,
        }
    }
}

impl Format for CuidFormat {
    fn id(&self) -> &'static str {
        "cuid"
    }

    fn name(&self) -> &'static str {
        "CUID"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Identifiers",
            description: "Collision-resistant Unique Identifier (CUID2)",
            examples: &["tz4a98xxat96iws9zmbrgj3a", "pfh0haxfpzowht3oi213cqos"],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Reasonable length range for CUID
        if trimmed.len() < 10 || trimmed.len() > 32 {
            return vec![];
        }

        // Must match CUID2 format
        if !Self::is_valid_cuid2(trimmed) {
            return vec![];
        }

        let confidence = Self::calculate_confidence(trimmed);

        // Skip if confidence is too low
        if confidence < 0.30 {
            return vec![];
        }

        let description = format!("CUID2 ({} chars)", trimmed.len());

        vec![Interpretation {
            value: CoreValue::String(trimmed.to_string()),
            source_format: "cuid".to_string(),
            confidence,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // We don't generate CUIDs
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["cuid2"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cuid2_default_length() {
        let format = CuidFormat;
        let results = format.parse("tz4a98xxat96iws9zmbrgj3a");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "cuid");
        assert!(results[0].confidence > 0.8);
    }

    #[test]
    fn test_parse_cuid2_example() {
        let format = CuidFormat;
        let results = format.parse("pfh0haxfpzowht3oi213cqos");

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_must_start_with_letter() {
        let format = CuidFormat;
        // Starts with number - not valid CUID2
        assert!(format.parse("1z4a98xxat96iws9zmbrgj3a").is_empty());
    }

    #[test]
    fn test_must_be_lowercase() {
        let format = CuidFormat;
        // Contains uppercase - not valid CUID2
        assert!(format.parse("Tz4a98xxat96iws9zmbrgj3a").is_empty());
        assert!(format.parse("tz4a98xxAt96iws9zmbrgj3a").is_empty());
    }

    #[test]
    fn test_no_special_chars() {
        let format = CuidFormat;
        // Contains special characters - not valid
        assert!(format.parse("tz4a98xx-t96iws9zmbrgj3a").is_empty());
        assert!(format.parse("tz4a98xx_t96iws9zmbrgj3a").is_empty());
    }

    #[test]
    fn test_wrong_length() {
        let format = CuidFormat;
        // Too short
        assert!(format.parse("abc").is_empty());
        // Too long
        assert!(format
            .parse("tz4a98xxat96iws9zmbrgj3atz4a98xxat96iws9zmbrgj3a")
            .is_empty());
    }
}
