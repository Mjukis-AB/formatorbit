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
use crate::truncate_str;
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, PacketSegment,
    ProtoField as PublicProtoField, ProtoValue as PublicProtoValue, RichDisplay, RichDisplayOption,
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

/// Internal field with its number, value, and byte positions.
#[derive(Debug, Clone)]
struct ProtoField {
    field_number: u32,
    wire_type: u8,
    value: ProtoValue,
    /// Byte offset where this field's tag starts.
    tag_offset: usize,
    /// Length of the tag in bytes.
    tag_length: usize,
    /// Byte offset where the value starts.
    value_offset: usize,
    /// Length of the value in bytes.
    value_length: usize,
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
        Self::decode_message_at_offset(bytes, 0)
    }

    /// Decode protobuf message, tracking offsets relative to a base offset.
    /// The base_offset is added to all positions for nested message tracking.
    fn decode_message_at_offset(bytes: &[u8], base_offset: usize) -> Option<Vec<ProtoField>> {
        let mut fields = Vec::new();
        let mut pos = 0;

        while pos < bytes.len() {
            let tag_offset = base_offset + pos;

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

            let value_offset = base_offset + pos;

            let (value, value_length) = match wire_type {
                0 => {
                    // VARINT
                    let (v, len) = Self::decode_varint(&bytes[pos..])?;
                    pos += len;
                    (ProtoValue::Varint(v), len)
                }
                1 => {
                    // I64 (8 bytes, little-endian)
                    if pos + 8 > bytes.len() {
                        return None;
                    }
                    let v = u64::from_le_bytes(bytes[pos..pos + 8].try_into().ok()?);
                    pos += 8;
                    (ProtoValue::Fixed64(v), 8)
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
                    let data_start = base_offset + pos;
                    pos += len;

                    // Try to recursively decode as nested message
                    let value =
                        if let Some(nested) = Self::decode_message_at_offset(&data, data_start) {
                            if !nested.is_empty() {
                                ProtoValue::Message(nested)
                            } else {
                                ProtoValue::LengthDelimited(data)
                            }
                        } else {
                            ProtoValue::LengthDelimited(data)
                        };
                    // Value length includes the length prefix + data
                    (value, len_bytes + len)
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
                    (ProtoValue::Fixed32(v), 4)
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
                tag_offset,
                tag_length: tag_len,
                value_offset,
                value_length,
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

    /// Get wire type name for display.
    fn wire_type_name(wire_type: u8) -> &'static str {
        match wire_type {
            0 => "varint",
            1 => "i64",
            2 => "len",
            5 => "i32",
            _ => "?",
        }
    }

    /// Build packet segments from decoded fields.
    fn build_segments(bytes: &[u8], fields: &[ProtoField]) -> Vec<PacketSegment> {
        let mut segments = Vec::new();

        for field in fields {
            // Tag segment
            let tag_bytes = bytes
                .get(field.tag_offset..field.tag_offset + field.tag_length)
                .map(|b| b.to_vec())
                .unwrap_or_default();
            segments.push(PacketSegment {
                offset: field.tag_offset,
                length: field.tag_length,
                bytes: tag_bytes,
                segment_type: "tag".to_string(),
                label: format!("tag{}", Self::subscript(field.field_number)),
                decoded: format!(
                    "field {}, {}",
                    field.field_number,
                    Self::wire_type_name(field.wire_type)
                ),
                children: vec![],
            });

            // Value segment
            let value_segment = Self::build_value_segment(bytes, field);
            segments.push(value_segment);
        }

        segments
    }

    /// Build a segment for a field's value.
    fn build_value_segment(bytes: &[u8], field: &ProtoField) -> PacketSegment {
        let value_bytes = bytes
            .get(field.value_offset..field.value_offset + field.value_length)
            .map(|b| b.to_vec())
            .unwrap_or_default();

        match &field.value {
            ProtoValue::Varint(v) => {
                let signed = Self::decode_zigzag(*v);
                let decoded = if *v <= 1 {
                    format!("{} (bool: {})", v, *v != 0)
                } else if signed.abs() < (*v as i64).abs() / 2 {
                    format!("{} (signed: {})", v, signed)
                } else {
                    v.to_string()
                };
                PacketSegment {
                    offset: field.value_offset,
                    length: field.value_length,
                    bytes: value_bytes,
                    segment_type: "varint".to_string(),
                    label: format!("field {}", field.field_number),
                    decoded,
                    children: vec![],
                }
            }
            ProtoValue::Fixed64(v) => {
                let as_double = f64::from_bits(*v);
                let decoded =
                    if as_double.is_finite() && as_double.abs() > 1e-100 && as_double.abs() < 1e100
                    {
                        format!("{} (double: {})", v, as_double)
                    } else {
                        v.to_string()
                    };
                PacketSegment {
                    offset: field.value_offset,
                    length: field.value_length,
                    bytes: value_bytes,
                    segment_type: "i64".to_string(),
                    label: format!("field {}", field.field_number),
                    decoded,
                    children: vec![],
                }
            }
            ProtoValue::Fixed32(v) => {
                let as_float = f32::from_bits(*v);
                let decoded =
                    if as_float.is_finite() && as_float.abs() > 1e-30 && as_float.abs() < 1e30 {
                        format!("{} (float: {})", v, as_float)
                    } else {
                        v.to_string()
                    };
                PacketSegment {
                    offset: field.value_offset,
                    length: field.value_length,
                    bytes: value_bytes,
                    segment_type: "i32".to_string(),
                    label: format!("field {}", field.field_number),
                    decoded,
                    children: vec![],
                }
            }
            ProtoValue::LengthDelimited(data) => {
                // For length-delimited, we need to show length prefix separately
                // The value_bytes includes length prefix + data
                let (len_val, len_bytes_count) =
                    Self::decode_varint(&value_bytes).unwrap_or((data.len() as u64, 1));

                let decoded = if let Ok(s) = std::str::from_utf8(data) {
                    if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                        format!("\"{}\"", s)
                    } else if data.len() <= 16 {
                        let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
                        format!("bytes[{}]: {}", data.len(), hex)
                    } else {
                        format!("bytes[{}]", data.len())
                    }
                } else if data.len() <= 16 {
                    let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
                    format!("bytes[{}]: {}", data.len(), hex)
                } else {
                    format!("bytes[{}]", data.len())
                };

                PacketSegment {
                    offset: field.value_offset,
                    length: len_bytes_count,
                    bytes: value_bytes[..len_bytes_count].to_vec(),
                    segment_type: "len".to_string(),
                    label: format!("len={}", len_val),
                    decoded: decoded.clone(),
                    children: vec![PacketSegment {
                        offset: field.value_offset + len_bytes_count,
                        length: data.len(),
                        bytes: data.clone(),
                        segment_type: "string".to_string(),
                        label: format!("field {}", field.field_number),
                        decoded,
                        children: vec![],
                    }],
                }
            }
            ProtoValue::Message(nested_fields) => {
                // For nested messages, recursively build child segments
                let (len_val, len_bytes_count) =
                    Self::decode_varint(&value_bytes).unwrap_or((0, 1));
                let data_len = field.value_length - len_bytes_count;

                // Get the nested message bytes
                let nested_bytes = bytes
                    .get(
                        field.value_offset + len_bytes_count
                            ..field.value_offset + field.value_length,
                    )
                    .unwrap_or(&[]);

                let children = Self::build_segments(nested_bytes, nested_fields);

                PacketSegment {
                    offset: field.value_offset,
                    length: len_bytes_count,
                    bytes: value_bytes[..len_bytes_count].to_vec(),
                    segment_type: "len".to_string(),
                    label: format!("len={}", len_val),
                    decoded: format!("message[{}]", data_len),
                    children,
                }
            }
        }
    }

    /// Format segments in compact inline style.
    fn format_compact(segments: &[PacketSegment]) -> String {
        Self::format_compact_recursive(segments, false)
    }

    fn format_compact_recursive(segments: &[PacketSegment], _is_child: bool) -> String {
        let mut parts = Vec::new();

        for seg in segments {
            let hex: String = seg
                .bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");

            // For tag/len segments, use subscript label; for value segments, use decoded value
            let label = if seg.segment_type == "tag" || seg.segment_type == "len" {
                seg.label.clone()
            } else {
                seg.decoded.clone()
            };

            parts.push(format!("[{}:{}]", hex, label));

            // Add children (for nested structures)
            if !seg.children.is_empty() {
                parts.push(Self::format_compact_recursive(&seg.children, true));
            }
        }

        parts.join("")
    }

    /// Format segments as detailed table.
    fn format_detailed(segments: &[PacketSegment]) -> String {
        let mut lines = vec![
            "Offset  Len  Field       Type     Value".to_string(),
            "------  ---  ----------  ------   -----".to_string(),
        ];

        Self::format_detailed_recursive(segments, &mut lines, 0);

        lines.join("\n")
    }

    fn format_detailed_recursive(
        segments: &[PacketSegment],
        lines: &mut Vec<String>,
        depth: usize,
    ) {
        let indent = "  ".repeat(depth);
        let max_label_len = 10_usize.saturating_sub(depth * 2);

        for seg in segments {
            // UTF-8 safe truncation
            let decoded = truncate_str(&seg.decoded, 30);
            let label = truncate_str(&seg.label, max_label_len);

            lines.push(format!(
                "0x{:04X}  {:3}  {}{:<width$}  {:6}   {}",
                seg.offset,
                seg.length,
                indent,
                label,
                seg.segment_type,
                decoded,
                width = max_label_len
            ));

            // Recurse into children
            if !seg.children.is_empty() {
                Self::format_detailed_recursive(&seg.children, lines, depth + 1);
            }
        }
    }

    /// Convert a number to subscript unicode characters.
    fn subscript(n: u32) -> String {
        const SUBSCRIPTS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
        n.to_string()
            .chars()
            .map(|c| {
                if let Some(d) = c.to_digit(10) {
                    SUBSCRIPTS[d as usize]
                } else {
                    c
                }
            })
            .collect()
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
            has_validation: true,
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

        // Build packet layout segments
        let segments = Self::build_segments(bytes, &fields);
        let compact = Self::format_compact(&segments);
        let detailed = Self::format_detailed(&segments);

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
            kind: ConversionKind::default(),
            hidden: false,
            rich_display: vec![RichDisplayOption::new(RichDisplay::PacketLayout {
                segments,
                compact,
                detailed,
            })],
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["proto", "pb"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        // First, try to interpret input as hex or base64
        let bytes = Self::try_decode_input(input);

        match bytes {
            None => {
                Some("protobuf requires binary data - input is not valid hex or base64".to_string())
            }
            Some(bytes) if bytes.is_empty() => Some("empty input".to_string()),
            Some(bytes) if bytes.len() < 2 => {
                Some("protobuf message too short (minimum 2 bytes)".to_string())
            }
            Some(bytes) => {
                // Try to decode as protobuf
                match Self::decode_message(&bytes) {
                    Some(fields) if !fields.is_empty() => {
                        // Valid protobuf - note: appears as a conversion from hex/base64
                        Some(format!(
                            "valid protobuf: {} fields (run without --only to see as conversion from hex)",
                            fields.len()
                        ))
                    }
                    _ => {
                        // Give a more specific error based on the first byte
                        let tag = bytes[0];
                        let wire_type = tag & 0x07;
                        let field_num = tag >> 3;

                        if field_num == 0 {
                            Some("invalid protobuf: field number 0 is not allowed".to_string())
                        } else if wire_type > 5 || wire_type == 3 || wire_type == 4 {
                            Some(format!(
                                "invalid protobuf: unknown wire type {} at byte 0",
                                wire_type
                            ))
                        } else {
                            Some("invalid protobuf: incomplete or malformed message".to_string())
                        }
                    }
                }
            }
        }
    }
}

impl ProtobufFormat {
    /// Try to decode input as hex or base64 to get bytes.
    fn try_decode_input(input: &str) -> Option<Vec<u8>> {
        let trimmed = input.trim();

        // Try hex (with or without 0x prefix)
        let hex_str = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);

        if hex_str.chars().all(|c| c.is_ascii_hexdigit() || c == ' ') {
            let hex_clean: String = hex_str.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            if hex_clean.len() >= 2 && hex_clean.len().is_multiple_of(2) {
                if let Ok(bytes) = (0..hex_clean.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex_clean[i..i + 2], 16))
                    .collect::<Result<Vec<u8>, _>>()
                {
                    return Some(bytes);
                }
            }
        }

        // Try base64
        use base64::{engine::general_purpose::STANDARD, Engine};
        if let Ok(bytes) = STANDARD.decode(trimmed) {
            return Some(bytes);
        }

        None
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

    #[test]
    fn test_packet_layout_metadata() {
        let format = ProtobufFormat;

        // Field 1: varint 150, Field 2: string "testing"
        let bytes = vec![
            0x08, 0x96, 0x01, // field 1 = 150
            0x12, 0x07, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67, // field 2 = "testing"
        ];
        let value = CoreValue::Bytes(bytes);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 1);

        // Check that rich_display contains PacketLayout
        assert!(!conversions[0].rich_display.is_empty());
        if let RichDisplay::PacketLayout {
            segments,
            compact,
            detailed,
        } = &conversions[0].rich_display[0].preferred
        {
            // Should have 4 segments: tag1, field1, tag2, field2 (len + string)
            assert_eq!(segments.len(), 4);

            // Check compact format contains expected elements
            assert!(compact.contains("tag₁"));
            assert!(compact.contains("150"));
            assert!(compact.contains("tag₂"));
            assert!(compact.contains("testing"));

            // Check detailed format has header
            assert!(detailed.contains("Offset"));
            assert!(detailed.contains("Len"));
        } else {
            panic!("Expected PacketLayout rich_display");
        }
    }
}
