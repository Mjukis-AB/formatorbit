//! Speed format.
//!
//! Parses and converts between speed units:
//! m/s, km/h, mph, knots, ft/s

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_decimal, format_scientific, format_value, parse_number};

pub struct SpeedFormat;

/// Units with multiplier to convert to m/s (base unit).
const UNITS: &[(&str, f64)] = &[
    // Full names
    ("meters per second", 1.0),
    ("kilometres per hour", 0.277778),
    ("kilometers per hour", 0.277778),
    ("miles per hour", 0.44704),
    ("knots", 0.514444),
    ("knot", 0.514444),
    ("feet per second", 0.3048),
    // Abbreviations
    ("m/s", 1.0),
    ("mps", 1.0),
    ("km/h", 0.277778),
    ("kmh", 0.277778),
    ("kph", 0.277778),
    ("mph", 0.44704),
    ("kn", 0.514444),
    ("kt", 0.514444),
    ("ft/s", 0.3048),
    ("fps", 0.3048),
];

const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("m/s", "m/s", 1.0),
    ("km/h", "km/h", 0.277778),
    ("mph", "mph", 0.44704),
    ("knots", "knots", 0.514444),
];

impl SpeedFormat {
    fn parse_speed(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        for (suffix, multiplier) in UNITS {
            if let Some(num_str) = input.strip_suffix(suffix) {
                if let Some(value) = parse_number(num_str) {
                    let mps = value * multiplier;
                    return Some((mps, suffix));
                }
            }

            if suffix.len() > 3 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let mps = value * multiplier;
                        return Some((mps, suffix));
                    }
                }
            }
        }
        None
    }
}

impl Format for SpeedFormat {
    fn id(&self) -> &'static str {
        "speed"
    }

    fn name(&self) -> &'static str {
        "Speed"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Speed (m/s, km/h, mph, knots)",
            examples: &["100km/h", "60mph", "10 m/s", "30 knots"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((mps, _unit)) = Self::parse_speed(input) else {
            return vec![];
        };

        if mps < 0.0 {
            return vec![];
        }

        let description = format!("{} m/s", format_value(mps));

        vec![Interpretation {
            value: CoreValue::Speed(mps),
            source_format: "speed".to_string(),
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
        let CoreValue::Speed(mps) = value else {
            return vec![];
        };

        let mps = *mps;
        if mps < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = mps / multiplier;
            let display = format!("{} {}", format_value(converted), abbrev);

            conversions.push(Conversion {
                value: CoreValue::Speed(mps),
                target_format: (*name).to_string(),
                display: display.clone(),
                path: vec![(*name).to_string()],
                steps: vec![ConversionStep {
                    format: (*name).to_string(),
                    value: CoreValue::Speed(mps),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                ..Default::default()
            });
        }

        // Additional representations for base unit (m/s)
        let sci_display = format!("{} m/s", format_scientific(mps));
        let dec_display = format!("{} m/s", format_decimal(mps));

        conversions.push(Conversion {
            value: CoreValue::Speed(mps),
            target_format: "m/s-scientific".to_string(),
            display: sci_display.clone(),
            path: vec!["m/s-scientific".to_string()],
            steps: vec![ConversionStep {
                format: "m/s-scientific".to_string(),
                value: CoreValue::Speed(mps),
                display: sci_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        conversions.push(Conversion {
            value: CoreValue::Speed(mps),
            target_format: "m/s-decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["m/s-decimal".to_string()],
            steps: vec![ConversionStep {
                format: "m/s-decimal".to_string(),
                value: CoreValue::Speed(mps),
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
        &["velocity"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_mps(input: &str) -> Option<f64> {
        let format = SpeedFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Speed(mps) => Some(*mps),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_mps("10m/s").unwrap() - 10.0).abs() < 0.01);
        assert!((parse_to_mps("36km/h").unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_imperial() {
        // 60 mph ≈ 26.82 m/s
        assert!((parse_to_mps("60mph").unwrap() - 26.82).abs() < 0.1);
    }

    #[test]
    fn test_parse_knots() {
        // 1 knot ≈ 0.514 m/s
        assert!((parse_to_mps("10 knots").unwrap() - 5.144).abs() < 0.01);
    }
}
