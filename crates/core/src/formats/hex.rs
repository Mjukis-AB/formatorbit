//! Hexadecimal format.
//!
//! Supports various common hex paste formats:
//! - Continuous: `691E01B8`
//! - With 0x prefix: `0x691E01B8`
//! - Space-separated bytes: `69 1E 01 B8`
//! - Colon-separated (MAC address style): `69:1E:01:B8`
//! - Dash-separated: `69-1E-01-B8`
//! - Comma-separated: `0x69, 0x1E, 0x01, 0xB8`
//! - C array style: `{0x69, 0x1E, 0x01, 0xB8}`
//! - Hex editor style with optional ASCII: `00000000  69 1E 01 B8  |i...|`

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct HexFormat;

/// Result of normalizing hex input
struct NormalizedHex {
    hex: String,
    format_hint: &'static str,
    high_confidence: bool,
}

impl HexFormat {
    /// Check if a string contains only valid hex characters.
    fn is_valid_hex(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Try to normalize various hex formats into a continuous hex string.
    fn normalize(input: &str) -> Option<NormalizedHex> {
        // Trim whitespace including newlines
        let trimmed = input.trim();

        // 1. Try 0x prefix (single value)
        if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
            if Self::is_valid_hex(hex) && hex.len().is_multiple_of(2) {
                return Some(NormalizedHex {
                    hex: hex.to_uppercase(),
                    format_hint: "0x prefix",
                    high_confidence: true,
                });
            }
        }

        // 2. Try C array style: {0x69, 0x1E} or [0x69, 0x1E]
        let array_content = trimmed
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .or_else(|| trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')));

        if let Some(content) = array_content {
            if let Some(normalized) = Self::parse_comma_separated(content) {
                return Some(NormalizedHex {
                    hex: normalized,
                    format_hint: "array",
                    high_confidence: true,
                });
            }
        }

        // 3. Try comma-separated: 0x69, 0x1E or 69, 1E
        if trimmed.contains(',') {
            if let Some(normalized) = Self::parse_comma_separated(trimmed) {
                return Some(NormalizedHex {
                    hex: normalized,
                    format_hint: "comma-separated",
                    high_confidence: true,
                });
            }
        }

        // 4. Try space-separated bytes: 69 1E 01 B8
        if trimmed.contains(' ') {
            if let Some(normalized) = Self::parse_separated(trimmed, ' ') {
                return Some(NormalizedHex {
                    hex: normalized,
                    format_hint: "space-separated",
                    high_confidence: true,
                });
            }
        }

        // 5. Try colon-separated (MAC style): 69:1E:01:B8
        if trimmed.contains(':') && !trimmed.contains("::") {
            if let Some(normalized) = Self::parse_separated(trimmed, ':') {
                return Some(NormalizedHex {
                    hex: normalized,
                    format_hint: "colon-separated",
                    high_confidence: true,
                });
            }
        }

        // 6. Try dash-separated: 69-1E-01-B8
        if trimmed.contains('-') && !trimmed.starts_with('-') {
            if let Some(normalized) = Self::parse_separated(trimmed, '-') {
                return Some(NormalizedHex {
                    hex: normalized,
                    format_hint: "dash-separated",
                    high_confidence: true,
                });
            }
        }

        // 7. Continuous hex (no separators)
        if Self::is_valid_hex(trimmed) && trimmed.len().is_multiple_of(2) {
            let has_letters = trimmed.chars().any(|c| c.is_ascii_alphabetic());
            return Some(NormalizedHex {
                hex: trimmed.to_uppercase(),
                format_hint: "hex",
                high_confidence: has_letters,
            });
        }

        None
    }

    /// Parse separator-delimited hex bytes (space, colon, dash).
    fn parse_separated(input: &str, sep: char) -> Option<String> {
        let mut result = String::new();

        for part in input.split(sep) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Strip 0x prefix if present
            let hex = part
                .strip_prefix("0x")
                .or_else(|| part.strip_prefix("0X"))
                .unwrap_or(part);

            // Each part should be 1-2 hex chars (allow single digit like "9" -> "09")
            if hex.is_empty() || hex.len() > 2 || !Self::is_valid_hex(hex) {
                return None;
            }

            if hex.len() == 1 {
                result.push('0');
            }
            result.push_str(&hex.to_uppercase());
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Parse comma-separated hex values: "0x69, 0x1E" or "69, 1E"
    fn parse_comma_separated(input: &str) -> Option<String> {
        let mut result = String::new();

        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Strip 0x prefix if present
            let hex = part
                .strip_prefix("0x")
                .or_else(|| part.strip_prefix("0X"))
                .unwrap_or(part);

            // Each part should be 1-2 hex chars
            if hex.is_empty() || hex.len() > 2 || !Self::is_valid_hex(hex) {
                return None;
            }

            if hex.len() == 1 {
                result.push('0');
            }
            result.push_str(&hex.to_uppercase());
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Parse continuous hex string to bytes.
    fn decode(s: &str) -> Option<Vec<u8>> {
        if !s.len().is_multiple_of(2) {
            return None;
        }

        let mut bytes = Vec::with_capacity(s.len() / 2);
        for chunk in s.as_bytes().chunks(2) {
            let high = char::from(chunk[0]).to_digit(16)?;
            let low = char::from(chunk[1]).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        }
        Some(bytes)
    }

    /// Encode bytes to hex string.
    fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect()
    }
}

impl Format for HexFormat {
    fn id(&self) -> &'static str {
        "hex"
    }

    fn name(&self) -> &'static str {
        "Hexadecimal"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Encoding",
            description: "Hexadecimal byte encoding with multiple input styles",
            examples: &[
                "691E01B8",
                "0x691E01B8",
                "69 1E 01 B8",
                "69:1E:01:B8",
                "{0x69, 0x1E}",
            ],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some(normalized) = Self::normalize(input) else {
            return vec![];
        };

        let Some(bytes) = Self::decode(&normalized.hex) else {
            return vec![];
        };

        // Determine confidence based on format detection
        let confidence = if normalized.high_confidence {
            0.92
        } else if bytes.len() >= 2 {
            0.6
        } else {
            0.4
        };

        let description = if normalized.format_hint == "hex" {
            format!("{} bytes", bytes.len())
        } else {
            format!("{} bytes ({})", bytes.len(), normalized.format_hint)
        };

        vec![Interpretation {
            value: CoreValue::Bytes(bytes),
            source_format: "hex".to_string(),
            confidence,
            description,
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) => Some(Self::encode(bytes)),
            _ => None,
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["h", "x"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_with_prefix() {
        let format = HexFormat;
        let results = format.parse("0x691E01B8");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "hex");
        assert!(results[0].confidence > 0.9);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_hex_without_prefix() {
        let format = HexFormat;
        let results = format.parse("691E01B8");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_invalid_hex() {
        let format = HexFormat;
        assert!(format.parse("GHIJ").is_empty());
        assert!(format.parse("123").is_empty()); // Odd length
    }

    #[test]
    fn test_format_bytes() {
        let format = HexFormat;
        let value = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        assert_eq!(format.format(&value), Some("691E01B8".to_string()));
    }

    #[test]
    fn test_parse_space_separated() {
        let format = HexFormat;
        let results = format.parse("69 1E 01 B8");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("space-separated"));

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_colon_separated() {
        let format = HexFormat;
        let results = format.parse("69:1E:01:B8");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_c_array_style() {
        let format = HexFormat;
        let results = format.parse("{0x69, 0x1E, 0x01, 0xB8}");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_comma_separated_with_0x() {
        let format = HexFormat;
        let results = format.parse("0x69, 0x1E, 0x01, 0xB8");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_dash_separated() {
        let format = HexFormat;
        let results = format.parse("69-1E-01-B8");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x69, 0x1E, 0x01, 0xB8]);
        } else {
            panic!("Expected Bytes");
        }
    }
}
