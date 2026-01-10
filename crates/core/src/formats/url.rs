//! URL encoding format.

use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation};

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

    /// Check if input looks like code/text rather than URL-encoded data.
    fn looks_like_code(s: &str) -> bool {
        // Multi-line text is unlikely to be URL-encoded
        if s.contains('\n') {
            return true;
        }

        // Common programming syntax suggests code
        let code_chars = ['(', ')', '{', '}', '[', ']', ';', '"', '\''];
        let code_char_count = s.chars().filter(|c| code_chars.contains(c)).count();
        if code_char_count >= 2 {
            return true;
        }

        false
    }

    /// Check if input looks like base64 data (which often contains +).
    fn looks_like_base64(s: &str) -> bool {
        // Base64 characteristics:
        // - Length is typically multiple of 4 (or close with padding)
        // - Contains only A-Z, a-z, 0-9, +, /, =
        // - Often ends with = or ==
        // - Relatively long strings without spaces
        if s.len() < 20 {
            return false;
        }

        // Count base64-valid characters
        let valid_count = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
            .count();

        // If almost all characters are base64-valid, it's probably base64
        if valid_count as f32 / s.len() as f32 > 0.95 {
            return true;
        }

        false
    }

    /// Truncate a string for display purposes (UTF-8 safe).
    fn truncate_display(s: &str, max_chars: usize) -> String {
        let char_count = s.chars().count();
        if char_count <= max_chars {
            s.to_string()
        } else {
            let remaining = char_count - max_chars;
            let truncated: String = s.chars().take(max_chars).collect();
            format!("{}... ({} more chars)", truncated, remaining)
        }
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
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Skip if input looks like code/text rather than URL-encoded data
        if Self::looks_like_code(input) {
            return vec![];
        }

        // Skip if input looks like base64 (which also uses +)
        if Self::looks_like_base64(input) {
            return vec![];
        }

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

        // Truncate description for long strings
        let display = Self::truncate_display(&decoded, 100);

        vec![Interpretation {
            value: CoreValue::String(decoded.to_string()),
            source_format: "url-encoded".to_string(),
            confidence,
            description: format!("Decoded: {}", display),
            rich_display: vec![],
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
            display_only: true, // Terminal format - don't chain further
            kind: ConversionKind::Conversion,
            hidden: false,
            rich_display: vec![],
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

    #[test]
    fn test_skip_code_like_input() {
        let format = UrlEncodingFormat;

        // Multi-line text should not be parsed as URL-encoded
        assert!(format.parse("hello+world\nfoo+bar").is_empty());

        // Code with brackets should not be parsed
        assert!(format.parse("print(\"hello+world\")").is_empty());

        // But simple word+word should still work
        assert!(!format.parse("hello+world").is_empty());
    }
}
