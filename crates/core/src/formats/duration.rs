//! Duration format.
//!
//! Parses human-readable durations like:
//! - `1h30m`, `2d`, `500ms`, `1h`, `30s` (compact)
//! - `1.5h`, `2.5d` (decimal)
//! - `5 days`, `2 hours`, `30 minutes` (spelled out)
//! - `5 days 2 hours`, `1 hour 30 minutes` (mixed)
//! - `5 d`, `2 h`, `30 m` (abbreviated with spaces)
//! - `P5D`, `PT2H30M`, `P1DT12H` (ISO 8601)
//! - `1:30:00` (HH:MM:SS format)
//!
//! Converts integers to human-readable durations and shows absolute time.

use chrono::Utc;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionMetadata, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct DurationFormat;

/// Duration in milliseconds (internal representation)
#[derive(Debug, Clone, Copy)]
struct Duration {
    millis: u64,
}

impl Duration {
    fn from_millis(millis: u64) -> Self {
        Self { millis }
    }

    fn from_seconds(seconds: u64) -> Self {
        Self {
            millis: seconds.saturating_mul(1000),
        }
    }

    fn as_seconds(&self) -> u64 {
        self.millis / 1000
    }

    /// Format as human-readable duration string.
    fn format_human(&self) -> String {
        let total_secs = self.millis / 1000;
        let millis_remainder = self.millis % 1000;

        if total_secs == 0 {
            return format!("{}ms", self.millis);
        }

        let years = total_secs / (365 * 86400);
        let days = (total_secs % (365 * 86400)) / 86400;
        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        let mut parts = Vec::new();

        if years > 0 {
            parts.push(format!("{}y", years));
        }
        if days > 0 {
            parts.push(format!("{}d", days));
        }
        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
        }
        if seconds > 0 || (parts.is_empty() && millis_remainder == 0) {
            parts.push(format!("{}s", seconds));
        }
        if millis_remainder > 0 && total_secs < 60 {
            // Only show ms for short durations
            parts.push(format!("{}ms", millis_remainder));
        }

