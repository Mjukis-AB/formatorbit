//! Integer formats (decimal, with endianness handling).

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, CoreValue, Interpretation};

pub struct DecimalFormat;

impl Format for DecimalFormat {
    fn id(&self) -> &'static str {
        "decimal"
    }

    fn name(&self) -> &'static str {
        "Decimal Integer"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Numbers",
            description: "Decimal integer parsing",
            examples: &["1763574200", "-42", "255"],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Ok(value) = input.parse::<i128>() else {
            return vec![];
        };

        // Higher confidence for pure numeric input
        let confidence = if input.starts_with('-') || input.starts_with('+') {
            0.9
        } else if input.chars().all(|c| c.is_ascii_digit()) {
            0.85
        } else {
            0.5
        };

        vec![Interpretation {
            value: CoreValue::Int {
                value,
                original_bytes: None,
            },
            source_format: "decimal".to_string(),
            confidence,
            description: format!("Integer: {value}"),
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Int { .. })
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Int { value, .. } => Some(value.to_string()),
            _ => None,
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["dec", "int", "num"]
    }
}

/// Converts bytes to integers (both endianness).
pub struct BytesToIntFormat;

impl BytesToIntFormat {
    /// Convert bytes to i128 (big-endian).
    fn bytes_to_int_be(bytes: &[u8]) -> i128 {
        let mut result: i128 = 0;
        for &b in bytes {
            result = (result << 8) | (b as i128);
        }
        result
    }

    /// Convert bytes to i128 (little-endian).
    fn bytes_to_int_le(bytes: &[u8]) -> i128 {
        let mut result: i128 = 0;
        for (i, &b) in bytes.iter().enumerate() {
            result |= (b as i128) << (i * 8);
        }
        result
    }
}

impl Format for BytesToIntFormat {
    fn id(&self) -> &'static str {
        "bytes-to-int"
    }

    fn name(&self) -> &'static str {
        "Bytes to Integer"
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // This format doesn't parse strings directly
        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Only convert reasonable byte lengths (1-16 bytes)
        if bytes.is_empty() || bytes.len() > 16 {
            return vec![];
        }

        let be_value = Self::bytes_to_int_be(bytes);
        let le_value = Self::bytes_to_int_le(bytes);

        let mut conversions = vec![
            Conversion {
                value: CoreValue::Int {
                    value: be_value,
                    original_bytes: Some(bytes.clone()),
                },
                target_format: "int-be".to_string(),
                display: be_value.to_string(),
                path: vec!["int-be".to_string()],
                is_lossy: false,
            },
        ];

        // Only add little-endian if it's different
        if le_value != be_value {
            conversions.push(Conversion {
                value: CoreValue::Int {
                    value: le_value,
                    original_bytes: Some(bytes.clone()),
                },
                target_format: "int-le".to_string(),
                display: le_value.to_string(),
                path: vec!["int-le".to_string()],
                is_lossy: false,
            });
        }

        conversions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal() {
        let format = DecimalFormat;
        let results = format.parse("1763574200");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1763574200);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_negative() {
        let format = DecimalFormat;
        let results = format.parse("-42");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, -42);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_bytes_to_int_be() {
        let bytes = vec![0x69, 0x1E, 0x01, 0xB8];
        let value = BytesToIntFormat::bytes_to_int_be(&bytes);
        assert_eq!(value, 1763574200);
    }

    #[test]
    fn test_bytes_to_int_le() {
        let bytes = vec![0x69, 0x1E, 0x01, 0xB8];
        let value = BytesToIntFormat::bytes_to_int_le(&bytes);
        // LE: bytes reversed = 0xB8011E69 = 3087081065
        assert_eq!(value, 3087081065);
    }

    #[test]
    fn test_bytes_to_int_conversions() {
        let format = BytesToIntFormat;
        let value = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 2);

        let be = conversions.iter().find(|c| c.target_format == "int-be").unwrap();
        assert_eq!(be.display, "1763574200");

        let le = conversions.iter().find(|c| c.target_format == "int-le").unwrap();
        assert_eq!(le.display, "3087081065");
    }
}
