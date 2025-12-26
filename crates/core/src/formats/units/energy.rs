//! Energy format.
//!
//! Parses and converts between energy units.
//! Supports all SI prefixes for joules (nJ, µJ, mJ, J, kJ, MJ, GJ, etc.)
//! plus other common units (cal, kcal, Wh, kWh, BTU).

use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

use super::{
    format_decimal, format_scientific, format_value, format_with_si_prefix, parse_number,
    SI_PREFIXES,
};

pub struct EnergyFormat;

/// Non-SI units with multiplier to joules (base unit).
const OTHER_UNITS: &[(&str, f64)] = &[
    // Full names
    ("kilocalories", 4184.0),
    ("kilocalorie", 4184.0),
    ("calories", 4.184),
    ("calorie", 4.184),
    ("kilowatt-hours", 3.6e6),
    ("kilowatt-hour", 3.6e6),
    ("kilowatthours", 3.6e6),
    ("kilowatthour", 3.6e6),
    ("watt-hours", 3600.0),
    ("watt-hour", 3600.0),
    ("watthours", 3600.0),
    ("watthour", 3600.0),
    // Abbreviations
    ("kcal", 4184.0),
    ("Cal", 4184.0), // Food calorie = kcal
    ("cal", 4.184),
    ("kWh", 3.6e6),
    ("Wh", 3600.0),
    ("BTU", 1055.06),
    ("btu", 1055.06),
    ("eV", 1.602e-19),
    ("electronvolt", 1.602e-19),
    ("electronvolts", 1.602e-19),
];

/// Units to display in conversions (most useful subset).
const DISPLAY_UNITS: &[(&str, &str, f64)] = &[
    ("joules", "J", 1.0),
    ("kilojoules", "kJ", 1000.0),
    ("megajoules", "MJ", 1e6),
    ("calories", "cal", 4.184),
    ("kilocalories", "kcal", 4184.0),
    ("kilowatt-hours", "kWh", 3.6e6),
];

