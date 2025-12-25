//! Core types for Formatorbit.
//!
//! These types represent the internal values that all formats convert between.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Internal value types that everything converts between.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "value")]
pub enum CoreValue {
    #[default]
    #[serde(skip)]
    Empty, // Default variant, should not be serialized
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
    ///
    /// Unlike msgpack/plist which decode to Json, protobuf has its own variant
    /// because schema-less protobuf has semantically different structure:
    /// - Field numbers (integers) instead of field names (strings)
    /// - Wire type metadata (varint, fixed64, etc.)
    /// - No inherent key ordering
    ///
    /// This allows UI apps to render protobuf with field number annotations
    /// and wire type information.
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

/// A segment within a binary packet layout.
///
/// Used for byte-level visualization of binary formats like protobuf and msgpack.
/// Each segment represents a contiguous range of bytes with a specific meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PacketSegment {
    /// Byte offset from start of packet (0-indexed).
    pub offset: usize,
    /// Length in bytes.
    pub length: usize,
    /// Raw bytes for this segment.
    pub bytes: Vec<u8>,
    /// Segment type (e.g., "tag", "varint", "len", "string", "fixint", "fixmap").
    pub segment_type: String,
    /// Human-readable label (e.g., "tag₁", "field 1", "len=7").
    pub label: String,
    /// Decoded value as string (e.g., "150", "testing").
    pub decoded: String,
    /// Nested segments (for embedded messages/structures).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<PacketSegment>,
}

impl CoreValue {
    /// Returns the type name as a string.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
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

/// Structured metadata for rich UI rendering.
///
/// This allows UIs to render conversions with appropriate widgets
/// (color swatches, relative time labels, etc.) without re-parsing
/// the display string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConversionMetadata {
    /// Color with RGBA components (0-255).
    Color { r: u8, g: u8, b: u8, a: u8 },

    /// Duration with human-readable form and detail.
    Duration {
        /// Human-readable duration, e.g. "4h38m31s"
        human: String,
        /// Additional context, e.g. "now + 4h38m31s = 2025-12-25T13:51:55Z"
        detail: String,
    },

    /// DateTime with ISO format and relative time.
    DateTime {
        /// ISO 8601 timestamp, e.g. "2025-12-25T13:51:55Z"
        iso: String,
        /// Relative time, e.g. "2 hours ago" or "in 3 days"
        relative: String,
    },

    /// Data size with byte count and human-readable form.
    DataSize {
        /// Raw byte count
        bytes: u64,
        /// Human-readable size, e.g. "15.94 MiB"
        human: String,
    },

    /// Packet layout with byte-level structure visualization.
    ///
    /// Used for binary formats like protobuf and msgpack to show
    /// exactly how bytes are structured.
    PacketLayout {
        /// Ordered list of segments comprising the packet.
        segments: Vec<PacketSegment>,
        /// Pre-formatted compact display (inline horizontal style).
        /// Example: `[08:tag₁][96 01:varint=150][12:tag₂]...`
        compact: String,
        /// Pre-formatted detailed display (vertical table style).
        /// Multi-line table with columns: Offset, Len, Field, Type, Value
        detailed: String,
    },
}

/// A possible conversion from a value.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// If true, this is a display-only format - don't explore further conversions.
    ///
    /// Use this for representations that are meant for human viewing, not as
    /// intermediate values for further conversion. Examples:
    /// - `hex-int` (0xC82) - showing an integer in hex notation
    /// - `binary-int` (0b110010) - showing an integer in binary notation
    ///
    /// Without this flag, the BFS would convert "0xC82" string to bytes and
    /// produce nonsense like "binary of ASCII bytes of hex string".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub display_only: bool,
    /// Structured metadata for rich UI rendering.
    ///
    /// Allows UIs to render appropriate widgets (color swatches, relative time, etc.)
    /// without re-parsing the display string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ConversionMetadata>,
}

impl Conversion {
    /// Create a new Conversion with default metadata (None).
    pub fn new(
        value: CoreValue,
        target_format: impl Into<String>,
        display: impl Into<String>,
    ) -> Self {
        Self {
            value,
            target_format: target_format.into(),
            display: display.into(),
            path: vec![],
            steps: vec![],
            is_lossy: false,
            priority: ConversionPriority::default(),
            display_only: false,
            metadata: None,
        }
    }
}

/// Complete result for an input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub input: String,
    pub interpretation: Interpretation,
    pub conversions: Vec<Conversion>,
}
