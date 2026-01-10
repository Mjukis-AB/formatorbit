//! C-style escape sequence format.
//!
//! Parses strings with escape sequences like:
//! - `\x48\x65\x6c\x6c\x6f` → "Hello" (hex escapes)
//! - `\u0048\u0065\u006C\u006C\u006F` → "Hello" (Unicode escapes)
//! - `\110\145\154\154\157` → "Hello" (octal escapes)
//! - `\n\t\r` → newline, tab, carriage return

use tracing::{debug, trace};

use crate::format::{Format, FormatInfo};
use crate::truncate_str;
use crate::types::{Conversion, ConversionPriority, ConversionStep, CoreValue, Interpretation};

pub struct EscapeFormat;

impl EscapeFormat {
    /// Check if string contains escape sequences.
    fn has_escapes(s: &str) -> bool {
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    // Check for valid escape sequence starters
                    if matches!(
                        next,
                        'x' | 'u' | 'U' | 'n' | 't' | 'r' | '0'..='7' | '\\' | '"' | '\''
                    ) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Count the number of escape sequences in a string.
    fn count_escapes(s: &str) -> usize {
        let mut count = 0;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    if matches!(
                        next,
                        'x' | 'u' | 'U' | 'n' | 't' | 'r' | '0'..='7' | '\\' | '"' | '\''
                    ) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Decode escape sequences in a string.
    fn decode_escapes(s: &str) -> Option<Vec<u8>> {
        let mut result = Vec::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next()? {
                    // Hex escape: \xNN
                    'x' => {
                        let h1 = chars.next()?;
                        let h2 = chars.next()?;
                        let byte = u8::from_str_radix(&format!("{}{}", h1, h2), 16).ok()?;
                        result.push(byte);
                    }
                    // Unicode escape: \uNNNN
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            hex.push(chars.next()?);
                        }
                        let codepoint = u32::from_str_radix(&hex, 16).ok()?;
                        let ch = char::from_u32(codepoint)?;
                        let mut buf = [0u8; 4];
                        result.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    // Unicode escape: \UNNNNNNNN (8 hex digits)
                    'U' => {
                        let mut hex = String::new();
                        for _ in 0..8 {
                            hex.push(chars.next()?);
                        }
                        let codepoint = u32::from_str_radix(&hex, 16).ok()?;
                        let ch = char::from_u32(codepoint)?;
                        let mut buf = [0u8; 4];
                        result.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    // Octal escape: \NNN (1-3 octal digits)
                    c @ '0'..='7' => {
                        let mut octal = String::from(c);
                        // Collect up to 2 more octal digits
                        for _ in 0..2 {
                            if let Some(&next) = chars.peek() {
                                if matches!(next, '0'..='7') {
                                    octal.push(chars.next().unwrap());
                                } else {
                                    break;
                                }
                            }
                        }
                        let byte = u8::from_str_radix(&octal, 8).ok()?;
                        result.push(byte);
                    }
                    // Common escapes
                    'n' => result.push(b'\n'),
                    't' => result.push(b'\t'),
                    'r' => result.push(b'\r'),
                    '\\' => result.push(b'\\'),
                    '"' => result.push(b'"'),
                    '\'' => result.push(b'\''),
                    'a' => result.push(0x07), // bell
                    'b' => result.push(0x08), // backspace
                    'f' => result.push(0x0C), // form feed
                    'v' => result.push(0x0B), // vertical tab
                    // Unknown escape - fail
                    _ => return None,
                }
            } else {
                // Regular character
                let mut buf = [0u8; 4];
                result.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }

        Some(result)
    }

    /// Encode bytes as hex escape sequence.
    fn encode_hex_escapes(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("\\x{:02X}", b)).collect()
    }

    /// Encode string as Unicode escape sequence.
    fn encode_unicode_escapes(s: &str) -> String {
        s.chars().map(|c| format!("\\u{:04X}", c as u32)).collect()
    }
}

