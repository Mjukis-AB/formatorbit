//! Pressure format.
//!
//! Parses and converts between pressure units:
//! Pa, kPa, bar, mbar, atm, psi, mmHg

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number};

pub struct PressureFormat;

/// Units with multiplier to convert to pascals (base unit).
const UNITS: &[(&str, f64)] = &[
    // Full names
    ("kilopascals", 1000.0),
    ("kilopascal", 1000.0),
    ("pascals", 1.0),
    ("pascal", 1.0),
    ("atmospheres", 101325.0),
    ("atmosphere", 101325.0),
    ("millibars", 100.0),
    ("millibar", 100.0),
    ("bars", 100000.0),
    ("bar", 100000.0),
    // Abbreviations
    ("kPa", 1000.0),
    ("hPa", 100.0), // hectopascal = millibar
    ("Pa", 1.0),
    ("atm", 101325.0),
    ("mbar", 100.0),
    ("psi", 6894.76),
    ("mmHg", 133.322),
    ("torr", 133.322),
    ("inHg", 3386.39),
];

const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("pascals", "Pa", 1.0),
    ("kilopascals", "kPa", 1000.0),
    ("bar", "bar", 100000.0),
    ("psi", "psi", 6894.76),
    ("atmospheres", "atm", 101325.0),
];

impl PressureFormat {
    fn parse_pressure(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        for (suffix, multiplier) in UNITS {
            if let Some(num_str) = input.strip_suffix(suffix) {
                if let Some(value) = parse_number(num_str) {
                    let pa = value * multiplier;
                    return Some((pa, suffix));
                }
            }

            if suffix.len() > 3 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let pa = value * multiplier;
                        return Some((pa, suffix));
                    }
                }
            }
        }
        None
    }
}

impl Format for PressureFormat {
    fn id(&self) -> &'static str {
        "pressure"
    }

    fn name(&self) -> &'static str {
        "Pressure"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Pressure (Pa, kPa, bar, psi, atm)",
            examples: &["101.3kPa", "14.7psi", "1 atm", "1013 mbar"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((pa, _unit)) = Self::parse_pressure(input) else {
            return vec![];
        };

        if pa < 0.0 {
            return vec![];
        }

        let description = format!("{} Pa", format_value(pa));

        vec![Interpretation {
            value: CoreValue::Float(pa),
            source_format: "pressure".to_string(),
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
        let CoreValue::Float(pa) = value else {
            return vec![];
        };

        let pa = *pa;
        if pa < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = pa / multiplier;
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

    fn parse_to_pa(input: &str) -> Option<f64> {
        let format = PressureFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Float(pa) => Some(*pa),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_pa("1000Pa").unwrap() - 1000.0).abs() < 0.01);
        assert!((parse_to_pa("101.3kPa").unwrap() - 101300.0).abs() < 1.0);
        assert!((parse_to_pa("1 bar").unwrap() - 100000.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_atmosphere() {
        assert!((parse_to_pa("1atm").unwrap() - 101325.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_psi() {
        // 14.7 psi ≈ 1 atm
        assert!((parse_to_pa("14.7psi").unwrap() - 101353.0).abs() < 10.0);
    }
}
