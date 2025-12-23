//! Core types for Formatorbit.
//!
//! These types represent the internal values that all formats convert between.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Internal value types that everything converts between.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CoreValue {
    Bytes(Vec<u8>),
    String(String),
    Int {
        value: i128,
        /// Original bytes to enable showing endianness variants
        #[serde(skip_serializing_if = "Option::is_none")]
        original_bytes: Option<Vec<u8>>,
    },
    Float(f64),
    Bool(bool),
    DateTime(DateTime<Utc>),
    Json(JsonValue),
    /// Decoded protobuf message (schema-less).
    Protobuf(Vec<ProtoField>),
}

/// A decoded protobuf field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtoField {
    /// Field number from the protobuf tag.
    pub field_number: u32,
    /// Wire type (0=varint, 1=i64, 2=len, 5=i32).
    pub wire_type: u8,
    /// The decoded value.
    pub value: ProtoValue,
}

/// A decoded protobuf value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtoValue {
    /// Variable-length integer (wire type 0).
    Varint(u64),
    /// Fixed 64-bit value (wire type 1).
    Fixed64(u64),
    /// Fixed 32-bit value (wire type 5).
    Fixed32(u32),
    /// Length-delimited bytes (wire type 2) - could be string, bytes, or nested message.
    Bytes(Vec<u8>),
    /// UTF-8 string (decoded from length-delimited).
    String(String),
    /// Nested protobuf message (decoded from length-delimited).
    Message(Vec<ProtoField>),
}

impl CoreValue {
    /// Returns the type name as a string.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bytes(_) => "bytes",
            Self::String(_) => "string",
            Self::Int { .. } => "int",
            Self::Float(_) => "float",
            Self::Bool(_) => "bool",
            Self::DateTime(_) => "datetime",
            Self::Json(_) => "json",
            Self::Protobuf(_) => "protobuf",
        }
    }
}

/// A possible interpretation of the input string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    pub value: CoreValue,
    pub source_format: String,
    pub confidence: f32,
    pub description: String,
}

/// Priority level for conversion results.
///
/// Higher priority conversions appear first in output.
/// Structured data (JSON, MessagePack) is most valuable,
/// followed by semantic interpretations (datetime, UUID),
/// then encodings (hex, base64), then raw representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum ConversionPriority {
    /// Structured data (json, msgpack) - most valuable
    Structured = 0,
    /// Semantic formats (datetime, uuid, ip, color) - meaningful interpretation
    Semantic = 1,
    /// Encoding formats (hex, base64, url) - different representations
    #[default]
    Encoding = 2,
    /// Raw formats (bytes, int) - low-level
    Raw = 3,
}

/// A single step in a conversion path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionStep {
    /// The format ID at this step.
    pub format: String,
    /// The value at this step.
    pub value: CoreValue,
    /// Human-readable display of the value.
    pub display: String,
}

/// A possible conversion from a value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    pub value: CoreValue,
    pub target_format: String,
    pub display: String,
    /// Legacy path field - just format IDs for backwards compatibility.
    #[serde(default)]
    pub path: Vec<String>,
    /// Full conversion path with intermediate values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<ConversionStep>,
    #[serde(default)]
    pub is_lossy: bool,
    /// Priority for sorting results (lower = shown first)
    #[serde(default)]
    pub priority: ConversionPriority,
}

/// Complete result for an input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub input: String,
    pub interpretation: Interpretation,
    pub conversions: Vec<Conversion>,
}
