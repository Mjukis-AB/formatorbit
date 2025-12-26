//! Area format.
//!
//! Parses and converts between area units.
//! Supports SI prefixes for square meters (nm², µm², mm², m², km², etc.)
//! plus imperial units (ft², acre, hectare).
//!
//! Note: For area, prefixes apply to the linear unit, then squared.
//! So 1 km² = (1000 m)² = 1,000,000 m²

use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number, SI_PREFIXES};

pub struct AreaFormat;

/// Non-metric units with multiplier to square meters (base unit).
const OTHER_UNITS: &[(&str, f64)] = &[
    // Full names
    ("square feet", 0.092903),
    ("square foot", 0.092903),
    ("square inches", 0.00064516),
    ("square inch", 0.00064516),
    ("square yards", 0.836127),
    ("square yard", 0.836127),
    ("square miles", 2.59e6),
    ("square mile", 2.59e6),
    ("hectares", 10000.0),
    ("hectare", 10000.0),
    ("acres", 4046.86),
    ("acre", 4046.86),
    // Abbreviations
    ("ft²", 0.092903),
    ("in²", 0.00064516),
    ("yd²", 0.836127),
    ("mi²", 2.59e6),
    ("ft2", 0.092903),
    ("in2", 0.00064516),
    ("yd2", 0.836127),
    ("mi2", 2.59e6),
    ("sqft", 0.092903),
    ("sqin", 0.00064516),
    ("sqyd", 0.836127),
    ("sqmi", 2.59e6),
    ("ha", 10000.0),
    ("ac", 4046.86),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("square meters", "m²", 1.0),
    ("square kilometers", "km²", 1e6),
    ("square centimeters", "cm²", 1e-4),
    ("square feet", "ft²", 0.092903),
    ("acres", "acres", 4046.86),
    ("hectares", "ha", 10000.0),
];

/// Get all area units (SI-prefixed square meters + others).
fn get_units() -> &'static Vec<(String, f64)> {
    static UNITS: OnceLock<Vec<(String, f64)>> = OnceLock::new();
    UNITS.get_or_init(|| {
        // Base square meter unit
        let mut units = vec![
            ("m²".to_string(), 1.0),
            ("m2".to_string(), 1.0),
            ("sqm".to_string(), 1.0),
            ("square meter".to_string(), 1.0),
            ("square meters".to_string(), 1.0),
            ("square metre".to_string(), 1.0),
            ("square metres".to_string(), 1.0),
        ];

        // SI prefixes for square meter
        // For area, the prefix applies to the linear dimension, then squared
        // So km² = (10³ m)² = 10⁶ m²
        for prefix in SI_PREFIXES {
            // Square the linear factor for area
            let factor = prefix.factor() * prefix.factor();

            // Symbol forms with superscript ²
            units.push((format!("{}m²", prefix.symbol), factor));
            units.push((format!("{}m2", prefix.symbol), factor));
            units.push((format!("sq{}m", prefix.symbol), factor));

            // Full name forms
            units.push((format!("square {}meter", prefix.name), factor));
            units.push((format!("square {}meters", prefix.name), factor));
            units.push((format!("square {}metre", prefix.name), factor));
            units.push((format!("square {}metres", prefix.name), factor));
        }

        // Other units
        for (suffix, multiplier) in OTHER_UNITS {
            units.push((suffix.to_string(), *multiplier));
        }

        // Sort by length descending to match longest first
        units.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        units
    })
}

impl AreaFormat {
    fn parse_area(input: &str) -> Option<(f64, String)> {
        let input = input.trim();
        let units = get_units();

        for (suffix, multiplier) in units {
            if let Some(num_str) = input.strip_suffix(suffix.as_str()) {
                if let Some(value) = parse_number(num_str) {
                    let sqm = value * multiplier;
                    return Some((sqm, suffix.clone()));
                }
            }

            // Case-insensitive matching only for full-word suffixes (no SI prefix symbols)
            // Skip case-insensitive matching for short suffixes that contain SI prefixes
            // where case matters (M vs m, G vs g, etc.)
            if suffix.len() > 6 && !suffix.contains('²') && !suffix.contains('2') {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let sqm = value * multiplier;
                        return Some((sqm, suffix.clone()));
                    }
                }
            }
        }
        None
    }
}

impl Format for AreaFormat {
    fn id(&self) -> &'static str {
        "area"
    }

    fn name(&self) -> &'static str {
        "Area"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Area with SI prefixes (mm², cm², m², km², etc.)",
            examples: &["100m²", "500 sqft", "2 acres", "1 km²"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((sqm, _unit)) = Self::parse_area(input) else {
            return vec![];
        };

        if sqm < 0.0 {
            return vec![];
        }

        let description = format!("{} m²", format_value(sqm));

        vec![Interpretation {
            value: CoreValue::Float(sqm),
            source_format: "area".to_string(),
            confidence: 0.85,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Float(sqm) = value else {
            return vec![];
        };

        let sqm = *sqm;
        if sqm < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = sqm / multiplier;
            let display = format!("{} {}", format_value(converted), abbrev);

            conversions.push(Conversion {
                value: CoreValue::Float(converted),
                target_format: (*name).to_string(),
                display: display.clone(),
                path: vec![(*name).to_string()],
                steps: vec![ConversionStep {
                    format: (*name).to_string(),
                    value: CoreValue::Float(converted),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                ..Default::default()
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_sqm(input: &str) -> Option<f64> {
        let format = AreaFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Float(sqm) => Some(*sqm),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_sqm("100m²").unwrap() - 100.0).abs() < 0.01);
        assert!((parse_to_sqm("1km²").unwrap() - 1e6).abs() < 1.0);
        assert!((parse_to_sqm("100 sqm").unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_si_prefixes() {
        // Square millimeters: (10⁻³)² = 10⁻⁶
        assert!((parse_to_sqm("1000000mm²").unwrap() - 1.0).abs() < 0.01);
        // Square centimeters: (10⁻²)² = 10⁻⁴
        assert!((parse_to_sqm("10000cm²").unwrap() - 1.0).abs() < 0.01);
        // Square micrometers: (10⁻⁶)² = 10⁻¹²
        assert!((parse_to_sqm("1µm²").unwrap() - 1e-12).abs() < 1e-15);
    }

    #[test]
    fn test_parse_imperial() {
        assert!((parse_to_sqm("100sqft").unwrap() - 9.2903).abs() < 0.01);
        assert!((parse_to_sqm("1 acre").unwrap() - 4046.86).abs() < 0.01);
    }

    #[test]
    fn test_parse_hectare() {
        assert!((parse_to_sqm("1ha").unwrap() - 10000.0).abs() < 0.01);
        assert!((parse_to_sqm("2 hectares").unwrap() - 20000.0).abs() < 0.01);
    }
}
