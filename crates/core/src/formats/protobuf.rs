//! Protobuf wire format decoder (schema-less).
//!
//! Decodes protobuf binary data without requiring a .proto schema file.
//! Shows field numbers, wire types, and raw values.
//!
//! Wire types:
//! - 0: VARINT (int32, int64, uint32, uint64, sint32, sint64, bool, enum)
//! - 1: I64 (fixed64, sfixed64, double)
//! - 2: LEN (string, bytes, embedded messages, packed repeated)
//! - 3: SGROUP (deprecated)
//! - 4: EGROUP (deprecated)
//! - 5: I32 (fixed32, sfixed32, float)

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionPriority, CoreValue, ProtoField as PublicProtoField,
    ProtoValue as PublicProtoValue,
};

pub struct ProtobufFormat;

/// Internal decoded protobuf value (before conversion to public types).
#[derive(Debug, Clone)]
enum ProtoValue {
    Varint(u64),
    Fixed64(u64),
    Fixed32(u32),
    LengthDelimited(Vec<u8>),
    /// Nested message (successfully parsed as protobuf)
    Message(Vec<ProtoField>),
}

/// Internal field with its number and value.
#[derive(Debug, Clone)]
struct ProtoField {
    field_number: u32,
    wire_type: u8,
    value: ProtoValue,
}

impl ProtobufFormat {
    /// Decode a varint from bytes, returning (value, bytes_consumed).
    fn decode_varint(bytes: &[u8]) -> Option<(u64, usize)> {
        let mut result: u64 = 0;
        let mut shift = 0;

        for (i, &byte) in bytes.iter().enumerate() {
            if shift >= 64 {
                return None; // Overflow
            }

            let value = (byte & 0x7F) as u64;
            result |= value << shift;
            shift += 7;

            if byte & 0x80 == 0 {
                return Some((result, i + 1));
            }

            if i >= 9 {
                // Varints are at most 10 bytes
                return None;
            }
        }

        None // Incomplete varint
    }

    /// Decode zigzag-encoded signed integer.
    fn decode_zigzag(n: u64) -> i64 {
        ((n >> 1) as i64) ^ (-((n & 1) as i64))
    }

