//! JSON format.

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

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
            aliases: self.aliases(),
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
            rich_display: vec![],
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

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        match value {
            CoreValue::Json(json) => {
                // Only offer formatting if the original might have been minified
                // (i.e., the JSON has some structure worth formatting)
                if !json.is_object() && !json.is_array() {
                    return vec![];
                }

                let formatted = serde_json::to_string_pretty(json).unwrap_or_default();

                vec![Conversion {
                    value: CoreValue::Json(json.clone()),
                    target_format: "json-formatted".to_string(),
                    display: formatted.clone(),
                    path: vec!["json-formatted".to_string()],
                    steps: vec![ConversionStep {
                        format: "json-formatted".to_string(),
                        value: CoreValue::Json(json.clone()),
                        display: formatted,
                    }],
                    is_lossy: false,
                    priority: ConversionPriority::Structured,
                    display_only: false,
                    kind: ConversionKind::default(),
                    hidden: false,
                    rich_display: vec![],
                }]
            }
            CoreValue::String(s) => {
                // Try to parse string as JSON (enables hex → utf8 → json chain)
                let trimmed = s.trim();
                if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                    return vec![];
                }

                let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                    return vec![];
                };

                // Only convert objects and arrays
                if !json.is_object() && !json.is_array() {
                    return vec![];
                }

                let formatted = serde_json::to_string_pretty(&json).unwrap_or_default();

                vec![Conversion {
                    value: CoreValue::Json(json.clone()),
                    target_format: "json".to_string(),
                    display: formatted.clone(),
                    path: vec!["json".to_string()],
                    steps: vec![ConversionStep {
                        format: "json".to_string(),
                        value: CoreValue::Json(json),
                        display: formatted,
                    }],
                    is_lossy: false,
                    priority: ConversionPriority::Structured,
                    display_only: true, // Don't explore further from JSON
                    kind: ConversionKind::default(),
                    hidden: false,
                    rich_display: vec![],
                }]
            }
            _ => vec![],
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["j"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        let trimmed = input.trim();

        // Check if it looks like JSON (starts with { or [)
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            return Some("JSON must start with '{' or '['".to_string());
        }

        // Try to parse and return the specific error
        match serde_json::from_str::<serde_json::Value>(input) {
            Ok(_) => None, // Valid JSON
            Err(e) => Some(format!("line {}, column {}: {}", e.line(), e.column(), e)),
        }
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
