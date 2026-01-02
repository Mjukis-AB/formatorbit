//! Core types for Formatorbit.
//!
//! These types represent the internal values that all formats convert between.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

// ============================================================================
// Rich Display Types
// ============================================================================

/// Rich display hints for UI rendering.
///
/// Each variant represents a different way to render data. UIs should handle
/// each variant appropriately based on their capabilities (CLI renders as text,
/// GUI can render maps, color swatches, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RichDisplay {
    /// Key-value pairs (IP parts, URL components, etc.)
    KeyValue { pairs: Vec<(String, String)> },

    /// Tabular data (packet layout, etc.)
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },

    /// Tree structure (JSON, protobuf, nested data)
    Tree { root: TreeNode },

    /// Color swatch with RGBA components (0-255)
    Color { r: u8, g: u8, b: u8, a: u8 },

    /// Geographic coordinates (render as map or text)
    Map {
        lat: f64,
        lon: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },

    /// Mermaid diagram source
    Mermaid { source: String },

    /// Graphviz DOT diagram source
    Dot { source: String },

    /// Syntax-highlighted code block
    Code { language: String, content: String },

    /// Duration with human-readable form
    Duration {
        /// Total milliseconds
        millis: u64,
        /// Human-readable, e.g. "4h38m31s"
        human: String,
    },

    /// DateTime with relative time
    DateTime {
        /// Unix epoch milliseconds (enables client-side ticking)
        epoch_millis: i64,
        /// ISO 8601 timestamp
        iso: String,
        /// Relative time snapshot, e.g. "2 hours ago"
        relative: String,
    },

    /// Data size with byte count
    DataSize {
        /// Raw byte count
        bytes: u64,
        /// Human-readable, e.g. "15.94 MiB"
        human: String,
    },

    /// Packet/binary layout visualization
    PacketLayout {
        /// Structured segments for programmatic access
        segments: Vec<PacketSegment>,
        /// Compact inline display: `[08:tag₁][96 01:150]...`
        compact: String,
        /// Detailed table display (multi-line)
        detailed: String,
    },

    /// Image data (QR codes, rendered diagrams, etc.)
    Image {
        /// Format: "png", "svg", "jpeg"
        format: String,
        /// Base64-encoded image data
        data: String,
    },

    /// Progress/percentage indicator
    Progress {
        /// Value between 0.0 and 1.0
        value: f64,
        /// Optional label, e.g. "75%"
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },

    /// Markdown-formatted text
    Markdown { content: String },
}

/// A node in a tree structure for hierarchical data display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    /// Label for this node (key name, index, etc.)
    pub label: String,
    /// Optional value (leaf nodes have values)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Child nodes (empty for leaf nodes)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

/// A display option with a preferred rendering and alternatives.
///
/// UIs should try to render the `preferred` variant first. If they don't
/// support it (e.g., CLI can't render a Map), they should fall back to
/// the first supported alternative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichDisplayOption {
    /// Primary/preferred way to display this data
    pub preferred: RichDisplay,
    /// Alternative renderings (fallbacks for less capable UIs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<RichDisplay>,
}

impl RichDisplayOption {
    /// Create a display option with just a preferred variant, no alternatives.
    pub fn new(preferred: RichDisplay) -> Self {
        Self {
            preferred,
            alternatives: vec![],
        }
    }

    /// Create a display option with a preferred variant and alternatives.
    pub fn with_alternatives(preferred: RichDisplay, alternatives: Vec<RichDisplay>) -> Self {
        Self {
            preferred,
            alternatives,
        }
    }
}

