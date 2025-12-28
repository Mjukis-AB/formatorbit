//! Unicode character format.
//!
//! Parses single Unicode characters and shows their codepoint value.

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct CharFormat;

impl Format for CharFormat {
    fn id(&self) -> &'static str {
        "char"
    }

    fn name(&self) -> &'static str {
        "Unicode Character"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Text",
            description: "Single Unicode character with codepoint information",
            examples: &["A", "😀", "中"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Must be exactly one character (grapheme clusters count as one visually,
        // but we work with Unicode scalar values here)
        let mut chars = trimmed.chars();
        let Some(ch) = chars.next() else {
            return vec![];
        };

        // Must be exactly one char
        if chars.next().is_some() {
            return vec![];
        }

        let codepoint = ch as u32;

        // Build description based on character type
        let description = if codepoint < 32 {
            // Control characters
            let name = match codepoint {
                0 => "NUL (null)",
                1 => "SOH (start of heading)",
                2 => "STX (start of text)",
                3 => "ETX (end of text)",
                4 => "EOT (end of transmission)",
                5 => "ENQ (enquiry)",
                6 => "ACK (acknowledge)",
                7 => "BEL (bell)",
                8 => "BS (backspace)",
                9 => "HT (horizontal tab)",
                10 => "LF (line feed)",
                11 => "VT (vertical tab)",
                12 => "FF (form feed)",
                13 => "CR (carriage return)",
                14 => "SO (shift out)",
                15 => "SI (shift in)",
                16 => "DLE (data link escape)",
                17 => "DC1 (device control 1)",
                18 => "DC2 (device control 2)",
                19 => "DC3 (device control 3)",
                20 => "DC4 (device control 4)",
                21 => "NAK (negative ack)",
                22 => "SYN (synchronous idle)",
                23 => "ETB (end of trans. block)",
                24 => "CAN (cancel)",
                25 => "EM (end of medium)",
                26 => "SUB (substitute)",
                27 => "ESC (escape)",
                28 => "FS (file separator)",
                29 => "GS (group separator)",
                30 => "RS (record separator)",
                31 => "US (unit separator)",
                _ => "control character",
            };
            format!("U+{:04X} {}", codepoint, name)
        } else if codepoint == 127 {
            format!("U+{:04X} DEL (delete)", codepoint)
        } else if codepoint <= 126 {
            // Printable ASCII
            format!("U+{:04X} '{}'", codepoint, ch)
        } else {
            // Unicode
            format!("U+{:04X} '{}'", codepoint, ch)
        };

        vec![Interpretation {
            value: CoreValue::Int {
                value: codepoint as i128,
                original_bytes: None,
            },
            source_format: "char".to_string(),
            confidence: 0.90,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int {
            value: codepoint, ..
        } = value
        else {
            return vec![];
        };

        // Only valid Unicode codepoints
        if *codepoint < 0 || *codepoint > 0x10FFFF {
            return vec![];
        }

        let codepoint = *codepoint as u32;
        let mut conversions = Vec::new();

        // Decimal codepoint
        let dec_display = codepoint.to_string();
        conversions.push(Conversion {
            value: CoreValue::Int {
                value: codepoint as i128,
                original_bytes: None,
            },
            target_format: "decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["decimal".to_string()],
            steps: vec![ConversionStep {
                format: "decimal".to_string(),
                value: CoreValue::Int {
                    value: codepoint as i128,
                    original_bytes: None,
                },
                display: dec_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // Hex codepoint
        let hex_display = format!("0x{:X}", codepoint);
        conversions.push(Conversion {
            value: CoreValue::String(hex_display.clone()),
            target_format: "hex-int".to_string(),
            display: hex_display.clone(),
            path: vec!["hex-int".to_string()],
            steps: vec![ConversionStep {
                format: "hex-int".to_string(),
                value: CoreValue::String(hex_display.clone()),
                display: hex_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // UTF-8 bytes
        if let Some(ch) = char::from_u32(codepoint) {
            let mut utf8_bytes = [0u8; 4];
            let utf8_str = ch.encode_utf8(&mut utf8_bytes);
            let utf8_hex: String = utf8_str
                .bytes()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            conversions.push(Conversion {
                value: CoreValue::String(utf8_hex.clone()),
                target_format: "utf8-bytes".to_string(),
                display: utf8_hex.clone(),
                path: vec!["utf8-bytes".to_string()],
                steps: vec![ConversionStep {
                    format: "utf8-bytes".to_string(),
                    value: CoreValue::String(utf8_hex.clone()),
                    display: utf8_hex,
                }],
                priority: ConversionPriority::Encoding,
                kind: ConversionKind::Conversion,
                display_only: true,
                ..Default::default()
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["unicode", "codepoint"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ascii() {
        let format = CharFormat;
        let results = format.parse("A");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "char");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 65);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_emoji() {
        let format = CharFormat;
        let results = format.parse("😀");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0x1F600);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_cjk() {
        let format = CharFormat;
        let results = format.parse("中");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0x4E2D);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_reject_multiple_chars() {
        let format = CharFormat;
        assert!(format.parse("AB").is_empty());
        assert!(format.parse("Hello").is_empty());
    }

    #[test]
    fn test_reject_empty() {
        let format = CharFormat;
        assert!(format.parse("").is_empty());
    }

    #[test]
    fn test_conversions() {
        let format = CharFormat;
        let value = CoreValue::Int {
            value: 65, // 'A'
            original_bytes: None,
        };
        let conversions = format.conversions(&value);

        assert!(!conversions.is_empty());
        assert!(conversions.iter().any(|c| c.target_format == "decimal"));
        assert!(conversions.iter().any(|c| c.target_format == "hex-int"));
        assert!(conversions.iter().any(|c| c.target_format == "utf8-bytes"));
    }
}
