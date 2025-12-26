//! Temperature format.
//!
//! Parses temperatures in Celsius, Fahrenheit, and Kelvin:
//! - `72°F`, `72F`, `72 F`, `72 Fahrenheit`
//! - `20°C`, `20C`, `20 C`, `20 Celsius`
//! - `300K`, `300 K`, `300 Kelvin`
//! - Negative: `-40°F`, `-40.5°C`
//! - Decimal with locale heuristics: `4.28°F`, `4,28°F`

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct TemperatureFormat;

impl TemperatureFormat {
    /// Parse a number with decimal separator heuristics.
    ///
    /// - Default: `.` is decimal
    /// - `,` is decimal if followed by 1-2 digits (not 3)
    /// - When both present, the last separator is decimal
    fn parse_number(s: &str) -> Option<f64> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let has_dot = s.contains('.');
        let has_comma = s.contains(',');

        if has_dot && has_comma {
            // Both present: last one is decimal
            let dot_pos = s.rfind('.').unwrap();
            let comma_pos = s.rfind(',').unwrap();

            if comma_pos > dot_pos {
                // Comma is decimal: "1.000,5" -> "1000.5"
                let normalized = s.replace('.', "").replace(',', ".");
                normalized.parse().ok()
            } else {
                // Dot is decimal: "1,000.5" -> "1000.5"
                let normalized = s.replace(',', "");
                normalized.parse().ok()
            }
        } else if has_comma {
            // Only comma: check if it's decimal or thousands
            let parts: Vec<&str> = s.split(',').collect();
            if parts.len() == 2 && parts[1].len() <= 2 {
                // 1-2 digits after comma = decimal
                let normalized = s.replace(',', ".");
                normalized.parse().ok()
            } else {
                // 3 digits = thousands separator
                let normalized = s.replace(',', "");
                normalized.parse().ok()
            }
        } else {
            // Only dot or no separator
            s.parse().ok()
        }
    }

    /// Parse temperature string like "72°F" or "-40.5 C".
    /// Returns (value, unit) where unit is 'C', 'F', or 'K'.
    fn parse_temperature(s: &str) -> Option<(f64, char)> {
        let s = s.trim();

        // Unit suffixes to try (longer first to avoid partial matches)
        const SUFFIXES: &[(&str, char)] = &[
            ("fahrenheit", 'F'),
            ("Fahrenheit", 'F'),
            ("°F", 'F'),
            ("°f", 'F'),
            ("F", 'F'),
            ("f", 'F'),
            ("celsius", 'C'),
            ("Celsius", 'C'),
            ("°C", 'C'),
            ("°c", 'C'),
            ("C", 'C'),
            ("c", 'C'),
            ("kelvin", 'K'),
            ("Kelvin", 'K'),
            ("K", 'K'),
            ("k", 'K'),
        ];

        for (suffix, unit) in SUFFIXES {
            if let Some(num_str) = s.strip_suffix(suffix) {
                if let Some(value) = Self::parse_number(num_str) {
                    return Some((value, *unit));
                }
            }
        }
        None
    }

    // Conversion functions
    fn c_to_k(c: f64) -> f64 {
        c + 273.15
    }
    fn k_to_c(k: f64) -> f64 {
        k - 273.15
    }
    fn f_to_k(f: f64) -> f64 {
        (f - 32.0) * 5.0 / 9.0 + 273.15
    }
    fn k_to_f(k: f64) -> f64 {
        (k - 273.15) * 9.0 / 5.0 + 32.0
    }

    /// Convert input value to Kelvin based on unit.
    fn to_kelvin(value: f64, unit: char) -> f64 {
        match unit {
            'C' => Self::c_to_k(value),
            'F' => Self::f_to_k(value),
            'K' => value,
            _ => value,
        }
    }

    /// Format a temperature value with 2 decimal places (or integer if whole).
    fn format_value(value: f64) -> String {
        if (value - value.round()).abs() < 0.01 {
            format!("{}", value.round() as i64)
        } else {
            format!("{:.2}", value)
        }
    }
}