impl RichDisplay {
    /// Get a compact single-line representation for compact/pipe mode.
    #[must_use]
    pub fn compact(&self) -> String {
        match self {
            Self::KeyValue { pairs } => pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" "),
            Self::Table { rows, .. } => format!("[{} rows]", rows.len()),
            Self::Tree { root } => format!("{}: ...", root.label),
            Self::Color { r, g, b, a } => {
                if *a == 255 {
                    format!("#{:02X}{:02X}{:02X}", r, g, b)
                } else {
                    format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
                }
            }
            Self::Map { lat, lon, .. } => format!("{:.6},{:.6}", lat, lon),
            Self::Mermaid { .. } => "[mermaid]".to_string(),
            Self::Dot { .. } => "[dot]".to_string(),
            Self::Code { language, .. } => format!("[{}]", language),
            Self::Duration { human, .. } => human.clone(),
            Self::DateTime { iso, .. } => iso.clone(),
            Self::DataSize { human, .. } => human.clone(),
            Self::PacketLayout { compact, .. } => compact.clone(),
            Self::Image { format, .. } => format!("[{} image]", format),
            Self::Progress { value, label } => label
                .clone()
                .unwrap_or_else(|| format!("{:.0}%", value * 100.0)),
            Self::Markdown { content } => {
                // First line or truncated
                content.lines().next().unwrap_or("").to_string()
            }
        }
    }

    /// Get a raw value for scripting/piping (minimal formatting).
    #[must_use]
    pub fn raw(&self) -> String {
        match self {
            Self::KeyValue { pairs } => pairs
                .iter()
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join("\t"),
            Self::Table { rows, .. } => rows
                .iter()
                .map(|row| row.join("\t"))
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Tree { root } => root.value.clone().unwrap_or_default(),
            Self::Color { r, g, b, .. } => format!("{},{},{}", r, g, b),
            Self::Map { lat, lon, .. } => format!("{},{}", lat, lon),
            Self::Mermaid { source } => source.clone(),
            Self::Dot { source } => source.clone(),
            Self::Code { content, .. } => content.clone(),
            Self::Duration { millis, .. } => millis.to_string(),
            Self::DateTime { iso, .. } => iso.clone(),
            Self::DataSize { bytes, .. } => bytes.to_string(),
            Self::PacketLayout { compact, .. } => compact.clone(),
            Self::Image { data, .. } => data.clone(),
            Self::Progress { value, .. } => format!("{:.4}", value),
            Self::Markdown { content } => content.clone(),
        }
    }
}

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

    // =========================================================================
    // Unit-specific value types
    // =========================================================================
    // These ensure that unit conversions only happen within the same category.
    // E.g., Length values won't accidentally be converted as Weight values.
    /// Length in meters (base SI unit).
    Length(f64),
    /// Weight/mass in grams (base unit).
    Weight(f64),
    /// Volume in milliliters (base unit).
    Volume(f64),
    /// Speed in meters per second (base SI unit).
    Speed(f64),
    /// Pressure in pascals (base SI unit).
    Pressure(f64),
    /// Energy in joules (base SI unit).
    Energy(f64),
    /// Angle in degrees (base unit).
    Angle(f64),
    /// Area in square meters (base SI unit).
    Area(f64),
    /// Temperature in Kelvin (base SI unit).
    Temperature(f64),
    /// Currency amount with ISO 4217 code.
    /// Amount is in base units (not cents).
    Currency {
        amount: f64,
        code: String,
    },
    /// Geographic coordinates (WGS84).
    /// Latitude in degrees (-90 to 90), Longitude in degrees (-180 to 180).
    Coordinates {
        lat: f64,
        lon: f64,
    },
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
            Self::Length(_) => "length",
            Self::Weight(_) => "weight",
            Self::Volume(_) => "volume",
            Self::Speed(_) => "speed",
            Self::Pressure(_) => "pressure",
            Self::Energy(_) => "energy",
            Self::Angle(_) => "angle",
            Self::Area(_) => "area",
            Self::Temperature(_) => "temperature",
            Self::Currency { .. } => "currency",
            Self::Coordinates { .. } => "coordinates",
        }
    }
}

/// A possible interpretation of the input string.
///
/// # Display Strategy for UI Apps
///
/// The `description` and `rich_display` fields may contain the same information
/// in different formats. UI applications should follow this priority:
///
/// 1. **If `rich_display` is not empty**: Render `rich_display[0].preferred` using
///    appropriate UI components (tables, key-value lists, etc.). Do NOT also show
///    `description` - it would be redundant.
///
/// 2. **If `rich_display` is empty**: Fall back to showing `description` as plain text.
///
/// The `description` field always contains a plain-text representation suitable for:
/// - CLI output (cannot render rich components)
/// - Accessibility/screen readers
/// - Simple UIs without rich display support
/// - Logging and debugging
///
/// # Example
///
/// For the rainbow flag emoji 🏳️‍🌈:
/// - `description`: `"'🏳️‍🌈' = U+1F3F3 '🏳' + U+FE0F VS16 (emoji presentation) + ..."`
/// - `rich_display`: `KeyValue { pairs: [("U+1F3F3", "🏳"), ("U+FE0F", "VS16 (emoji presentation)"), ...] }`
///
/// A GUI should render the KeyValue as a nice table and hide the description.
/// A CLI should ignore rich_display and print the description.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Interpretation {
    /// The parsed value in a canonical internal representation.
    #[serde(default)]
    pub value: CoreValue,

    /// Format ID that produced this interpretation (e.g., "hex", "base64", "char").
    #[serde(default)]
    pub source_format: String,

    /// Confidence score from 0.0 to 1.0 indicating how likely this interpretation is correct.
    #[serde(default)]
    pub confidence: f32,

    /// Plain-text description of the interpretation.
    ///
    /// Always populated. Used as fallback when `rich_display` is empty or unsupported.
    /// CLI and simple UIs should display this. GUI apps should prefer `rich_display`
    /// when available and hide this to avoid redundancy.
    #[serde(default)]
    pub description: String,

    /// Rich display hints for UI rendering.
    ///
    /// When not empty, GUI applications should render this instead of `description`.
    /// Multiple display options can be provided, each with a preferred rendering
    /// and alternatives. UIs choose based on their capabilities.
    ///
    /// See struct-level docs for the display strategy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rich_display: Vec<RichDisplayOption>,
}

