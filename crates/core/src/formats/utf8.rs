//! UTF-8 string format.

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, CoreValue, Interpretation};

pub struct Utf8Format;

impl Format for Utf8Format {
    fn id(&self) -> &'static str {
        "utf8"
    }

    fn name(&self) -> &'static str {
        "UTF-8 String"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "UTF-8 text (fallback interpretation)",
            examples: &[],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Everything is valid UTF-8 (we already have a &str)
        // This acts as a fallback interpretation
        vec![Interpretation {
            value: CoreValue::String(input.to_string()),
            source_format: "utf8".to_string(),
            confidence: 0.1, // Low confidence as fallback
            description: format!("{} characters", input.chars().count()),
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
                    }]
                } else {
                    vec![]
                }
            }
            CoreValue::String(s) => {
                // Convert string to bytes
                vec![Conversion {
                    value: CoreValue::Bytes(s.as_bytes().to_vec()),
                    target_format: "bytes".to_string(),
                    display: format!("{} bytes", s.len()),
                    path: vec!["bytes".to_string()],
                    is_lossy: false,
                }]
            }
            _ => vec![],
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["str", "string", "text"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_utf8() {
        let format = Utf8Format;
        let results = format.parse("Hello, World!");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "utf8");

        if let CoreValue::String(s) = &results[0].value {
            assert_eq!(s, "Hello, World!");
        } else {
            panic!("Expected String");
        }
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
}
