//! Apple plist format (XML and binary).

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

pub struct PlistFormat;

impl PlistFormat {
    /// Convert plist::Value to serde_json::Value for display.
    fn plist_to_json(value: &plist::Value) -> serde_json::Value {
        match value {
            plist::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::plist_to_json).collect())
            }
            plist::Value::Dictionary(dict) => {
                let map: serde_json::Map<String, serde_json::Value> = dict
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::plist_to_json(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
            plist::Value::Boolean(b) => serde_json::Value::Bool(*b),
            plist::Value::Data(data) => {
                // Encode binary data as base64 string
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(data);
                serde_json::Value::String(format!(
                    "<data: {} bytes, base64: {}>",
                    data.len(),
                    encoded
                ))
            }
            plist::Value::Date(date) => serde_json::Value::String(date.to_xml_format()),
            plist::Value::Integer(i) => {
                if let Some(n) = i.as_signed() {
                    serde_json::Value::Number(n.into())
                } else if let Some(n) = i.as_unsigned() {
                    serde_json::Value::Number(n.into())
                } else {
                    serde_json::Value::Null
                }
            }
            plist::Value::Real(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            plist::Value::String(s) => serde_json::Value::String(s.clone()),
            plist::Value::Uid(uid) => serde_json::Value::String(format!("<UID: {}>", uid.get())),
            _ => serde_json::Value::Null,
        }
    }

    /// Check if input looks like XML plist.
    fn looks_like_xml_plist(input: &str) -> bool {
        let trimmed = input.trim();
        // Check for XML declaration + plist, or just plist tag
        (trimmed.starts_with("<?xml") && trimmed.contains("<plist"))
            || trimmed.starts_with("<plist")
            || trimmed.starts_with("<!DOCTYPE plist")
    }

    /// Check if bytes look like binary plist (magic bytes).
    fn is_binary_plist(bytes: &[u8]) -> bool {
        bytes.len() >= 8 && &bytes[0..6] == b"bplist"
    }
}

impl Format for PlistFormat {
    fn id(&self) -> &'static str {
        "plist"
    }

    fn name(&self) -> &'static str {
        "Apple plist"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "Apple property list (XML and binary formats)",
            examples: &["<?xml...<plist>...</plist>", "bplist00..."],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Only try to parse if it looks like XML plist
        if !Self::looks_like_xml_plist(input) {
            return vec![];
        }

        // Try to parse as plist
        let Ok(value) = plist::from_bytes::<plist::Value>(input.as_bytes()) else {
            return vec![];
        };

        let json = Self::plist_to_json(&value);

        // Determine confidence based on how clear the plist markers are
        let confidence = if input.trim().starts_with("<?xml") && input.contains("<!DOCTYPE plist") {
            0.95
        } else if input.trim().starts_with("<?xml") || input.contains("<plist") {
            0.92
        } else {
            0.85
        };

        vec![Interpretation {
            value: CoreValue::Json(json),
            source_format: "plist".to_string(),
            confidence,
            description: "XML plist".to_string(),
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Plist only decodes TO Json, it doesn't encode FROM Json
        // (Json is a terminal format)
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        // We don't format to plist - only decode from plist bytes
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Check for binary plist magic bytes
        if !Self::is_binary_plist(bytes) {
            return vec![];
        }

        // Try to decode binary plist
        let Ok(plist_value) = plist::from_bytes::<plist::Value>(bytes) else {
            return vec![];
        };

        let json = Self::plist_to_json(&plist_value);
        let display = serde_json::to_string_pretty(&json).unwrap_or_default();

        vec![Conversion {
            value: CoreValue::Json(json),
            target_format: "plist".to_string(),
            display: format!("(decoded) {}", display),
            path: vec!["plist".to_string()],
            is_lossy: false,
            steps: vec![],
            priority: ConversionPriority::Structured,
            terminal: false,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["pl"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xml_plist() {
        let format = PlistFormat;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Name</key>
    <string>Test</string>
    <key>Count</key>
    <integer>42</integer>
</dict>
</plist>"#;

        let results = format.parse(xml);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "plist");
        assert!(results[0].confidence > 0.9);

        if let CoreValue::Json(json) = &results[0].value {
            assert_eq!(json["Name"], "Test");
            assert_eq!(json["Count"], 42);
        } else {
            panic!("Expected Json");
        }
    }

    #[test]
    fn test_parse_minimal_xml_plist() {
        let format = PlistFormat;
        let xml = r#"<plist version="1.0">
<dict>
    <key>key</key>
    <string>value</string>
</dict>
</plist>"#;

        let results = format.parse(xml);

        assert_eq!(results.len(), 1);
        if let CoreValue::Json(json) = &results[0].value {
            assert_eq!(json["key"], "value");
        } else {
            panic!("Expected Json");
        }
    }

    #[test]
    fn test_decode_binary_plist() {
        let format = PlistFormat;

        // Create a simple binary plist
        let dict = plist::Dictionary::from_iter([
            ("name".to_string(), plist::Value::String("test".to_string())),
            ("number".to_string(), plist::Value::Integer(123.into())),
        ]);
        let plist_value = plist::Value::Dictionary(dict);

        let mut bytes = Vec::new();
        plist::to_writer_binary(&mut bytes, &plist_value).unwrap();

        // Verify it has the magic bytes
        assert!(PlistFormat::is_binary_plist(&bytes));

        let value = CoreValue::Bytes(bytes);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "plist");
        assert_eq!(conversions[0].priority, ConversionPriority::Structured);
        assert!(conversions[0].display.contains("name"));
        assert!(conversions[0].display.contains("test"));
    }

    #[test]
    fn test_not_plist() {
        let format = PlistFormat;
        // Plain text
        assert!(format.parse("hello world").is_empty());
        // JSON (not plist)
        assert!(format.parse(r#"{"key": "value"}"#).is_empty());
        // Random XML (not plist)
        assert!(format.parse("<html><body>test</body></html>").is_empty());
    }

    #[test]
    fn test_binary_plist_magic_detection() {
        assert!(PlistFormat::is_binary_plist(b"bplist00rest"));
        assert!(PlistFormat::is_binary_plist(b"bplist01rest"));
        assert!(!PlistFormat::is_binary_plist(b"notplist"));
        assert!(!PlistFormat::is_binary_plist(b"bplis")); // Too short
    }

    #[test]
    fn test_plist_with_array() {
        let format = PlistFormat;
        let xml = r#"<?xml version="1.0"?>
<plist version="1.0">
<array>
    <string>one</string>
    <string>two</string>
    <integer>3</integer>
</array>
</plist>"#;

        let results = format.parse(xml);
        assert_eq!(results.len(), 1);

        if let CoreValue::Json(json) = &results[0].value {
            assert!(json.is_array());
            let arr = json.as_array().unwrap();
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], "one");
            assert_eq!(arr[1], "two");
            assert_eq!(arr[2], 3);
        } else {
            panic!("Expected Json");
        }
    }

    #[test]
    fn test_plist_does_not_format_json() {
        // Plist is decode-only - it doesn't encode JSON to plist
        let format = PlistFormat;
        let json = serde_json::json!({"name": "test", "count": 42});
        let value = CoreValue::Json(json);

        // format() should return None since we disabled JSON→plist encoding
        assert!(format.format(&value).is_none());
        assert!(!format.can_format(&value));
    }
}
