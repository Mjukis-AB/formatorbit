//! Binary format (0b prefix, space-separated, etc.).

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

pub struct BinaryFormat;

/// Result of normalizing binary input.
struct NormalizedBinary {
    /// The binary string with only 0s and 1s.
    bits: String,
    /// Description of the input format.
    format_hint: String,
    /// Confidence score for this interpretation.
    confidence: f32,
}

impl BinaryFormat {
    /// Check if a string contains only binary digits (0 and 1).
    fn is_valid_binary(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c == '0' || c == '1')
    }

    /// Normalize binary input from various formats.
    fn normalize(input: &str) -> Option<NormalizedBinary> {
        let trimmed = input.trim();

        // Try 0b/0B prefix (highest confidence)
        if let Some(rest) = trimmed
            .strip_prefix("0b")
            .or_else(|| trimmed.strip_prefix("0B"))
        {
            // Remove underscores (Rust/Python style separators)
            let clean: String = rest.chars().filter(|c| *c != '_').collect();
            if Self::is_valid_binary(&clean) {
                return Some(NormalizedBinary {
                    bits: clean,
                    format_hint: "0b prefix".to_string(),
                    confidence: 0.95,
                });
            }
        }

        // Try % prefix (assembly style)
        if let Some(rest) = trimmed.strip_prefix('%') {
            let clean: String = rest.chars().filter(|c| *c != '_').collect();
            if Self::is_valid_binary(&clean) {
                return Some(NormalizedBinary {
                    bits: clean,
                    format_hint: "% prefix (assembly)".to_string(),
                    confidence: 0.90,
                });
            }
        }

        // Try space-separated (like "1010 1010")
        if trimmed.contains(' ') {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.iter().all(|p| Self::is_valid_binary(p)) {
                let bits: String = parts.concat();
                return Some(NormalizedBinary {
                    bits,
                    format_hint: "space-separated".to_string(),
                    confidence: 0.85,
                });
            }
        }

        // Try continuous binary (lowest confidence - ambiguous with decimal)
        // Only parse if it contains at least one '0' AND at least one '1'
        // (to avoid matching pure "0" or "1" or "111111" which is likely decimal)
        let clean: String = trimmed.chars().filter(|c| *c != '_').collect();
        if Self::is_valid_binary(&clean) && clean.contains('0') && clean.contains('1') {
            // Additional heuristic: longer strings are more likely to be binary
            let confidence = if clean.len() >= 8 { 0.50 } else { 0.30 };
            return Some(NormalizedBinary {
                bits: clean,
                format_hint: "continuous".to_string(),
                confidence,
            });
        }

        None
    }

    /// Convert a binary string to bytes (pads to multiple of 8 bits).
    fn bits_to_bytes(bits: &str) -> Vec<u8> {
        // Pad to multiple of 8
        let padded_len = bits.len().div_ceil(8) * 8;
        let padded = format!("{:0>width$}", bits, width = padded_len);

        padded
            .as_bytes()
            .chunks(8)
            .map(|chunk| {
                let s = std::str::from_utf8(chunk).unwrap();
                u8::from_str_radix(s, 2).unwrap()
            })
            .collect()
    }

    /// Format bytes as binary string (space-separated groups of 8).
    fn bytes_to_binary_grouped(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:08b}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Format bytes as binary string with 0b prefix.
    fn bytes_to_binary_0b(bytes: &[u8]) -> String {
        let bits: String = bytes.iter().map(|b| format!("{b:08b}")).collect();
        format!("0b{bits}")
    }
}

