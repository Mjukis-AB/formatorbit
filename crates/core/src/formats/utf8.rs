//! UTF-8 string format.

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

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
            aliases: self.aliases(),
        }
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // Don't parse text as "utf8" - it just creates noise.
        // The utf8 format is only useful for bytes→string conversion.
        vec![]
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
                        metadata: None,
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
                    steps: vec![],
                    priority: ConversionPriority::Raw,
                    display_only: false,
                    metadata: None,
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
    fn test_parse_utf8_disabled() {
        // utf8 parsing is disabled - it only does bytes→string conversion
        let format = Utf8Format;
        let results = format.parse("Hello, World!");
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
}
