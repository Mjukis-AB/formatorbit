//! Volume format.
//!
//! Parses and converts between metric and imperial volume units.
//! Supports all SI prefixes for liters (nL, µL, mL, L, kL, etc.)
//! plus imperial units (fl oz, cup, pt, qt, gal).

use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{
    format_decimal, format_scientific, format_value, format_with_si_prefix, parse_number,
    SI_PREFIXES,
};

pub struct VolumeFormat;

/// Imperial units with multiplier to milliliters (base unit).
const IMPERIAL_UNITS: &[(&str, f64)] = &[
    // Full names (longest first)
    ("fluid ounces", 29.5735),
    ("fluid ounce", 29.5735),
    ("gallons", 3785.41),
    ("gallon", 3785.41),
    ("quarts", 946.353),
    ("quart", 946.353),
    ("pints", 473.176),
    ("pint", 473.176),
    ("cups", 236.588),
    ("cup", 236.588),
    // Abbreviations
    ("fl oz", 29.5735),
    ("floz", 29.5735),
    ("gal", 3785.41),
    ("qt", 946.353),
    ("pt", 473.176),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("milliliters", "mL", 1.0),
    ("liters", "L", 1000.0),
    ("gallons", "gal", 3785.41),
    ("fluid ounces", "fl oz", 29.5735),
    ("cups", "cups", 236.588),
];

/// Get all volume units (SI-prefixed liters + imperial).
fn get_units() -> &'static Vec<(String, f64)> {
    static UNITS: OnceLock<Vec<(String, f64)>> = OnceLock::new();
    UNITS.get_or_init(|| {
        // Base liter unit (milliliter is our internal base)
        // 1 L = 1000 mL
        let mut units = vec![
            ("L".to_string(), 1000.0),
            ("l".to_string(), 1000.0),
            ("liter".to_string(), 1000.0),
            ("liters".to_string(), 1000.0),
            ("litre".to_string(), 1000.0),
            ("litres".to_string(), 1000.0),
        ];

        // All SI prefixes for liter
        for prefix in SI_PREFIXES {
            // Factor relative to base liter, then multiply by 1000 to get mL
            let factor = prefix.factor() * 1000.0;

            // Symbol forms (kL, mL, µL, nL, etc.)
            units.push((format!("{}L", prefix.symbol), factor));
            units.push((format!("{}l", prefix.symbol), factor));

            // Full name forms (kiloliter, milliliter, etc.)
            units.push((format!("{}liter", prefix.name), factor));
            units.push((format!("{}liters", prefix.name), factor));
            units.push((format!("{}litre", prefix.name), factor));
            units.push((format!("{}litres", prefix.name), factor));
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

impl VolumeFormat {
    fn parse_volume(input: &str) -> Option<(f64, String)> {
        let input = input.trim();
        let units = get_units();

        for (suffix, multiplier) in units {
            if let Some(num_str) = input.strip_suffix(suffix.as_str()) {
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let ml = value * multiplier;
                    return Some((ml, suffix.clone()));
                }
            }

            if suffix.len() > 2 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let ml = value * multiplier;
                        return Some((ml, suffix.clone()));
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
            description: "Volume with SI prefixes (nL, µL, mL, L, kL, etc.)",
            examples: &["500mL", "2L", "1 gallon", "8 fl oz", "100µL"],
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

        let description = format!("{} mL", format_value(ml));

        vec![Interpretation {
            value: CoreValue::Volume(ml),
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
        let CoreValue::Volume(ml) = value else {
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

        // Multiple representations for the base unit (milliliters)
        let si_display = format_with_si_prefix(ml / 1000.0, "L"); // Convert mL to L for SI prefix
        let sci_display = format!("{} mL", format_scientific(ml));
        let dec_display = format!("{} mL", format_decimal(ml));

        conversions.push(Conversion {
            value: CoreValue::Volume(ml),
            target_format: "liters-si".to_string(),
            display: si_display.clone(),
            path: vec!["liters-si".to_string()],
            steps: vec![ConversionStep {
                format: "liters-si".to_string(),
                value: CoreValue::Volume(ml),
                display: si_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        conversions.push(Conversion {
            value: CoreValue::Volume(ml),
            target_format: "milliliters-scientific".to_string(),
            display: sci_display.clone(),
            path: vec!["milliliters-scientific".to_string()],
            steps: vec![ConversionStep {
                format: "milliliters-scientific".to_string(),
                value: CoreValue::Volume(ml),
                display: sci_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        conversions.push(Conversion {
            value: CoreValue::Volume(ml),
            target_format: "milliliters-decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["milliliters-decimal".to_string()],
            steps: vec![ConversionStep {
                format: "milliliters-decimal".to_string(),
                value: CoreValue::Volume(ml),
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
            CoreValue::Volume(ml) => Some(*ml),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_ml("500mL").unwrap() - 500.0).abs() < 0.01);
        assert!((parse_to_ml("2L").unwrap() - 2000.0).abs() < 0.01);
        assert!((parse_to_ml("1.5 liters").unwrap() - 1500.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_si_prefixes() {
        // Microliters
        assert!((parse_to_ml("100µL").unwrap() - 0.1).abs() < 1e-6);
        assert!((parse_to_ml("100uL").unwrap() - 0.1).abs() < 1e-6);
        // Nanoliters
        assert!((parse_to_ml("1000nL").unwrap() - 0.001).abs() < 1e-9);
        // Kiloliters
        assert!((parse_to_ml("1kL").unwrap() - 1e6).abs() < 0.01);
    }

    #[test]
    fn test_parse_imperial() {
        assert!((parse_to_ml("1gal").unwrap() - 3785.41).abs() < 0.01);
        assert!((parse_to_ml("8 fl oz").unwrap() - 236.588).abs() < 0.01);
        assert!((parse_to_ml("2 cups").unwrap() - 473.176).abs() < 0.01);
    }

    #[test]
    fn test_unit_alone_rejected() {
        assert!(parse_to_ml("L").is_none());
        assert!(parse_to_ml("mL").is_none());
    }
}
