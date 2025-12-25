//! UUID format.

use uuid::Uuid;

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation};

pub struct UuidFormat;

impl Format for UuidFormat {
    fn id(&self) -> &'static str {
        "uuid"
    }

    fn name(&self) -> &'static str {
        "UUID"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Identifiers",
            description: "UUID parsing with version detection (v1-v8)",
            examples: &[
                "550e8400-e29b-41d4-a716-446655440000",
                "550e8400e29b41d4a716446655440000",
            ],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Ok(uuid) = Uuid::parse_str(input) else {
            return vec![];
        };

        let version_desc = match uuid.get_version() {
            Some(uuid::Version::Nil) => "NIL UUID",
            Some(uuid::Version::Mac) => "UUID v1 (MAC address + timestamp)",
            Some(uuid::Version::Dce) => "UUID v2 (DCE)",
            Some(uuid::Version::Md5) => "UUID v3 (MD5 hash)",
            Some(uuid::Version::Random) => "UUID v4 (random)",
            Some(uuid::Version::Sha1) => "UUID v5 (SHA-1 hash)",
            Some(uuid::Version::SortMac) => "UUID v6 (sortable MAC)",
            Some(uuid::Version::SortRand) => "UUID v7 (sortable random)",
            Some(uuid::Version::Custom) => "UUID v8 (custom)",
            Some(uuid::Version::Max) => "MAX UUID",
            _ => "UUID",
        };

        vec![Interpretation {
            value: CoreValue::Bytes(uuid.as_bytes().to_vec()),
            source_format: "uuid".to_string(),
            confidence: 0.95,
            description: version_desc.to_string(),
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        match value {
            CoreValue::Bytes(bytes) => bytes.len() == 16,
            _ => false,
        }
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) if bytes.len() == 16 => {
                let uuid = Uuid::from_slice(bytes).ok()?;
                Some(uuid.to_string())
            }
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        if bytes.len() != 16 {
            return vec![];
        }

        let Ok(uuid) = Uuid::from_slice(bytes) else {
            return vec![];
        };

        vec![Conversion {
            value: CoreValue::String(uuid.to_string()),
            target_format: "uuid".to_string(),
            display: uuid.to_string(),
            path: vec!["uuid".to_string()],
            is_lossy: false,
            steps: vec![],
            priority: ConversionPriority::Semantic,
            display_only: false,
            kind: ConversionKind::default(),
            metadata: None,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["guid"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uuid_v4() {
        let format = UuidFormat;
        let results = format.parse("550e8400-e29b-41d4-a716-446655440000");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "uuid");
        assert!(results[0].description.contains("v4"));
    }

    #[test]
    fn test_parse_uuid_without_dashes() {
        let format = UuidFormat;
        let results = format.parse("550e8400e29b41d4a716446655440000");

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_format_bytes_to_uuid() {
        let format = UuidFormat;
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let value = CoreValue::Bytes(uuid.as_bytes().to_vec());

        let formatted = format.format(&value).unwrap();
        assert_eq!(formatted, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_invalid_uuid() {
        let format = UuidFormat;
        assert!(format.parse("not-a-uuid").is_empty());
        assert!(format.parse("550e8400-e29b-41d4-a716").is_empty());
    }
}
