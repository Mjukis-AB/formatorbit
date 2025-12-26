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
    AngleFormat, AreaFormat, Base64Format, BinaryFormat, BytesToIntFormat, ColorFormat, CuidFormat,
    CurrencyFormat, DataSizeFormat, DateTimeFormat, DecimalFormat, DurationFormat, EnergyFormat,
    EscapeFormat, ExprFormat, HashFormat, HexFormat, HexdumpFormat, IpAddrFormat, JsonFormat,
    JwtFormat, LengthFormat, MsgPackFormat, NanoIdFormat, OctalFormat, PlistFormat, PressureFormat,
    ProtobufFormat, SpeedFormat, TemperatureFormat, UlidFormat, UrlEncodingFormat, Utf8Format,
    UuidFormat, VolumeFormat, WeightFormat,
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
                Box::new(JwtFormat),
                Box::new(UlidFormat),
                Box::new(UuidFormat),
                Box::new(IpAddrFormat),
                Box::new(ColorFormat),
                Box::new(UrlEncodingFormat),
                // Identifier formats (lower specificity)
                Box::new(CuidFormat),
                Box::new(NanoIdFormat),
                // Common formats
                Box::new(HashFormat),
                Box::new(HexFormat),
                Box::new(BinaryFormat),
                Box::new(OctalFormat),
                Box::new(Base64Format),
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
                Box::new(Utf8Format),
                // Conversion-only formats (don't parse strings directly)
                Box::new(BytesToIntFormat),
                Box::new(HexdumpFormat),
                Box::new(MsgPackFormat),
                Box::new(PlistFormat),
                Box::new(ProtobufFormat),
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
        convert::find_all_conversions(&self.formats, value, None, None)
    }

    /// Find all possible conversions, excluding the source format (to avoid hex→hex etc.)
    /// The source_format is also included in the path to show the full conversion chain.
    pub fn convert_excluding(&self, value: &CoreValue, source_format: &str) -> Vec<Conversion> {
        convert::find_all_conversions(
            &self.formats,
            value,
            Some(source_format),
            Some(source_format),
        )
    }

    /// Combined: interpret input and find all conversions.
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
}
