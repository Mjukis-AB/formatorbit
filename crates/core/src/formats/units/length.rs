//! Length/distance format.
//!
//! Parses and converts between metric and imperial length units.
//! Supports all SI prefixes for meters (nm, µm, mm, cm, m, km, Mm, etc.)
//! plus imperial units (in, ft, yd, mi).

use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{
    format_decimal, format_scientific, format_value, format_with_si_prefix, parse_number,
    SI_PREFIXES,
};

pub struct LengthFormat;

/// Imperial units with multiplier to meters.
const IMPERIAL_UNITS: &[(&str, f64)] = &[
    // Full names (longest first)
    ("miles", 1609.344),
    ("mile", 1609.344),
    ("yards", 0.9144),
    ("yard", 0.9144),
    ("inches", 0.0254),
    ("inch", 0.0254),
    ("feet", 0.3048),
    ("foot", 0.3048),
    // Abbreviations
    ("mi", 1609.344),
    ("yd", 0.9144),
    ("in", 0.0254),
    ("ft", 0.3048),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("meters", "m", 1.0),
    ("kilometers", "km", 1000.0),
    ("centimeters", "cm", 0.01),
    ("millimeters", "mm", 0.001),
    ("feet", "ft", 0.3048),
    ("miles", "mi", 1609.344),
    ("inches", "in", 0.0254),
];

/// Get all length units (SI-prefixed meters + imperial).
/// Generated once and cached.
fn get_units() -> &'static Vec<(String, f64)> {
    static UNITS: OnceLock<Vec<(String, f64)>> = OnceLock::new();
    UNITS.get_or_init(|| {
        // Base meter unit
        let mut units = vec![
            ("m".to_string(), 1.0),
            ("meter".to_string(), 1.0),
            ("meters".to_string(), 1.0),
            ("metre".to_string(), 1.0),
            ("metres".to_string(), 1.0),
        ];

        // All SI prefixes for meter
        for prefix in SI_PREFIXES {
            let factor = prefix.factor();

            // Symbol form (km, mm, µm, nm, etc.)
            units.push((format!("{}m", prefix.symbol), factor));

            // Full name forms (kilometer, millimeter, etc.)
            units.push((format!("{}meter", prefix.name), factor));
            units.push((format!("{}meters", prefix.name), factor));
            units.push((format!("{}metre", prefix.name), factor));
            units.push((format!("{}metres", prefix.name), factor));
        }

        // Imperial units
        for (suffix, multiplier) in IMPERIAL_UNITS {
            units.push((suffix.to_string(), *multiplier));
        }

        // Sort by length descending to match longest first
        units.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        units
    })
}

impl LengthFormat {
    fn parse_length(input: &str) -> Option<(f64, String)> {
        let input = input.trim();
        let units = get_units();

        for (suffix, multiplier) in units {
            // Try exact suffix match
            if let Some(num_str) = input.strip_suffix(suffix.as_str()) {
                // For short units (≤2 chars), require number attached
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let meters = value * multiplier;
                    return Some((meters, suffix.clone()));
                }
            }

            // Try case-insensitive for longer suffixes
            if suffix.len() > 2 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let meters = value * multiplier;
                        return Some((meters, suffix.clone()));
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
            description: "Length/distance with SI prefixes (nm, µm, mm, m, km, etc.)",
            examples: &["5km", "100m", "3.5 miles", "500nm", "2.5µm"],
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

        // Use SI-prefixed display for extreme values, plain meters otherwise
        let abs_meters = meters.abs();
        let description = if abs_meters > 0.0 && !(0.01..1_000_000.0).contains(&abs_meters) {
            format_with_si_prefix(meters, "m")
        } else {
            format!("{} m", format_value(meters))
        };

        vec![Interpretation {
            value: CoreValue::Length(meters),
            source_format: "length".to_string(),
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
        let CoreValue::Length(meters) = value else {
            return vec![];
        };

        let meters = *meters;
        if meters < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        // Standard unit conversions
        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = meters / multiplier;
            let display = format!("{} {}", format_value(converted), abbrev);

            conversions.push(Conversion {
                value: CoreValue::Length(converted * multiplier), // Keep in meters
                target_format: (*name).to_string(),
                display: display.clone(),
                path: vec![(*name).to_string()],
                steps: vec![ConversionStep {
                    format: (*name).to_string(),
                    value: CoreValue::Length(converted * multiplier),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                ..Default::default()
            });
        }

        // Multiple representations for the base unit (meters)
        // These show the same value in different notations
        let si_display = format_with_si_prefix(meters, "m");
        let sci_display = format!("{} m", format_scientific(meters));
        let dec_display = format!("{} m", format_decimal(meters));

        // SI prefix representation (e.g., "5 nm", "2.5 µm", "3 km")
        conversions.push(Conversion {
            value: CoreValue::Length(meters),
            target_format: "meters-si".to_string(),
            display: si_display.clone(),
            path: vec!["meters-si".to_string()],
            steps: vec![ConversionStep {
                format: "meters-si".to_string(),
                value: CoreValue::Length(meters),
                display: si_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // Scientific notation (e.g., "5e-9 m")
        conversions.push(Conversion {
            value: CoreValue::Length(meters),
            target_format: "meters-scientific".to_string(),
            display: sci_display.clone(),
            path: vec!["meters-scientific".to_string()],
            steps: vec![ConversionStep {
                format: "meters-scientific".to_string(),
                value: CoreValue::Length(meters),
                display: sci_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // Full decimal representation (e.g., "0.[8zeros]5 m")
        conversions.push(Conversion {
            value: CoreValue::Length(meters),
            target_format: "meters-decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["meters-decimal".to_string()],
            steps: vec![ConversionStep {
                format: "meters-decimal".to_string(),
                value: CoreValue::Length(meters),
                display: dec_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

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
            CoreValue::Length(m) => Some(*m),
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
    fn test_parse_si_prefixes() {
        // Nanometers
        assert!((parse_to_meters("500nm").unwrap() - 5e-7).abs() < 1e-10);
        // Micrometers
        assert!((parse_to_meters("2µm").unwrap() - 2e-6).abs() < 1e-10);
        assert!((parse_to_meters("2um").unwrap() - 2e-6).abs() < 1e-10);
        // Megameters
        assert!((parse_to_meters("1Mm").unwrap() - 1e6).abs() < 0.01);
        // Gigameters
        assert!((parse_to_meters("1Gm").unwrap() - 1e9).abs() < 0.01);
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
        // SI prefix full names
        assert!((parse_to_meters("5 nanometers").unwrap() - 5e-9).abs() < 1e-12);
        assert!((parse_to_meters("100 micrometers").unwrap() - 1e-4).abs() < 1e-8);
    }

    #[test]
    fn test_parse_with_space() {
        assert!((parse_to_meters("5 km").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_meters("10 m").unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_unit_alone_rejected() {
        assert!(parse_to_meters("m").is_none());
        assert!(parse_to_meters("in").is_none());
        assert!(parse_to_meters("nm").is_none());
    }

    #[test]
    fn test_conversions() {
        let format = LengthFormat;
        let value = CoreValue::Length(1000.0); // 1000 meters = 1 km
        let conversions = format.conversions(&value);

        let km = conversions.iter().find(|c| c.target_format == "kilometers");
        assert!(km.is_some());
        assert!(km.unwrap().display.contains("1 km"));

        let mi = conversions.iter().find(|c| c.target_format == "miles");
        assert!(mi.is_some());
        assert!(mi.unwrap().display.contains("0.62")); // ~0.62 miles
    }
}
