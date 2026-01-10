//! Octal format (0o prefix).

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, CoreValue, Interpretation};

pub struct OctalFormat;

/// Result of normalizing octal input.
struct NormalizedOctal {
    /// The octal string with only 0-7 digits.
    digits: String,
    /// Description of the input format.
    format_hint: String,
    /// Confidence score for this interpretation.
    confidence: f32,
}

impl OctalFormat {
    /// Check if a string contains only octal digits (0-7).
    fn is_valid_octal(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| matches!(c, '0'..='7'))
    }

    /// Normalize octal input from various formats.
    fn normalize(input: &str) -> Option<NormalizedOctal> {
        let trimmed = input.trim();

        // Try 0o/0O prefix (highest confidence)
        if let Some(rest) = trimmed
            .strip_prefix("0o")
            .or_else(|| trimmed.strip_prefix("0O"))
        {
            // Remove underscores (Rust/Python style separators)
            let clean: String = rest.chars().filter(|c| *c != '_').collect();
            if Self::is_valid_octal(&clean) {
                return Some(NormalizedOctal {
                    digits: clean,
                    format_hint: "0o prefix".to_string(),
                    confidence: 0.95,
                });
            }
        }

        // Try leading zero (C style: 0755) - but not just "0"
        if trimmed.starts_with('0')
            && trimmed.len() > 1
            && !trimmed.starts_with("0x")
            && !trimmed.starts_with("0X")
            && !trimmed.starts_with("0b")
            && !trimmed.starts_with("0B")
            && !trimmed.starts_with("0o")
            && !trimmed.starts_with("0O")
        {
            let clean: String = trimmed.chars().filter(|c| *c != '_').collect();
            if Self::is_valid_octal(&clean) {
                return Some(NormalizedOctal {
                    digits: clean,
                    format_hint: "leading zero (C-style)".to_string(),
                    confidence: 0.70,
                });
            }
        }

        None
    }

    /// Convert an octal string to an integer value.
    fn octal_to_int(digits: &str) -> Option<i128> {
        i128::from_str_radix(digits, 8).ok()
    }
}

impl Format for OctalFormat {
    fn id(&self) -> &'static str {
        "octal"
    }

    fn name(&self) -> &'static str {
        "Octal"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Numbers",
            description: "Octal literals (0o prefix or leading zero)",
            examples: &["0o755", "0o31002", "0755"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some(normalized) = Self::normalize(input) else {
            return vec![];
        };

        let Some(value) = Self::octal_to_int(&normalized.digits) else {
            return vec![];
        };

        vec![Interpretation {
            value: CoreValue::Int {
                value,
                original_bytes: None,
            },
            source_format: "octal".to_string(),
            confidence: normalized.confidence,
            description: format!(
                "Octal {} = {} decimal ({})",
                normalized.digits, value, normalized.format_hint
            ),
            rich_display: vec![],
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Int { value, .. } if *value >= 0)
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Int { value, .. } if *value >= 0 => Some(format!("0o{:o}", *value as u128)),
            _ => None,
        }
    }

    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        // Octal doesn't produce conversions - it parses to Int,
        // and DecimalFormat handles Int conversions (hex-int, binary-int, octal-int)
        vec![]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["oct", "o"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_0o_prefix() {
        let format = OctalFormat;
        let results = format.parse("0o755");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "octal");
        assert!(results[0].confidence > 0.9);

        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755); // 493 in decimal
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_0o_with_underscores() {
        let format = OctalFormat;
        let results = format.parse("0o7_5_5");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_leading_zero() {
        let format = OctalFormat;
        let results = format.parse("0755");

        assert_eq!(results.len(), 1);
        // Lower confidence for C-style
        assert!(results[0].confidence < 0.9);
        assert!(results[0].confidence > 0.5);

        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_31002() {
        let format = OctalFormat;
        let results = format.parse("0o31002");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            // 0o31002 = 3*8^4 + 1*8^3 + 0*8^2 + 0*8^1 + 2*8^0
            //         = 12288 + 512 + 2 = 12802
            assert_eq!(*value, 12802);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_invalid_octal() {
        let format = OctalFormat;
        // Contains 8 or 9 (invalid in octal)
        assert!(format.parse("0o789").is_empty());
        assert!(format.parse("0o128").is_empty());
    }

    #[test]
    fn test_just_zero_not_parsed() {
        let format = OctalFormat;
        // "0" alone should not be parsed as C-style octal
        assert!(format.parse("0").is_empty());
    }

    #[test]
    fn test_format_int_to_octal() {
        let format = OctalFormat;
        let value = CoreValue::Int {
            value: 493, // 0o755
            original_bytes: None,
        };
        let formatted = format.format(&value);
        assert_eq!(formatted, Some("0o755".to_string()));
    }

    #[test]
    fn test_no_parse_hex_prefix() {
        let format = OctalFormat;
        // Should not parse hex as octal
        assert!(format.parse("0x755").is_empty());
    }
}
