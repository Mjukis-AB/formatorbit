//! Data size format.
//!
//! Parses human-readable data sizes like:
//! - `1KB`, `1 KB`, `1kb` → 1000 bytes (SI/decimal)
//! - `1KiB`, `1 KiB` → 1024 bytes (IEC/binary)
//! - `1.5MB` → 1500000 bytes
//! - `512MiB` → 536870912 bytes
//!
//! Also converts raw byte counts to human-readable sizes.

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionPriority, ConversionStep, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct DataSizeFormat;

/// SI (decimal) units: KB = 1000, MB = 1000^2, etc.
const SI_UNITS: &[(&str, u64)] = &[
    ("KB", 1_000),
    ("MB", 1_000_000),
    ("GB", 1_000_000_000),
    ("TB", 1_000_000_000_000),
    ("PB", 1_000_000_000_000_000),
];

/// IEC (binary) units: KiB = 1024, MiB = 1024^2, etc.
const IEC_UNITS: &[(&str, u64)] = &[
    ("KiB", 1_024),
    ("MiB", 1_048_576),
    ("GiB", 1_073_741_824),
    ("TiB", 1_099_511_627_776),
    ("PiB", 1_125_899_906_842_624),
];

impl DataSizeFormat {
    /// Parse a data size string like "1.5MB" or "512 KiB".
    fn parse_size(s: &str) -> Option<(u64, &'static str)> {
        let s = s.trim();

        // Try IEC units first (more specific: KiB vs KB)
        for (unit, multiplier) in IEC_UNITS {
            if let Some(num_str) = s
                .strip_suffix(unit)
                .or_else(|| s.strip_suffix(&unit.to_lowercase()))
            {
                let num_str = num_str.trim();
                if let Ok(num) = num_str.parse::<f64>() {
                    if num >= 0.0 {
                        return Some(((num * *multiplier as f64) as u64, unit));
                    }
                }
            }
        }

        // Try SI units
        for (unit, multiplier) in SI_UNITS {
            if let Some(num_str) = s
                .strip_suffix(unit)
                .or_else(|| s.strip_suffix(&unit.to_lowercase()))
            {
                let num_str = num_str.trim();
                if let Ok(num) = num_str.parse::<f64>() {
                    if num >= 0.0 {
                        return Some(((num * *multiplier as f64) as u64, unit));
                    }
                }
            }
        }

        // Try just "B" or "bytes"
        let num_str = s
            .strip_suffix("bytes")
            .or_else(|| s.strip_suffix("B"))
            .or_else(|| s.strip_suffix("b"))?;
        let num_str = num_str.trim();
        let num: u64 = num_str.parse().ok()?;
        Some((num, "B"))
    }

    /// Format bytes as human-readable IEC (binary) size.
    fn format_iec(bytes: u64) -> String {
        for (unit, multiplier) in IEC_UNITS.iter().rev() {
            if bytes >= *multiplier {
                let value = bytes as f64 / *multiplier as f64;
                return if value.fract() < 0.01 {
                    format!("{} {}", value as u64, unit)
                } else {
                    format!("{:.2} {}", value, unit)
                };
            }
        }
        format!("{} B", bytes)
    }

    /// Format bytes as human-readable SI (decimal) size.
    fn format_si(bytes: u64) -> String {
        for (unit, multiplier) in SI_UNITS.iter().rev() {
            if bytes >= *multiplier {
                let value = bytes as f64 / *multiplier as f64;
                return if value.fract() < 0.01 {
                    format!("{} {}", value as u64, unit)
                } else {
                    format!("{:.2} {}", value, unit)
                };
            }
        }
        format!("{} B", bytes)
    }

    /// Format bytes with thousands separator.
    fn format_with_commas(n: u64) -> String {
        let s = n.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.chars().rev().collect()
    }
}