impl Interpretation {
    /// Create a new interpretation with empty rich_display.
    pub fn new(
        value: CoreValue,
        source_format: impl Into<String>,
        confidence: f32,
        description: impl Into<String>,
    ) -> Self {
        Self {
            value,
            source_format: source_format.into(),
            confidence,
            description: description.into(),
            rich_display: vec![],
        }
    }
}

/// The kind of conversion - distinguishes between actual transformations,
/// display representations, and traits/observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConversionKind {
    /// Actual transformation (bytes -> int, int -> datetime)
    #[default]
    Conversion,
    /// Same value, different notation (1024 -> 0x400, 1024 -> 1 KiB)
    Representation,
    /// Observation/property of the value (is power-of-2, is prime, is fibonacci)
    Trait,
}

/// Priority level for conversion results.
///
/// Higher priority conversions appear first in output.
/// Primary is the canonical result value (always first),
/// followed by structured data (JSON, MessagePack),
/// then semantic interpretations (datetime, UUID),
/// then encodings (hex, base64), then raw representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum ConversionPriority {
    /// The canonical/primary result - always shown first
    /// Used for expression results, computed values, etc.
    Primary = 0,
    /// Structured data (json, msgpack) - most valuable
    Structured = 1,
    /// Semantic formats (datetime, uuid, ip, color) - meaningful interpretation
    Semantic = 2,
    /// Encoding formats (hex, base64, url) - different representations
    #[default]
    Encoding = 3,
    /// Raw formats (bytes, int) - low-level
    Raw = 4,
}

impl ConversionPriority {
    /// Parse from string (case-insensitive).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "primary" => Some(Self::Primary),
            "structured" => Some(Self::Structured),
            "semantic" => Some(Self::Semantic),
            "encoding" => Some(Self::Encoding),
            "raw" => Some(Self::Raw),
            _ => None,
        }
    }

    /// Convert to string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Structured => "Structured",
            Self::Semantic => "Semantic",
            Self::Encoding => "Encoding",
            Self::Raw => "Raw",
        }
    }
}

// ============================================================================
// User Configuration Types
// ============================================================================

/// Priority adjustment for a format.
///
/// Can be either a numeric offset (within-category) or a category name (move to category).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PriorityAdjustment {
    /// Relative adjustment within current category (+10 = higher/earlier, -5 = lower/later)
    Offset(i32),
    /// Move to a different category entirely
    Category(String),
}

/// User-configurable priority settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriorityConfig {
    /// Category order (highest to lowest priority).
    /// Default: ["Primary", "Structured", "Semantic", "Encoding", "Raw"]
    #[serde(default)]
    pub category_order: Vec<String>,

    /// Per-format priority adjustments.
    /// Integer: +/- offset within category
    /// String: move to different category
    #[serde(default)]
    pub format_priority: std::collections::HashMap<String, PriorityAdjustment>,
}

impl PriorityConfig {
    /// Check if this config has any customizations.
    #[must_use]
    pub fn is_customized(&self) -> bool {
        !self.category_order.is_empty() || !self.format_priority.is_empty()
    }

    /// Get sort key for a category (lower = higher priority).
    #[must_use]
    pub fn category_sort_key(&self, priority: ConversionPriority) -> usize {
        if self.category_order.is_empty() {
            return priority as usize;
        }
        let name = priority.as_str();
        self.category_order
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
            .unwrap_or(100 + priority as usize) // Unlisted categories go last
    }

    /// Get the effective priority for a format, applying any overrides.
    #[must_use]
    pub fn resolve_priority(
        &self,
        format_id: &str,
        default: ConversionPriority,
    ) -> ConversionPriority {
        if let Some(PriorityAdjustment::Category(cat)) = self.format_priority.get(format_id) {
            if let Some(p) = ConversionPriority::parse(cat) {
                return p;
            }
        }
        default
    }

    /// Get the offset adjustment for a format (used for within-category sorting).
    #[must_use]
    pub fn format_offset(&self, format_id: &str) -> i32 {
        if let Some(PriorityAdjustment::Offset(off)) = self.format_priority.get(format_id) {
            *off
        } else {
            0
        }
    }
}

