//! Pressure format.
//!
//! Parses and converts between pressure units.
//! Supports all SI prefixes for pascals (nPa, µPa, mPa, Pa, kPa, MPa, GPa, etc.)
//! plus other common units (bar, atm, psi, mmHg).

use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{
    format_decimal, format_scientific, format_value, format_with_si_prefix, parse_number,
    SI_PREFIXES,
};

pub struct PressureFormat;

/// Non-SI units with multiplier to pascals (base unit).
const OTHER_UNITS: &[(&str, f64)] = &[
    // Full names
    ("atmospheres", 101325.0),
    ("atmosphere", 101325.0),
    ("millibars", 100.0),
    ("millibar", 100.0),
    ("bars", 100000.0),
    ("bar", 100000.0),
    // Abbreviations
    ("atm", 101325.0),
    ("mbar", 100.0),
    ("psi", 6894.76),
    ("mmHg", 133.322),
    ("torr", 133.322),
    ("inHg", 3386.39),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("pascals", "Pa", 1.0),
    ("kilopascals", "kPa", 1000.0),
    ("megapascals", "MPa", 1e6),
    ("bar", "bar", 100000.0),
    ("psi", "psi", 6894.76),
    ("atmospheres", "atm", 101325.0),
];

/// Get all pressure units (SI-prefixed pascals + others).
fn get_units() -> &'static Vec<(String, f64)> {
    static UNITS: OnceLock<Vec<(String, f64)>> = OnceLock::new();
    UNITS.get_or_init(|| {
        let mut units = Vec::new();

        // Base pascal unit
        units.push(("Pa".to_string(), 1.0));
        units.push(("pascal".to_string(), 1.0));
        units.push(("pascals".to_string(), 1.0));

        // All SI prefixes for pascal
        for prefix in SI_PREFIXES {
            let factor = prefix.factor();

            // Symbol form (kPa, MPa, GPa, etc.)
            units.push((format!("{}Pa", prefix.symbol), factor));

            // Full name forms (kilopascal, megapascal, etc.)
            units.push((format!("{}pascal", prefix.name), factor));
            units.push((format!("{}pascals", prefix.name), factor));
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

impl PressureFormat {
    fn parse_pressure(input: &str) -> Option<(f64, String)> {
        let input = input.trim();
        let units = get_units();

        for (suffix, multiplier) in units {
            if let Some(num_str) = input.strip_suffix(suffix.as_str()) {
                // For short units (Pa), require number attached
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let pa = value * multiplier;
                    return Some((pa, suffix.clone()));
                }
            }

            if suffix.len() > 3 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let pa = value * multiplier;
                        return Some((pa, suffix.clone()));
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
            description: "Pressure with SI prefixes (Pa, kPa, MPa, GPa, etc.)",
            examples: &["101.3kPa", "14.7psi", "1 atm", "1 GPa"],
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
            value: CoreValue::Pressure(pa),
            source_format: "pressure".to_string(),
            confidence: 0.85,
            description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Pressure(pa) = value else {
            return vec![];
        };

        let pa = *pa;
        if pa < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        // Primary result: decimal pascals (canonical base unit value)
        let dec_display = format!("{} Pa", format_decimal(pa));
        conversions.push(Conversion {
            value: CoreValue::Pressure(pa),
            target_format: "pascals-decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["pascals-decimal".to_string()],
            steps: vec![ConversionStep {
                format: "pascals-decimal".to_string(),
                value: CoreValue::Pressure(pa),
                display: dec_display,
            }],
            priority: ConversionPriority::Primary,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // Standard unit conversions
        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = pa / multiplier;
            let display = format!("{} {}", format_value(converted), abbrev);

            // psi and atmospheres are true conversions (different systems)
            // pascals, kPa, MPa, bar are metric representations
            let is_non_metric = matches!(*name, "psi" | "atmospheres");
            let kind = if is_non_metric {
                ConversionKind::Conversion
            } else {
                ConversionKind::Representation
            };

            conversions.push(Conversion {
                value: CoreValue::Pressure(pa),
                target_format: (*name).to_string(),
                display: display.clone(),
                path: vec![(*name).to_string()],
                steps: vec![ConversionStep {
                    format: (*name).to_string(),
                    value: CoreValue::Pressure(pa),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind,
                ..Default::default()
            });
        }

        // Additional representations for the base unit (pascals)
        let si_display = format_with_si_prefix(pa, "Pa");
        let sci_display = format!("{} Pa", format_scientific(pa));

        conversions.push(Conversion {
            value: CoreValue::Pressure(pa),
            target_format: "pascals-si".to_string(),
            display: si_display.clone(),
            path: vec!["pascals-si".to_string()],
            steps: vec![ConversionStep {
                format: "pascals-si".to_string(),
                value: CoreValue::Pressure(pa),
                display: si_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        conversions.push(Conversion {
            value: CoreValue::Pressure(pa),
            target_format: "pascals-scientific".to_string(),
            display: sci_display.clone(),
            path: vec!["pascals-scientific".to_string()],
            steps: vec![ConversionStep {
                format: "pascals-scientific".to_string(),
                value: CoreValue::Pressure(pa),
                display: sci_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

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
            CoreValue::Pressure(pa) => Some(*pa),
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
    fn test_parse_si_prefixes() {
        // Megapascals
        assert!((parse_to_pa("1MPa").unwrap() - 1e6).abs() < 0.01);
        // Gigapascals
        assert!((parse_to_pa("1GPa").unwrap() - 1e9).abs() < 0.01);
        // Hectopascals (millibar equivalent)
        assert!((parse_to_pa("1013hPa").unwrap() - 101300.0).abs() < 1.0);
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