impl Format for TemperatureFormat {
    fn id(&self) -> &'static str {
        "temperature"
    }

    fn name(&self) -> &'static str {
        "Temperature"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Numbers",
            description: "Temperature (Celsius, Fahrenheit, Kelvin)",
            examples: &["72°F", "20°C", "300K", "-40 Fahrenheit"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((value, unit)) = Self::parse_temperature(input) else {
            return vec![];
        };

        let kelvin = Self::to_kelvin(value, unit);

        // Sanity check: reject physically impossible temperatures (below absolute zero)
        // Allow small negative due to floating point
        if kelvin < -0.01 {
            return vec![];
        }

        let unit_name = match unit {
            'C' => "Celsius",
            'F' => "Fahrenheit",
            'K' => "Kelvin",
            _ => "unknown",
        };

        let description = format!(
            "{}{} ({})",
            Self::format_value(value),
            match unit {
                'C' => "°C",
                'F' => "°F",
                'K' => " K",
                _ => "",
            },
            unit_name
        );

        vec![Interpretation {
            value: CoreValue::Temperature(kelvin),
            source_format: "temperature".to_string(),
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
        let CoreValue::Temperature(kelvin) = value else {
            return vec![];
        };

        let kelvin = *kelvin;
        let celsius = Self::k_to_c(kelvin);
        let fahrenheit = Self::k_to_f(kelvin);

        let mut conversions = Vec::new();

        // Celsius
        let c_display = format!("{}°C", Self::format_value(celsius));
        conversions.push(Conversion {
            value: CoreValue::Temperature(kelvin),
            target_format: "celsius".to_string(),
            display: c_display.clone(),
            path: vec!["celsius".to_string()],
            steps: vec![ConversionStep {
                format: "celsius".to_string(),
                value: CoreValue::Temperature(kelvin),
                display: c_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            ..Default::default()
        });

        // Fahrenheit
        let f_display = format!("{}°F", Self::format_value(fahrenheit));
        conversions.push(Conversion {
            value: CoreValue::Temperature(kelvin),
            target_format: "fahrenheit".to_string(),
            display: f_display.clone(),
            path: vec!["fahrenheit".to_string()],
            steps: vec![ConversionStep {
                format: "fahrenheit".to_string(),
                value: CoreValue::Temperature(kelvin),
                display: f_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            ..Default::default()
        });

        // Kelvin
        let k_display = format!("{} K", Self::format_value(kelvin));
        conversions.push(Conversion {
            value: CoreValue::Temperature(kelvin),
            target_format: "kelvin".to_string(),
            display: k_display.clone(),
            path: vec!["kelvin".to_string()],
            steps: vec![ConversionStep {
                format: "kelvin".to_string(),
                value: CoreValue::Temperature(kelvin),
                display: k_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            ..Default::default()
        });

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["temp", "celsius", "fahrenheit", "kelvin"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_kelvin(input: &str) -> Option<f64> {
        let format = TemperatureFormat;
        let results = format.parse(input);
        if results.is_empty() {
            return None;
        }
        match &results[0].value {
            CoreValue::Temperature(k) => Some(*k),
            _ => None,
        }
    }

    #[test]
    fn test_parse_celsius() {
        // 0°C = 273.15K
        let k = parse_to_kelvin("0°C").unwrap();
        assert!((k - 273.15).abs() < 0.01);

        // 100°C = 373.15K
        let k = parse_to_kelvin("100C").unwrap();
        assert!((k - 373.15).abs() < 0.01);

        // 20 Celsius
        let k = parse_to_kelvin("20 Celsius").unwrap();
        assert!((k - 293.15).abs() < 0.01);
    }

    #[test]
    fn test_parse_fahrenheit() {
        // 32°F = 0°C = 273.15K
        let k = parse_to_kelvin("32°F").unwrap();
        assert!((k - 273.15).abs() < 0.01);

        // 212°F = 100°C = 373.15K
        let k = parse_to_kelvin("212F").unwrap();
        assert!((k - 373.15).abs() < 0.01);

        // 98.6°F = 37°C = 310.15K
        let k = parse_to_kelvin("98.6F").unwrap();
        assert!((k - 310.15).abs() < 0.01);
    }

    #[test]
    fn test_parse_kelvin() {
        let k = parse_to_kelvin("273.15K").unwrap();
        assert!((k - 273.15).abs() < 0.01);

        let k = parse_to_kelvin("300 Kelvin").unwrap();
        assert!((k - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_negative() {
        // -40°C = -40°F = 233.15K
        let k = parse_to_kelvin("-40°C").unwrap();
        assert!((k - 233.15).abs() < 0.01);

        let k = parse_to_kelvin("-40°F").unwrap();
        assert!((k - 233.15).abs() < 0.01);
    }

    #[test]
    fn test_parse_comma_decimal() {
        // 4,28°F should parse as 4.28°F
        let k = parse_to_kelvin("4,28°F").unwrap();
        let expected = TemperatureFormat::f_to_k(4.28);
        assert!((k - expected).abs() < 0.01);
    }

    #[test]
    fn test_parse_comma_thousands() {
        // 1,000°F should parse as 1000°F
        let k = parse_to_kelvin("1,000°F").unwrap();
        let expected = TemperatureFormat::f_to_k(1000.0);
        assert!((k - expected).abs() < 0.01);
    }

    #[test]
    fn test_parse_mixed_separators() {
        // 1.000,5°C should parse as 1000.5°C
        let k = parse_to_kelvin("1.000,5°C").unwrap();
        let expected = TemperatureFormat::c_to_k(1000.5);
        assert!((k - expected).abs() < 0.01);

        // 1,000.5°C should parse as 1000.5°C
        let k = parse_to_kelvin("1,000.5°C").unwrap();
        assert!((k - expected).abs() < 0.01);
    }

    #[test]
    fn test_below_absolute_zero() {
        // -300°C is below absolute zero, should not parse
        assert!(parse_to_kelvin("-300°C").is_none());
    }

    #[test]
    fn test_conversions() {
        let format = TemperatureFormat;
        // 0°C = 273.15K
        let value = CoreValue::Temperature(273.15);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 3);

        let celsius = conversions.iter().find(|c| c.target_format == "celsius");
        assert!(celsius.is_some());
        assert_eq!(celsius.unwrap().display, "0°C");

        let fahrenheit = conversions.iter().find(|c| c.target_format == "fahrenheit");
        assert!(fahrenheit.is_some());
        assert_eq!(fahrenheit.unwrap().display, "32°F");

        let kelvin = conversions.iter().find(|c| c.target_format == "kelvin");
        assert!(kelvin.is_some());
        assert_eq!(kelvin.unwrap().display, "273.15 K");
    }

    #[test]
    fn test_format_value() {
        assert_eq!(TemperatureFormat::format_value(0.0), "0");
        assert_eq!(TemperatureFormat::format_value(100.0), "100");
        assert_eq!(TemperatureFormat::format_value(98.6), "98.60");
        assert_eq!(TemperatureFormat::format_value(-40.0), "-40");
    }
}
