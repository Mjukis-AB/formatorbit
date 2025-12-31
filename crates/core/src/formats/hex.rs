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
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

/// Maximum bytes to display in hex output before truncating.
const MAX_HEX_DISPLAY_BYTES: usize = 64;

pub struct HexFormat;

/// Known hash types by byte length (not hex length).
/// Only includes common hash sizes that are unlikely to be coincidental.
/// Excludes CRC-32 (4 bytes) as it's too common to be meaningful.
const HASH_BYTE_LENGTHS: &[(usize, &str)] = &[
    (16, "MD5/MD4"),
    (20, "SHA-1/RIPEMD-160"),
    (28, "SHA-224"),
    (32, "SHA-256"),
    (48, "SHA-384"),
    (64, "SHA-512"),
];

/// Get hash type hint for a given byte length.
fn hash_hint_for_length(byte_len: usize) -> Option<&'static str> {
    HASH_BYTE_LENGTHS
        .iter()
        .find(|(len, _)| *len == byte_len)
        .map(|(_, name)| *name)
}

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
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
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

    /// Encode bytes to hex string with truncation for large data.
    fn encode_truncated(bytes: &[u8], max_bytes: usize) -> String {
        if bytes.len() <= max_bytes {
            Self::encode(bytes)
        } else {
            let truncated: String = bytes[..max_bytes]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect();
            let remaining = bytes.len() - max_bytes;
            format!("{}... ({} more bytes)", truncated, remaining)
        }
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
            aliases: self.aliases(),
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

        // Build description with optional hash hint
        let hash_hint = hash_hint_for_length(bytes.len());
        let description = match (normalized.format_hint, hash_hint) {
            ("hex", Some(hash)) => format!("{} bytes — possible {} hash", bytes.len(), hash),
            ("hex", None) => format!("{} bytes", bytes.len()),
            (fmt, Some(hash)) => {
                format!("{} bytes ({}) — possible {} hash", bytes.len(), fmt, hash)
            }
            (fmt, None) => format!("{} bytes ({})", bytes.len(), fmt),
        };

        vec![Interpretation {
            value: CoreValue::Bytes(bytes),
            source_format: "hex".to_string(),
            confidence,
            description,
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
        &["h", "x"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        let trimmed = input.trim();

        // Try to normalize - if it fails, we need to explain why
        if Self::normalize(trimmed).is_some() {
            return None; // Valid hex
        }

        // Determine the specific error
        let stripped = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);

        // Check for invalid characters
        for c in stripped.chars() {
            if !c.is_ascii_hexdigit()
                && c != ' '
                && c != ':'
                && c != '-'
                && c != ','
                && c != '{'
                && c != '}'
                && c != '['
                && c != ']'
            {
                return Some(format!("invalid hex character: '{}'", c));
            }
        }

        // Check for odd length
        let hex_only: String = stripped.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if !hex_only.is_empty() && !hex_only.len().is_multiple_of(2) {
            return Some(format!(
                "odd number of hex digits ({}), expected even count",
                hex_only.len()
            ));
        }

        Some("not a valid hex format".to_string())
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Show truncated hex for large data
        let display = Self::encode_truncated(bytes, MAX_HEX_DISPLAY_BYTES);

        vec![Conversion {
            value: CoreValue::String(Self::encode(bytes)),
            target_format: "hex".to_string(),
            display,
            path: vec!["hex".to_string()],
            steps: vec![ConversionStep {
                format: "hex".to_string(),
                value: CoreValue::String(Self::encode(bytes)),
                display: Self::encode_truncated(bytes, MAX_HEX_DISPLAY_BYTES),
            }],
            is_lossy: false,
            priority: ConversionPriority::Encoding,
            display_only: true, // Don't explore further from hex string (avoids codepoints noise)
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
    fn test_conversions_bytes_to_hex() {
        let format = HexFormat;
        let value = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = format.conversions(&value);
        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "hex");
        if let CoreValue::String(s) = &conversions[0].value {
            assert_eq!(s, "691E01B8");
        } else {
            panic!("Expected String value");
        }
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
