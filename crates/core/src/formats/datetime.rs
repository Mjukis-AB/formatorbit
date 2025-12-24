//! DateTime format (epoch timestamp handling).

use chrono::{DateTime, TimeZone, Utc};

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

/// Reasonable epoch range: 2000-01-01 to 2100-01-01
/// We use 2000 as minimum to avoid false positives from small integers
/// (like IP octets converted to int, which gives values in 1970s-1980s).
const MIN_EPOCH_SECONDS: i64 = 946_684_800; // 2000-01-01
const MAX_EPOCH_SECONDS: i64 = 4_102_444_800; // 2100-01-01

/// For milliseconds, multiply by 1000
const MIN_EPOCH_MILLIS: i64 = MIN_EPOCH_SECONDS * 1000;
const MAX_EPOCH_MILLIS: i64 = MAX_EPOCH_SECONDS * 1000;

/// Apple/Cocoa reference date: 2001-01-01 00:00:00 UTC
/// This is 978307200 seconds after Unix epoch (1970-01-01)
const APPLE_REFERENCE_DATE: i64 = 978_307_200;

/// Valid range for Apple timestamps (2010-01-01 to 2100-01-01 in Apple time)
/// We use 2010 as minimum to avoid false positives from small integers.
/// 2010-01-01 = Unix 1262304000 = Apple 283996800
const MIN_APPLE_SECONDS: i64 = 283_996_800; // 2010-01-01 in Apple time
const MAX_APPLE_SECONDS: i64 = MAX_EPOCH_SECONDS - APPLE_REFERENCE_DATE;

/// Windows FILETIME: 100-nanosecond intervals since 1601-01-01
/// Difference between 1601-01-01 and 1970-01-01 in seconds: 11644473600
const FILETIME_EPOCH_DIFF: i64 = 11_644_473_600;
/// 100-nanosecond intervals per second
const FILETIME_TICKS_PER_SECOND: i64 = 10_000_000;
/// Reasonable FILETIME range (1970 to 2100)
const MIN_FILETIME: i64 = FILETIME_EPOCH_DIFF * FILETIME_TICKS_PER_SECOND;
const MAX_FILETIME: i64 = (MAX_EPOCH_SECONDS + FILETIME_EPOCH_DIFF) * FILETIME_TICKS_PER_SECOND;

pub struct DateTimeFormat;

impl DateTimeFormat {
    /// Check if a value is a reasonable epoch in seconds.
    fn is_valid_epoch_seconds(value: i64) -> bool {
        (MIN_EPOCH_SECONDS..=MAX_EPOCH_SECONDS).contains(&value)
    }

    /// Check if a value is a reasonable epoch in milliseconds.
    fn is_valid_epoch_millis(value: i64) -> bool {
        (MIN_EPOCH_MILLIS..=MAX_EPOCH_MILLIS).contains(&value)
    }

    /// Check if a value is a reasonable Apple/Cocoa timestamp.
    fn is_valid_apple_timestamp(value: i64) -> bool {
        (MIN_APPLE_SECONDS..=MAX_APPLE_SECONDS).contains(&value)
    }

    /// Check if a value is a reasonable Windows FILETIME.
    fn is_valid_filetime(value: i128) -> bool {
        value >= MIN_FILETIME as i128 && value <= MAX_FILETIME as i128
    }

    /// Convert FILETIME to Unix timestamp.
    fn filetime_to_unix(filetime: i128) -> Option<(i64, u32)> {
        // FILETIME is in 100-nanosecond intervals since 1601-01-01
        let unix_ticks =
            filetime - (FILETIME_EPOCH_DIFF as i128 * FILETIME_TICKS_PER_SECOND as i128);
        let secs = unix_ticks / FILETIME_TICKS_PER_SECOND as i128;
        let nanos = ((unix_ticks % FILETIME_TICKS_PER_SECOND as i128) * 100) as u32;

        i64::try_from(secs).ok().map(|s| (s, nanos))
    }
}

