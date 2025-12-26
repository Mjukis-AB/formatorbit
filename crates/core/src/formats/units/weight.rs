//! Weight/mass format.
//!
//! Parses and converts between metric and imperial weight units:
//! mg, g, kg, oz, lb, stone

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{format_value, parse_number};

pub struct WeightFormat;

/// Units with multiplier to convert to grams (base unit).
const UNITS: &[(&str, f64)] = &[
    // Metric - full names
    ("kilograms", 1000.0),
    ("kilogram", 1000.0),
    ("milligrams", 0.001),
    ("milligram", 0.001),
    ("grams", 1.0),
    ("gram", 1.0),
    // Metric - abbreviations
    ("kg", 1000.0),
    ("mg", 0.001),
    ("g", 1.0),
    // Imperial - full names
    ("pounds", 453.592),
    ("pound", 453.592),
    ("ounces", 28.3495),
    ("ounce", 28.3495),
    ("stone", 6350.29),
    // Imperial - abbreviations
    ("lbs", 453.592),
    ("lb", 453.592),
    ("oz", 28.3495),
    ("st", 6350.29),
];

const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("grams", "g", 1.0),
    ("kilograms", "kg", 1000.0),
    ("pounds", "lb", 453.592),
    ("ounces", "oz", 28.3495),
];

impl WeightFormat {
    fn parse_weight(input: &str) -> Option<(f64, &'static str)> {
        let input = input.trim();

        for (suffix, multiplier) in UNITS {
            if let Some(num_str) = input.strip_suffix(suffix) {
                if suffix.len() <= 2 && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let grams = value * multiplier;
                    return Some((grams, suffix));
                }
            }

            if suffix.len() > 2 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(*suffix) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let grams = value * multiplier;
                        return Some((grams, suffix));
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
            description: "Weight/mass (g, kg, lb, oz)",
            examples: &["5kg", "150lbs", "100 grams", "8 oz"],
            aliases: self.aliases(),
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
            value: CoreValue::Float(grams),
            source_format: "weight".to_string(),
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
        let CoreValue::Float(grams) = value else {
            return vec![];
        };

        let grams = *grams;
        if grams < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = grams / multiplier;
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
            CoreValue::Float(g) => Some(*g),
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
    fn test_parse_imperial() {
        assert!((parse_to_grams("1lb").unwrap() - 453.592).abs() < 0.01);
        assert!((parse_to_grams("16oz").unwrap() - 453.592).abs() < 0.1);
    }

    #[test]
    fn test_parse_full_names() {
        assert!((parse_to_grams("5 kilograms").unwrap() - 5000.0).abs() < 0.01);
        assert!((parse_to_grams("2 pounds").unwrap() - 907.184).abs() < 0.01);
    }
}
