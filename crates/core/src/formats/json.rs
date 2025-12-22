//! JSON format.

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct JsonFormat;

impl Format for JsonFormat {
    fn id(&self) -> &'static str {
        "json"
    }

    fn name(&self) -> &'static str {
        "JSON"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "JSON objects and arrays",
            examples: &[r#"{"key": "value"}"#, "[1, 2, 3]"],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Only parse if it looks like JSON (starts with { or [)
        let trimmed = input.trim();
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            return vec![];
        }

        let Ok(value) = serde_json::from_str::<serde_json::Value>(input) else {
            return vec![];
        };

        vec![Interpretation {
            value: CoreValue::Json(value),
            source_format: "json".to_string(),
            confidence: 0.95,
            description: "JSON object".to_string(),
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Json(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Json(json) => serde_json::to_string_pretty(json).ok(),
            _ => None,
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["j"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_object() {
        let format = JsonFormat;
        let results = format.parse(r#"{"key": "value"}"#);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "json");
        assert!(results[0].confidence > 0.9);
    }

    #[test]
    fn test_parse_json_array() {
        let format = JsonFormat;
        let results = format.parse(r#"[1, 2, 3]"#);

        assert_eq!(results.len(), 1);
        if let CoreValue::Json(json) = &results[0].value {
            assert!(json.is_array());
        } else {
            panic!("Expected Json");
        }
    }

    #[test]
    fn test_not_json() {
        let format = JsonFormat;
        assert!(format.parse("hello").is_empty());
        assert!(format.parse("123").is_empty());
    }

    #[test]
    fn test_format_json() {
        let format = JsonFormat;
        let value = CoreValue::Json(serde_json::json!({"key": "value"}));
        let formatted = format.format(&value).unwrap();

        assert!(formatted.contains("key"));
        assert!(formatted.contains("value"));
    }
}
