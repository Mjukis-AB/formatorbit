//! Formatorbit Core
//!
//! A cross-platform data format converter. Input data (e.g., `691E01B8`) and
//! get all possible interpretations and conversions automatically.
//!
//! # Quick Start
//!
//! ```
//! use formatorbit_core::Formatorbit;
//!
//! let forb = Formatorbit::new();
//!
//! // Get all interpretations and conversions
//! let results = forb.convert_all("691E01B8");
//! assert!(!results.is_empty());
//!
//! // The highest-confidence interpretation is first
//! let best = &results[0];
//! println!("Format: {}", best.interpretation.source_format);
//! println!("Confidence: {:.0}%", best.interpretation.confidence * 100.0);
//!
//! // Each interpretation has conversions to other formats
//! for conv in &best.conversions[..3.min(best.conversions.len())] {
//!     println!("  → {}: {}", conv.target_format, conv.display);
//! }
//! ```
//!
//! # Filtering by Format
//!
//! ```
//! use formatorbit_core::Formatorbit;
//!
//! let forb = Formatorbit::new();
//!
//! // Force interpretation as a specific format
//! let results = forb.convert_all_filtered("1703456789", &["epoch".into()]);
//! assert_eq!(results[0].interpretation.source_format, "epoch-seconds");
//! ```

pub mod convert;

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
///
/// This is UTF-8 safe - it counts characters, not bytes.
#[must_use]
pub fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}
pub mod format;
pub mod formats;
pub mod types;

pub use format::{Format, FormatInfo};
pub use types::*;

use formats::{
    AngleFormat, ArchiveFormat, AreaFormat, AudioFormat, Base64Format, BinaryFormat,
    BytesToIntFormat, CharFormat, CidrFormat, ColorFormat, ConstantsFormat, CoordsFormat,
    CuidFormat, CurrencyFormat, DataSizeFormat, DateTimeFormat, DecimalFormat, DigestFormat,
    DurationFormat, EnergyFormat, EpochFormat, EscapeFormat, ExprFormat, FontFormat, GraphFormat,
    HashFormat, HexFormat, HexdumpFormat, ImageFormat, IpAddrFormat, IsbnFormat, JsonFormat,
    JwtFormat, LengthFormat, MsgPackFormat, NanoIdFormat, OctalFormat, OfficeFormat, PdfFormat,
    PermissionsFormat, PlistFormat, PressureFormat, ProtobufFormat, SpeedFormat, TemperatureFormat,
    UlidFormat, UrlEncodingFormat, Utf8Format, UuidFormat, VideoFormat, VolumeFormat, WeightFormat,
};

/// Main entry point - a configured converter instance.
pub struct Formatorbit {
    formats: Vec<Box<dyn Format>>,
    config: Option<ConversionConfig>,
}

