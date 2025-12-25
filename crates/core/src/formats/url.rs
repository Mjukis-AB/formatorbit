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

    /// Check if + signs look like URL form encoding (word+word) vs math (num + num).
    fn has_url_style_plus(s: &str) -> bool {
        // Look for patterns like "word+word" not "num + num" or "num+num"
        // URL form encoding: no spaces around +, typically between words
        for (i, c) in s.char_indices() {
            if c == '+' {
                // Check characters before and after
                let before = s[..i].chars().last();
                let after = s[i + 1..].chars().next();

                // If there are spaces around the +, it's likely math
                if before == Some(' ') || after == Some(' ') {
                    return false;
                }

                // If both sides are digits, it's likely math (123+456)
                if before.is_some_and(|c| c.is_ascii_digit())
                    && after.is_some_and(|c| c.is_ascii_digit())
                {
                    return false;
                }

                // If surrounded by alphanumeric (word+word), it's likely URL
                if before.is_some_and(|c| c.is_alphanumeric())
                    && after.is_some_and(|c| c.is_alphanumeric())
                {
                    return true;
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
        let has_percent = Self::has_percent_encoding(input);
        let has_url_plus = Self::has_url_style_plus(input);

        if !has_percent && !has_url_plus {
            return vec![];
        }

        // Replace + with space (form encoding) only if it looks like URL encoding
        let with_spaces = if has_url_plus || has_percent {
            input.replace('+', " ")
        } else {
            input.to_string()
        };

        let Ok(decoded) = percent_decode_str(&with_spaces).decode_utf8() else {
            return vec![];
        };

        // Don't return if nothing changed
        if decoded == input {
            return vec![];
        }

        // Lower confidence if only + replacement (no percent encoding)
        let confidence = if has_percent { 0.85 } else { 0.70 };

        vec![Interpretation {
            value: CoreValue::String(decoded.to_string()),
            source_format: "url-encoded".to_string(),
            confidence,
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
            metadata: None,
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
