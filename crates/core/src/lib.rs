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
pub mod expr_context;
pub mod format;
pub mod formats;
pub mod plugin;
pub mod types;

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

pub use format::{Format, FormatInfo};
pub use plugin::{PluginError, PluginLoadReport, PluginRegistry};
pub use types::*;

use formats::{
    AngleFormat, ArchiveFormat, AreaFormat, AudioFormat, Base64Format, BinaryFormat,
    BytesToIntFormat, CharFormat, CidrFormat, ColorFormat, ConstantsFormat, CoordsFormat,
    CronFormat, CuidFormat, CurrencyFormat, DataSizeFormat, DateTimeFormat, DecimalFormat,
    DurationFormat, EnergyFormat, EpochFormat, EscapeFormat, ExprFormat, FontFormat, GraphFormat,
    HashFormat, HexFormat, HexdumpFormat, ImageFormat, IpAddrFormat, IsbnFormat, JsonFormat,
    JwtFormat, LengthFormat, MsgPackFormat, NanoIdFormat, NaturalDateFormat, OctalFormat,
    OfficeFormat, PdfFormat, PermissionsFormat, PlistFormat, PressureFormat, ProtobufFormat,
    SpeedFormat, TemperatureFormat, UlidFormat, UrlEncodingFormat, UrlParserFormat, Utf8Format,
    UuidFormat, VideoFormat, VolumeFormat, WeightFormat,
};

