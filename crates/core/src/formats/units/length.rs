//! Length/distance format.
//!
//! Parses and converts between metric and imperial length units:
//! mm, cm, m, km, in, ft, yd, mi

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number};

pub struct LengthFormat;

/// Units with multiplier to convert to meters (base unit).
/// Sorted longest-first to avoid partial matches.
const UNITS: &[(&str, f64)] = &[
    // Metric - full names
    ("kilometers", 1000.0),
    ("kilometer", 1000.0),
    ("centimeters", 0.01),
    ("centimeter", 0.01),
    ("millimeters", 0.001),
    ("millimeter", 0.001),
    ("meters", 1.0),
    ("meter", 1.0),
    // Metric - abbreviations
    ("km", 1000.0),
    ("cm", 0.01),
    ("mm", 0.001),
    ("m", 1.0),
    // Imperial - full names
    ("miles", 1609.344),
    ("mile", 1609.344),
    ("yards", 0.9144),
    ("yard", 0.9144),
    ("inches", 0.0254),
    ("inch", 0.0254),
    ("feet", 0.3048),
    ("foot", 0.3048),
    // Imperial - abbreviations
    ("mi", 1609.344),
    ("yd", 0.9144),
    ("in", 0.0254),
    ("ft", 0.3048),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("meters", "m", 1.0),
    ("kilometers", "km", 1000.0),
    ("feet", "ft", 0.3048),
    ("miles", "mi", 1609.344),
    ("inches", "in", 0.0254),
    ("centimeters", "cm", 0.01),
];

impl LengthFormat {
    fn parse_length(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        for (suffix, multiplier) in UNITS {
            // Try exact suffix match
            if let Some(num_str) = input.strip_suffix(suffix) {
                // For short units (m, in), require number attached
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let meters = value * multiplier;
                    return Some((meters, suffix));
                }
            }

            // Try case-insensitive for longer suffixes
            if suffix.len() > 2 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(*suffix) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let meters = value * multiplier;
                        return Some((meters, suffix));
                    }
                }
            }
        }
        None
    }
}

impl Format for LengthFormat {
    fn id(&self) -> &'static str {
        "length"
    }

    fn name(&self) -> &'static str {
        "Length"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Length/distance (m, km, ft, mi, in)",
            examples: &["5km", "100m", "3.5 miles", "12 inches"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((meters, _unit)) = Self::parse_length(input) else {
            return vec![];
        };

        // Reject negative lengths
        if meters < 0.0 {
            return vec![];
        }

        let description = format!("{} {}", format_value(meters), "m");

        vec![Interpretation {
            value: CoreValue::Float(meters),
            source_format: "length".to_string(),
            confidence: 0.85,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Don't claim to format generic floats - conversions handle display
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Float(meters) = value else {
            return vec![];
        };

        let meters = *meters;
        if meters < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = meters / multiplier;
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
        &["distance", "len"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_meters(input: &str) -> Option<f64> {
        let format = LengthFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Float(m) => Some(*m),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_meters("5km").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_meters("100m").unwrap() - 100.0).abs() < 0.01);
        assert!((parse_to_meters("50cm").unwrap() - 0.5).abs() < 0.01);
        assert!((parse_to_meters("10mm").unwrap() - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_parse_imperial() {
        assert!((parse_to_meters("1mi").unwrap() - 1609.344).abs() < 0.01);
        assert!((parse_to_meters("3ft").unwrap() - 0.9144).abs() < 0.01);
        assert!((parse_to_meters("12in").unwrap() - 0.3048).abs() < 0.01);
    }

    #[test]
    fn test_parse_full_names() {
        assert!((parse_to_meters("5 kilometers").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_meters("10 meters").unwrap() - 10.0).abs() < 0.01);
        assert!((parse_to_meters("3 miles").unwrap() - 4828.032).abs() < 0.01);
        assert!((parse_to_meters("6 feet").unwrap() - 1.8288).abs() < 0.01);
    }

    #[test]
    fn test_parse_with_space() {
        assert!((parse_to_meters("5 km").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_meters("10 m").unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_unit_alone_rejected() {
        // Short units alone should not parse
        assert!(parse_to_meters("m").is_none());
        assert!(parse_to_meters("in").is_none());
    }

    #[test]
    fn test_conversions() {
        let format = LengthFormat;
        let value = CoreValue::Float(1000.0); // 1000 meters = 1 km
        let conversions = format.conversions(&value);

        let km = conversions.iter().find(|c| c.target_format == "kilometers");
        assert!(km.is_some());
        assert!(km.unwrap().display.contains("1 km"));

        let mi = conversions.iter().find(|c| c.target_format == "miles");
        assert!(mi.is_some());
        assert!(mi.unwrap().display.contains("0.62")); // ~0.62 miles
    }
}
