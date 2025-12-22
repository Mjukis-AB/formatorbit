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

/// A possible conversion from a value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    pub value: CoreValue,
    pub target_format: String,
    pub display: String,
    pub path: Vec<String>,
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