/// Main entry point - a configured converter instance.
pub struct Formatorbit {
    formats: Vec<Box<dyn Format>>,
    config: Option<ConversionConfig>,
    plugins: Option<PluginRegistry>,
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
            plugins: None,
        }
    }

    /// Create a new converter with custom configuration.
    #[must_use]
    pub fn with_config(config: ConversionConfig) -> Self {
        Self {
            formats: Self::create_format_list(),
            config: Some(config),
            plugins: None,
        }
    }

    /// Create a new converter with plugins enabled.
    ///
    /// Loads plugins from `~/.config/forb/plugins/` and any additional
    /// configured directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the Python runtime fails to initialize.
    /// Individual plugin load failures are logged but don't prevent
    /// the converter from being created.
    #[cfg(feature = "python")]
    pub fn with_plugins() -> Result<(Self, PluginLoadReport), PluginError> {
        let mut registry = PluginRegistry::new();
        let report = registry.load_default()?;

        // Set the global expression context for plugin variables/functions
        expr_context::set_from_registry(&registry);

        // Register plugin currencies with the rate cache
        Self::register_plugin_currencies(&registry);

        Ok((
            Self {
                formats: Self::create_format_list(),
                config: None,
                plugins: Some(registry),
            },
            report,
        ))
    }

    /// Register plugin currencies with the rate cache.
    #[cfg(feature = "python")]
    fn register_plugin_currencies(registry: &PluginRegistry) {
        use formats::currency_rates::{register_plugin_currency, PluginCurrencyInfo};

        for currency in registry.currencies() {
            if let Some((rate, base)) = currency.rate() {
                register_plugin_currency(
                    currency.code(),
                    PluginCurrencyInfo {
                        rate,
                        base_currency: base,
                        symbol: currency.symbol().to_string(),
                        decimals: currency.decimals(),
                    },
                );
            }
        }
    }

    /// Set the plugin registry.
    #[must_use]
    pub fn set_plugins(mut self, plugins: PluginRegistry) -> Self {
        self.plugins = Some(plugins);
        self
    }

    /// Get the plugin registry (if any).
    #[must_use]
    pub fn plugins(&self) -> Option<&PluginRegistry> {
        self.plugins.as_ref()
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
            Box::new(NaturalDateFormat),
            Box::new(ConstantsFormat),
            Box::new(PermissionsFormat),
            Box::new(UrlEncodingFormat),
            Box::new(UrlParserFormat),
            Box::new(CronFormat),
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

        // Built-in formats
        for format in &self.formats {
            // Skip blocked formats
            if let Some(ref config) = self.config {
                if config.blocking.is_format_blocked(format.id()) {
                    continue;
                }
            }
            results.extend(format.parse(input));
        }

        // Plugin decoders
        if let Some(ref plugins) = self.plugins {
            for decoder in plugins.decoders() {
                // Skip blocked plugins
                if let Some(ref config) = self.config {
                    if config.blocking.is_format_blocked(decoder.id()) {
                        continue;
                    }
                }
                results.extend(decoder.parse(input));
            }
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
        #[allow(unused_mut)]
        let mut conversions = convert::find_all_conversions(
            &self.formats,
            value,
            Some(source_format),
            Some(source_format),
            self.config.as_ref(),
        );

        // Add plugin traits
        #[cfg(feature = "python")]
        if let Some(ref plugins) = self.plugins {
            conversions.extend(self.get_plugin_traits(value, source_format, plugins));
        }

        conversions
    }

    /// Get trait conversions from plugins.
    #[cfg(feature = "python")]
    fn get_plugin_traits(
        &self,
        value: &CoreValue,
        source_format: &str,
        plugins: &PluginRegistry,
    ) -> Vec<Conversion> {
        use types::{ConversionKind, ConversionPriority, ConversionStep};

        let mut traits = Vec::new();

        // Get the value type name for filtering
        let value_type = match value {
            CoreValue::Int { .. } => "int",
            CoreValue::Float(_) => "float",
            CoreValue::String(_) => "string",
            CoreValue::Bytes(_) => "bytes",
            CoreValue::Bool(_) => "bool",
            CoreValue::DateTime(_) => "datetime",
            CoreValue::Json(_) => "json",
            _ => "",
        };

        for trait_plugin in plugins.traits() {
            // Check if this trait applies to this value type
            let applies = trait_plugin.value_types().is_empty()
                || trait_plugin.value_types().iter().any(|t| t == value_type);

            if !applies {
                continue;
            }

            // Call the trait's check method
            if let Some(description) = trait_plugin.check(value) {
                traits.push(Conversion {
                    value: value.clone(),
                    target_format: trait_plugin.id().to_string(),
                    display: description.clone(),
                    path: vec![source_format.to_string(), trait_plugin.id().to_string()],
                    is_lossy: false,
                    steps: vec![ConversionStep {
                        format: trait_plugin.id().to_string(),
                        value: value.clone(),
                        display: description,
                    }],
                    priority: ConversionPriority::Semantic,
                    kind: ConversionKind::Trait,
                    display_only: true,
                    hidden: false,
                    rich_display: vec![],
                });
            }
        }

        traits
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

    /// Convert raw bytes and return all possible interpretations.
    ///
    /// This creates a single bytes interpretation and runs the conversion graph.
    /// Specialized formats (image, archive, etc.) will be detected from bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use formatorbit_core::Formatorbit;
    ///
    /// let forb = Formatorbit::new();
    /// let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    /// let results = forb.convert_bytes(&png_header);
    /// assert!(!results.is_empty());
    /// ```
    #[must_use]
    pub fn convert_bytes(&self, data: &[u8]) -> Vec<ConversionResult> {
        self.convert_bytes_internal(data, &[])
    }

    /// Convert raw bytes with only the specified formats.
    #[must_use]
    pub fn convert_bytes_filtered(
        &self,
        data: &[u8],
        format_filter: &[String],
    ) -> Vec<ConversionResult> {
        self.convert_bytes_internal(data, format_filter)
    }

    /// Internal: Convert raw bytes with optional format filter.
    ///
    /// Creates interpretations directly from bytes:
    /// 1. Try specialized binary formats (image, archive, etc.)
    /// 2. Fall back to generic "bytes" interpretation
    fn convert_bytes_internal(
        &self,
        data: &[u8],
        format_filter: &[String],
    ) -> Vec<ConversionResult> {
        use base64::Engine;

        // For specialized formats (image, archive, etc.), we need to pass
        // the data as base64 since they expect string input.
        // But we only create ONE interpretation to avoid duplicate processing.
        let base64_input = base64::engine::general_purpose::STANDARD.encode(data);

        let mut interpretations = Vec::new();

        // Try specialized binary formats that can parse base64-encoded data
        let binary_formats = [
            "image", "archive", "video", "audio", "font", "pdf", "office",
        ];

        for format in &self.formats {
            // If filter is active, check if format matches
            if !format_filter.is_empty() {
                let matches = format_filter.iter().any(|name| format.matches_name(name));
                if !matches {
                    continue;
                }
            }

            // Only try formats that handle binary data
            let is_binary_format = binary_formats
                .iter()
                .any(|&bf| format.id() == bf || format.aliases().contains(&bf));
            if !is_binary_format {
                continue;
            }

            // Skip blocked formats
            if let Some(ref config) = self.config {
                if config.blocking.is_format_blocked(format.id()) {
                    continue;
                }
            }

            interpretations.extend(format.parse(&base64_input));
        }

        // If no specialized format matched, create a generic bytes interpretation
        if interpretations.is_empty() {
            interpretations.push(Interpretation {
                value: CoreValue::Bytes(data.to_vec()),
                source_format: "bytes".to_string(),
                confidence: 1.0,
                description: format!("{} bytes", data.len()),
                rich_display: vec![],
            });
        }

        // Sort by confidence, highest first
        interpretations.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));

        // Convert each interpretation
        interpretations
            .into_iter()
            .map(|interp| {
                let conversions = self.convert_excluding(&interp.value, &interp.source_format);
                ConversionResult {
                    input: base64_input.clone(),
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

    /// Get info about formats that have validation support.
    ///
    /// These formats provide detailed error messages when `--only` is used
    /// and parsing fails.
    #[must_use]
    pub fn formats_with_validation(&self) -> Vec<FormatInfo> {
        self.formats
            .iter()
            .map(|f| f.info())
            .filter(|info| info.has_validation)
            .collect()
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

    /// Test that words are not parsed as geohash (geohash input parsing removed)
    #[test]
    fn test_words_not_parsed_as_geohash() {
        let forb = Formatorbit::new();
        // "rustfmt" was previously parsed as geohash, now it should only be text
        let results = forb.convert_all("rustfmt");
        let formats: Vec<_> = results
            .iter()
            .map(|r| &r.interpretation.source_format)
            .collect();

        assert!(
            !formats.contains(&&"coords".to_string()),
            "should NOT have coords interpretation (geohash parsing removed)"
        );
        assert!(
            formats.contains(&&"text".to_string()),
            "should have text interpretation"
        );
    }
}