impl Formatorbit {
    /// Create a new converter with all built-in formats.
    ///
    /// # Examples
    ///
    /// ```
    /// use formatorbit_core::Formatorbit;
    ///
    /// let forb = Formatorbit::new();
    /// let results = forb.convert_all("0xDEADBEEF");
    /// assert!(!results.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            formats: Self::create_format_list(),
            config: None,
        }
    }

    /// Create a new converter with custom configuration.
    #[must_use]
    pub fn with_config(config: ConversionConfig) -> Self {
        Self {
            formats: Self::create_format_list(),
            config: Some(config),
        }
    }

    /// Set the configuration.
    #[must_use]
    pub fn set_config(mut self, config: ConversionConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Get the current configuration (if any).
    #[must_use]
    pub fn config(&self) -> Option<&ConversionConfig> {
        self.config.as_ref()
    }

    /// Create the list of built-in formats.
    fn create_format_list() -> Vec<Box<dyn Format>> {
        vec![
            // High-specificity formats first
            Box::new(JwtFormat),
            Box::new(UlidFormat),
            Box::new(UuidFormat),
            Box::new(IpAddrFormat),
            Box::new(CidrFormat),
            Box::new(CoordsFormat),
            Box::new(ColorFormat),
            Box::new(CharFormat),
            Box::new(ConstantsFormat),
            Box::new(PermissionsFormat),
            Box::new(UrlEncodingFormat),
            // Identifier formats (lower specificity)
            Box::new(IsbnFormat),
            Box::new(CuidFormat),
            Box::new(NanoIdFormat),
            // Common formats
            Box::new(HashFormat),
            Box::new(HexFormat),
            Box::new(BinaryFormat),
            Box::new(OctalFormat),
            Box::new(Base64Format),
            Box::new(EpochFormat),
            Box::new(DecimalFormat),
            Box::new(DataSizeFormat),
            Box::new(TemperatureFormat),
            // Unit conversions
            Box::new(LengthFormat),
            Box::new(WeightFormat),
            Box::new(VolumeFormat),
            Box::new(SpeedFormat),
            Box::new(PressureFormat),
            Box::new(AngleFormat),
            Box::new(AreaFormat),
            Box::new(EnergyFormat),
            Box::new(CurrencyFormat),
            Box::new(ExprFormat),
            Box::new(EscapeFormat),
            Box::new(DurationFormat),
            Box::new(DateTimeFormat),
            Box::new(JsonFormat),
            Box::new(GraphFormat),
            Box::new(Utf8Format),
            // Conversion-only formats (don't parse strings directly)
            Box::new(BytesToIntFormat),
            Box::new(DigestFormat),
            Box::new(HexdumpFormat),
            Box::new(ImageFormat),
            Box::new(MsgPackFormat),
            Box::new(PlistFormat),
            Box::new(ProtobufFormat),
            // Binary file metadata formats
            Box::new(ArchiveFormat),
            Box::new(AudioFormat),
            Box::new(FontFormat),
            Box::new(OfficeFormat),
            Box::new(PdfFormat),
            Box::new(VideoFormat),
        ]
    }

    /// Parse input and return all possible interpretations.
    ///
    /// Returns interpretations sorted by confidence (highest first).
    ///
    /// # Examples
    ///
    /// ```
    /// use formatorbit_core::Formatorbit;
    ///
    /// let forb = Formatorbit::new();
    /// let interps = forb.interpret("550e8400-e29b-41d4-a716-446655440000");
    ///
    /// // UUID has high confidence due to its distinctive format
    /// assert_eq!(interps[0].source_format, "uuid");
    /// assert!(interps[0].confidence > 0.9);
    /// ```
    #[must_use]
    pub fn interpret(&self, input: &str) -> Vec<Interpretation> {
        let mut results = Vec::new();
        for format in &self.formats {
            // Skip blocked formats
            if let Some(ref config) = self.config {
                if config.blocking.is_format_blocked(format.id()) {
                    continue;
                }
            }
            results.extend(format.parse(input));
        }
        // Sort by confidence, highest first
        results.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        results
    }

    /// Find all possible conversions from a value.
    #[must_use]
    pub fn convert(&self, value: &CoreValue) -> Vec<Conversion> {
        convert::find_all_conversions(&self.formats, value, None, None, self.config.as_ref())
    }

    /// Find all possible conversions, excluding the source format (to avoid hex→hex etc.)
    /// The source_format is also included in the path to show the full conversion chain.
    #[must_use]
    pub fn convert_excluding(&self, value: &CoreValue, source_format: &str) -> Vec<Conversion> {
        convert::find_all_conversions(
            &self.formats,
            value,
            Some(source_format),
            Some(source_format),
            self.config.as_ref(),
        )
    }

    /// Combined: interpret input and find all conversions.
    ///
    /// This is the main entry point for most use cases. It parses the input,
    /// finds all possible interpretations, and for each interpretation,
    /// discovers all possible conversions via BFS traversal.
    ///
    /// # Examples
    ///
    /// ```
    /// use formatorbit_core::Formatorbit;
    ///
    /// let forb = Formatorbit::new();
    /// let results = forb.convert_all("1703456789");
    ///
    /// // Find the epoch timestamp interpretation
    /// let epoch = results.iter()
    ///     .find(|r| r.interpretation.source_format == "epoch-seconds")
    ///     .expect("should find epoch interpretation");
    ///
    /// // Check that datetime conversion is available
    /// let has_datetime = epoch.conversions.iter()
    ///     .any(|c| c.target_format == "datetime");
    /// assert!(has_datetime);
    /// ```
    #[must_use]
    pub fn convert_all(&self, input: &str) -> Vec<ConversionResult> {
        self.interpret(input)
            .into_iter()
            .map(|interp| {
                // Skip self-conversion (e.g., hex→hex)
                let conversions = self.convert_excluding(&interp.value, &interp.source_format);
                ConversionResult {
                    input: input.to_string(),
                    interpretation: interp,
                    conversions,
                }
            })
            .collect()
    }

    /// Get info about all registered formats (for help/documentation).
    #[must_use]
    pub fn format_infos(&self) -> Vec<FormatInfo> {
        self.formats.iter().map(|f| f.info()).collect()
    }

    /// Parse input with only the specified formats (by id or alias).
    /// If `format_filter` is empty, all formats are used.
    #[must_use]
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
        results.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        results
    }

    /// Combined: interpret input (with filter) and find all conversions.
    #[must_use]
    pub fn convert_all_filtered(
        &self,
        input: &str,
        format_filter: &[String],
    ) -> Vec<ConversionResult> {
        self.interpret_filtered(input, format_filter)
            .into_iter()
            .map(|interp| {
                // Skip self-conversion (e.g., hex→hex)
                let conversions = self.convert_excluding(&interp.value, &interp.source_format);
                ConversionResult {
                    input: input.to_string(),
                    interpretation: interp,
                    conversions,
                }
            })
            .collect()
    }

    /// Validate input for a specific format and return an error message if invalid.
    ///
    /// This is useful when a user requests a specific format (e.g., `--only json`)
    /// and we want to explain why parsing failed.
    ///
    /// Returns `None` if the format doesn't provide validation or the input is valid.
    pub fn validate(&self, input: &str, format_name: &str) -> Option<String> {
        for format in &self.formats {
            if format.matches_name(format_name) {
                return format.validate(input);
            }
        }
        None
    }

    /// Check if a format name (id or alias) is valid.
    #[must_use]
    pub fn is_valid_format(&self, name: &str) -> bool {
        self.formats.iter().any(|f| f.matches_name(name))
    }

    /// Get a list of all valid format names (ids only, not aliases).
    #[must_use]
    pub fn format_ids(&self) -> Vec<&'static str> {
        self.formats.iter().map(|f| f.id()).collect()
    }
}

