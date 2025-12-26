//! Angle format.
//!
//! Parses and converts between angle units:
//! degrees, radians, gradians, turns

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number};

pub struct AngleFormat;

/// Units with multiplier to convert to degrees (base unit).
const UNITS: &[(&str, f64)] = &[
    // Full names
    ("degrees", 1.0),
    ("degree", 1.0),
    ("radians", 57.2957795),
    ("radian", 57.2957795),
    ("gradians", 0.9),
    ("gradian", 0.9),
    ("turns", 360.0),
    ("turn", 360.0),
    ("revolutions", 360.0),
    ("revolution", 360.0),
    // Abbreviations
    ("deg", 1.0),
    ("rad", 57.2957795),
    ("grad", 0.9),
    ("gon", 0.9),
    ("rev", 360.0),
    // Note: Skip "°" alone to avoid conflict with temperature
    // Temperature will check for C/F/K after °
];

const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("degrees", "°", 1.0),
    ("radians", "rad", 57.2957795),
    ("gradians", "grad", 0.9),
    ("turns", "turns", 360.0),
];

impl AngleFormat {
    fn parse_angle(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        // Special handling for degree symbol without letter suffix
        // Only match if it's just "°" not "°C" or "°F"
        if input.ends_with('°')
            && !input.ends_with("°C")
            && !input.ends_with("°F")
            && !input.ends_with("°c")
            && !input.ends_with("°f")
        {
            let num_str = &input[..input.len() - '°'.len_utf8()];
            if let Some(value) = parse_number(num_str) {
                return Some((value, "°"));
            }
        }

        for (suffix, multiplier) in UNITS {
            if let Some(num_str) = input.strip_suffix(suffix) {
                if let Some(value) = parse_number(num_str) {
                    let degrees = value * multiplier;
                    return Some((degrees, suffix));
                }
            }

            if suffix.len() > 3 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let degrees = value * multiplier;
                        return Some((degrees, suffix));
                    }
                }
            }
        }
        None
    }
}

impl Format for AngleFormat {
    fn id(&self) -> &'static str {
        "angle"
    }

    fn name(&self) -> &'static str {
        "Angle"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Angles (degrees, radians, gradians)",
            examples: &["90deg", "3.14rad", "45°", "0.25 turns"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((degrees, _unit)) = Self::parse_angle(input) else {
            return vec![];
        };

        let description = format!("{}°", format_value(degrees));

        vec![Interpretation {
            value: CoreValue::Float(degrees),
            source_format: "angle".to_string(),
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
        let CoreValue::Float(degrees) = value else {
            return vec![];
        };

        let degrees = *degrees;
        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = degrees / multiplier;
            let display = format!("{}{}", format_value(converted), abbrev);

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
    use std::f64::consts::PI;

    fn parse_to_degrees(input: &str) -> Option<f64> {
        let format = AngleFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Float(deg) => Some(*deg),
            _ => None,
        }
    }

    #[test]
    fn test_parse_degrees() {
        assert!((parse_to_degrees("90deg").unwrap() - 90.0).abs() < 0.01);
        assert!((parse_to_degrees("45°").unwrap() - 45.0).abs() < 0.01);
        assert!((parse_to_degrees("180 degrees").unwrap() - 180.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_radians() {
        // π rad = 180°
        let deg = parse_to_degrees(&format!("{}rad", PI)).unwrap();
        assert!((deg - 180.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_turns() {
        assert!((parse_to_degrees("0.5 turns").unwrap() - 180.0).abs() < 0.01);
        assert!((parse_to_degrees("1 turn").unwrap() - 360.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_gradians() {
        // 100 grad = 90°
        assert!((parse_to_degrees("100grad").unwrap() - 90.0).abs() < 0.01);
    }
}
