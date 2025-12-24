//! URL encoding format.

use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

pub struct UrlEncodingFormat;

impl UrlEncodingFormat {
    /// Check if a string contains URL-encoded sequences.
    fn has_percent_encoding(s: &str) -> bool {
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                // Check for two hex digits
                let h1 = chars.next();
                let h2 = chars.next();
                if let (Some(a), Some(b)) = (h1, h2) {
                    if a.is_ascii_hexdigit() && b.is_ascii_hexdigit() {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Format for UrlEncodingFormat {
    fn id(&self) -> &'static str {
        "url-encoded"
    }

    fn name(&self) -> &'static str {
        "URL Encoded"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Encoding",
            description: "URL percent-encoding (%20, +, etc.)",
            examples: &["Hello%20World", "foo+bar", "a%3Db"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Only try to decode if it looks URL-encoded
        if !Self::has_percent_encoding(input) && !input.contains('+') {
            return vec![];
        }

        // Replace + with space (form encoding)
        let with_spaces = input.replace('+', " ");

        let Ok(decoded) = percent_decode_str(&with_spaces).decode_utf8() else {
            return vec![];
        };

        // Don't return if nothing changed
        if decoded == input {
            return vec![];
        }

        vec![Interpretation {
            value: CoreValue::String(decoded.to_string()),
            source_format: "url-encoded".to_string(),
            confidence: 0.85,
            description: format!("Decoded: {}", decoded),
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::String(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::String(s) => Some(utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::String(s) = value else {
            return vec![];
        };

        let encoded = utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();

        // Only show if encoding changes something
        if encoded == *s {
            return vec![];
        }

        vec![Conversion {
            value: CoreValue::String(encoded.clone()),
            target_format: "url-encoded".to_string(),
            display: encoded,
            path: vec!["url-encoded".to_string()],
            is_lossy: false,
            steps: vec![],
            priority: ConversionPriority::Encoding,
            display_only: false,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["url", "percent"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_encoded() {
        let format = UrlEncodingFormat;
        let results = format.parse("Hello%20World");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hello World");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_parse_plus_as_space() {
        let format = UrlEncodingFormat;
        let results = format.parse("Hello+World");

        assert_eq!(results.len(), 1);
        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hello World");
        } else {
            panic!("Expected String");
        }
    }

    #[test]
    fn test_format_to_url_encoded() {
        let format = UrlEncodingFormat;
        let value = CoreValue::String("Hello World!".to_string());
        let encoded = format.format(&value).unwrap();

        assert!(encoded.contains("%20"));
        assert!(encoded.contains("%21"));
    }

    #[test]
    fn test_not_url_encoded() {
        let format = UrlEncodingFormat;
        // Plain text without encoding
        assert!(format.parse("HelloWorld").is_empty());
    }
}