impl Format for DateTimeFormat {
    fn id(&self) -> &'static str {
        "datetime"
    }

    fn name(&self) -> &'static str {
        "Date/Time"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Timestamps",
            description: "Date/time parsing (ISO 8601, RFC 2822/3339) and epoch conversions",
            examples: &["2025-11-19T17:43:20Z", "1763574200", "1763574200000"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to parse ISO 8601 / RFC 3339 format
        if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt.with_timezone(&Utc)),
                source_format: "datetime".to_string(),
                confidence: 0.95,
                description: "ISO 8601 / RFC 3339 datetime".to_string(),
            }];
        }

        // Try RFC 2822 format
        if let Ok(dt) = DateTime::parse_from_rfc2822(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt.with_timezone(&Utc)),
                source_format: "datetime".to_string(),
                confidence: 0.9,
                description: "RFC 2822 datetime".to_string(),
            }];
        }

        vec![]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::DateTime(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::DateTime(dt) => Some(dt.to_rfc3339()),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: int_val, .. } = value else {
            return vec![];
        };

        let mut conversions = vec![];

        // Try as epoch seconds
        if let Ok(secs) = i64::try_from(*int_val) {
            if Self::is_valid_epoch_seconds(secs) {
                if let Some(dt) = Utc.timestamp_opt(secs, 0).single() {
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "epoch-seconds".to_string(),
                        display: dt.to_rfc3339(),
                        path: vec!["epoch-seconds".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                    });
                }
            }

            // Try as epoch milliseconds
            if Self::is_valid_epoch_millis(secs) {
                let epoch_secs = secs / 1000;
                let nanos = ((secs % 1000) * 1_000_000) as u32;
                if let Some(dt) = Utc.timestamp_opt(epoch_secs, nanos).single() {
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "epoch-millis".to_string(),
                        display: dt.to_rfc3339(),
                        path: vec!["epoch-millis".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                    });
                }
            }

            // Try as Apple/Cocoa timestamp (seconds since 2001-01-01)
            if Self::is_valid_apple_timestamp(secs) {
                let unix_secs = secs + APPLE_REFERENCE_DATE;
                if let Some(dt) = Utc.timestamp_opt(unix_secs, 0).single() {
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "apple-cocoa".to_string(),
                        display: dt.to_rfc3339(),
                        path: vec!["apple-cocoa".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                    });
                }
            }
        }

        // Try as Windows FILETIME (100-nanosecond intervals since 1601-01-01)
        // FILETIME values are typically large (> 100 trillion for modern dates)
        if Self::is_valid_filetime(*int_val) {
            if let Some((unix_secs, nanos)) = Self::filetime_to_unix(*int_val) {
                if let Some(dt) = Utc.timestamp_opt(unix_secs, nanos).single() {
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "filetime".to_string(),
                        display: dt.to_rfc3339(),
                        path: vec!["filetime".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                    });
                }
            }
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ts", "time", "date"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rfc3339() {
        let format = DateTimeFormat;
        let results = format.parse("2025-11-19T17:43:20Z");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "datetime");

        if let CoreValue::DateTime(dt) = &results[0].value {
            assert_eq!(dt.timestamp(), 1763574200);
        } else {
            panic!("Expected DateTime");
        }
    }

    #[test]
    fn test_epoch_seconds_conversion() {
        let format = DateTimeFormat;
        let value = CoreValue::Int {
            value: 1763574200,
            original_bytes: None,
        };

        let conversions = format.conversions(&value);

        let epoch_secs = conversions
            .iter()
            .find(|c| c.target_format == "epoch-seconds")
            .unwrap();
        assert!(epoch_secs.display.contains("2025"));
    }

    #[test]
    fn test_epoch_millis_conversion() {
        let format = DateTimeFormat;
        let value = CoreValue::Int {
            value: 1763574200000, // milliseconds
            original_bytes: None,
        };

        let conversions = format.conversions(&value);

        let epoch_millis = conversions
            .iter()
            .find(|c| c.target_format == "epoch-millis")
            .unwrap();
        assert!(epoch_millis.display.contains("2025"));
    }

    #[test]
    fn test_out_of_range_epoch() {
        let format = DateTimeFormat;
        let value = CoreValue::Int {
            value: -1000000000000, // way out of range
            original_bytes: None,
        };

        let conversions = format.conversions(&value);
        assert!(conversions.is_empty());
    }

    #[test]
    fn test_apple_cocoa_timestamp() {
        let format = DateTimeFormat;
        // 785267000 seconds since 2001-01-01 = 2025-11-19T17:43:20Z
        // (1763574200 - 978307200 = 785267000)
        let value = CoreValue::Int {
            value: 785267000,
            original_bytes: None,
        };

        let conversions = format.conversions(&value);

        let apple = conversions
            .iter()
            .find(|c| c.target_format == "apple-cocoa")
            .expect("Should have apple-cocoa conversion");
        assert!(apple.display.contains("2025-11-19"));
    }

    #[test]
    fn test_windows_filetime() {
        let format = DateTimeFormat;
        // FILETIME for 2025-01-01 00:00:00 UTC
        // Unix timestamp for 2025-01-01: 1735689600
        // FILETIME = (unix_timestamp + 11644473600) * 10_000_000
        // = (1735689600 + 11644473600) * 10_000_000 = 133801632000000000
        let value = CoreValue::Int {
            value: 133801632000000000,
            original_bytes: None,
        };

        let conversions = format.conversions(&value);

        let filetime = conversions
            .iter()
            .find(|c| c.target_format == "filetime")
            .expect("Should have filetime conversion");
        assert!(filetime.display.contains("2025-01-01"));
    }

    #[test]
    fn test_filetime_out_of_range() {
        let format = DateTimeFormat;
        // Very small value - before 1970
        let value = CoreValue::Int {
            value: 100,
            original_bytes: None,
        };

        let conversions = format.conversions(&value);

        // Should not have filetime (too small)
        assert!(!conversions.iter().any(|c| c.target_format == "filetime"));
    }
}
