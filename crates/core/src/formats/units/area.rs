//! Area format.
//!
//! Parses and converts between area units:
//! m², km², ft², acre, hectare

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number};

pub struct AreaFormat;

/// Units with multiplier to convert to square meters (base unit).
const UNITS: &[(&str, f64)] = &[
    // Full names
    ("square kilometers", 1e6),
    ("square kilometer", 1e6),
    ("square metres", 1.0),
    ("square meters", 1.0),
    ("square metre", 1.0),
    ("square meter", 1.0),
    ("square centimeters", 1e-4),
    ("square centimeter", 1e-4),
    ("square millimeters", 1e-6),
    ("square millimeter", 1e-6),
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
    // Abbreviations with superscript
    ("km²", 1e6),
    ("m²", 1.0),
    ("cm²", 1e-4),
    ("mm²", 1e-6),
    ("ft²", 0.092903),
    ("in²", 0.00064516),
    ("yd²", 0.836127),
    ("mi²", 2.59e6),
    // Abbreviations with 2
    ("km2", 1e6),
    ("m2", 1.0),
    ("cm2", 1e-4),
    ("mm2", 1e-6),
    ("ft2", 0.092903),
    ("in2", 0.00064516),
    ("yd2", 0.836127),
    ("mi2", 2.59e6),
    // sq prefix
    ("sqkm", 1e6),
    ("sqm", 1.0),
    ("sqcm", 1e-4),
    ("sqmm", 1e-6),
    ("sqft", 0.092903),
    ("sqin", 0.00064516),
    ("sqyd", 0.836127),
    ("sqmi", 2.59e6),
    // Other
    ("ha", 10000.0),
    ("ac", 4046.86),
];

const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("square meters", "m²", 1.0),
    ("square feet", "ft²", 0.092903),
    ("acres", "acres", 4046.86),
    ("hectares", "ha", 10000.0),
    ("square kilometers", "km²", 1e6),
];

impl AreaFormat {
    fn parse_area(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        for (suffix, multiplier) in UNITS {
            if let Some(num_str) = input.strip_suffix(suffix) {
                if let Some(value) = parse_number(num_str) {
                    let sqm = value * multiplier;
                    return Some((sqm, suffix));
                }
            }

            if suffix.len() > 3 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let sqm = value * multiplier;
                        return Some((sqm, suffix));
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
            description: "Area (m², ft², acre, hectare)",
            examples: &["100m²", "500 sqft", "2 acres", "1 hectare"],
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