impl Format for BinaryFormat {
    fn id(&self) -> &'static str {
        "binary"
    }

    fn name(&self) -> &'static str {
        "Binary"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Numbers",
            description: "Binary literals (0b prefix, space-separated)",
            examples: &["0b10101010", "1010 1010", "0b1111_0000", "%10101010"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some(normalized) = Self::normalize(input) else {
            return vec![];
        };

        // Limit to 128 bits (16 bytes) like i128
        if normalized.bits.len() > 128 {
            return vec![];
        }

        let bytes = Self::bits_to_bytes(&normalized.bits);
        let bit_count = normalized.bits.len();

        vec![Interpretation {
            value: CoreValue::Bytes(bytes),
            source_format: "binary".to_string(),
            confidence: normalized.confidence,
            description: format!("{} bits ({})", bit_count, normalized.format_hint),
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(b) if !b.is_empty() && b.len() <= 16)
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) if !bytes.is_empty() && bytes.len() <= 16 => {
                Some(Self::bytes_to_binary_grouped(bytes))
            }
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        if bytes.is_empty() || bytes.len() > 16 {
            return vec![];
        }

        vec![
            Conversion {
                value: CoreValue::String(Self::bytes_to_binary_grouped(bytes)),
                target_format: "binary".to_string(),
                display: Self::bytes_to_binary_grouped(bytes),
                path: vec!["binary".to_string()],
                is_lossy: false,
                steps: vec![],
                priority: ConversionPriority::Encoding,
                // Display-only: the string "01001101 10101010" shouldn't be
                // converted further (e.g. to bytes of ASCII digits)
                display_only: true,
                metadata: None,
            },
            Conversion {
                value: CoreValue::String(Self::bytes_to_binary_0b(bytes)),
                target_format: "binary-0b".to_string(),
                display: Self::bytes_to_binary_0b(bytes),
                path: vec!["binary-0b".to_string()],
                is_lossy: false,
                steps: vec![],
                priority: ConversionPriority::Encoding,
                // Display-only: the string "0b01001101" shouldn't be
                // converted further (e.g. to bytes of ASCII digits)
                display_only: true,
                metadata: None,
            },
        ]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["bin", "b"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_0b_prefix() {
        let format = BinaryFormat;
        let results = format.parse("0b10101010");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "binary");
        assert!(results[0].confidence > 0.9);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0b10101010]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_0b_with_underscores() {
        let format = BinaryFormat;
        let results = format.parse("0b1010_1010");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0b10101010]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_percent_prefix() {
        let format = BinaryFormat;
        let results = format.parse("%10101010");

        assert_eq!(results.len(), 1);
        assert!(results[0].confidence > 0.85);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0b10101010]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_space_separated() {
        let format = BinaryFormat;
        let results = format.parse("1010 1010 1111 0000");

        assert_eq!(results.len(), 1);
        assert!(results[0].confidence > 0.8);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0b10101010, 0b11110000]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_continuous_low_confidence() {
        let format = BinaryFormat;
        let results = format.parse("10101010");

        assert_eq!(results.len(), 1);
        assert!(results[0].confidence <= 0.5);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0b10101010]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_padding_to_byte_boundary() {
        let format = BinaryFormat;
        // 4 bits should be padded to 8 bits
        let results = format.parse("0b1010");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0b00001010]); // Padded with leading zeros
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_multi_byte() {
        let format = BinaryFormat;
        let results = format.parse("0b1111111100000000");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0xFF, 0x00]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_format_bytes_to_binary() {
        let format = BinaryFormat;
        let value = CoreValue::Bytes(vec![0b10101010, 0b11110000]);
        let formatted = format.format(&value).unwrap();

        assert_eq!(formatted, "10101010 11110000");
    }

    #[test]
    fn test_conversions() {
        let format = BinaryFormat;
        let value = CoreValue::Bytes(vec![0xAA]);
        let conversions = format.conversions(&value);

        assert!(conversions.iter().any(|c| c.target_format == "binary"));
        assert!(conversions.iter().any(|c| c.target_format == "binary-0b"));

        let bin_0b = conversions
            .iter()
            .find(|c| c.target_format == "binary-0b")
            .unwrap();
        assert_eq!(bin_0b.display, "0b10101010");
    }

    #[test]
    fn test_invalid_binary() {
        let format = BinaryFormat;
        // Contains non-binary digits
        assert!(format.parse("0b10102010").is_empty());
        assert!(format.parse("0b1010abc").is_empty());
    }

    #[test]
    fn test_too_long() {
        let format = BinaryFormat;
        // More than 128 bits
        let long = "0b".to_string() + &"1".repeat(129);
        assert!(format.parse(&long).is_empty());
    }

    #[test]
    fn test_all_same_digit_not_parsed() {
        let format = BinaryFormat;
        // All 1s or all 0s without prefix should not be parsed
        // (too ambiguous with decimal)
        assert!(format.parse("11111111").is_empty());
        assert!(format.parse("00000000").is_empty());
    }
}
