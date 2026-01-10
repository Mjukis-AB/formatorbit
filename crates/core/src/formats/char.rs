//! Unicode character format.
//!
//! Parses Unicode characters and grapheme clusters, showing codepoint breakdown.
//! Handles composite emojis like 👨‍👩‍👧‍👦 (family) which are multiple codepoints joined with ZWJ.

use unicode_segmentation::UnicodeSegmentation;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
    RichDisplay, RichDisplayOption,
};

pub struct CharFormat;

impl CharFormat {
    /// Get a human-readable name for special Unicode codepoints.
    fn codepoint_name(cp: u32) -> Option<&'static str> {
        match cp {
            // Control characters
            0x0000 => Some("NUL (null)"),
            0x0001 => Some("SOH (start of heading)"),
            0x0002 => Some("STX (start of text)"),
            0x0003 => Some("ETX (end of text)"),
            0x0004 => Some("EOT (end of transmission)"),
            0x0005 => Some("ENQ (enquiry)"),
            0x0006 => Some("ACK (acknowledge)"),
            0x0007 => Some("BEL (bell)"),
            0x0008 => Some("BS (backspace)"),
            0x0009 => Some("HT (horizontal tab)"),
            0x000A => Some("LF (line feed)"),
            0x000B => Some("VT (vertical tab)"),
            0x000C => Some("FF (form feed)"),
            0x000D => Some("CR (carriage return)"),
            0x000E => Some("SO (shift out)"),
            0x000F => Some("SI (shift in)"),
            0x0010 => Some("DLE (data link escape)"),
            0x0011 => Some("DC1 (device control 1)"),
            0x0012 => Some("DC2 (device control 2)"),
            0x0013 => Some("DC3 (device control 3)"),
            0x0014 => Some("DC4 (device control 4)"),
            0x0015 => Some("NAK (negative ack)"),
            0x0016 => Some("SYN (synchronous idle)"),
            0x0017 => Some("ETB (end of trans. block)"),
            0x0018 => Some("CAN (cancel)"),
            0x0019 => Some("EM (end of medium)"),
            0x001A => Some("SUB (substitute)"),
            0x001B => Some("ESC (escape)"),
            0x001C => Some("FS (file separator)"),
            0x001D => Some("GS (group separator)"),
            0x001E => Some("RS (record separator)"),
            0x001F => Some("US (unit separator)"),
            0x007F => Some("DEL (delete)"),
            // Special Unicode characters
            0x200D => Some("ZWJ (zero width joiner)"),
            0x200C => Some("ZWNJ (zero width non-joiner)"),
            0x200B => Some("ZWSP (zero width space)"),
            0x00A0 => Some("NBSP (non-breaking space)"),
            0xFEFF => Some("BOM (byte order mark)"),
            0xFE0F => Some("VS16 (emoji presentation)"),
            0xFE0E => Some("VS15 (text presentation)"),
            // Skin tone modifiers
            0x1F3FB => Some("light skin tone"),
            0x1F3FC => Some("medium-light skin tone"),
            0x1F3FD => Some("medium skin tone"),
            0x1F3FE => Some("medium-dark skin tone"),
            0x1F3FF => Some("dark skin tone"),
            _ => None,
        }
    }

    /// Format a single codepoint for display.
    fn format_codepoint(cp: u32) -> String {
        if let Some(name) = Self::codepoint_name(cp) {
            format!("U+{:04X} {}", cp, name)
        } else if let Some(ch) = char::from_u32(cp) {
            if cp < 32 || cp == 127 {
                format!("U+{:04X}", cp)
            } else {
                format!("U+{:04X} '{}'", cp, ch)
            }
        } else {
            format!("U+{:04X}", cp)
        }
    }

    /// Get the display name/character for a codepoint (without the U+XXXX prefix).
    fn codepoint_display_value(cp: u32) -> String {
        if let Some(name) = Self::codepoint_name(cp) {
            name.to_string()
        } else if let Some(ch) = char::from_u32(cp) {
            if cp < 32 || cp == 127 {
                String::new()
            } else {
                ch.to_string()
            }
        } else {
            String::new()
        }
    }

    /// Get UTF-8 bytes as hex string.
    fn utf8_hex(s: &str) -> String {
        s.bytes()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

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
            description: "Unicode character/grapheme cluster with codepoint breakdown",
            examples: &["A", "😀", "👨‍👩‍👧‍👦", "🏳️‍🌈"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return vec![];
        }

        // Use grapheme clusters to handle composite characters
        let graphemes: Vec<&str> = trimmed.graphemes(true).collect();

        // Must be exactly one grapheme cluster
        if graphemes.len() != 1 {
            return vec![];
        }

        let grapheme = graphemes[0];
        let chars: Vec<char> = grapheme.chars().collect();

        // Build rich display with codepoint breakdown
        let rich_display = vec![RichDisplayOption {
            preferred: RichDisplay::KeyValue {
                pairs: chars
                    .iter()
                    .map(|ch| {
                        let cp = *ch as u32;
                        (format!("U+{:04X}", cp), Self::codepoint_display_value(cp))
                    })
                    .collect(),
            },
            alternatives: vec![],
        }];

        // Build description based on complexity
        let (value, description) = if chars.len() == 1 {
            // Simple single codepoint
            let cp = chars[0] as u32;
            let desc = Self::format_codepoint(cp);
            (
                CoreValue::Int {
                    value: cp as i128,
                    original_bytes: None,
                },
                desc,
            )
        } else {
            // Composite grapheme cluster - show breakdown
            let mut parts = Vec::new();
            for ch in &chars {
                parts.push(Self::format_codepoint(*ch as u32));
            }

            let breakdown = parts.join(" + ");
            let desc = format!("'{}' = {}", grapheme, breakdown);

            // Store as string since it's multiple codepoints
            (CoreValue::String(grapheme.to_string()), desc)
        };

        vec![Interpretation {
            value,
            source_format: "char".to_string(),
            confidence: 0.90,
            description,
            rich_display,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let mut conversions = Vec::new();

        match value {
            CoreValue::Int {
                value: codepoint, ..
            } => {
                // Single codepoint
                if *codepoint < 0 || *codepoint > 0x10FFFF {
                    return vec![];
                }

                let cp = *codepoint as u32;

                // Decimal codepoint
                let dec_display = cp.to_string();
                conversions.push(Conversion {
                    value: CoreValue::Int {
                        value: cp as i128,
                        original_bytes: None,
                    },
                    target_format: "decimal".to_string(),
                    display: dec_display.clone(),
                    path: vec!["decimal".to_string()],
                    steps: vec![ConversionStep {
                        format: "decimal".to_string(),
                        value: CoreValue::Int {
                            value: cp as i128,
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
                let hex_display = format!("0x{:X}", cp);
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
                if let Some(ch) = char::from_u32(cp) {
                    let mut buf = [0u8; 4];
                    let utf8_str = ch.encode_utf8(&mut buf);
                    let utf8_hex = Self::utf8_hex(utf8_str);
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
            }
            CoreValue::String(s) => {
                // Composite grapheme - show codepoints and UTF-8
                let chars: Vec<char> = s.chars().collect();

                // Only show codepoints for short strings (max 100 chars)
                // Long strings would produce massive output
                if chars.len() > 100 {
                    return vec![];
                }

                // List of codepoints
                let codepoints: String = chars
                    .iter()
                    .map(|c| format!("U+{:04X}", *c as u32))
                    .collect::<Vec<_>>()
                    .join(" ");
                conversions.push(Conversion {
                    value: CoreValue::String(codepoints.clone()),
                    target_format: "codepoints".to_string(),
                    display: codepoints.clone(),
                    path: vec!["codepoints".to_string()],
                    steps: vec![ConversionStep {
                        format: "codepoints".to_string(),
                        value: CoreValue::String(codepoints.clone()),
                        display: codepoints,
                    }],
                    priority: ConversionPriority::Semantic,
                    kind: ConversionKind::Representation,
                    display_only: true,
                    ..Default::default()
                });

                // Note: Character count (length) removed - Utf8Format's is-ascii/encoding
                // traits already show character count information

                // UTF-8 bytes
                let utf8_hex = Self::utf8_hex(s);
                let byte_count = s.len();
                let utf8_display = format!("{} ({} bytes)", utf8_hex, byte_count);
                conversions.push(Conversion {
                    value: CoreValue::String(utf8_hex.clone()),
                    target_format: "utf8-bytes".to_string(),
                    display: utf8_display.clone(),
                    path: vec!["utf8-bytes".to_string()],
                    steps: vec![ConversionStep {
                        format: "utf8-bytes".to_string(),
                        value: CoreValue::String(utf8_hex),
                        display: utf8_display,
                    }],
                    priority: ConversionPriority::Encoding,
                    kind: ConversionKind::Conversion,
                    display_only: true,
                    ..Default::default()
                });
            }
            _ => {}
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
    fn test_parse_composite_emoji() {
        let format = CharFormat;
        // Family emoji: 👨‍👩‍👧‍👦
        let results = format.parse("👨‍👩‍👧‍👦");

        assert_eq!(results.len(), 1);
        // Should be a String (composite) not Int
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "👨‍👩‍👧‍👦");
        } else {
            panic!("Expected String for composite emoji");
        }
        // Description should show breakdown
        assert!(results[0].description.contains("ZWJ"));
    }

    #[test]
    fn test_parse_flag_emoji() {
        let format = CharFormat;
        // Rainbow flag: 🏳️‍🌈
        let results = format.parse("🏳️‍🌈");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(_) = &results[0].value {
            // Good - it's a composite
        } else {
            panic!("Expected String for flag emoji");
        }
    }

    #[test]
    fn test_parse_skin_tone_emoji() {
        let format = CharFormat;
        // Thumbs up with skin tone: 👍🏽
        let results = format.parse("👍🏽");

        assert_eq!(results.len(), 1);
        // Should show skin tone modifier in description
        assert!(
            results[0].description.contains("skin tone")
                || results[0].description.contains("1F3FD")
        );
    }

    #[test]
    fn test_reject_multiple_graphemes() {
        let format = CharFormat;
        assert!(format.parse("AB").is_empty());
        assert!(format.parse("Hello").is_empty());
        assert!(format.parse("😀😀").is_empty());
    }

    #[test]
    fn test_reject_empty() {
        let format = CharFormat;
        assert!(format.parse("").is_empty());
    }

    #[test]
    fn test_conversions_single() {
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

    #[test]
    fn test_conversions_composite() {
        let format = CharFormat;
        let value = CoreValue::String("👨‍👩‍👧‍👦".to_string());
        let conversions = format.conversions(&value);

        assert!(!conversions.is_empty());
        assert!(conversions.iter().any(|c| c.target_format == "codepoints"));
        // Note: length removed - Utf8Format's is-ascii/encoding traits show char count
        assert!(conversions.iter().any(|c| c.target_format == "utf8-bytes"));
    }

    #[test]
    fn test_rich_display_single_char() {
        let format = CharFormat;
        let results = format.parse("A");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].rich_display.len(), 1);

        if let RichDisplay::KeyValue { pairs } = &results[0].rich_display[0].preferred {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, "U+0041");
            assert_eq!(pairs[0].1, "A");
        } else {
            panic!("Expected KeyValue rich display");
        }
    }

    #[test]
    fn test_rich_display_composite_emoji() {
        let format = CharFormat;
        // Rainbow flag: 🏳️‍🌈
        let results = format.parse("🏳️‍🌈");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].rich_display.len(), 1);

        if let RichDisplay::KeyValue { pairs } = &results[0].rich_display[0].preferred {
            // Should have 4 codepoints: flag, VS16, ZWJ, rainbow
            assert_eq!(pairs.len(), 4);
            assert_eq!(pairs[0].0, "U+1F3F3");
            assert_eq!(pairs[1].0, "U+FE0F");
            assert_eq!(pairs[1].1, "VS16 (emoji presentation)");
            assert_eq!(pairs[2].0, "U+200D");
            assert_eq!(pairs[2].1, "ZWJ (zero width joiner)");
            assert_eq!(pairs[3].0, "U+1F308");
        } else {
            panic!("Expected KeyValue rich display");
        }
    }
}