/// Get all energy units (SI-prefixed joules + others).
fn get_units() -> &'static Vec<(String, f64)> {
    static UNITS: OnceLock<Vec<(String, f64)>> = OnceLock::new();
    UNITS.get_or_init(|| {
        let mut units = Vec::new();

        // Base joule unit
        units.push(("J".to_string(), 1.0));
        units.push(("joule".to_string(), 1.0));
        units.push(("joules".to_string(), 1.0));

        // All SI prefixes for joule
        for prefix in SI_PREFIXES {
            let factor = prefix.factor();

            // Symbol form (kJ, MJ, GJ, etc.)
            units.push((format!("{}J", prefix.symbol), factor));

            // Full name forms (kilojoule, megajoule, etc.)
            units.push((format!("{}joule", prefix.name), factor));
            units.push((format!("{}joules", prefix.name), factor));
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

impl EnergyFormat {
    fn parse_energy(input: &str) -> Option<(f64, String)> {
        let input = input.trim();
        let units = get_units();

        for (suffix, multiplier) in units {
            if let Some(num_str) = input.strip_suffix(suffix.as_str()) {
                // For single letter J, require number attached
                if suffix == "J" && num_str.trim().is_empty() {
                    continue;
                }
                if let Some(value) = parse_number(num_str) {
                    let joules = value * multiplier;
                    return Some((joules, suffix.clone()));
                }
            }

            if suffix.len() > 3 {
                let input_lower = input.to_lowercase();
                if input_lower.ends_with(&suffix.to_lowercase()) {
                    let num_str = &input[..input.len() - suffix.len()];
                    if let Some(value) = parse_number(num_str) {
                        let joules = value * multiplier;
                        return Some((joules, suffix.clone()));
                    }
                }
            }
        }
        None
    }
}

impl Format for EnergyFormat {
    fn id(&self) -> &'static str {
        "energy"
    }

    fn name(&self) -> &'static str {
        "Energy"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Energy with SI prefixes (J, kJ, MJ, GJ, etc.)",
            examples: &["100kJ", "500 calories", "1 kWh", "1 MJ"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((joules, _unit)) = Self::parse_energy(input) else {
            return vec![];
        };

        if joules < 0.0 {
            return vec![];
        }

        let description = format!("{} J", format_value(joules));

        vec![Interpretation {
            value: CoreValue::Energy(joules),
            source_format: "energy".to_string(),
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
        let CoreValue::Energy(joules) = value else {
            return vec![];
        };

        let joules = *joules;
        if joules < 0.0 {
            return vec![];
        }

        let mut conversions = Vec::new();

        // Primary result: decimal joules (canonical base unit value)
        let dec_display = format!("{} J", format_decimal(joules));
        conversions.push(Conversion {
            value: CoreValue::Energy(joules),
            target_format: "joules-decimal".to_string(),
            display: dec_display.clone(),
            path: vec!["joules-decimal".to_string()],
            steps: vec![ConversionStep {
                format: "joules-decimal".to_string(),
                value: CoreValue::Energy(joules),
                display: dec_display,
            }],
            priority: ConversionPriority::Primary,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        // Standard unit conversions
        for (name, abbrev, multiplier) in DISPLAY_UNITS {
            let converted = joules / multiplier;
            let display = format!("{} {}", format_value(converted), abbrev);

            // calories, kcal, kWh are true conversions (different systems)
            // joules, kJ, MJ are metric representations
            let is_non_si = matches!(*name, "calories" | "kilocalories" | "kilowatt-hours");
            let kind = if is_non_si {
                ConversionKind::Conversion
            } else {
                ConversionKind::Representation
            };

            conversions.push(Conversion {
                value: CoreValue::Energy(joules),
                target_format: (*name).to_string(),
                display: display.clone(),
                path: vec![(*name).to_string()],
                steps: vec![ConversionStep {
                    format: (*name).to_string(),
                    value: CoreValue::Energy(joules),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind,
                ..Default::default()
            });
        }

        // Additional representations for the base unit (joules)
        let si_display = format_with_si_prefix(joules, "J");
        let sci_display = format!("{} J", format_scientific(joules));

        conversions.push(Conversion {
            value: CoreValue::Energy(joules),
            target_format: "joules-si".to_string(),
            display: si_display.clone(),
            path: vec!["joules-si".to_string()],
            steps: vec![ConversionStep {
                format: "joules-si".to_string(),
                value: CoreValue::Energy(joules),
                display: si_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });

        conversions.push(Conversion {
            value: CoreValue::Energy(joules),
            target_format: "joules-scientific".to_string(),
            display: sci_display.clone(),
            path: vec!["joules-scientific".to_string()],
            steps: vec![ConversionStep {
                format: "joules-scientific".to_string(),
                value: CoreValue::Energy(joules),
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

    fn parse_to_joules(input: &str) -> Option<f64> {
        let format = EnergyFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Energy(j) => Some(*j),
            _ => None,
        }
    }

    #[test]
    fn test_parse_joules() {
        assert!((parse_to_joules("1000J").unwrap() - 1000.0).abs() < 0.01);
        assert!((parse_to_joules("1kJ").unwrap() - 1000.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_si_prefixes() {
        // Megajoules
        assert!((parse_to_joules("1MJ").unwrap() - 1e6).abs() < 0.01);
        // Gigajoules
        assert!((parse_to_joules("1GJ").unwrap() - 1e9).abs() < 0.01);
        // Millijoules
        assert!((parse_to_joules("1000mJ").unwrap() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_calories() {
        assert!((parse_to_joules("1cal").unwrap() - 4.184).abs() < 0.01);
        assert!((parse_to_joules("1kcal").unwrap() - 4184.0).abs() < 0.01);
        assert!((parse_to_joules("100 calories").unwrap() - 418.4).abs() < 0.1);
    }

    #[test]
    fn test_parse_kwh() {
        assert!((parse_to_joules("1kWh").unwrap() - 3.6e6).abs() < 1.0);
        assert!((parse_to_joules("1 Wh").unwrap() - 3600.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_btu() {
        assert!((parse_to_joules("1BTU").unwrap() - 1055.06).abs() < 0.1);
    }
}
