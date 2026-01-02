//! FFI-safe types for UniFFI export.
//!
//! These types mirror the core types but are designed to be UniFFI-compatible.
//! They avoid problematic patterns like DateTime<Utc>, JsonValue, and complex
//! nested enums that UniFFI cannot handle.

use formatorbit_core::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionResult, ConversionStep,
    Interpretation, PacketSegment, RichDisplay, RichDisplayOption, TreeNode,
};

// ============================================================================
// Simple Enums (UniFFI handles these directly)
// ============================================================================

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiConversionKind {
    Conversion,
    Representation,
    Trait,
}

impl From<ConversionKind> for FfiConversionKind {
    fn from(k: ConversionKind) -> Self {
        match k {
            ConversionKind::Conversion => Self::Conversion,
            ConversionKind::Representation => Self::Representation,
            ConversionKind::Trait => Self::Trait,
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiConversionPriority {
    Primary,
    Structured,
    Semantic,
    Encoding,
    Raw,
}

impl From<ConversionPriority> for FfiConversionPriority {
    fn from(p: ConversionPriority) -> Self {
        match p {
            ConversionPriority::Primary => Self::Primary,
            ConversionPriority::Structured => Self::Structured,
            ConversionPriority::Semantic => Self::Semantic,
            ConversionPriority::Encoding => Self::Encoding,
            ConversionPriority::Raw => Self::Raw,
        }
    }
}

// ============================================================================
// Helper Structs
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiKeyValuePair {
    pub key: String,
    pub value: String,
}

// ============================================================================
// TreeNode (recursive struct)
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiTreeNode {
    pub label: String,
    pub value: Option<String>,
    pub children: Vec<FfiTreeNode>,
}

impl From<TreeNode> for FfiTreeNode {
    fn from(n: TreeNode) -> Self {
        Self {
            label: n.label,
            value: n.value,
            children: n.children.into_iter().map(Into::into).collect(),
        }
    }
}

// ============================================================================
// PacketSegment (recursive struct with bytes)
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiPacketSegment {
    pub offset: u32,
    pub length: u32,
    pub bytes: Vec<u8>,
    pub segment_type: String,
    pub label: String,
    pub decoded: String,
    pub children: Vec<FfiPacketSegment>,
}

impl From<PacketSegment> for FfiPacketSegment {
    fn from(s: PacketSegment) -> Self {
        Self {
            offset: s.offset as u32,
            length: s.length as u32,
            bytes: s.bytes,
            segment_type: s.segment_type,
            label: s.label,
            decoded: s.decoded,
            children: s.children.into_iter().map(Into::into).collect(),
        }
    }
}

// ============================================================================
// RichDisplay (complex enum with 14 variants)
// ============================================================================

#[derive(uniffi::Enum, Debug, Clone)]
pub enum FfiRichDisplay {
    KeyValue {
        pairs: Vec<FfiKeyValuePair>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Tree {
        root: FfiTreeNode,
    },
    Color {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
    Map {
        lat: f64,
        lon: f64,
        label: Option<String>,
    },
    Mermaid {
        source: String,
    },
    Dot {
        source: String,
    },
    Code {
        language: String,
        content: String,
    },
    Duration {
        millis: u64,
        human: String,
    },
    DateTime {
        epoch_millis: i64,
        iso: String,
        relative: String,
    },
    DataSize {
        bytes: u64,
        human: String,
    },
    PacketLayout {
        segments: Vec<FfiPacketSegment>,
        compact: String,
        detailed: String,
    },
    Image {
        format: String,
        /// Raw image bytes (not base64 encoded)
        data: Vec<u8>,
    },
    Progress {
        value: f64,
        label: Option<String>,
    },
    Markdown {
        content: String,
    },
    LiveClock {
        label: String,
    },
}

impl From<RichDisplay> for FfiRichDisplay {
    fn from(d: RichDisplay) -> Self {
        match d {
            RichDisplay::KeyValue { pairs } => Self::KeyValue {
                pairs: pairs
                    .into_iter()
                    .map(|(k, v)| FfiKeyValuePair { key: k, value: v })
                    .collect(),
            },
            RichDisplay::Table { headers, rows } => Self::Table { headers, rows },
            RichDisplay::Tree { root } => Self::Tree { root: root.into() },
            RichDisplay::Color { r, g, b, a } => Self::Color { r, g, b, a },
            RichDisplay::Map { lat, lon, label } => Self::Map { lat, lon, label },
            RichDisplay::Mermaid { source } => Self::Mermaid { source },
            RichDisplay::Dot { source } => Self::Dot { source },
            RichDisplay::Code { language, content } => Self::Code { language, content },
            RichDisplay::Duration { millis, human } => Self::Duration { millis, human },
            RichDisplay::DateTime {
                epoch_millis,
                iso,
                relative,
            } => Self::DateTime {
                epoch_millis,
                iso,
                relative,
            },
            RichDisplay::DataSize { bytes, human } => Self::DataSize { bytes, human },
            RichDisplay::PacketLayout {
                segments,
                compact,
                detailed,
            } => Self::PacketLayout {
                segments: segments.into_iter().map(Into::into).collect(),
                compact,
                detailed,
            },
            RichDisplay::Image { format, data } => Self::Image {
                format,
                // Decode base64 to raw bytes for FFI
                data: base64_decode(&data),
            },
            RichDisplay::Progress { value, label } => Self::Progress { value, label },
            RichDisplay::Markdown { content } => Self::Markdown { content },
            RichDisplay::LiveClock { label } => Self::LiveClock { label },
        }
    }
}

/// Decode base64 string to bytes, returning empty vec on error
fn base64_decode(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .unwrap_or_default()
}

// ============================================================================
// RichDisplayOption
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiRichDisplayOption {
    pub preferred: FfiRichDisplay,
    pub alternatives: Vec<FfiRichDisplay>,
}

impl From<RichDisplayOption> for FfiRichDisplayOption {
    fn from(o: RichDisplayOption) -> Self {
        Self {
            preferred: o.preferred.into(),
            alternatives: o.alternatives.into_iter().map(Into::into).collect(),
        }
    }
}

// ============================================================================
// ConversionStep
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiConversionStep {
    pub format: String,
    pub display: String,
}

impl From<ConversionStep> for FfiConversionStep {
    fn from(s: ConversionStep) -> Self {
        Self {
            format: s.format,
            display: s.display,
        }
    }
}

// ============================================================================
// Interpretation
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiInterpretation {
    pub source_format: String,
    pub confidence: f32,
    pub description: String,
    pub rich_display: Vec<FfiRichDisplayOption>,
}

impl From<Interpretation> for FfiInterpretation {
    fn from(i: Interpretation) -> Self {
        Self {
            source_format: i.source_format,
            confidence: i.confidence,
            description: i.description,
            rich_display: i.rich_display.into_iter().map(Into::into).collect(),
        }
    }
}

// ============================================================================
// Conversion
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiConversion {
    pub target_format: String,
    pub display: String,
    pub path: Vec<String>,
    pub steps: Vec<FfiConversionStep>,
    pub is_lossy: bool,
    pub priority: FfiConversionPriority,
    pub kind: FfiConversionKind,
    pub rich_display: Vec<FfiRichDisplayOption>,
}

impl From<Conversion> for FfiConversion {
    fn from(c: Conversion) -> Self {
        Self {
            target_format: c.target_format,
            display: c.display,
            path: c.path,
            steps: c.steps.into_iter().map(Into::into).collect(),
            is_lossy: c.is_lossy,
            priority: c.priority.into(),
            kind: c.kind.into(),
            rich_display: c.rich_display.into_iter().map(Into::into).collect(),
        }
    }
}

// ============================================================================
// ConversionResult (top-level)
// ============================================================================

#[derive(uniffi::Record, Debug, Clone)]
pub struct FfiConversionResult {
    pub input: String,
    pub interpretation: FfiInterpretation,
    pub conversions: Vec<FfiConversion>,
}

impl From<ConversionResult> for FfiConversionResult {
    fn from(r: ConversionResult) -> Self {
        Self {
            input: r.input,
            interpretation: r.interpretation.into(),
            conversions: r.conversions.into_iter().map(Into::into).collect(),
        }
    }
}
