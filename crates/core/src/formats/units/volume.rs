//! Volume format.
//!
//! Parses and converts between metric and imperial volume units:
//! ml, L, fl oz, cup, pt, qt, gal

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number};

pub struct VolumeFormat;

/// Units with multiplier to convert to milliliters (base unit).
const UNITS: &[(&str, f64)] = &[
    // Metric - full names
    ("milliliters", 1.0),
    ("milliliter", 1.0),
    ("millilitres", 1.0),
    ("millilitre", 1.0),
    ("liters", 1000.0),
    ("liter", 1000.0),
    ("litres", 1000.0),
    ("litre", 1000.0),
    // Metric - abbreviations
    ("mL", 1.0),
    ("ml", 1.0),
    ("L", 1000.0),
    ("l", 1000.0),
    // Imperial - full names
    ("gallons", 3785.41),
    ("gallon", 3785.41),
    ("quarts", 946.353),
    ("quart", 946.353),
    ("pints", 473.176),
    ("pint", 473.176),
    ("cups", 236.588),
    ("cup", 236.588),
    ("fluid ounces", 29.5735),
    ("fluid ounce", 29.5735),
    // Imperial - abbreviations
    ("gal", 3785.41),
    ("qt", 946.353),
    ("pt", 473.176),
    ("fl oz", 29.5735),
    ("floz", 29.5735),
];

const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("milliliters", "ml", 1.0),
    ("liters", "L", 1000.0),
    ("gallons", "gal", 3785.41),
    ("fluid ounces", "fl oz", 29.5735),
    ("cups", "cups", 236.588),
];

impl VolumeFormat {
    fn parse_volume(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        for (suffix, multiplier) in UNITS {
            if let Some(num_str) = input.strip_suffix(suffix) {
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let ml = value * multiplier;
                    return Some((ml, suffix));
                }
            }

            if suffix.len() > 2 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let ml = value * multiplier;
                        return Some((ml, suffix));
                    }
                }
            }
        }
        None
    }
}

impl Format for VolumeFormat {
    fn id(&self) -> &'static str {
        "volume"
    }

    fn name(&self) -> &'static str {
        "Volume"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Volume (ml, L, gal, fl oz)",
            examples: &["500ml", "2L", "1 gallon", "8 fl oz"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((ml, _unit)) = Self::parse_volume(input) else {
            return vec![];
        };

        if ml < 0.0 {
            return vec![];
        }

        let description = format!("{} ml", format_value(ml));

        vec![Interpretation {
            value: CoreValue::Float(ml),
            source_format: "volume".to_string(),
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
        let CoreValue::Float(ml) = value else {
            return vec![];
        };

        let ml = *ml;
        if ml < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = ml / multiplier;
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
        &["vol", "capacity"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_ml(input: &str) -> Option<f64> {
        let format = VolumeFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Float(ml) => Some(*ml),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_ml("500ml").unwrap() - 500.0).abs() < 0.01);
        assert!((parse_to_ml("2L").unwrap() - 2000.0).abs() < 0.01);
        assert!((parse_to_ml("1.5 liters").unwrap() - 1500.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_imperial() {
        assert!((parse_to_ml("1gal").unwrap() - 3785.41).abs() < 0.01);
        assert!((parse_to_ml("8 fl oz").unwrap() - 236.588).abs() < 0.01);
        assert!((parse_to_ml("2 cups").unwrap() - 473.176).abs() < 0.01);
    }
}
