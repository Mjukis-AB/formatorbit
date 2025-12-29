//! Plain text format.
//!
//! Fallback format that interprets any input as plain text.
//! Shows ASCII/UTF-8 properties and enables conversion to bytes for hashing.

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation};

pub struct Utf8Format;

impl Format for Utf8Format {
    fn id(&self) -> &'static str {
        "text"
    }

    fn name(&self) -> &'static str {
        "Plain Text"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "Plain text with ASCII/UTF-8 detection",
            examples: &["Hello", "Héllo 👋"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Accept any non-empty string as plain text (very low confidence fallback)
        if input.is_empty() {
            return vec![];
        }

        let bytes = input.as_bytes();
        let is_ascii = bytes.iter().all(|b| b.is_ascii());

        let description = if is_ascii {
            format!("{} chars (ASCII)", input.chars().count())
        } else {
            let char_count = input.chars().count();
            let byte_count = bytes.len();
            format!("{} chars, {} bytes (UTF-8)", char_count, byte_count)
        };

        vec![Interpretation {
            value: CoreValue::String(input.to_string()),
            source_format: "text".to_string(),
            confidence: 0.10, // Very low - fallback interpretation
            description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::String(_) | CoreValue::Bytes(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::String(s) => Some(s.clone()),
            CoreValue::Bytes(bytes) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        match value {
            CoreValue::Bytes(bytes) => {
                // Try to convert bytes to UTF-8 string
                if let Ok(s) = String::from_utf8(bytes.clone()) {
                    vec![Conversion {
                        value: CoreValue::String(s.clone()),
                        target_format: "utf8".to_string(),
                        display: s,
                        path: vec!["utf8".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Encoding,
                        display_only: false,
                        kind: ConversionKind::default(),
                        rich_display: vec![],
                    }]
                } else {
                    vec![]
                }
            }
            CoreValue::String(s) => {
                let mut conversions = vec![];

                // String → Bytes (enables digest calculation)
                conversions.push(Conversion {
                    value: CoreValue::Bytes(s.as_bytes().to_vec()),
                    target_format: "bytes".to_string(),
                    display: format!("{} bytes", s.len()),
                    path: vec!["bytes".to_string()],
                    is_lossy: false,
                    steps: vec![],
                    priority: ConversionPriority::Raw,
                    display_only: false,
                    kind: ConversionKind::default(),
                    rich_display: vec![],
                });

                // ASCII codes (for short strings, max 20 bytes)
                if s.len() <= 20 {
                    let ascii_dec: String = s
                        .bytes()
                        .map(|b| b.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    conversions.push(Conversion {
                        value: CoreValue::String(ascii_dec.clone()),
                        target_format: "ascii-decimal".to_string(),
                        display: ascii_dec,
                        path: vec!["ascii-decimal".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Encoding,
                        display_only: true,
                        kind: ConversionKind::Representation,
                        rich_display: vec![],
                    });

                    let ascii_hex: String = s
                        .bytes()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    conversions.push(Conversion {
                        value: CoreValue::String(ascii_hex.clone()),
                        target_format: "ascii-hex".to_string(),
                        display: ascii_hex,
                        path: vec!["ascii-hex".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Encoding,
                        display_only: true,
                        kind: ConversionKind::Representation,
                        rich_display: vec![],
                    });
                }

                // ASCII/UTF-8 detection trait
                let is_ascii = s.bytes().all(|b| b.is_ascii());
                if is_ascii {
                    conversions.push(Conversion {
                        value: CoreValue::Bool(true),
                        target_format: "is-ascii".to_string(),
                        display: "ASCII".to_string(),
                        path: vec!["is-ascii".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: true,
                        kind: ConversionKind::Trait,
                        rich_display: vec![],
                    });
                } else {
                    // Show char vs byte count for multi-byte UTF-8
                    let char_count = s.chars().count();
                    let byte_count = s.len();
                    conversions.push(Conversion {
                        value: CoreValue::String(format!(
                            "{} chars, {} bytes",
                            char_count, byte_count
                        )),
                        target_format: "encoding".to_string(),
                        display: format!("UTF-8 ({} chars, {} bytes)", char_count, byte_count),
                        path: vec!["encoding".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: true,
                        kind: ConversionKind::Trait,
                        rich_display: vec![],
                    });
                }

                conversions
            }
            _ => vec![],
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["utf8", "str", "string", "ascii"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_ascii() {
        let format = Utf8Format;
        let results = format.parse("Hello");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "text");
        assert!((results[0].confidence - 0.10).abs() < 0.001);
        assert!(results[0].description.contains("ASCII"));
    }

    #[test]
    fn test_parse_text_utf8() {
        let format = Utf8Format;
        let results = format.parse("Héllo 👋");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("UTF-8"));
        assert!(results[0].description.contains("chars"));
        assert!(results[0].description.contains("bytes"));
    }

    #[test]
    fn test_parse_empty() {
        let format = Utf8Format;
        let results = format.parse("");
        assert!(results.is_empty());
    }

    #[test]
    fn test_bytes_to_utf8() {
        let format = Utf8Format;
        let value = CoreValue::Bytes(b"Hello".to_vec());
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "utf8");
        assert_eq!(conversions[0].display, "Hello");
    }

    #[test]
    fn test_invalid_utf8_bytes() {
        let format = Utf8Format;
        let value = CoreValue::Bytes(vec![0xFF, 0xFE]); // Invalid UTF-8
        let conversions = format.conversions(&value);

        assert!(conversions.is_empty());
    }

    #[test]
    fn test_string_to_bytes_and_ascii() {
        let format = Utf8Format;
        let value = CoreValue::String("Hi".to_string());
        let conversions = format.conversions(&value);

        // Should have: bytes, ascii-decimal, ascii-hex, is-ascii
        assert!(conversions.iter().any(|c| c.target_format == "bytes"));
        assert!(conversions
            .iter()
            .any(|c| c.target_format == "ascii-decimal"));
        assert!(conversions.iter().any(|c| c.target_format == "ascii-hex"));
        assert!(conversions.iter().any(|c| c.target_format == "is-ascii"));

        let ascii_dec = conversions
            .iter()
            .find(|c| c.target_format == "ascii-decimal")
            .unwrap();
        assert_eq!(ascii_dec.display, "72 105"); // 'H' = 72, 'i' = 105

        let ascii_hex = conversions
            .iter()
            .find(|c| c.target_format == "ascii-hex")
            .unwrap();
        assert_eq!(ascii_hex.display, "48 69"); // 'H' = 0x48, 'i' = 0x69
    }

    #[test]
    fn test_string_utf8_encoding_trait() {
        let format = Utf8Format;
        let value = CoreValue::String("Héllo".to_string());
        let conversions = format.conversions(&value);

        // Should have encoding trait, not is-ascii
        assert!(conversions.iter().any(|c| c.target_format == "encoding"));
        assert!(!conversions.iter().any(|c| c.target_format == "is-ascii"));

        let encoding = conversions
            .iter()
            .find(|c| c.target_format == "encoding")
            .unwrap();
        assert!(encoding.display.contains("UTF-8"));
    }
}