        parts.join("")
    }

    /// Format as HH:MM:SS.
    #[allow(dead_code)] // Used in tests, may be useful for future conversions
    fn format_hms(&self) -> String {
        let total_secs = self.millis / 1000;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

impl DurationFormat {
    /// Parse duration string in various formats.
    fn parse_duration(s: &str) -> Option<Duration> {
        let s = s.trim();

        // Try ISO 8601 format first (P5D, PT2H30M)
        if let Some(dur) = Self::parse_iso8601(s) {
            return Some(dur);
        }

        // Try HH:MM:SS format
        if let Some(dur) = Self::parse_hms(s) {
            return Some(dur);
        }

        // Try human-readable formats (handles all: compact, decimal, spelled out, mixed)
        Self::parse_human_readable(s)
    }

    /// Parse ISO 8601 duration format: P5D, PT2H30M, P1DT12H30M15S
    fn parse_iso8601(s: &str) -> Option<Duration> {
        let s = s.to_uppercase();
        if !s.starts_with('P') {
            return None;
        }

        let s = &s[1..]; // Remove 'P'
        let mut total_millis: u64 = 0;
        let mut in_time_part = false;
        let mut current_num = String::new();

        for c in s.chars() {
            if c == 'T' {
                in_time_part = true;
                continue;
            }

            if c.is_ascii_digit() || c == '.' {
                current_num.push(c);
            } else if !current_num.is_empty() {
                let num: f64 = current_num.parse().ok()?;
                current_num.clear();

                let millis = match (c, in_time_part) {
                    ('Y', false) => (num * 365.0 * 86400.0 * 1000.0) as u64,
                    ('M', false) => (num * 30.0 * 86400.0 * 1000.0) as u64, // Approximate month
                    ('W', false) => (num * 7.0 * 86400.0 * 1000.0) as u64,
                    ('D', false) => (num * 86400.0 * 1000.0) as u64,
                    ('H', true) => (num * 3600.0 * 1000.0) as u64,
                    ('M', true) => (num * 60.0 * 1000.0) as u64,
                    ('S', true) => (num * 1000.0) as u64,
                    _ => return None,
                };

                total_millis += millis;
            } else {
                return None;
            }
        }

        if total_millis > 0 {
            Some(Duration::from_millis(total_millis))
        } else {
            None
        }
    }

    /// Parse HH:MM:SS or MM:SS format.
    fn parse_hms(s: &str) -> Option<Duration> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.len() {
            2 => {
                // MM:SS
                let minutes: u64 = parts[0].parse().ok()?;
                let seconds: u64 = parts[1].parse().ok()?;
                Some(Duration::from_seconds(minutes * 60 + seconds))
            }
            3 => {
                // HH:MM:SS
                let hours: u64 = parts[0].parse().ok()?;
                let minutes: u64 = parts[1].parse().ok()?;
                let seconds: u64 = parts[2].parse().ok()?;
                Some(Duration::from_seconds(
                    hours * 3600 + minutes * 60 + seconds,
                ))
            }
            _ => None,
        }
    }

    /// Parse human-readable duration formats.
    /// Handles: "1h30m", "1.5h", "5 days", "5 days 2 hours", "5 d"
    fn parse_human_readable(s: &str) -> Option<Duration> {
        let s = s.to_lowercase();
        let mut total_millis: u64 = 0;
        let mut found_any = false;

        // Split by whitespace and process tokens
        let mut tokens = s.split_whitespace().peekable();

        while let Some(token) = tokens.next() {
            // Try to parse compound tokens like "1h30m" or simple "5d"
            if let Some(millis) = Self::parse_compound_token(token) {
                total_millis += millis;
                found_any = true;
                continue;
            }

            // Try to parse as number followed by separate unit token
            if let Ok(num) = token.parse::<f64>() {
                // Look for unit in next token
                if let Some(&next) = tokens.peek() {
                    if let Some(unit) = Self::normalize_unit(next) {
                        tokens.next(); // consume the unit token
                        total_millis += Self::unit_to_millis(num, unit)?;
                        found_any = true;
                        continue;
                    }
                }
                // No unit found after number
                return None;
            }

            // Unknown token
            return None;
        }

        if found_any {
            Some(Duration::from_millis(total_millis))
        } else {
            None
        }
    }

    /// Parse a compound token like "1h30m", "5d", "1.5h", "500ms".
    /// Returns total milliseconds.
    fn parse_compound_token(s: &str) -> Option<u64> {
        let mut total_millis: u64 = 0;
        let mut current_num = String::new();
        let mut found_any = false;
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c.is_ascii_digit() || c == '.' {
                current_num.push(c);
            } else if !current_num.is_empty() {
                let num: f64 = current_num.parse().ok()?;
                current_num.clear();

                // Collect the unit characters
                let mut unit_str = String::new();
                unit_str.push(c);

                // Check for multi-char units like "ms", "min", "days"
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_ascii_alphabetic() {
                        unit_str.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                let unit = Self::normalize_unit(&unit_str)?;
                total_millis += Self::unit_to_millis(num, unit)?;
                found_any = true;
            } else {
                // Non-digit at start or after unit without number
                return None;
            }
        }

        // Handle trailing number without unit (invalid)
        if !current_num.is_empty() {
            return None;
        }

        if found_any {
            Some(total_millis)
        } else {
            None
        }
    }

    /// Normalize various unit representations to a standard form.
    fn normalize_unit(s: &str) -> Option<&'static str> {
        match s.trim() {
            // Milliseconds
            "ms" | "millisecond" | "milliseconds" | "millis" => Some("ms"),
            // Seconds
            "s" | "sec" | "secs" | "second" | "seconds" => Some("s"),
            // Minutes
            "m" | "min" | "mins" | "minute" | "minutes" => Some("m"),
            // Hours
            "h" | "hr" | "hrs" | "hour" | "hours" => Some("h"),
            // Days
            "d" | "day" | "days" => Some("d"),
            // Weeks
            "w" | "wk" | "wks" | "week" | "weeks" => Some("w"),
            // Years
            "y" | "yr" | "yrs" | "year" | "years" => Some("y"),
            _ => None,
        }
    }

    /// Convert a number and unit to milliseconds.
    fn unit_to_millis(num: f64, unit: &str) -> Option<u64> {
        let millis = match unit {
            "ms" => num,
            "s" => num * 1000.0,
            "m" => num * 60.0 * 1000.0,
            "h" => num * 3600.0 * 1000.0,
            "d" => num * 86400.0 * 1000.0,
            "w" => num * 7.0 * 86400.0 * 1000.0,
            "y" => num * 365.0 * 86400.0 * 1000.0,
            _ => return None,
        };
        Some(millis as u64)
    }

    /// Format seconds as human-readable.
    fn seconds_to_human(secs: u64) -> String {
        Duration::from_seconds(secs).format_human()
    }

    /// Format milliseconds as human-readable.
    fn millis_to_human(millis: u64) -> String {
        Duration::from_millis(millis).format_human()
    }

    /// Format absolute time (now + duration).
    fn format_absolute(secs: i64) -> String {
        let now = Utc::now();
        let future = now + chrono::Duration::seconds(secs);
        future.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
}