impl Format for EscapeFormat {
    fn id(&self) -> &'static str {
        "escape"
    }

    fn name(&self) -> &'static str {
        "Escape Sequences"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Encoding",
            description: "C-style escape sequences (\\x, \\u, \\n, etc.)",
            examples: &[
                "\\x48\\x65\\x6c\\x6c\\x6f",
                "\\u0048\\u0065\\u006C\\u006C\\u006F",
                "Hello\\nWorld",
            ],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        trace!(input_len = input.len(), "escape: checking input");

        if !Self::has_escapes(input) {
            trace!("escape: rejected - no escape sequences found");
            return vec![];
        }

        // Count escape sequences vs total length to determine if this is
        // primarily escape-encoded data or just text that happens to contain escapes
        let escape_count = Self::count_escapes(input);
        let input_len = input.len();

        // Require at least 10% of the input to be escape sequences
        // A \xNN sequence is 4 chars representing 1 byte, so ~25% overhead
        // If we have very few escapes in a long string, it's probably not escape data
        let min_escape_ratio = 0.10;
        let escape_chars = escape_count * 4; // rough estimate of chars consumed by escapes
        let escape_ratio = escape_chars as f32 / input_len as f32;

        if escape_ratio < min_escape_ratio && input_len > 20 {
            debug!(
                escape_count,
                escape_ratio, input_len, "escape: rejected - escape density too low"
            );
            return vec![];
        }

        let Some(decoded) = Self::decode_escapes(input) else {
            trace!("escape: rejected - decode failed");
            return vec![];
        };

        debug!(decoded_len = decoded.len(), escape_count, "escape: matched");

        // Try to interpret as UTF-8 string
        let (value, description) = if let Ok(s) = std::str::from_utf8(&decoded) {
            let char_count = s.chars().count();
            let display = if char_count > 50 {
                // UTF-8 safe truncation
                let truncated = truncate_str(s, 47);
                format!("\"{}\" ({} chars)", truncated, char_count)
            } else {
                format!("\"{}\"", s)
            };
            (
                CoreValue::String(s.to_string()),
                format!("Decoded: {}", display),
            )
        } else {
            (
                CoreValue::Bytes(decoded.clone()),
                format!("Decoded: {} bytes", decoded.len()),
            )
        };

        vec![Interpretation {
            value,
            source_format: "escape".to_string(),
            confidence: 0.90,
            description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Don't use generic formatting path - use conversions() instead
        // to get specific escape-hex and escape-unicode format names
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let mut conversions = Vec::new();

        match value {
            CoreValue::Bytes(bytes) if !bytes.is_empty() && bytes.len() <= 64 => {
                let escaped = Self::encode_hex_escapes(bytes);
                conversions.push(Conversion {
                    value: CoreValue::String(escaped.clone()),
                    target_format: "escape-hex".to_string(),
                    display: escaped.clone(),
                    path: vec!["escape-hex".to_string()],
                    steps: vec![ConversionStep {
                        format: "escape-hex".to_string(),
                        value: CoreValue::String(escaped.clone()),
                        display: escaped,
                    }],
                    priority: ConversionPriority::Encoding,
                    display_only: true,
                    ..Default::default()
                });
            }
            CoreValue::String(s) if !s.is_empty() && s.len() <= 64 => {
                let escaped = Self::encode_unicode_escapes(s);
                conversions.push(Conversion {
                    value: CoreValue::String(escaped.clone()),
                    target_format: "escape-unicode".to_string(),
                    display: escaped.clone(),
                    path: vec!["escape-unicode".to_string()],
                    steps: vec![ConversionStep {
                        format: "escape-unicode".to_string(),
                        value: CoreValue::String(escaped.clone()),
                        display: escaped,
                    }],
                    priority: ConversionPriority::Encoding,
                    display_only: true,
                    ..Default::default()
                });
            }
            _ => {}
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["esc", "escaped", "cstring"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_escapes() {
        let format = EscapeFormat;
        let results = format.parse("\\x48\\x65\\x6c\\x6c\\x6f");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hello");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_parse_unicode_escapes() {
        let format = EscapeFormat;
        let results = format.parse("\\u0048\\u0065\\u006C\\u006C\\u006F");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hello");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_parse_octal_escapes() {
        let format = EscapeFormat;
        // "Hi" = \110\151
        let results = format.parse("\\110\\151");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hi");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_parse_common_escapes() {
        let format = EscapeFormat;
        let results = format.parse("Hello\\nWorld");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hello\nWorld");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_parse_mixed_escapes() {
        let format = EscapeFormat;
        let results = format.parse("\\x48i\\n");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hi\n");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_no_escapes() {
        let format = EscapeFormat;
        let results = format.parse("Hello World");
        assert!(results.is_empty());
    }

    #[test]
    fn test_encode_hex() {
        let escaped = EscapeFormat::encode_hex_escapes(b"Hi");
        assert_eq!(escaped, "\\x48\\x69");
    }

    #[test]
    fn test_encode_unicode() {
        let escaped = EscapeFormat::encode_unicode_escapes("Hi");
        assert_eq!(escaped, "\\u0048\\u0069");
    }

    #[test]
    fn test_conversions() {
        let format = EscapeFormat;
        let value = CoreValue::Bytes(vec![0x48, 0x69]);
        let conversions = format.conversions(&value);

        assert!(!conversions.is_empty());
        let hex = conversions
            .iter()
            .find(|c| c.target_format == "escape-hex")
            .unwrap();
        assert_eq!(hex.display, "\\x48\\x69");
    }
}