    /// Try to decode bytes as a protobuf message.
    /// Returns None if it doesn't look like valid protobuf.
    fn decode_message(bytes: &[u8]) -> Option<Vec<ProtoField>> {
        let mut fields = Vec::new();
        let mut pos = 0;

        while pos < bytes.len() {
            // Decode tag (field_number << 3 | wire_type)
            let (tag, tag_len) = Self::decode_varint(&bytes[pos..])?;
            pos += tag_len;

            let wire_type = (tag & 0x07) as u8;
            let field_number = (tag >> 3) as u32;

            // Field number 0 is invalid
            if field_number == 0 {
                return None;
            }

            // Very high field numbers are suspicious
            if field_number > 536_870_911 {
                // Max allowed field number
                return None;
            }

            let value = match wire_type {
                0 => {
                    // VARINT
                    let (v, len) = Self::decode_varint(&bytes[pos..])?;
                    pos += len;
                    ProtoValue::Varint(v)
                }
                1 => {
                    // I64 (8 bytes, little-endian)
                    if pos + 8 > bytes.len() {
                        return None;
                    }
                    let v = u64::from_le_bytes(bytes[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    ProtoValue::Fixed64(v)
                }
                2 => {
                    // LEN (length-prefixed)
                    let (len, len_bytes) = Self::decode_varint(&bytes[pos..])?;
                    pos += len_bytes;

                    let len = len as usize;
                    if pos + len > bytes.len() {
                        return None;
                    }

                    let data = bytes[pos..pos + len].to_vec();
                    pos += len;

                    // Try to recursively decode as nested message
                    if let Some(nested) = Self::decode_message(&data) {
                        if !nested.is_empty() {
                            ProtoValue::Message(nested)
                        } else {
                            ProtoValue::LengthDelimited(data)
                        }
                    } else {
                        ProtoValue::LengthDelimited(data)
                    }
                }
                3 | 4 => {
                    // SGROUP/EGROUP (deprecated, rarely used)
                    return None;
                }
                5 => {
                    // I32 (4 bytes, little-endian)
                    if pos + 4 > bytes.len() {
                        return None;
                    }
                    let v = u32::from_le_bytes(bytes[pos..pos + 4].try_into().ok()?);
                    pos += 4;
                    ProtoValue::Fixed32(v)
                }
                _ => {
                    // Unknown wire type
                    return None;
                }
            };

            fields.push(ProtoField {
                field_number,
                wire_type,
                value,
            });
        }

        Some(fields)
    }

    /// Format a ProtoValue for display.
    fn format_value(value: &ProtoValue, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match value {
            ProtoValue::Varint(v) => {
                // Show both unsigned and signed (zigzag) interpretation
                let signed = Self::decode_zigzag(*v);
                if *v <= 1 {
                    // Could be bool
                    format!("{v} (bool: {})", *v != 0)
                } else if signed.abs() < (*v as i64).abs() / 2 {
                    // Zigzag makes it smaller, likely signed
                    format!("{v} (signed: {signed})")
                } else {
                    format!("{v}")
                }
            }
            ProtoValue::Fixed64(v) => {
                // Could be double
                let as_double = f64::from_bits(*v);
                if as_double.is_finite() && as_double.abs() > 1e-100 && as_double.abs() < 1e100 {
                    format!("{v} (double: {as_double})")
                } else {
                    format!("{v}")
                }
            }
            ProtoValue::Fixed32(v) => {
                // Could be float
                let as_float = f32::from_bits(*v);
                if as_float.is_finite() && as_float.abs() > 1e-30 && as_float.abs() < 1e30 {
                    format!("{v} (float: {as_float})")
                } else {
                    format!("{v}")
                }
            }
            ProtoValue::LengthDelimited(data) => {
                // Try to interpret as UTF-8 string
                if let Ok(s) = std::str::from_utf8(data) {
                    if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                        return format!("\"{s}\"");
                    }
                }
                // Show as hex if short, otherwise length
                if data.len() <= 32 {
                    let hex: String = data.iter().map(|b| format!("{b:02x}")).collect();
                    format!("bytes[{}]: {hex}", data.len())
                } else {
                    format!("bytes[{}]", data.len())
                }
            }
            ProtoValue::Message(fields) => {
                let mut lines = vec!["{".to_string()];
                for field in fields {
                    let wire_name = match field.wire_type {
                        0 => "varint",
                        1 => "i64",
                        2 => "len",
                        5 => "i32",
                        _ => "?",
                    };
                    let val = Self::format_value(&field.value, indent + 1);
                    lines.push(format!(
                        "{pad}  {}: {} [{}]",
                        field.field_number, val, wire_name
                    ));
                }
                lines.push(format!("{pad}}}"));
                lines.join("\n")
            }
        }
    }

    /// Calculate a confidence score based on how protobuf-like the data is.
    fn calculate_confidence(fields: &[ProtoField], byte_len: usize) -> f32 {
        if fields.is_empty() {
            return 0.0;
        }

        let mut score: f32 = 0.5;

        // Multiple fields increase confidence
        if fields.len() >= 2 {
            score += 0.15;
        }
        if fields.len() >= 4 {
            score += 0.10;
        }

        // Sequential field numbers (1, 2, 3...) are very common
        let has_sequential = fields
            .windows(2)
            .any(|w| w[1].field_number == w[0].field_number + 1);
        if has_sequential {
            score += 0.10;
        }

        // Low field numbers (1-15) are more common
        let low_fields = fields.iter().filter(|f| f.field_number <= 15).count();
        if low_fields == fields.len() {
            score += 0.10;
        }

        // Nested messages are strong signal
        let has_nested = fields
            .iter()
            .any(|f| matches!(f.value, ProtoValue::Message(_)));
        if has_nested {
            score += 0.15;
        }

        // Strings in length-delimited fields
        let has_strings = fields.iter().any(|f| {
            if let ProtoValue::LengthDelimited(data) = &f.value {
                if let Ok(s) = std::str::from_utf8(data) {
                    return s.len() >= 3 && s.chars().all(|c| !c.is_control());
                }
            }
            false
        });
        if has_strings {
            score += 0.10;
        }

        // Very short data with single field is suspicious
        if byte_len <= 4 && fields.len() == 1 {
            score -= 0.20;
        }

        score.clamp(0.3, 0.95)
    }

    /// Convert internal ProtoField to public ProtoField.
    fn to_public_field(field: &ProtoField) -> PublicProtoField {
        PublicProtoField {
            field_number: field.field_number,
            wire_type: field.wire_type,
            value: Self::to_public_value(&field.value),
        }
    }

    /// Convert internal ProtoValue to public ProtoValue.
    fn to_public_value(value: &ProtoValue) -> PublicProtoValue {
        match value {
            ProtoValue::Varint(v) => PublicProtoValue::Varint(*v),
            ProtoValue::Fixed64(v) => PublicProtoValue::Fixed64(*v),
            ProtoValue::Fixed32(v) => PublicProtoValue::Fixed32(*v),
            ProtoValue::LengthDelimited(data) => {
                // Try to interpret as UTF-8 string
                if let Ok(s) = std::str::from_utf8(data) {
                    if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                        return PublicProtoValue::String(s.to_string());
                    }
                }
                PublicProtoValue::Bytes(data.clone())
            }
            ProtoValue::Message(fields) => {
                PublicProtoValue::Message(fields.iter().map(Self::to_public_field).collect())
            }
        }
    }

    /// Convert internal fields to public fields.
    fn to_public_fields(fields: &[ProtoField]) -> Vec<PublicProtoField> {
        fields.iter().map(Self::to_public_field).collect()
    }
}

