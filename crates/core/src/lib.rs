//! Formatorbit Core
//!
//! A cross-platform data format converter. Input data (e.g., `691E01B8`) and
//! get all possible interpretations and conversions automatically.

pub mod convert;
pub mod format;
pub mod formats;
pub mod types;

pub use format::{Format, FormatInfo};
pub use types::*;

use formats::{
    Base64Format, BinaryFormat, BytesToIntFormat, ColorFormat, DateTimeFormat, DecimalFormat,
    HexFormat, IpAddrFormat, JsonFormat, MsgPackFormat, PlistFormat, UrlEncodingFormat, Utf8Format,
    UuidFormat,
};

/// Main entry point - a configured converter instance.
pub struct Formatorbit {
    formats: Vec<Box<dyn Format>>,
}

impl Formatorbit {
    /// Create with only built-in formats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            formats: vec![
                // High-specificity formats first
                Box::new(UuidFormat),
                Box::new(IpAddrFormat),
                Box::new(ColorFormat),
                Box::new(UrlEncodingFormat),
                // Common formats
                Box::new(HexFormat),
                Box::new(BinaryFormat),
                Box::new(Base64Format),
                Box::new(DecimalFormat),
                Box::new(DateTimeFormat),
                Box::new(JsonFormat),
                Box::new(Utf8Format),
                // Conversion-only formats (don't parse strings directly)
                Box::new(BytesToIntFormat),
                Box::new(MsgPackFormat),
                Box::new(PlistFormat),
            ],
        }
    }

    /// Parse input and return all possible interpretations.
    pub fn interpret(&self, input: &str) -> Vec<Interpretation> {
        let mut results = Vec::new();
        for format in &self.formats {
            results.extend(format.parse(input));
        }
        // Sort by confidence, highest first
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }

    /// Find all possible conversions from a value.
    pub fn convert(&self, value: &CoreValue) -> Vec<Conversion> {
        convert::find_all_conversions(&self.formats, value)
    }

    /// Combined: interpret input and find all conversions.
    pub fn convert_all(&self, input: &str) -> Vec<ConversionResult> {
        self.interpret(input)
            .into_iter()
            .map(|interp| {
                let conversions = self.convert(&interp.value);
                ConversionResult {
                    input: input.to_string(),
                    interpretation: interp,
                    conversions,
                }
            })
            .collect()
    }

    /// Get info about all registered formats (for help/documentation).
    pub fn format_infos(&self) -> Vec<FormatInfo> {
        self.formats.iter().map(|f| f.info()).collect()
    }

    /// Parse input with only the specified formats (by id or alias).
    /// If `format_filter` is empty, all formats are used.
    pub fn interpret_filtered(&self, input: &str, format_filter: &[String]) -> Vec<Interpretation> {
        if format_filter.is_empty() {
            return self.interpret(input);
        }

        let mut results = Vec::new();
        for format in &self.formats {
            // Check if this format matches any of the filter names
            let matches = format_filter.iter().any(|name| format.matches_name(name));
            if matches {
                results.extend(format.parse(input));
            }
        }
        // Sort by confidence, highest first
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }

    /// Combined: interpret input (with filter) and find all conversions.
    pub fn convert_all_filtered(
        &self,
        input: &str,
        format_filter: &[String],
    ) -> Vec<ConversionResult> {
        self.interpret_filtered(input, format_filter)
            .into_iter()
            .map(|interp| {
                let conversions = self.convert(&interp.value);
                ConversionResult {
                    input: input.to_string(),
                    interpretation: interp,
                    conversions,
                }
            })
            .collect()
    }
}

impl Default for Formatorbit {
    fn default() -> Self {
        Self::new()
    }
}
