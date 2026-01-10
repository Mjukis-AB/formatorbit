//! Weight/mass format.
//!
//! Parses and converts between metric and imperial weight units.
//! Supports all SI prefixes for grams (ng, µg, mg, g, kg, Mg, etc.)
//! plus imperial units (oz, lb, stone).

use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{
    format_decimal, format_scientific, format_value, format_with_si_prefix, parse_number,
    SI_PREFIXES,
};

pub struct WeightFormat;

/// Imperial units with multiplier to grams.
const IMPERIAL_UNITS: &[(&str, f64)] = &[
    // Full names (longest first)
    ("pounds", 453.592),
    ("pound", 453.592),
    ("ounces", 28.3495),
    ("ounce", 28.3495),
    ("stone", 6350.29),
    ("stones", 6350.29),
    // Abbreviations
    ("lbs", 453.592),
    ("lb", 453.592),
    ("oz", 28.3495),
    ("st", 6350.29),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("grams", "g", 1.0),
    ("kilograms", "kg", 1000.0),
    ("milligrams", "mg", 0.001),
    ("pounds", "lb", 453.592),
    ("ounces", "oz", 28.3495),
];

/// Get all weight units (SI-prefixed grams + imperial).
fn get_units() -> &'static Vec<(String, f64)> {
    static UNITS: OnceLock<Vec<(String, f64)>> = OnceLock::new();
    UNITS.get_or_init(|| {
        // Base gram unit
        let mut units = vec![
            ("g".to_string(), 1.0),
            ("gram".to_string(), 1.0),
            ("grams".to_string(), 1.0),
            ("gramme".to_string(), 1.0),
            ("grammes".to_string(), 1.0),
        ];

        // All SI prefixes for gram
        for prefix in SI_PREFIXES {
            let factor = prefix.factor();

            // Symbol form (kg, mg, µg, ng, etc.)
            units.push((format!("{}g", prefix.symbol), factor));

            // Full name forms (kilogram, milligram, etc.)
            units.push((format!("{}gram", prefix.name), factor));
            units.push((format!("{}grams", prefix.name), factor));
            units.push((format!("{}gramme", prefix.name), factor));
            units.push((format!("{}grammes", prefix.name), factor));
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

impl WeightFormat {
    fn parse_weight(input: &str) -> Option<(f64, String)> {
        let input = input.trim();
        let units = get_units();

        for (suffix, multiplier) in units {
            if let Some(num_str) = input.strip_suffix(suffix.as_str()) {
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let grams = value * multiplier;
                    return Some((grams, suffix.clone()));
                }
            }

            if suffix.len() > 2 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let grams = value * multiplier;
                        return Some((grams, suffix.clone()));
                    }
                }
            }
        }
        None
    }
}

impl Format for WeightFormat {
    fn id(&self) -> &'static str {
        "weight"
    }

    fn name(&self) -> &'static str {
        "Weight"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Weight/mass with SI prefixes (ng, µg, mg, g, kg, etc.)",
            examples: &["5kg", "150lbs", "100mg", "50µg"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((grams, _unit)) = Self::parse_weight(input) else {
            return vec![];
        };

        if grams < 0.0 {
            return vec![];
        }

        let description = format!("{} g", format_value(grams));

        vec![Interpretation {
            value: CoreValue::Weight(grams),
            source_format: "weight".to_string(),
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
        let CoreValue::Weight(grams) = value else {
            return vec![];
        };

        let grams = *grams;
        if grams < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        // Primary result: decimal grams (canonical base unit value)
        let dec_display = format!("{} g", format_decimal(grams));
        conversions.push(Conversion {
            value: CoreValue::Weight(grams),
            target_format: "grams-decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["grams-decimal".to_string()],
            steps: vec![ConversionStep {
                format: "grams-decimal".to_string(),
                value: CoreValue::Weight(grams),
                display: dec_display,
            }],
            priority: ConversionPriority::Primary,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // Standard unit conversions
        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = grams / multiplier;
            let display = format!("{} {}", format_value(converted), abbrev);

            // Imperial units (pounds, ounces) are true conversions
            // Metric units (grams, kg, mg) are representations
            let is_imperial = matches!(*name, "pounds" | "ounces");
            let kind = if is_imperial {
                ConversionKind::Conversion
            } else {
                ConversionKind::Representation
            };

            conversions.push(Conversion {
                value: CoreValue::Weight(grams),
                target_format: (*name).to_string(),
                display: display.clone(),
                path: vec![(*name).to_string()],
                steps: vec![ConversionStep {
                    format: (*name).to_string(),
                    value: CoreValue::Weight(grams),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind,
                ..Default::default()
            });
        }

        // Additional representations for the base unit (grams)
        let si_display = format_with_si_prefix(grams, "g");
        let sci_display = format!("{} g", format_scientific(grams));

        conversions.push(Conversion {
            value: CoreValue::Weight(grams),
            target_format: "grams-si".to_string(),
            display: si_display.clone(),
            path: vec!["grams-si".to_string()],
            steps: vec![ConversionStep {
                format: "grams-si".to_string(),
                value: CoreValue::Weight(grams),
                display: si_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        conversions.push(Conversion {
            value: CoreValue::Weight(grams),
            target_format: "grams-scientific".to_string(),
            display: sci_display.clone(),
            path: vec!["grams-scientific".to_string()],
            steps: vec![ConversionStep {
                format: "grams-scientific".to_string(),
                value: CoreValue::Weight(grams),
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
        &["mass"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_grams(input: &str) -> Option<f64> {
        let format = WeightFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Weight(g) => Some(*g),
            _ => None,
        }
    }

    #[test]
    fn test_parse_metric() {
        assert!((parse_to_grams("5kg").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_grams("100g").unwrap() - 100.0).abs() < 0.01);
        assert!((parse_to_grams("500mg").unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_si_prefixes() {
        // Micrograms
        assert!((parse_to_grams("50µg").unwrap() - 5e-5).abs() < 1e-8);
        assert!((parse_to_grams("50ug").unwrap() - 5e-5).abs() < 1e-8);
        // Nanograms
        assert!((parse_to_grams("100ng").unwrap() - 1e-7).abs() < 1e-10);
        // Megagrams (metric tons)
        assert!((parse_to_grams("1Mg").unwrap() - 1e6).abs() < 0.01);
    }

    #[test]
    fn test_parse_imperial() {
        assert!((parse_to_grams("1lb").unwrap() - 453.592).abs() < 0.01);
        assert!((parse_to_grams("16oz").unwrap() - 453.592).abs() < 0.1);
    }

    #[test]
    fn test_parse_full_names() {
        assert!((parse_to_grams("5 kilograms").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_grams("2 pounds").unwrap() - 907.184).abs() < 0.01);
        assert!((parse_to_grams("100 micrograms").unwrap() - 1e-4).abs() < 1e-8);
    }

    #[test]
    fn test_unit_alone_rejected() {
        assert!(parse_to_grams("g").is_none());
        assert!(parse_to_grams("kg").is_none());
    }
}