impl Default for Formatorbit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test: SHA-1 hash detection should appear in results
    #[test]
    fn test_sha1_hash_interpretation() {
        let forb = Formatorbit::new();
        // SHA-1 of empty string
        let results = forb.convert_all("da39a3ee5e6b4b0d3255bfef95601890afd80709");

        let has_hash = results
            .iter()
            .any(|r| r.interpretation.source_format == "hash");

        assert!(
            has_hash,
            "Expected 'hash' interpretation but got: {:?}",
            results
                .iter()
                .map(|r| &r.interpretation.source_format)
                .collect::<Vec<_>>()
        );

        // Verify hash description mentions SHA-1
        let hash_result = results
            .iter()
            .find(|r| r.interpretation.source_format == "hash")
            .unwrap();
        assert!(hash_result.interpretation.description.contains("SHA-1"));
    }

    /// Test that geohash-like words show both coords and text in core
    /// (CLI may filter low-confidence interpretations for cleaner output)
    #[test]
    fn test_geohash_word_returns_multiple_interpretations() {
        let forb = Formatorbit::new();
        // "rustfmt" is valid geohash but core should return both interpretations
        let results = forb.convert_all("rustfmt");
        let formats: Vec<_> = results
            .iter()
            .map(|r| &r.interpretation.source_format)
            .collect();

        assert!(
            formats.contains(&&"coords".to_string()),
            "should have coords interpretation"
        );
        assert!(
            formats.contains(&&"text".to_string()),
            "should have text interpretation (low confidence fallback)"
        );
    }
}