/// User-configurable blocking settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockingConfig {
    /// Blocked format IDs (never parse or convert to/from these formats).
    #[serde(default)]
    pub formats: Vec<String>,

    /// Blocked paths: specific source→target conversions to block.
    /// Format: "source:target" or "source:via:target" for multi-hop paths.
    #[serde(default)]
    pub paths: Vec<String>,

    /// Root-based blocking: block target formats based on root interpretation.
    /// Format: "root:target" - blocks reaching target from any path starting at root.
    /// Example: "text:ipv4" blocks text→bytes→ipv4, text→hex→bytes→ipv4, etc.
    #[serde(default)]
    pub root_paths: Vec<String>,
}

impl BlockingConfig {
    /// Check if this config has any customizations.
    #[must_use]
    pub fn is_customized(&self) -> bool {
        !self.formats.is_empty() || !self.paths.is_empty() || !self.root_paths.is_empty()
    }

    /// Check if a format is blocked.
    #[must_use]
    pub fn is_format_blocked(&self, format_id: &str) -> bool {
        self.formats
            .iter()
            .any(|f| f.eq_ignore_ascii_case(format_id))
    }

    /// Check if a conversion path is blocked.
    /// `path` should be the full path like ["hex", "int-be", "datetime"].
    #[must_use]
    pub fn is_path_blocked(&self, path: &[String]) -> bool {
        if path.is_empty() {
            return false;
        }
        let path_str = path.join(":");
        self.paths.iter().any(|blocked| {
            // Exact match or suffix match (for blocking a specific target)
            path_str.eq_ignore_ascii_case(blocked) || path_str.ends_with(&format!(":{}", blocked))
        })
    }

    /// Check if a target is blocked based on root interpretation.
    /// `root_format` is the original interpretation format (e.g., "text").
    /// `target_format` is the format we're trying to convert to.
    #[must_use]
    pub fn is_root_blocked(&self, root_format: &str, target_format: &str) -> bool {
        let pattern = format!("{}:{}", root_format, target_format);
        self.root_paths
            .iter()
            .any(|blocked| blocked.eq_ignore_ascii_case(&pattern))
    }
}

/// Combined user configuration for conversion behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionConfig {
    /// Priority settings.
    #[serde(default)]
    pub priority: PriorityConfig,

    /// Blocking settings.
    #[serde(default)]
    pub blocking: BlockingConfig,
}

impl ConversionConfig {
    /// Check if this config has any customizations.
    #[must_use]
    pub fn is_customized(&self) -> bool {
        self.priority.is_customized() || self.blocking.is_customized()
    }
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
///
/// # Display Strategy for UI Apps
///
/// Similar to [`Interpretation`], the `display` and `rich_display` fields may contain
/// the same information in different formats. UI applications should follow this priority:
///
/// 1. **If `rich_display` is not empty**: Render `rich_display[0].preferred` using
///    appropriate UI components. Do NOT also show `display` - it would be redundant.
///
/// 2. **If `rich_display` is empty**: Fall back to showing `display` as plain text.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Conversion {
    /// The converted value.
    pub value: CoreValue,

    /// Format ID of the conversion target (e.g., "base64", "datetime", "hex").
    pub target_format: String,

    /// Plain-text display of the converted value.
    ///
    /// Always populated. Used as fallback when `rich_display` is empty or unsupported.
    /// GUI apps should prefer `rich_display` when available.
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
    /// The kind of conversion (transformation, representation, or trait).
    #[serde(default)]
    pub kind: ConversionKind,
    /// If true, this is a display-only format - don't explore further conversions.
    ///
    /// Use this for representations that are meant for human viewing, not as
    /// intermediate values for further conversion. Examples:
    /// - `hex-int` (0xC82) - showing an integer in hex notation
    /// - `binary-int` (0b110010) - showing an integer in binary notation
    ///
    /// Without this flag, the BFS would convert "0xC82" string to bytes and
    /// produce nonsense like "binary of ASCII bytes of hex string".
    ///
    /// This is an internal implementation detail and not exposed via FFI/API.
    #[serde(skip)]
    pub display_only: bool,
    /// If true, this conversion is for internal chaining only - don't show in output.
    ///
    /// Use this for intermediate conversions that enable other conversions but
    /// whose display value is redundant. Example:
    /// - `bytes` from text - enables hashes/hex/base64 but "15 bytes" is shown by utf8-bytes
    #[serde(skip)]
    pub hidden: bool,
    /// Rich display hints for UI rendering.
    ///
    /// Multiple display options can be provided, each with a preferred
    /// rendering and alternatives. UIs choose based on their capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rich_display: Vec<RichDisplayOption>,
}

impl Conversion {
    /// Create a new Conversion with default rich_display (empty).
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
            kind: ConversionKind::default(),
            display_only: false,
            hidden: false,
            rich_display: vec![],
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
