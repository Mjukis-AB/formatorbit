//! Format trait definition.

use crate::types::{Conversion, CoreValue, Interpretation};
use serde::{Serialize, Serializer};

/// Metadata about a format for help/documentation.
#[derive(Debug, Clone, Serialize)]
pub struct FormatInfo {
    /// Unique identifier (e.g., "hex")
    pub id: &'static str,
    /// Human-readable name (e.g., "Hexadecimal")
    pub name: &'static str,
    /// Category for grouping in help (e.g., "Encoding", "Timestamps")
    pub category: &'static str,
    /// Short description
    pub description: &'static str,
    /// Example input strings
    #[serde(serialize_with = "serialize_static_slice")]
    pub examples: &'static [&'static str],
    /// Short aliases (e.g., ["h", "x"] for "hex")
    #[serde(serialize_with = "serialize_static_slice")]
    pub aliases: &'static [&'static str],
}

fn serialize_static_slice<S>(
    slice: &&'static [&'static str],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    slice.serialize(serializer)
}

/// Trait for built-in formats and Rust plugins.
///
/// This is the "fast path" without FFI overhead.
/// External plugins (dylibs) are wrapped to implement this trait.
pub trait Format: Send + Sync {
    /// Unique identifier for this format (e.g., "hex", "base64").
    fn id(&self) -> &'static str;

    /// Human-readable name (e.g., "Hexadecimal").
    fn name(&self) -> &'static str;

    /// Get format metadata for help/documentation.
    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Other",
            description: "",
            examples: &[],
            aliases: self.aliases(),
        }
    }

    /// Try to parse an input string into interpretations.
    fn parse(&self, input: &str) -> Vec<Interpretation>;

    /// Check if this format can format the given value type.
    fn can_format(&self, value: &CoreValue) -> bool;

    /// Format a value to a string.
    fn format(&self, value: &CoreValue) -> Option<String>;

    /// Get possible conversions from a value (for graph traversal).
    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        vec![]
    }

    /// Short aliases for this format (e.g., "b64" for "base64").
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Check if the given name matches this format's id or any alias.
    fn matches_name(&self, name: &str) -> bool {
        self.id() == name || self.aliases().contains(&name)
    }
}
