//! DateTime format (epoch timestamp handling).

use chrono::{DateTime, TimeZone, Utc};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

/// Reasonable epoch range: 2000-01-01 to 2100-01-01
/// We use 2000 as minimum to avoid false positives from small integers
/// (like IP octets converted to int, which gives values in 1970s-1980s).
pub(crate) const MIN_EPOCH_SECONDS: i64 = 946_684_800; // 2000-01-01
pub(crate) const MAX_EPOCH_SECONDS: i64 = 4_102_444_800; // 2100-01-01

/// For milliseconds, multiply by 1000
pub(crate) const MIN_EPOCH_MILLIS: i64 = MIN_EPOCH_SECONDS * 1000;
pub(crate) const MAX_EPOCH_MILLIS: i64 = MAX_EPOCH_SECONDS * 1000;

/// For microseconds, multiply by 1,000,000
pub(crate) const MIN_EPOCH_MICROS: i64 = MIN_EPOCH_SECONDS * 1_000_000;
pub(crate) const MAX_EPOCH_MICROS: i64 = MAX_EPOCH_SECONDS * 1_000_000;

/// For nanoseconds, multiply by 1,000,000,000
/// Note: i64 max is ~9.2e18, and max_epoch_nanos is ~4.1e18, so this fits
pub(crate) const MIN_EPOCH_NANOS: i64 = MIN_EPOCH_SECONDS * 1_000_000_000;
pub(crate) const MAX_EPOCH_NANOS: i64 = MAX_EPOCH_SECONDS * 1_000_000_000;

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
    /// Format a datetime relative to now (e.g., "2 hours ago", "in 3 days").
    fn format_relative(dt: DateTime<Utc>) -> String {
        let now = Utc::now();
        let diff = dt.signed_duration_since(now);
        let secs = diff.num_seconds();
        let abs_secs = secs.abs();

        let (value, unit) = if abs_secs < 60 {
            (abs_secs, if abs_secs == 1 { "second" } else { "seconds" })
        } else if abs_secs < 3600 {
            let mins = abs_secs / 60;
            (mins, if mins == 1 { "minute" } else { "minutes" })
        } else if abs_secs < 86400 {
            let hours = abs_secs / 3600;
            (hours, if hours == 1 { "hour" } else { "hours" })
        } else if abs_secs < 604800 {
            let days = abs_secs / 86400;
            (days, if days == 1 { "day" } else { "days" })
        } else if abs_secs < 2592000 {
            let weeks = abs_secs / 604800;
            (weeks, if weeks == 1 { "week" } else { "weeks" })
        } else if abs_secs < 31536000 {
            let months = abs_secs / 2592000;
            (months, if months == 1 { "month" } else { "months" })
        } else {
            let years = abs_secs / 31536000;
            (years, if years == 1 { "year" } else { "years" })
        };

        if secs < 0 {
            format!("{} {} ago", value, unit)
        } else if secs > 0 {
            format!("in {} {}", value, unit)
        } else {
            "now".to_string()
        }
    }

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

    /// Convert a DateTime to epoch conversions (seconds, millis, relative time).
    fn conversions_from_datetime(dt: &DateTime<Utc>) -> Vec<Conversion> {
        let epoch_secs = dt.timestamp();
        let epoch_millis = dt.timestamp_millis();
        let relative = Self::format_relative(*dt);

        vec![
            Conversion {
                value: CoreValue::Int {
                    value: epoch_secs as i128,
                    original_bytes: None,
                },
                target_format: "epoch-seconds".to_string(),
                display: epoch_secs.to_string(),
                path: vec!["epoch-seconds".to_string()],
                is_lossy: false,
                steps: vec![],
                priority: ConversionPriority::Semantic,
                display_only: true,
                kind: ConversionKind::Conversion,
                hidden: false,
                rich_display: vec![],
            },
            Conversion {
                value: CoreValue::Int {
                    value: epoch_millis as i128,
                    original_bytes: None,
                },
                target_format: "epoch-millis".to_string(),
                display: epoch_millis.to_string(),
                path: vec!["epoch-millis".to_string()],
                is_lossy: false,
                steps: vec![],
                priority: ConversionPriority::Semantic,
                display_only: true,
                kind: ConversionKind::Conversion,
                hidden: false,
                rich_display: vec![],
            },
            Conversion {
                value: CoreValue::String(relative.clone()),
                target_format: "relative-time".to_string(),
                display: relative,
                path: vec!["relative-time".to_string()],
                is_lossy: false,
                steps: vec![],
                priority: ConversionPriority::Semantic,
                display_only: true,
                kind: ConversionKind::Representation,
                hidden: false,
                rich_display: vec![],
            },
        ]
    }

    /// Try to parse ISO 8601 date-only format: YYYY-MM-DD
    fn parse_iso_date(input: &str) -> Option<DateTime<Utc>> {
        use chrono::NaiveDate;

        let trimmed = input.trim();

        // Must be exactly 10 chars: YYYY-MM-DD
        if trimmed.len() != 10 {
            return None;
        }

        // Check format: digits, dash, digits, dash, digits
        let chars: Vec<char> = trimmed.chars().collect();
        if chars[4] != '-' || chars[7] != '-' {
            return None;
        }

        // Parse components
        let year: i32 = trimmed[0..4].parse().ok()?;
        let month: u32 = trimmed[5..7].parse().ok()?;
        let day: u32 = trimmed[8..10].parse().ok()?;

        // Validate reasonable year range (1900-2100)
        if !(1900..=2100).contains(&year) {
            return None;
        }

        NaiveDate::from_ymd_opt(year, month, day)?
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc())
    }

    /// Try to parse EU dot format: DD.MM.YYYY
    fn parse_eu_dot_date(input: &str) -> Option<DateTime<Utc>> {
        use chrono::NaiveDate;

        let trimmed = input.trim();
        let parts: Vec<&str> = trimmed.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let day: u32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let year: i32 = parts[2].parse().ok()?;

        // Validate
        if day > 31 || month > 12 || !(1900..=2100).contains(&year) {
            return None;
        }

        NaiveDate::from_ymd_opt(year, month, day)?
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc())
    }

    /// Try to parse Asian/ISO slash format: YYYY/MM/DD
    fn parse_asian_date(input: &str) -> Option<DateTime<Utc>> {
        use chrono::NaiveDate;

        let trimmed = input.trim();
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() != 3 {
            return None;
        }

        // Year must be 4 digits and come first
        let year_str = parts[0];
        if year_str.len() != 4 {
            return None;
        }

        let year: i32 = year_str.parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let day: u32 = parts[2].parse().ok()?;

        // Validate
        if day > 31 || month > 12 || !(1900..=2100).contains(&year) {
            return None;
        }

        NaiveDate::from_ymd_opt(year, month, day)?
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc())
    }

    /// Try to parse short date without year: MM/DD or DD/MM
    /// Returns both interpretations with low confidence (so expr can win for ambiguous cases like 25/2)
    fn parse_short_date(input: &str) -> Vec<(DateTime<Utc>, f32, String)> {
        use chrono::{Datelike, Local, NaiveDate};

        let trimmed = input.trim();
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() != 2 {
            return vec![];
        }

        let a: u32 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let b: u32 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        // Both must be reasonable day/month values
        if a > 31 || b > 31 || a == 0 || b == 0 {
            return vec![];
        }

        let mut results = vec![];
        let current_year = Local::now().year();

        // Try US format: MM/DD (a = month, b = day)
        if a <= 12 && b <= 31 {
            if let Some(date) = NaiveDate::from_ymd_opt(current_year, a, b) {
                let dt = date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                if let Some(dt) = dt {
                    // Low confidence - could be division, and no year specified
                    let confidence = if b > 12 { 0.65 } else { 0.45 };
                    let desc = format!(
                        "{} {} (MM/DD, year {})",
                        Self::month_name(a),
                        b,
                        current_year
                    );
                    results.push((dt, confidence, desc));
                }
            }
        }

        // Try EU format: DD/MM (a = day, b = month)
        if b <= 12 && a <= 31 && a != b {
            if let Some(date) = NaiveDate::from_ymd_opt(current_year, b, a) {
                let dt = date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                if let Some(dt) = dt {
                    let confidence = if a > 12 { 0.65 } else { 0.45 };
                    let desc = format!(
                        "{} {} (DD/MM, year {})",
                        a,
                        Self::month_name(b),
                        current_year
                    );
                    results.push((dt, confidence, desc));
                }
            }
        }

        results
    }

    /// Try to parse "Dec 28, 2025" or "December 28, 2025" format.
    fn parse_month_day_year(input: &str) -> Option<DateTime<Utc>> {
        use chrono::NaiveDate;

        let months = [
            ("jan", 1),
            ("feb", 2),
            ("mar", 3),
            ("apr", 4),
            ("may", 5),
            ("jun", 6),
            ("jul", 7),
            ("aug", 8),
            ("sep", 9),
            ("oct", 10),
            ("nov", 11),
            ("dec", 12),
        ];

        let input_lower = input.to_lowercase();
        let trimmed = input_lower.trim();

        // Pattern: "Dec 28, 2025" or "December 28, 2025"
        for (month_prefix, month_num) in months {
            if let Some(rest) = trimmed.strip_prefix(month_prefix) {
                // Skip any remaining month name letters and spaces
                let rest = rest.trim_start_matches(|c: char| c.is_alphabetic()).trim();
                // Parse "28, 2025" or "28 2025"
                let parts: Vec<&str> = rest
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .collect();
                if parts.len() >= 2 {
                    if let (Ok(day), Ok(year)) = (parts[0].parse::<u32>(), parts[1].parse::<i32>())
                    {
                        if let Some(date) = NaiveDate::from_ymd_opt(year, month_num, day) {
                            return date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                        }
                    }
                }
            }
        }
        None
    }

    /// Try to parse "28 Dec 2025" or "28 December 2025" format.
    fn parse_day_month_year(input: &str) -> Option<DateTime<Utc>> {
        use chrono::NaiveDate;

        let months = [
            ("jan", 1),
            ("feb", 2),
            ("mar", 3),
            ("apr", 4),
            ("may", 5),
            ("jun", 6),
            ("jul", 7),
            ("aug", 8),
            ("sep", 9),
            ("oct", 10),
            ("nov", 11),
            ("dec", 12),
        ];

        let input_lower = input.to_lowercase();
        let trimmed = input_lower.trim();

        // Pattern: "28 Dec 2025"
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            if let Ok(day) = parts[0].parse::<u32>() {
                let month_str = parts[1];
                for (month_prefix, month_num) in months {
                    if month_str.starts_with(month_prefix) {
                        if let Ok(year) = parts[2].parse::<i32>() {
                            if let Some(date) = NaiveDate::from_ymd_opt(year, month_num, day) {
                                return date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Try to parse "12/28/2025 @ 10:41am" format (US date with @ and time).
    fn parse_us_at_format(input: &str) -> Option<DateTime<Utc>> {
        use chrono::NaiveDate;

        // Split on @
        let parts: Vec<&str> = input.split('@').collect();
        if parts.len() != 2 {
            return None;
        }

        let date_part = parts[0].trim();
        let time_part = parts[1].trim().to_lowercase();

        // Parse date: MM/DD/YYYY
        let date_parts: Vec<&str> = date_part.split('/').collect();
        if date_parts.len() != 3 {
            return None;
        }

        let month: u32 = date_parts[0].parse().ok()?;
        let day: u32 = date_parts[1].parse().ok()?;
        let year: i32 = date_parts[2].parse().ok()?;

        // Parse time: "10:41am" or "10:41pm" or "10:41"
        let (time_str, is_pm) = if time_part.ends_with("pm") {
            (&time_part[..time_part.len() - 2], true)
        } else if time_part.ends_with("am") {
            (&time_part[..time_part.len() - 2], false)
        } else {
            (time_part.as_str(), false)
        };

        let time_parts: Vec<&str> = time_str.trim().split(':').collect();
        let mut hour: u32 = time_parts.first()?.parse().ok()?;
        let minute: u32 = time_parts.get(1).unwrap_or(&"0").parse().ok()?;
        let second: u32 = time_parts.get(2).unwrap_or(&"0").parse().ok()?;

        // Handle 12-hour format
        if is_pm && hour < 12 {
            hour += 12;
        } else if !is_pm && hour == 12 {
            hour = 0;
        }

        NaiveDate::from_ymd_opt(year, month, day)?
            .and_hms_opt(hour, minute, second)
            .map(|dt| dt.and_utc())
    }

    /// Try to parse numeric date format MM/DD/YYYY or DD/MM/YYYY.
    /// Returns both interpretations for ambiguous cases.
    fn parse_numeric_date(input: &str) -> Vec<(DateTime<Utc>, f32, String)> {
        use chrono::NaiveDate;

        let trimmed = input.trim();
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() != 3 {
            return vec![];
        }

        let a: u32 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let b: u32 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let year: i32 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        let mut results = vec![];

        // If a > 12, it must be day (European format: DD/MM/YYYY)
        // If b > 12, it must be day (US format: MM/DD/YYYY)
        // If both <= 12, ambiguous - return both

        // Try US format: MM/DD/YYYY (a = month, b = day)
        if a <= 12 && b <= 31 {
            if let Some(date) = NaiveDate::from_ymd_opt(year, a, b) {
                let dt = date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                if let Some(dt) = dt {
                    let confidence = if b > 12 { 0.80 } else { 0.55 }; // Higher if unambiguous
                    let desc = if b > 12 {
                        "US date (MM/DD/YYYY)".to_string()
                    } else {
                        format!(
                            "US date (MM/DD/YYYY): {} {}, {}",
                            Self::month_name(a),
                            b,
                            year
                        )
                    };
                    results.push((dt, confidence, desc));
                }
            }
        }

        // Try European format: DD/MM/YYYY (a = day, b = month)
        if b <= 12 && a <= 31 && a != b {
            // Only add if it would be different from US format
            if let Some(date) = NaiveDate::from_ymd_opt(year, b, a) {
                let dt = date.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
                if let Some(dt) = dt {
                    let confidence = if a > 12 { 0.80 } else { 0.55 }; // Higher if unambiguous
                    let desc = if a > 12 {
                        "European date (DD/MM/YYYY)".to_string()
                    } else {
                        format!(
                            "European date (DD/MM/YYYY): {} {}, {}",
                            a,
                            Self::month_name(b),
                            year
                        )
                    };
                    results.push((dt, confidence, desc));
                }
            }
        }

        results
    }

    /// Get month name from number.
    fn month_name(month: u32) -> &'static str {
        match month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        }
    }

    /// Convert an integer to epoch timestamp conversions.
    fn conversions_from_int(int_val: i128) -> Vec<Conversion> {
        let mut conversions = vec![];

        // Try as epoch seconds
        if let Ok(secs) = i64::try_from(int_val) {
            if Self::is_valid_epoch_seconds(secs) {
                if let Some(dt) = Utc.timestamp_opt(secs, 0).single() {
                    let iso = dt.to_rfc3339();
                    let relative = Self::format_relative(dt);
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "epoch-seconds".to_string(),
                        display: format!("{} ({})", iso, relative),
                        path: vec!["epoch-seconds".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                        kind: ConversionKind::default(),
                        hidden: false,
                        rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                            epoch_millis: secs * 1000,
                            iso: iso.clone(),
                            relative,
                        })],
                    });
                }
            }

            // Try as epoch milliseconds
            if Self::is_valid_epoch_millis(secs) {
                let epoch_secs = secs / 1000;
                let nanos = ((secs % 1000) * 1_000_000) as u32;
                if let Some(dt) = Utc.timestamp_opt(epoch_secs, nanos).single() {
                    let iso = dt.to_rfc3339();
                    let relative = Self::format_relative(dt);
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "epoch-millis".to_string(),
                        display: format!("{} ({})", iso, relative),
                        path: vec!["epoch-millis".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                        kind: ConversionKind::default(),
                        hidden: false,
                        rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                            epoch_millis: secs,
                            iso: iso.clone(),
                            relative,
                        })],
                    });
                }
            }

            // Try as Apple/Cocoa timestamp (seconds since 2001-01-01)
            if Self::is_valid_apple_timestamp(secs) {
                let unix_secs = secs + APPLE_REFERENCE_DATE;
                if let Some(dt) = Utc.timestamp_opt(unix_secs, 0).single() {
                    let iso = dt.to_rfc3339();
                    let relative = Self::format_relative(dt);
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "apple-cocoa".to_string(),
                        display: format!("{} ({})", iso, relative),
                        path: vec!["apple-cocoa".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                        kind: ConversionKind::default(),
                        hidden: false,
                        rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                            epoch_millis: unix_secs * 1000,
                            iso: iso.clone(),
                            relative,
                        })],
                    });
                }
            }
        }

        // Try as Windows FILETIME (100-nanosecond intervals since 1601-01-01)
        // FILETIME values are typically large (> 100 trillion for modern dates)
        if Self::is_valid_filetime(int_val) {
            if let Some((unix_secs, nanos)) = Self::filetime_to_unix(int_val) {
                if let Some(dt) = Utc.timestamp_opt(unix_secs, nanos).single() {
                    let iso = dt.to_rfc3339();
                    let relative = Self::format_relative(dt);
                    conversions.push(Conversion {
                        value: CoreValue::DateTime(dt),
                        target_format: "filetime".to_string(),
                        display: format!("{} ({})", iso, relative),
                        path: vec!["filetime".to_string()],
                        is_lossy: false,
                        steps: vec![],
                        priority: ConversionPriority::Semantic,
                        display_only: false,
                        kind: ConversionKind::default(),
                        hidden: false,
                        rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                            epoch_millis: unix_secs * 1000 + (nanos / 1_000_000) as i64,
                            iso: iso.clone(),
                            relative,
                        })],
                    });
                }
            }
        }

        conversions
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
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to parse ISO 8601 / RFC 3339 format (full datetime with timezone)
        if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt.with_timezone(&Utc)),
                source_format: "datetime".to_string(),
                confidence: 0.95,
                description: "ISO 8601 / RFC 3339 datetime".to_string(),
                rich_display: vec![],
            }];
        }

        // Try ISO 8601 date-only format: YYYY-MM-DD
        if let Some(dt) = Self::parse_iso_date(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt),
                source_format: "datetime".to_string(),
                confidence: 0.95,
                description: "ISO 8601 date".to_string(),
                rich_display: vec![],
            }];
        }

        // Try Asian/ISO slash format: YYYY/MM/DD
        if let Some(dt) = Self::parse_asian_date(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt),
                source_format: "datetime".to_string(),
                confidence: 0.90,
                description: "Date (YYYY/MM/DD)".to_string(),
                rich_display: vec![],
            }];
        }

        // Try EU dot format: DD.MM.YYYY
        if let Some(dt) = Self::parse_eu_dot_date(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt),
                source_format: "datetime".to_string(),
                confidence: 0.85,
                description: "European date (DD.MM.YYYY)".to_string(),
                rich_display: vec![],
            }];
        }

        // Try RFC 2822 format
        if let Ok(dt) = DateTime::parse_from_rfc2822(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt.with_timezone(&Utc)),
                source_format: "datetime".to_string(),
                confidence: 0.9,
                description: "RFC 2822 datetime".to_string(),
                rich_display: vec![],
            }];
        }

        // Try US format with @ (e.g., "12/28/2025 @ 10:41am")
        if let Some(dt) = Self::parse_us_at_format(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt),
                source_format: "datetime".to_string(),
                confidence: 0.85,
                description: "US date with time (MM/DD/YYYY @ HH:MMam/pm)".to_string(),
                rich_display: vec![],
            }];
        }

        // Try "Dec 28, 2025" or "December 28, 2025" format
        if let Some(dt) = Self::parse_month_day_year(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt),
                source_format: "datetime".to_string(),
                confidence: 0.80,
                description: "Date (Month Day, Year)".to_string(),
                rich_display: vec![],
            }];
        }

        // Try "28 Dec 2025" or "28 December 2025" format
        if let Some(dt) = Self::parse_day_month_year(input) {
            return vec![Interpretation {
                value: CoreValue::DateTime(dt),
                source_format: "datetime".to_string(),
                confidence: 0.80,
                description: "Date (Day Month Year)".to_string(),
                rich_display: vec![],
            }];
        }

        // Try numeric date format (returns both US and European for ambiguous cases)
        let numeric_results = Self::parse_numeric_date(input);
        if !numeric_results.is_empty() {
            return numeric_results
                .into_iter()
                .map(|(dt, confidence, desc)| Interpretation {
                    value: CoreValue::DateTime(dt),
                    source_format: "datetime".to_string(),
                    confidence,
                    description: desc,
                    rich_display: vec![],
                })
                .collect();
        }

        // Try short date without year: MM/DD or DD/MM
        // Low confidence so expr wins for ambiguous cases like 25/2
        let short_results = Self::parse_short_date(input);
        if !short_results.is_empty() {
            return short_results
                .into_iter()
                .map(|(dt, confidence, desc)| Interpretation {
                    value: CoreValue::DateTime(dt),
                    source_format: "datetime".to_string(),
                    confidence,
                    description: desc,
                    rich_display: vec![],
                })
                .collect();
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
        match value {
            CoreValue::Int { value: int_val, .. } => Self::conversions_from_int(*int_val),
            CoreValue::DateTime(dt) => Self::conversions_from_datetime(dt),
            _ => vec![],
        }
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