impl Format for ProtobufFormat {
    fn id(&self) -> &'static str {
        "protobuf"
    }

    fn name(&self) -> &'static str {
        "Protocol Buffers"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "Protocol Buffers wire format (schema-less decode)",
            examples: &[],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, _input: &str) -> Vec<crate::types::Interpretation> {
        // Protobuf is binary, parsed via conversions from bytes
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

        // Need at least 2 bytes for a minimal protobuf message
        if bytes.len() < 2 {
            return vec![];
        }

        // Try to decode as protobuf
        let Some(fields) = Self::decode_message(bytes) else {
            return vec![];
        };

        if fields.is_empty() {
            return vec![];
        }

        let confidence = Self::calculate_confidence(&fields, bytes.len());

        // Skip if too unlikely
        if confidence < 0.4 {
            return vec![];
        }

        // Convert to public types
        let public_fields = Self::to_public_fields(&fields);

        // Format for display (fallback for non-colorized output)
        let mut display = String::from("{\n");
        for field in &fields {
            let wire_name = match field.wire_type {
                0 => "varint",
                1 => "i64",
                2 => "len",
                5 => "i32",
                _ => "?",
            };
            let val = Self::format_value(&field.value, 0);
            display.push_str(&format!(
                "  {}: {} [{}]\n",
                field.field_number, val, wire_name
            ));
        }
        display.push('}');

        let priority = if confidence >= 0.7 {
            ConversionPriority::Structured
        } else {
            ConversionPriority::Raw
        };

        vec![Conversion {
            value: CoreValue::Protobuf(public_fields),
            target_format: "protobuf".to_string(),
            display,
            path: vec!["protobuf".to_string()],
            is_lossy: false,
            steps: vec![],
            priority,
            display_only: false,
            metadata: None,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["proto", "pb"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_varint() {
        // 150 = 0x96 0x01
        assert_eq!(ProtobufFormat::decode_varint(&[0x96, 0x01]), Some((150, 2)));

        // 1 = 0x01
        assert_eq!(ProtobufFormat::decode_varint(&[0x01]), Some((1, 1)));

        // 300 = 0xAC 0x02
        assert_eq!(ProtobufFormat::decode_varint(&[0xAC, 0x02]), Some((300, 2)));
    }

    #[test]
    fn test_decode_zigzag() {
        assert_eq!(ProtobufFormat::decode_zigzag(0), 0);
        assert_eq!(ProtobufFormat::decode_zigzag(1), -1);
        assert_eq!(ProtobufFormat::decode_zigzag(2), 1);
        assert_eq!(ProtobufFormat::decode_zigzag(3), -2);
        assert_eq!(ProtobufFormat::decode_zigzag(4294967294), 2147483647);
        assert_eq!(ProtobufFormat::decode_zigzag(4294967295), -2147483648);
    }

    #[test]
    fn test_decode_simple_message() {
        // Field 1, varint, value 150 (from protobuf docs example)
        // tag: 08 (field 1, wire type 0)
        // value: 96 01 (150 in varint)
        let bytes = vec![0x08, 0x96, 0x01];
        let fields = ProtobufFormat::decode_message(&bytes).unwrap();

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_number, 1);
        assert_eq!(fields[0].wire_type, 0);
        if let ProtoValue::Varint(v) = fields[0].value {
            assert_eq!(v, 150);
        } else {
            panic!("Expected varint");
        }
    }

    #[test]
    fn test_decode_string_field() {
        // Field 2, length-delimited, value "testing"
        // tag: 12 (field 2, wire type 2)
        // length: 07
        // value: "testing"
        let bytes = vec![0x12, 0x07, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67];
        let fields = ProtobufFormat::decode_message(&bytes).unwrap();

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_number, 2);
        assert_eq!(fields[0].wire_type, 2);
        if let ProtoValue::LengthDelimited(data) = &fields[0].value {
            assert_eq!(std::str::from_utf8(data).unwrap(), "testing");
        } else {
            panic!("Expected length-delimited");
        }
    }

    #[test]
    fn test_decode_multiple_fields() {
        // Field 1: varint 150
        // Field 2: string "testing"
        let bytes = vec![
            0x08, 0x96, 0x01, // field 1 = 150
            0x12, 0x07, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, // field 2 = "testing"
        ];
        let fields = ProtobufFormat::decode_message(&bytes).unwrap();

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].field_number, 1);
        assert_eq!(fields[1].field_number, 2);
    }

    #[test]
    fn test_conversions_from_bytes() {
        let format = ProtobufFormat;

        // Multiple fields with sequential numbers and string
        let bytes = vec![
            0x08, 0x96, 0x01, // field 1 = 150
            0x12, 0x07, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, // field 2 = "testing"
        ];
        let value = CoreValue::Bytes(bytes);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "protobuf");
        // Value should be CoreValue::Protobuf with the decoded fields
        if let CoreValue::Protobuf(fields) = &conversions[0].value {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].field_number, 1);
            assert_eq!(fields[1].field_number, 2);
        } else {
            panic!("Expected CoreValue::Protobuf");
        }
    }

    #[test]
    fn test_invalid_protobuf() {
        // Random bytes that don't form valid protobuf
        let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
        assert!(ProtobufFormat::decode_message(&bytes).is_none());
    }

    #[test]
    fn test_field_number_zero_invalid() {
        // Field number 0 is invalid
        let bytes = vec![0x00, 0x01]; // tag 0 = field 0, wire type 0
        assert!(ProtobufFormat::decode_message(&bytes).is_none());
    }
}