impl Format for DataSizeFormat {
    fn id(&self) -> &'static str {
        "datasize"
    }

    fn name(&self) -> &'static str {
        "Data Size"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Numbers",
            description: "Data sizes (KB, MB, KiB, MiB) to/from bytes",
            examples: &["1MB", "512 KiB", "1.5GB", "1048576"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((bytes, unit)) = Self::parse_size(input) else {
            return vec![];
        };

        // Don't parse plain "B" without a number prefix differently
        if unit == "B" && input.trim() == "B" {
            return vec![];
        }

        let description = format!(
            "{} = {} bytes ({})",
            input.trim(),
            Self::format_with_commas(bytes),
            if unit.contains('i') {
                "binary"
            } else {
                "decimal"
            }
        );

        let human = Self::format_iec(bytes);
        vec![Interpretation {
            value: CoreValue::Int {
                value: bytes as i128,
                original_bytes: None,
            },
            source_format: "datasize".to_string(),
            confidence: 0.90,
            description,
            rich_display: vec![RichDisplayOption::new(RichDisplay::DataSize {
                bytes,
                human,
            })],
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Int { value, .. } if *value >= 0)
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Int { value, .. } if *value >= 0 => Some(Self::format_iec(*value as u64)),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: bytes, .. } = value else {
            return vec![];
        };

        // Only show for positive values that are at least 1KB
        if *bytes < 1000 {
            return vec![];
        }

        let bytes = *bytes as u64;
        let mut conversions = Vec::new();

        // IEC (binary) format
        let iec = Self::format_iec(bytes);
        if !iec.ends_with(" B") {
            conversions.push(Conversion {
                value: CoreValue::String(iec.clone()),
                target_format: "datasize-iec".to_string(),
                display: iec.clone(),
                path: vec!["datasize-iec".to_string()],
                steps: vec![ConversionStep {
                    format: "datasize-iec".to_string(),
                    value: CoreValue::String(iec.clone()),
                    display: iec.clone(),
                }],
                priority: ConversionPriority::Semantic,
                display_only: true,
                rich_display: vec![RichDisplayOption::new(RichDisplay::DataSize {
                    bytes,
                    human: iec,
                })],
                ..Default::default()
            });
        }

        // SI (decimal) format
        let si = Self::format_si(bytes);
        if !si.ends_with(" B") && si != Self::format_iec(bytes) {
            conversions.push(Conversion {
                value: CoreValue::String(si.clone()),
                target_format: "datasize-si".to_string(),
                display: si.clone(),
                path: vec!["datasize-si".to_string()],
                steps: vec![ConversionStep {
                    format: "datasize-si".to_string(),
                    value: CoreValue::String(si.clone()),
                    display: si.clone(),
                }],
                priority: ConversionPriority::Semantic,
                display_only: true,
                rich_display: vec![RichDisplayOption::new(RichDisplay::DataSize {
                    bytes,
                    human: si,
                })],
                ..Default::default()
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["size", "bytes", "filesize"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_si_units() {
        let format = DataSizeFormat;

        let results = format.parse("1KB");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1000);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1MB");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1_000_000);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1.5GB");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1_500_000_000);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_iec_units() {
        let format = DataSizeFormat;

        let results = format.parse("1KiB");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1024);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1MiB");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1_048_576);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("512 MiB");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 536_870_912);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_lowercase() {
        let format = DataSizeFormat;

        let results = format.parse("1mb");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1_000_000);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1kib");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1024);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_format_iec() {
        assert_eq!(DataSizeFormat::format_iec(1024), "1 KiB");
        assert_eq!(DataSizeFormat::format_iec(1048576), "1 MiB");
        assert_eq!(DataSizeFormat::format_iec(1536 * 1024), "1.50 MiB");
        assert_eq!(DataSizeFormat::format_iec(500), "500 B");
    }

    #[test]
    fn test_format_si() {
        assert_eq!(DataSizeFormat::format_si(1000), "1 KB");
        assert_eq!(DataSizeFormat::format_si(1_000_000), "1 MB");
        assert_eq!(DataSizeFormat::format_si(1_500_000), "1.50 MB");
        assert_eq!(DataSizeFormat::format_si(500), "500 B");
    }

    #[test]
    fn test_conversions() {
        let format = DataSizeFormat;
        let value = CoreValue::Int {
            value: 1_048_576,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);

        let iec = conversions
            .iter()
            .find(|c| c.target_format == "datasize-iec");
        assert!(iec.is_some());
        assert_eq!(iec.unwrap().display, "1 MiB");

        let si = conversions
            .iter()
            .find(|c| c.target_format == "datasize-si");
        assert!(si.is_some());
        assert_eq!(si.unwrap().display, "1.05 MB");
    }

    #[test]
    fn test_format_with_commas() {
        assert_eq!(DataSizeFormat::format_with_commas(1000), "1,000");
        assert_eq!(DataSizeFormat::format_with_commas(1000000), "1,000,000");
        assert_eq!(DataSizeFormat::format_with_commas(1048576), "1,048,576");
    }
}
