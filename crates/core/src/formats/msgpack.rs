//! MessagePack format.

use crate::format::{Format, FormatInfo};
use crate::truncate_str;
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, PacketSegment,
    RichDisplay, RichDisplayOption,
};

pub struct MsgPackFormat;

/// Result of decoding a MessagePack value with offset tracking.
struct DecodedValue {
    /// The decoded JSON value.
    value: serde_json::Value,
    /// Segments describing the byte layout.
    segments: Vec<PacketSegment>,
    /// Total bytes consumed.
    bytes_consumed: usize,
}

impl Format for MsgPackFormat {
    fn id(&self) -> &'static str {
        "msgpack"
    }

    fn name(&self) -> &'static str {
        "MessagePack"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Data",
            description: "MessagePack binary serialization (decoded from bytes)",
            examples: &[],
            aliases: self.aliases(),
            has_validation: true,
        }
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // MessagePack is binary, so we don't parse from string directly.
        // It would be parsed from bytes (e.g., after hex/base64 decode).
        vec![]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        // We can serialize most types to MessagePack, but NOT Json
        // (Json is a terminal format - we decode TO Json, not encode FROM Json)
        matches!(
            value,
            CoreValue::String(_) | CoreValue::Int { .. } | CoreValue::Float(_) | CoreValue::Bool(_)
        )
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        // Format as hex-encoded msgpack bytes
        let bytes = self.to_msgpack_bytes(value)?;
        Some(bytes.iter().map(|b| format!("{b:02X}")).collect())
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Try to decode MessagePack with offset tracking
        let Some(decoded) = Self::decode_with_offsets(bytes) else {
            return vec![];
        };

        // Make sure we consumed all bytes
        if decoded.bytes_consumed != bytes.len() {
            return vec![];
        }

        // Rate the conversion based on how likely this is intentional msgpack
        // vs random bytes that happen to be valid msgpack.
        let dominated_score = Self::msgpack_likelihood(&decoded.value, bytes.len());

        // Skip if too unlikely to be intentional
        if dominated_score < 0.3 {
            return vec![];
        }

        // Build packet layout
        let compact = Self::format_compact(&decoded.segments);
        let detailed = Self::format_detailed(&decoded.segments);

        // Format as JSON for display
        let display = match &decoded.value {
            serde_json::Value::String(s) => format!("(decoded) \"{}\"", s),
            serde_json::Value::Number(n) => format!("(decoded) {}", n),
            serde_json::Value::Bool(b) => format!("(decoded) {}", b),
            serde_json::Value::Null => "(decoded) null".to_string(),
            _ => format!(
                "(decoded) {}",
                serde_json::to_string(&decoded.value).unwrap_or_default()
            ),
        };

        // Use the likelihood score to set priority
        let priority = if dominated_score >= 0.7 {
            ConversionPriority::Structured
        } else {
            ConversionPriority::Raw
        };

        vec![Conversion {
            value: CoreValue::Json(decoded.value),
            target_format: "msgpack".to_string(),
            display,
            path: vec!["msgpack".to_string()],
            is_lossy: false,
            steps: vec![],
            priority,
            display_only: false,
            kind: ConversionKind::default(),
            hidden: false,
            rich_display: vec![RichDisplayOption::new(RichDisplay::PacketLayout {
                segments: decoded.segments,
                compact,
                detailed,
            })],
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["mp", "mpack"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        // First, try to interpret input as hex or base64
        let bytes = Self::try_decode_input(input);

        match bytes {
            None => {
                Some("msgpack requires binary data - input is not valid hex or base64".to_string())
            }
            Some(bytes) if bytes.is_empty() => Some("empty input".to_string()),
            Some(bytes) => {
                // Try to decode using our decoder (which tracks bytes consumed)
                match Self::decode_with_offsets(&bytes) {
                    Some(decoded) if decoded.bytes_consumed == bytes.len() => {
                        // Check if it would pass our likelihood threshold
                        let score = Self::msgpack_likelihood(&decoded.value, bytes.len());
                        if score < 0.3 {
                            Some(format!(
                                "decoded as {} but unlikely to be intentional msgpack (confidence: {:.0}%)",
                                Self::describe_value(&decoded.value),
                                score * 100.0
                            ))
                        } else {
                            // Valid msgpack - note: msgpack appears as a conversion from hex/base64
                            // so --only msgpack won't work, need to run without filter
                            Some(format!(
                                "valid msgpack: {} (run without --only to see as conversion from hex)",
                                Self::describe_value(&decoded.value)
                            ))
                        }
                    }
                    Some(decoded) => Some(format!(
                        "only {} of {} bytes consumed - trailing data",
                        decoded.bytes_consumed,
                        bytes.len()
                    )),
                    None => {
                        // Try rmp_serde for a more specific error
                        match rmp_serde::from_slice::<serde_json::Value>(&bytes) {
                            Ok(_) => Some("valid msgpack but our decoder failed".to_string()),
                            Err(e) => Some(format!("invalid msgpack: {}", e)),
                        }
                    }
                }
            }
        }
    }
}

impl MsgPackFormat {
    /// Describe a JSON value briefly for error messages.
    fn describe_value(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(b) => format!("bool({})", b),
            serde_json::Value::Number(n) => format!("number({})", n),
            serde_json::Value::String(s) if s.len() <= 20 => format!("\"{}\"", s),
            serde_json::Value::String(s) => format!("string({} chars)", s.len()),
            serde_json::Value::Array(a) => format!("array({} items)", a.len()),
            serde_json::Value::Object(o) => format!("object({} keys)", o.len()),
        }
    }

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

impl MsgPackFormat {
    /// Score how likely bytes are to be intentional msgpack vs random bytes.
    /// Returns 0.0-1.0 where higher = more likely intentional.
    fn msgpack_likelihood(decoded: &serde_json::Value, byte_len: usize) -> f32 {
        match decoded {
            // Objects and arrays are very unlikely to be accidental
            serde_json::Value::Object(_) => 0.90,
            serde_json::Value::Array(arr) if !arr.is_empty() => 0.85,
            serde_json::Value::Array(_) => 0.60, // empty array

            // Strings are unlikely accidental (need valid UTF-8 + length prefix)
            serde_json::Value::String(s) if s.len() > 3 => 0.85,
            serde_json::Value::String(s) if !s.is_empty() => 0.70,
            serde_json::Value::String(_) => 0.40, // empty string

            // Booleans and null: could be accidental (single byte)
            serde_json::Value::Bool(_) => 0.50,
            serde_json::Value::Null => 0.40,

            // Integers: show for small-medium sizes, skip for very large
            // Path-based filtering handles IP/UUID cases separately
            serde_json::Value::Number(_) => match byte_len {
                1..=2 => 0.60,  // Small values like 0x34 → 52
                3..=4 => 0.50,  // Could be intentional 32-bit value
                5..=8 => 0.40,  // 64-bit values
                9..=16 => 0.30, // Borderline (UUIDs filtered by path)
                _ => 0.0,       // Skip very large
            },
        }
    }

    fn to_msgpack_bytes(&self, value: &CoreValue) -> Option<Vec<u8>> {
        match value {
            CoreValue::Json(json) => rmp_serde::to_vec(json).ok(),
            CoreValue::String(s) => rmp_serde::to_vec(s).ok(),
            CoreValue::Int { value, .. } => {
                // Try to fit in i64 for msgpack
                if let Ok(v) = i64::try_from(*value) {
                    rmp_serde::to_vec(&v).ok()
                } else {
                    None
                }
            }
            CoreValue::Float(f) => rmp_serde::to_vec(f).ok(),
            CoreValue::Bool(b) => rmp_serde::to_vec(b).ok(),
            _ => None,
        }
    }

    /// Decode MessagePack with offset tracking.
    fn decode_with_offsets(bytes: &[u8]) -> Option<DecodedValue> {
        Self::decode_value(bytes, 0)
    }

    /// Decode a single MessagePack value at the given offset.
    fn decode_value(bytes: &[u8], offset: usize) -> Option<DecodedValue> {
        if bytes.is_empty() {
            return None;
        }

        let format_byte = bytes[0];

        match format_byte {
            // Positive fixint (0x00 - 0x7f)
            0x00..=0x7f => {
                let value = serde_json::Value::Number(format_byte.into());
                Some(DecodedValue {
                    value,
                    segments: vec![PacketSegment {
                        offset,
                        length: 1,
                        bytes: vec![format_byte],
                        segment_type: "fixint".to_string(),
                        label: format!("{}", format_byte),
                        decoded: format!("{}", format_byte),
                        children: vec![],
                    }],
                    bytes_consumed: 1,
                })
            }

            // Fixmap (0x80 - 0x8f)
            0x80..=0x8f => {
                let count = (format_byte & 0x0f) as usize;
                Self::decode_map(bytes, offset, count, 1, "fixmap")
            }

            // Fixarray (0x90 - 0x9f)
            0x90..=0x9f => {
                let count = (format_byte & 0x0f) as usize;
                Self::decode_array(bytes, offset, count, 1, "fixarray")
            }

            // Fixstr (0xa0 - 0xbf)
            0xa0..=0xbf => {
                let len = (format_byte & 0x1f) as usize;
                Self::decode_string(bytes, offset, len, 1, "fixstr")
            }

            // nil
            0xc0 => Some(DecodedValue {
                value: serde_json::Value::Null,
                segments: vec![PacketSegment {
                    offset,
                    length: 1,
                    bytes: vec![0xc0],
                    segment_type: "nil".to_string(),
                    label: "nil".to_string(),
                    decoded: "null".to_string(),
                    children: vec![],
                }],
                bytes_consumed: 1,
            }),

            // (never used) 0xc1
            0xc1 => None,

            // false
            0xc2 => Some(DecodedValue {
                value: serde_json::Value::Bool(false),
                segments: vec![PacketSegment {
                    offset,
                    length: 1,
                    bytes: vec![0xc2],
                    segment_type: "bool".to_string(),
                    label: "false".to_string(),
                    decoded: "false".to_string(),
                    children: vec![],
                }],
                bytes_consumed: 1,
            }),

            // true
            0xc3 => Some(DecodedValue {
                value: serde_json::Value::Bool(true),
                segments: vec![PacketSegment {
                    offset,
                    length: 1,
                    bytes: vec![0xc3],
                    segment_type: "bool".to_string(),
                    label: "true".to_string(),
                    decoded: "true".to_string(),
                    children: vec![],
                }],
                bytes_consumed: 1,
            }),

            // bin8
            0xc4 => {
                if bytes.len() < 2 {
                    return None;
                }
                let len = bytes[1] as usize;
                Self::decode_binary(bytes, offset, len, 2, "bin8")
            }

            // bin16
            0xc5 => {
                if bytes.len() < 3 {
                    return None;
                }
                let len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
                Self::decode_binary(bytes, offset, len, 3, "bin16")
            }

            // bin32
            0xc6 => {
                if bytes.len() < 5 {
                    return None;
                }
                let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                Self::decode_binary(bytes, offset, len, 5, "bin32")
            }

            // ext8, ext16, ext32 (0xc7-0xc9) - skip for now
            0xc7..=0xc9 => None,

            // float32
            0xca => {
                if bytes.len() < 5 {
                    return None;
                }
                let v = f32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let value = serde_json::Number::from_f64(v as f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null);
                Some(DecodedValue {
                    value,
                    segments: vec![PacketSegment {
                        offset,
                        length: 5,
                        bytes: bytes[..5].to_vec(),
                        segment_type: "float32".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 5,
                })
            }

            // float64
            0xcb => {
                if bytes.len() < 9 {
                    return None;
                }
                let v = f64::from_be_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                let value = serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null);
                Some(DecodedValue {
                    value,
                    segments: vec![PacketSegment {
                        offset,
                        length: 9,
                        bytes: bytes[..9].to_vec(),
                        segment_type: "float64".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 9,
                })
            }

            // uint8
            0xcc => {
                if bytes.len() < 2 {
                    return None;
                }
                let v = bytes[1];
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 2,
                        bytes: bytes[..2].to_vec(),
                        segment_type: "uint8".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 2,
                })
            }

            // uint16
            0xcd => {
                if bytes.len() < 3 {
                    return None;
                }
                let v = u16::from_be_bytes([bytes[1], bytes[2]]);
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 3,
                        bytes: bytes[..3].to_vec(),
                        segment_type: "uint16".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 3,
                })
            }

            // uint32
            0xce => {
                if bytes.len() < 5 {
                    return None;
                }
                let v = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 5,
                        bytes: bytes[..5].to_vec(),
                        segment_type: "uint32".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 5,
                })
            }

            // uint64
            0xcf => {
                if bytes.len() < 9 {
                    return None;
                }
                let v = u64::from_be_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 9,
                        bytes: bytes[..9].to_vec(),
                        segment_type: "uint64".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 9,
                })
            }

            // int8
            0xd0 => {
                if bytes.len() < 2 {
                    return None;
                }
                let v = bytes[1] as i8;
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 2,
                        bytes: bytes[..2].to_vec(),
                        segment_type: "int8".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 2,
                })
            }

            // int16
            0xd1 => {
                if bytes.len() < 3 {
                    return None;
                }
                let v = i16::from_be_bytes([bytes[1], bytes[2]]);
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 3,
                        bytes: bytes[..3].to_vec(),
                        segment_type: "int16".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 3,
                })
            }

            // int32
            0xd2 => {
                if bytes.len() < 5 {
                    return None;
                }
                let v = i32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 5,
                        bytes: bytes[..5].to_vec(),
                        segment_type: "int32".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 5,
                })
            }

            // int64
            0xd3 => {
                if bytes.len() < 9 {
                    return None;
                }
                let v = i64::from_be_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 9,
                        bytes: bytes[..9].to_vec(),
                        segment_type: "int64".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 9,
                })
            }

            // fixext1-fixext16 (0xd4-0xd8) - skip for now
            0xd4..=0xd8 => None,

            // str8
            0xd9 => {
                if bytes.len() < 2 {
                    return None;
                }
                let len = bytes[1] as usize;
                Self::decode_string(bytes, offset, len, 2, "str8")
            }

            // str16
            0xda => {
                if bytes.len() < 3 {
                    return None;
                }
                let len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
                Self::decode_string(bytes, offset, len, 3, "str16")
            }

            // str32
            0xdb => {
                if bytes.len() < 5 {
                    return None;
                }
                let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                Self::decode_string(bytes, offset, len, 5, "str32")
            }

            // array16
            0xdc => {
                if bytes.len() < 3 {
                    return None;
                }
                let count = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
                Self::decode_array(bytes, offset, count, 3, "array16")
            }

            // array32
            0xdd => {
                if bytes.len() < 5 {
                    return None;
                }
                let count = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                Self::decode_array(bytes, offset, count, 5, "array32")
            }

            // map16
            0xde => {
                if bytes.len() < 3 {
                    return None;
                }
                let count = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
                Self::decode_map(bytes, offset, count, 3, "map16")
            }

            // map32
            0xdf => {
                if bytes.len() < 5 {
                    return None;
                }
                let count = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                Self::decode_map(bytes, offset, count, 5, "map32")
            }

            // Negative fixint (0xe0 - 0xff)
            0xe0..=0xff => {
                let v = format_byte as i8;
                Some(DecodedValue {
                    value: serde_json::Value::Number(v.into()),
                    segments: vec![PacketSegment {
                        offset,
                        length: 1,
                        bytes: vec![format_byte],
                        segment_type: "fixint".to_string(),
                        label: format!("{}", v),
                        decoded: format!("{}", v),
                        children: vec![],
                    }],
                    bytes_consumed: 1,
                })
            }
        }
    }

    /// Decode a string value.
    fn decode_string(
        bytes: &[u8],
        offset: usize,
        len: usize,
        header_len: usize,
        type_name: &str,
    ) -> Option<DecodedValue> {
        let total_len = header_len + len;
        if bytes.len() < total_len {
            return None;
        }

        let str_bytes = &bytes[header_len..total_len];
        let s = std::str::from_utf8(str_bytes).ok()?;

        let header_bytes = bytes[..header_len].to_vec();
        let decoded_str = format!("\"{}\"", s);

        Some(DecodedValue {
            value: serde_json::Value::String(s.to_string()),
            segments: vec![PacketSegment {
                offset,
                length: header_len,
                bytes: header_bytes,
                segment_type: type_name.to_string(),
                label: format!("str{}", Self::subscript(len)),
                decoded: decoded_str.clone(),
                children: vec![PacketSegment {
                    offset: offset + header_len,
                    length: len,
                    bytes: str_bytes.to_vec(),
                    segment_type: "string".to_string(),
                    label: decoded_str.clone(),
                    decoded: decoded_str,
                    children: vec![],
                }],
            }],
            bytes_consumed: total_len,
        })
    }

    /// Decode a binary value.
    fn decode_binary(
        bytes: &[u8],
        offset: usize,
        len: usize,
        header_len: usize,
        type_name: &str,
    ) -> Option<DecodedValue> {
        let total_len = header_len + len;
        if bytes.len() < total_len {
            return None;
        }

        let bin_bytes = bytes[header_len..total_len].to_vec();
        let header_bytes = bytes[..header_len].to_vec();

        // Encode as base64 for JSON compatibility
        use base64::{engine::general_purpose::STANDARD, Engine};
        let b64 = STANDARD.encode(&bin_bytes);

        let decoded = if len <= 16 {
            let hex: String = bin_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            format!("bytes[{}]: {}", len, hex)
        } else {
            format!("bytes[{}]", len)
        };

        Some(DecodedValue {
            value: serde_json::Value::String(b64),
            segments: vec![PacketSegment {
                offset,
                length: header_len,
                bytes: header_bytes,
                segment_type: type_name.to_string(),
                label: format!("bin{}", Self::subscript(len)),
                decoded: decoded.clone(),
                children: vec![PacketSegment {
                    offset: offset + header_len,
                    length: len,
                    bytes: bin_bytes,
                    segment_type: "binary".to_string(),
                    label: decoded.clone(),
                    decoded,
                    children: vec![],
                }],
            }],
            bytes_consumed: total_len,
        })
    }

    /// Decode an array value.
    fn decode_array(
        bytes: &[u8],
        offset: usize,
        count: usize,
        header_len: usize,
        type_name: &str,
    ) -> Option<DecodedValue> {
        let header_bytes = bytes[..header_len].to_vec();
        let mut pos = header_len;
        let mut items = Vec::new();
        let mut child_segments = Vec::new();

        for _ in 0..count {
            if pos >= bytes.len() {
                return None;
            }
            let decoded = Self::decode_value(&bytes[pos..], offset + pos)?;
            items.push(decoded.value);
            child_segments.extend(decoded.segments);
            pos += decoded.bytes_consumed;
        }

        Some(DecodedValue {
            value: serde_json::Value::Array(items),
            segments: vec![PacketSegment {
                offset,
                length: header_len,
                bytes: header_bytes,
                segment_type: type_name.to_string(),
                label: format!("arr{}", Self::subscript(count)),
                decoded: format!("[{}]", count),
                children: child_segments,
            }],
            bytes_consumed: pos,
        })
    }

    /// Decode a map value.
    fn decode_map(
        bytes: &[u8],
        offset: usize,
        count: usize,
        header_len: usize,
        type_name: &str,
    ) -> Option<DecodedValue> {
        let header_bytes = bytes[..header_len].to_vec();
        let mut pos = header_len;
        let mut map = serde_json::Map::new();
        let mut child_segments = Vec::new();

        for _ in 0..count {
            if pos >= bytes.len() {
                return None;
            }

            // Decode key
            let key_decoded = Self::decode_value(&bytes[pos..], offset + pos)?;
            let key = match &key_decoded.value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            child_segments.extend(key_decoded.segments);
            pos += key_decoded.bytes_consumed;

            // Decode value
            if pos >= bytes.len() {
                return None;
            }
            let val_decoded = Self::decode_value(&bytes[pos..], offset + pos)?;
            map.insert(key, val_decoded.value);
            child_segments.extend(val_decoded.segments);
            pos += val_decoded.bytes_consumed;
        }

        Some(DecodedValue {
            value: serde_json::Value::Object(map),
            segments: vec![PacketSegment {
                offset,
                length: header_len,
                bytes: header_bytes,
                segment_type: type_name.to_string(),
                label: format!("map{}", Self::subscript(count)),
                decoded: format!("{{{}}}", count),
                children: child_segments,
            }],
            bytes_consumed: pos,
        })
    }

    /// Format segments in compact inline style.
    fn format_compact(segments: &[PacketSegment]) -> String {
        Self::format_compact_recursive(segments)
    }

    fn format_compact_recursive(segments: &[PacketSegment]) -> String {
        let mut parts = Vec::new();

        for seg in segments {
            let hex: String = seg
                .bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");

            parts.push(format!("[{}:{}]", hex, seg.label));

            // Add children
            if !seg.children.is_empty() {
                parts.push(Self::format_compact_recursive(&seg.children));
            }
        }

        parts.join("")
    }

    /// Format segments as detailed table.
    fn format_detailed(segments: &[PacketSegment]) -> String {
        let mut lines = vec![
            "Offset  Len  Field       Type      Value".to_string(),
            "------  ---  ----------  --------  -----".to_string(),
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
            let decoded = truncate_str(&seg.decoded, 25);
            let label = truncate_str(&seg.label, max_label_len);

            lines.push(format!(
                "0x{:04X}  {:3}  {}{:<width$}  {:8}  {}",
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
    fn subscript(n: usize) -> String {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: conversions() tests removed because that method is now disabled
    // to avoid noise from arbitrary bytes→msgpack decoding.

    #[test]
    fn test_encode_json_to_msgpack() {
        let format = MsgPackFormat;
        let value = CoreValue::Json(serde_json::json!({"key": 42}));

        let encoded = format.format(&value).unwrap();
        // Should be hex-encoded msgpack
        assert!(!encoded.is_empty());

        // Verify we can decode it back
        let bytes: Vec<u8> = (0..encoded.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap())
            .collect();

        let decoded: serde_json::Value = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded["key"], 42);
    }

    #[test]
    fn test_decode_fixarray() {
        // fixarray with 3 elements: [1, 2, 3]
        // 93 = fixarray with 3 elements
        // 01, 02, 03 = fixint 1, 2, 3
        let bytes = vec![0x93, 0x01, 0x02, 0x03];
        let decoded = MsgPackFormat::decode_with_offsets(&bytes).unwrap();

        assert_eq!(decoded.bytes_consumed, 4);
        assert_eq!(decoded.value, serde_json::json!([1, 2, 3]));
        assert_eq!(decoded.segments.len(), 1);
        assert_eq!(decoded.segments[0].segment_type, "fixarray");
        assert_eq!(decoded.segments[0].children.len(), 3);
    }

    #[test]
    fn test_decode_fixmap() {
        // fixmap with 1 entry: {"key": 42}
        // 81 = fixmap with 1 entry
        // a3 6b 65 79 = fixstr "key"
        // 2a = fixint 42
        let bytes = vec![0x81, 0xa3, 0x6b, 0x65, 0x79, 0x2a];
        let decoded = MsgPackFormat::decode_with_offsets(&bytes).unwrap();

        assert_eq!(decoded.bytes_consumed, 6);
        assert_eq!(decoded.value, serde_json::json!({"key": 42}));
        assert_eq!(decoded.segments.len(), 1);
        assert_eq!(decoded.segments[0].segment_type, "fixmap");
    }

    #[test]
    fn test_packet_layout_metadata() {
        let format = MsgPackFormat;

        // fixarray [1, 2, 3]
        let bytes = vec![0x93, 0x01, 0x02, 0x03];
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
            assert_eq!(segments.len(), 1);
            assert!(compact.contains("arr₃"));
            assert!(detailed.contains("Offset"));
            assert!(detailed.contains("fixarray"));
        } else {
            panic!("Expected PacketLayout rich_display");
        }
    }
}
