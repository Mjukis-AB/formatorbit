//! MessagePack format.

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct MsgPackFormat;

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
        }
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // MessagePack is binary, so we don't parse from string directly.
        // It would be parsed from bytes (e.g., after hex/base64 decode).
        vec![]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        // We can serialize most types to MessagePack
        matches!(
            value,
            CoreValue::Json(_)
                | CoreValue::String(_)
                | CoreValue::Int { .. }
                | CoreValue::Float(_)
                | CoreValue::Bool(_)
        )
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        // Format as hex-encoded msgpack bytes
        let bytes = self.to_msgpack_bytes(value)?;
        Some(bytes.iter().map(|b| format!("{b:02X}")).collect())
    }

    // Note: No conversions() - msgpack decoding is too noisy in conversion paths.
    // Almost any bytes decode to "valid" msgpack (small integers, etc.).
    // If we add msgpack parsing from string input (e.g., hex-encoded msgpack),
    // the parsed result will show the decoded content in the description.

    fn aliases(&self) -> &'static [&'static str] {
        &["mp", "mpack"]
    }
}

impl MsgPackFormat {
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
}
