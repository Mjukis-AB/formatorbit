//! MessagePack format.

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

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
            aliases: self.aliases(),
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

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Try to decode MessagePack
        let Ok(decoded): Result<serde_json::Value, _> = rmp_serde::from_slice(bytes) else {
            return vec![];
        };

        // Rate the conversion based on how likely this is intentional msgpack
        // vs random bytes that happen to be valid msgpack.
        let dominated_score = Self::msgpack_likelihood(&decoded, bytes.len());

        // Skip if too unlikely to be intentional
        if dominated_score < 0.3 {
            return vec![];
        }

        // Format as JSON for display
        let display = match &decoded {
            serde_json::Value::String(s) => format!("(decoded) \"{}\"", s),
            serde_json::Value::Number(n) => format!("(decoded) {}", n),
            serde_json::Value::Bool(b) => format!("(decoded) {}", b),
            serde_json::Value::Null => "(decoded) null".to_string(),
            _ => format!(
                "(decoded) {}",
                serde_json::to_string(&decoded).unwrap_or_default()
            ),
        };

        // Use the likelihood score to set priority
        let priority = if dominated_score >= 0.7 {
            ConversionPriority::Structured
        } else {
            ConversionPriority::Raw
        };

        vec![Conversion {
            value: CoreValue::Json(decoded),
            target_format: "msgpack".to_string(),
            display,
            path: vec!["msgpack".to_string()],
            is_lossy: false,
            steps: vec![],
            priority,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["mp", "mpack"]
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