impl Format for DurationFormat {
    fn id(&self) -> &'static str {
        "duration"
    }

    fn name(&self) -> &'static str {
        "Duration"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Time",
            description: "Time durations (1h30m, 5 days, 1.5h, PT2H30M)",
            examples: &["1h30m", "5 days", "1.5h", "PT2H30M", "1:30:00"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some(duration) = Self::parse_duration(input) else {
            return vec![];
        };

        let secs = duration.as_seconds();
        let absolute = Self::format_absolute(secs as i64);

        let description = format!("{} = {} seconds ({})", input.trim(), secs, absolute);

        vec![Interpretation {
            value: CoreValue::Int {
                value: secs as i128,
                original_bytes: None,
            },
            source_format: "duration".to_string(),
            confidence: 0.90,
            description,
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Int { value, .. } if *value >= 0)
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Int { value, .. } if *value >= 0 => {
                Some(Self::seconds_to_human(*value as u64))
            }
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: int_val, .. } = value else {
            return vec![];
        };

        // Only show for positive values that make sense as durations
        // Skip very small values (< 1 second) and very large values
        if *int_val <= 0 || *int_val > 10_000_000_000_000 {
            return vec![];
        }

        let val = *int_val as u64;
        let mut conversions = Vec::new();

        // Determine if this is likely seconds or milliseconds
        // > 1 billion is likely milliseconds (> 31 years as seconds)
        let likely_millis = val > 1_000_000_000;
        let likely_seconds = val < 1_000_000_000;

        // Duration as seconds interpretation
        if likely_seconds && val >= 60 {
            let human = Self::seconds_to_human(val);
            let absolute = Self::format_absolute(val as i64);
            let detail = format!("now + {} = {}", human, absolute);
            let display = format!("{} ({})", human, detail);

            conversions.push(Conversion {
                value: CoreValue::String(human.clone()),
                target_format: "duration".to_string(),
                display,
                path: vec!["duration".to_string()],
                steps: vec![ConversionStep {
                    format: "duration".to_string(),
                    value: CoreValue::String(human.clone()),
                    display: "as seconds".to_string(),
                }],
                priority: ConversionPriority::Semantic,
                display_only: true,
                metadata: Some(ConversionMetadata::Duration { human, detail }),
                ..Default::default()
            });
        }

        // Duration as milliseconds interpretation
        if likely_millis || (1000..1_000_000_000).contains(&val) {
            let human = Self::millis_to_human(val);
            let secs = val / 1000;
            if secs >= 1 {
                let absolute = Self::format_absolute(secs as i64);
                let detail = format!("now + {} = {}", human, absolute);
                let display = format!("{} ({})", human, detail);

                conversions.push(Conversion {
                    value: CoreValue::String(human.clone()),
                    target_format: "duration-ms".to_string(),
                    display,
                    path: vec!["duration-ms".to_string()],
                    steps: vec![ConversionStep {
                        format: "duration-ms".to_string(),
                        value: CoreValue::String(human.clone()),
                        display: "as milliseconds".to_string(),
                    }],
                    priority: ConversionPriority::Semantic,
                    display_only: true,
                    metadata: Some(ConversionMetadata::Duration { human, detail }),
                    ..Default::default()
                });
            }
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["dur", "time", "interval"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_units() {
        let format = DurationFormat;

        let results = format.parse("1h");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 3600);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("30m");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1800);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("2d");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 172800);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_compound() {
        let format = DurationFormat;

        let results = format.parse("1h30m");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 5400); // 3600 + 1800
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1d12h");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 129600); // 86400 + 43200
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_milliseconds() {
        let format = DurationFormat;

        let results = format.parse("500ms");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0); // 500ms < 1 second
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1500ms");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1); // 1500ms = 1 second (truncated)
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_hms() {
        let format = DurationFormat;

        let results = format.parse("1:30:00");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 5400);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("10:30");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 630); // 10 min 30 sec
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_format_human() {
        assert_eq!(Duration::from_seconds(3600).format_human(), "1h");
        assert_eq!(Duration::from_seconds(5400).format_human(), "1h30m");
        assert_eq!(Duration::from_seconds(86400).format_human(), "1d");
        assert_eq!(Duration::from_seconds(90061).format_human(), "1d1h1m1s");
        assert_eq!(Duration::from_millis(500).format_human(), "500ms");
    }

    #[test]
    fn test_format_hms() {
        assert_eq!(Duration::from_seconds(3600).format_hms(), "01:00:00");
        assert_eq!(Duration::from_seconds(5400).format_hms(), "01:30:00");
        assert_eq!(Duration::from_seconds(90061).format_hms(), "25:01:01");
    }

    #[test]
    fn test_conversions() {
        let format = DurationFormat;
        let value = CoreValue::Int {
            value: 3600,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);

        assert!(!conversions.is_empty());
        let dur = conversions.iter().find(|c| c.target_format == "duration");
        assert!(dur.is_some());
        assert!(dur.unwrap().display.contains("1h"));
    }

    #[test]
    fn test_week() {
        let format = DurationFormat;

        let results = format.parse("1w");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 604800); // 7 * 86400
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_spelled_out_units() {
        let format = DurationFormat;

        let results = format.parse("5 days");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 432000); // 5 * 86400
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("2 hours");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 7200);
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("30 minutes");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1800);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_mixed_format() {
        let format = DurationFormat;

        let results = format.parse("5 days 2 hours");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 439200); // 5*86400 + 2*3600
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1 hour 30 minutes");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 5400);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_decimal() {
        let format = DurationFormat;

        let results = format.parse("1.5h");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 5400); // 1.5 * 3600
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("2.5d");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 216000); // 2.5 * 86400
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_iso8601() {
        let format = DurationFormat;

        let results = format.parse("P5D");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 432000); // 5 days
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("PT2H30M");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 9000); // 2h30m
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("P1DT12H");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 129600); // 1d12h = 86400 + 43200
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_year() {
        let format = DurationFormat;

        let results = format.parse("1 year");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 31536000); // 365 * 86400
        } else {
            panic!("Expected Int");
        }

        let results = format.parse("1y");
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 31536000);
        } else {
            panic!("Expected Int");
        }
    }
}
