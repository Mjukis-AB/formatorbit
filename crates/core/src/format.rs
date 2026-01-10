//! Format trait definition.

use crate::types::{Conversion, CoreValue, Interpretation};
use serde::{Serialize, Serializer};

/// Metadata about a format for help/documentation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct FormatInfo {
    /// Unique identifier (e.g., "hex")
    #[serde(default)]
    pub id: &'static str,
    /// Human-readable name (e.g., "Hexadecimal")
    #[serde(default)]
    pub name: &'static str,
    /// Category for grouping in help (e.g., "Encoding", "Timestamps")
    #[serde(default)]
    pub category: &'static str,
    /// Short description
    #[serde(default)]
    pub description: &'static str,
    /// Example input strings
    #[serde(serialize_with = "serialize_static_slice", default)]
    pub examples: &'static [&'static str],
    /// Short aliases (e.g., ["h", "x"] for "hex")
    #[serde(serialize_with = "serialize_static_slice", default)]
    pub aliases: &'static [&'static str],
    /// Whether this format provides detailed validation error messages.
    /// When true, `validate()` returns helpful error messages for invalid input.
    #[serde(default)]
    pub has_validation: bool,
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
            has_validation: false,
        }
    }

    /// Try to parse an input string into interpretations.
    fn parse(&self, input: &str) -> Vec<Interpretation>;

    /// Check if this format can format the given value type.
    fn can_format(&self, value: &CoreValue) -> bool;

    /// Format a value to a string.
    fn format(&self, value: &CoreValue) -> Option<String>;

    /// Get possible conversions from a value (for graph traversal).
    ///
    /// Called on ALL values during BFS traversal, regardless of which format
    /// originally parsed the input. Use this for conversions that apply to
    /// any value of a given type (e.g., DecimalFormat converting any Int to hex).
    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        vec![]
    }

    /// Get conversions that should only appear when this format was the source.
    ///
    /// Called once per interpretation, only for the format that originally parsed
    /// the input. Use this for conversions that are specific to this format's
    /// interpretation (e.g., ExprFormat showing the expression result).
    ///
    /// These conversions are added before BFS traversal begins.
    fn source_conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
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

    /// Validate input and return an error message explaining why it cannot be parsed.
    ///
    /// This is called when a specific format is requested (e.g., `--only json`)
    /// but parsing fails. It provides helpful feedback about what's wrong with the input.
    ///
    /// Returns `None` if no specific error message is available.
    fn validate(&self, _input: &str) -> Option<String> {
        None
    }
}
