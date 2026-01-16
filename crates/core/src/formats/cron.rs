//! Cron expression format.
//!
//! Parses standard 5-field cron expressions and provides:
//! - Human-readable descriptions ("every 5 minutes", "at 2:00 daily")
//! - Next execution times
//!
//! Supports:
//! - Standard 5-field format: minute hour day-of-month month day-of-week
//! - Special characters: * , - /
//! - Non-standard aliases: @yearly, @monthly, @weekly, @daily, @hourly

use chrono::{Datelike, Local, TimeZone, Timelike, Utc};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct CronFormat;

/// Parsed cron field with all possible values.
#[derive(Debug, Clone)]
struct CronField {
    values: Vec<u32>,
}

impl CronField {
    /// Parse a cron field string into a set of values.
    fn parse(field: &str, min: u32, max: u32) -> Option<Self> {
        let mut values = Vec::new();

        for part in field.split(',') {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }

            // Handle step values: */5, 1-10/2
            let (range_part, step) = if let Some((range, step_str)) = part.split_once('/') {
                let step: u32 = step_str.parse().ok()?;
                if step == 0 {
                    return None;
                }
                (range, Some(step))
            } else {
                (part, None)
            };

            // Handle range or wildcard
            let (start, end) = if range_part == "*" {
                (min, max)
            } else if let Some((start_str, end_str)) = range_part.split_once('-') {
                let start: u32 = start_str.parse().ok()?;
                let end: u32 = end_str.parse().ok()?;
                if start > end || start < min || end > max {
                    return None;
                }
                (start, end)
            } else {
                let val: u32 = range_part.parse().ok()?;
                if val < min || val > max {
                    return None;
                }
                (val, val)
            };

            // Generate values with optional step
            let step = step.unwrap_or(1);
            let mut v = start;
            while v <= end {
                if !values.contains(&v) {
                    values.push(v);
                }
                v += step;
            }
        }

        if values.is_empty() {
            return None;
        }

        values.sort_unstable();
        Some(Self { values })
    }

    /// Check if a value matches this field.
    fn matches(&self, value: u32) -> bool {
        self.values.contains(&value)
    }

    /// Get the next value >= current, wrapping if needed.
    fn next_value(&self, current: u32) -> (u32, bool) {
        for &v in &self.values {
            if v >= current {
                return (v, false);
            }
        }
        // Wrap around to first value
        (self.values[0], true)
    }
}

/// Parsed cron expression with all 5 fields.
#[derive(Debug, Clone)]
struct CronExpr {
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
    original: String,
}

impl CronExpr {
    /// Parse a cron expression string.
    fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();

        // Handle non-standard aliases
        let expanded = match trimmed.to_lowercase().as_str() {
            "@yearly" | "@annually" => "0 0 1 1 *",
            "@monthly" => "0 0 1 * *",
            "@weekly" => "0 0 * * 0",
            "@daily" | "@midnight" => "0 0 * * *",
            "@hourly" => "0 * * * *",
            _ => trimmed,
        };

        let parts: Vec<&str> = expanded.split_whitespace().collect();
        if parts.len() != 5 {
            return None;
        }

        let minute = CronField::parse(parts[0], 0, 59)?;
        let hour = CronField::parse(parts[1], 0, 23)?;
        let day_of_month = CronField::parse(parts[2], 1, 31)?;
        let month = CronField::parse(parts[3], 1, 12)?;
        let day_of_week = CronField::parse(parts[4], 0, 6)?;

        Some(Self {
            minute,
            hour,
            day_of_month,
            month,
            day_of_week,
            original: trimmed.to_string(),
        })
    }

    /// Generate a human-readable description.
    fn describe(&self) -> String {
        // Handle common patterns
        let trimmed = self.original.to_lowercase();
        let normalized = self.original.trim();

        // Check for aliases first
        match trimmed.as_str() {
            "@yearly" | "@annually" => return "At 00:00 on January 1st".to_string(),
            "@monthly" => return "At 00:00 on day 1 of every month".to_string(),
            "@weekly" => return "At 00:00 on Sunday".to_string(),
            "@daily" | "@midnight" => return "At 00:00 every day".to_string(),
            "@hourly" => return "At minute 0 of every hour".to_string(),
            _ => {}
        }

        let parts: Vec<&str> = normalized.split_whitespace().collect();
        if parts.len() != 5 {
            return "Invalid cron expression".to_string();
        }

        let (min, hour, dom, mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);

        // Common patterns
        match (min, hour, dom, mon, dow) {
            // Every minute
            ("*", "*", "*", "*", "*") => "Every minute".to_string(),

            // Every N minutes
            (m, "*", "*", "*", "*") if m.starts_with("*/") => {
                if let Ok(n) = m[2..].parse::<u32>() {
                    format!("Every {} minute{}", n, if n == 1 { "" } else { "s" })
                } else {
                    self.describe_complex()
                }
            }

            // Every hour at specific minute
            (m, "*", "*", "*", "*") if !m.contains('/') && !m.contains(',') && !m.contains('-') => {
                if let Ok(minute) = m.parse::<u32>() {
                    format!("At minute {} of every hour", minute)
                } else {
                    self.describe_complex()
                }
            }

            // Every N hours
            ("0", h, "*", "*", "*") if h.starts_with("*/") => {
                if let Ok(n) = h[2..].parse::<u32>() {
                    format!("Every {} hour{}", n, if n == 1 { "" } else { "s" })
                } else {
                    self.describe_complex()
                }
            }

            // Daily at specific time
            (m, h, "*", "*", "*")
                if !m.contains('/') && !h.contains('/') && !m.contains(',') && !h.contains(',') =>
            {
                if let (Ok(minute), Ok(hour)) = (m.parse::<u32>(), h.parse::<u32>()) {
                    format!("At {:02}:{:02}", hour, minute)
                } else {
                    self.describe_complex()
                }
            }

            // Weekly on specific day(s) at specific time
            (m, h, "*", "*", d)
                if !m.contains('/') && !h.contains('/') && !m.contains(',') && !h.contains(',') =>
            {
                if let (Ok(minute), Ok(hour)) = (m.parse::<u32>(), h.parse::<u32>()) {
                    let days = self.describe_dow(d);
                    format!("At {:02}:{:02} on {}", hour, minute, days)
                } else {
                    self.describe_complex()
                }
            }

            // Monthly on specific day at specific time
            (m, h, d, "*", "*")
                if !m.contains('/')
                    && !h.contains('/')
                    && !d.contains('/')
                    && !m.contains(',')
                    && !h.contains(',')
                    && !d.contains(',') =>
            {
                if let (Ok(minute), Ok(hour), Ok(day)) =
                    (m.parse::<u32>(), h.parse::<u32>(), d.parse::<u32>())
                {
                    format!("At {:02}:{:02} on day {} of every month", hour, minute, day)
                } else {
                    self.describe_complex()
                }
            }

            _ => self.describe_complex(),
        }
    }

    /// Describe day of week field.
    fn describe_dow(&self, dow: &str) -> String {
        if dow == "*" {
            return "every day".to_string();
        }

        let day_names = [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ];

        let days: Vec<String> = dow
            .split(',')
            .filter_map(|d| {
                let d = d.trim();
                if d.contains('-') {
                    let parts: Vec<&str> = d.split('-').collect();
                    if parts.len() == 2 {
                        if let (Ok(start), Ok(end)) =
                            (parts[0].parse::<usize>(), parts[1].parse::<usize>())
                        {
                            if start <= 6 && end <= 6 {
                                return Some(format!(
                                    "{} through {}",
                                    day_names[start], day_names[end]
                                ));
                            }
                        }
                    }
                    None
                } else if let Ok(n) = d.parse::<usize>() {
                    if n <= 6 {
                        Some(day_names[n].to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if days.is_empty() {
            dow.to_string()
        } else if days.len() == 1 {
            days[0].clone()
        } else {
            let last = days.last().unwrap().clone();
            let rest = &days[..days.len() - 1];
            format!("{} and {}", rest.join(", "), last)
        }
    }

    /// Generate a complex description for non-standard patterns.
    fn describe_complex(&self) -> String {
        let mut parts = Vec::new();

        // Minute description
        let min_desc = self.describe_field_values(&self.minute.values, "minute", 0, 59);
        if !min_desc.is_empty() {
            parts.push(min_desc);
        }

        // Hour description
        let hour_desc = self.describe_field_values(&self.hour.values, "hour", 0, 23);
        if !hour_desc.is_empty() {
            parts.push(hour_desc);
        }

        // Day of month
        if self.day_of_month.values.len() < 31 {
            let days: Vec<String> = self
                .day_of_month
                .values
                .iter()
                .map(|d| d.to_string())
                .collect();
            parts.push(format!("on day {} of the month", days.join(", ")));
        }

        // Month
        if self.month.values.len() < 12 {
            let month_names = [
                "",
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ];
            let months: Vec<&str> = self
                .month
                .values
                .iter()
                .filter_map(|&m| month_names.get(m as usize).copied())
                .collect();
            parts.push(format!("in {}", months.join(", ")));
        }

        // Day of week
        if self.day_of_week.values.len() < 7 {
            let day_names = [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ];
            let days: Vec<&str> = self
                .day_of_week
                .values
                .iter()
                .filter_map(|&d| day_names.get(d as usize).copied())
                .collect();
            parts.push(format!("on {}", days.join(", ")));
        }

        if parts.is_empty() {
            "Every minute".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Describe a field's values.
    fn describe_field_values(&self, values: &[u32], unit: &str, min: u32, max: u32) -> String {
        if values.len() as u32 == (max - min + 1) {
            // All values = wildcard
            return String::new();
        }

        if values.len() == 1 {
            format!("at {} {}", values[0], unit)
        } else if self.is_step(values, min, max) {
            let step = values[1] - values[0];
            format!("every {} {}s", step, unit)
        } else {
            let vals: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            format!("at {}s {}", unit, vals.join(", "))
        }
    }

    /// Check if values form a step pattern.
    fn is_step(&self, values: &[u32], min: u32, _max: u32) -> bool {
        if values.len() < 2 {
            return false;
        }
        if values[0] != min {
            return false;
        }
        let step = values[1] - values[0];
        for i in 1..values.len() {
            if values[i] - values[i - 1] != step {
                return false;
            }
        }
        true
    }

    /// Calculate the next N execution times from now.
    fn next_times(&self, count: usize) -> Vec<chrono::DateTime<Local>> {
        let mut times = Vec::with_capacity(count);
        let mut current = Local::now();

        // Start from the next minute
        current += chrono::Duration::minutes(1);
        current = current
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(current);

        let max_iterations = 366 * 24 * 60; // Max 1 year of minutes
        let mut iterations = 0;

        while times.len() < count && iterations < max_iterations {
            iterations += 1;

            let minute = current.minute();
            let hour = current.hour();
            let day = current.day();
            let month = current.month();
            let weekday = current.weekday().num_days_from_sunday();

            // Check if current time matches
            if self.minute.matches(minute)
                && self.hour.matches(hour)
                && self.day_of_month.matches(day)
                && self.month.matches(month)
                && self.day_of_week.matches(weekday)
            {
                times.push(current);
                current += chrono::Duration::minutes(1);
                continue;
            }

            // Find next matching time - try to skip ahead efficiently
            // First, check month
            if !self.month.matches(month) {
                let (next_month, wrapped) = self.month.next_value(month);
                let year = if wrapped {
                    current.year() + 1
                } else {
                    current.year()
                };
                if let Some(new_time) = Local
                    .with_ymd_and_hms(year, next_month, 1, 0, 0, 0)
                    .single()
                {
                    current = new_time;
                    continue;
                }
            }

            // Check day of month and day of week
            let dom_matches = self.day_of_month.matches(day);
            let dow_matches = self.day_of_week.matches(weekday);

            // Standard cron: if both DOM and DOW are restricted, match either (OR)
            // If only one is restricted, match that one
            let dom_is_wildcard = self.day_of_month.values.len() == 31;
            let dow_is_wildcard = self.day_of_week.values.len() == 7;

            let day_matches = if dom_is_wildcard && dow_is_wildcard {
                true
            } else if dom_is_wildcard {
                dow_matches
            } else if dow_is_wildcard {
                dom_matches
            } else {
                dom_matches || dow_matches
            };

            if !day_matches {
                // Move to next day
                current = (current + chrono::Duration::days(1))
                    .with_hour(0)
                    .and_then(|t| t.with_minute(0))
                    .and_then(|t| t.with_second(0))
                    .unwrap_or(current + chrono::Duration::days(1));
                continue;
            }

            // Check hour
            if !self.hour.matches(hour) {
                let (next_hour, wrapped) = self.hour.next_value(hour);
                if wrapped {
                    current = (current + chrono::Duration::days(1))
                        .with_hour(next_hour)
                        .and_then(|t| t.with_minute(0))
                        .and_then(|t| t.with_second(0))
                        .unwrap_or(current + chrono::Duration::hours(1));
                } else if let Some(new_time) = current
                    .with_hour(next_hour)
                    .and_then(|t| t.with_minute(0))
                    .and_then(|t| t.with_second(0))
                {
                    current = new_time;
                } else {
                    current += chrono::Duration::hours(1);
                }
                continue;
            }

            // Check minute
            if !self.minute.matches(minute) {
                let (next_minute, wrapped) = self.minute.next_value(minute);
                if wrapped {
                    current += chrono::Duration::hours(1);
                    if let Some(new_time) = current
                        .with_minute(next_minute)
                        .and_then(|t| t.with_second(0))
                    {
                        current = new_time;
                    }
                } else if let Some(new_time) = current
                    .with_minute(next_minute)
                    .and_then(|t| t.with_second(0))
                {
                    current = new_time;
                } else {
                    current += chrono::Duration::minutes(1);
                }
                continue;
            }

            // Should have matched above
            current += chrono::Duration::minutes(1);
        }

        times
    }
}

impl CronFormat {
    /// Check if input looks like a cron expression.
    fn looks_like_cron(input: &str) -> bool {
        let trimmed = input.trim();

        // Check for aliases
        if trimmed.starts_with('@') {
            let lower = trimmed.to_lowercase();
            return matches!(
                lower.as_str(),
                "@yearly"
                    | "@annually"
                    | "@monthly"
                    | "@weekly"
                    | "@daily"
                    | "@midnight"
                    | "@hourly"
            );
        }

        // Must have exactly 5 space-separated parts
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() != 5 {
            return false;
        }

        // Each part must contain only valid cron characters
        for part in parts {
            if !part
                .chars()
                .all(|c| c.is_ascii_digit() || c == '*' || c == '/' || c == '-' || c == ',')
            {
                return false;
            }
        }

        true
    }
}

impl Format for CronFormat {
    fn id(&self) -> &'static str {
        "cron"
    }

    fn name(&self) -> &'static str {
        "Cron Expression"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Scheduling",
            description: "Cron schedule expressions (5-field format and @aliases)",
            examples: &["*/5 * * * *", "0 2 * * *", "0 0 1 * *", "@daily", "@hourly"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        if !Self::looks_like_cron(input) {
            return vec![];
        }

        let Some(expr) = CronExpr::parse(input) else {
            return vec![];
        };

        let description = expr.describe();
        let next_times = expr.next_times(5);

        // Build description with next execution times
        let next_times_str: Vec<String> = next_times
            .iter()
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .collect();

        let full_description = if next_times.is_empty() {
            description.clone()
        } else {
            format!("{}\nNext: {}", description, next_times_str.join(", "))
        };

        // Build rich display with key-value pairs
        let mut pairs = vec![("Schedule".to_string(), description.clone())];

        if let Some(first) = next_times.first() {
            pairs.push((
                "Next".to_string(),
                first.format("%Y-%m-%d %H:%M:%S").to_string(),
            ));
        }

        for (i, time) in next_times.iter().skip(1).take(4).enumerate() {
            pairs.push((
                format!("Then #{}", i + 2),
                time.format("%Y-%m-%d %H:%M:%S").to_string(),
            ));
        }

        vec![Interpretation {
            value: CoreValue::String(input.trim().to_string()),
            source_format: "cron".to_string(),
            confidence: 0.9,
            description: full_description,
            rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue { pairs })],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        vec![]
    }

    fn source_conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::String(cron_str) = value else {
            return vec![];
        };

        let Some(expr) = CronExpr::parse(cron_str) else {
            return vec![];
        };

        let next_times = expr.next_times(5);
        let mut conversions = Vec::new();

        // Add conversion showing the human-readable description
        conversions.push(Conversion {
            value: CoreValue::String(expr.describe()),
            target_format: "cron-description".to_string(),
            display: expr.describe(),
            path: vec!["cron-description".to_string()],
            is_lossy: false,
            steps: vec![],
            priority: ConversionPriority::Semantic,
            display_only: true,
            kind: ConversionKind::Representation,
            hidden: false,
            rich_display: vec![],
        });

        // Add conversion for next execution time as DateTime
        if let Some(next) = next_times.first() {
            let utc_time = next.with_timezone(&Utc);
            let iso = utc_time.to_rfc3339();
            let relative = format_relative(*next);

            conversions.push(Conversion {
                value: CoreValue::DateTime(utc_time),
                target_format: "cron-next".to_string(),
                display: format!("{} ({})", next.format("%Y-%m-%d %H:%M:%S"), relative),
                path: vec!["cron-next".to_string()],
                is_lossy: false,
                steps: vec![],
                priority: ConversionPriority::Semantic,
                display_only: false,
                kind: ConversionKind::Conversion,
                hidden: false,
                rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                    epoch_millis: utc_time.timestamp_millis(),
                    iso,
                    relative,
                })],
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["crontab"]
    }
}

/// Format a relative time string.
fn format_relative(dt: chrono::DateTime<Local>) -> String {
    let now = Local::now();
    let diff = dt.signed_duration_since(now);
    let secs = diff.num_seconds();

    if secs < 0 {
        return "in the past".to_string();
    }

    if secs < 60 {
        return format!("in {} seconds", secs);
    }

    let mins = secs / 60;
    if mins < 60 {
        return format!("in {} minute{}", mins, if mins == 1 { "" } else { "s" });
    }

    let hours = mins / 60;
    if hours < 24 {
        let remaining_mins = mins % 60;
        if remaining_mins > 0 {
            return format!("in {}h {}m", hours, remaining_mins);
        }
        return format!("in {} hour{}", hours, if hours == 1 { "" } else { "s" });
    }

    let days = hours / 24;
    if days < 7 {
        return format!("in {} day{}", days, if days == 1 { "" } else { "s" });
    }

    format!("in {} days", days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_every_minute() {
        let format = CronFormat;
        let results = format.parse("* * * * *");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("Every minute"));
    }

    #[test]
    fn test_parse_every_5_minutes() {
        let format = CronFormat;
        let results = format.parse("*/5 * * * *");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("Every 5 minutes"));
    }

    #[test]
    fn test_parse_daily_at_2am() {
        let format = CronFormat;
        let results = format.parse("0 2 * * *");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("02:00"));
    }

    #[test]
    fn test_parse_alias_daily() {
        let format = CronFormat;
        let results = format.parse("@daily");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("00:00"));
    }

    #[test]
    fn test_parse_alias_hourly() {
        let format = CronFormat;
        let results = format.parse("@hourly");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("every hour"));
    }

    #[test]
    fn test_parse_weekly() {
        let format = CronFormat;
        let results = format.parse("0 9 * * 1");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("Monday"));
        assert!(results[0].description.contains("09:00"));
    }

    #[test]
    fn test_parse_monthly() {
        let format = CronFormat;
        let results = format.parse("0 0 1 * *");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("day 1"));
    }

    #[test]
    fn test_invalid_cron() {
        let format = CronFormat;
        assert!(format.parse("not a cron").is_empty());
        assert!(format.parse("* * *").is_empty()); // Too few fields
        assert!(format.parse("60 * * * *").is_empty()); // Invalid minute
    }

    #[test]
    fn test_cron_field_parse() {
        // Simple value
        let field = CronField::parse("5", 0, 59).unwrap();
        assert_eq!(field.values, vec![5]);

        // Wildcard
        let field = CronField::parse("*", 0, 5).unwrap();
        assert_eq!(field.values, vec![0, 1, 2, 3, 4, 5]);

        // Range
        let field = CronField::parse("1-3", 0, 10).unwrap();
        assert_eq!(field.values, vec![1, 2, 3]);

        // Step
        let field = CronField::parse("*/2", 0, 6).unwrap();
        assert_eq!(field.values, vec![0, 2, 4, 6]);

        // List
        let field = CronField::parse("1,3,5", 0, 10).unwrap();
        assert_eq!(field.values, vec![1, 3, 5]);

        // Range with step
        let field = CronField::parse("0-10/2", 0, 59).unwrap();
        assert_eq!(field.values, vec![0, 2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_next_times() {
        let expr = CronExpr::parse("* * * * *").unwrap();
        let times = expr.next_times(3);
        assert_eq!(times.len(), 3);

        // Each time should be 1 minute apart
        for i in 1..times.len() {
            let diff = times[i].signed_duration_since(times[i - 1]);
            assert_eq!(diff.num_minutes(), 1);
        }
    }

    #[test]
    fn test_confidence() {
        let format = CronFormat;
        let results = format.parse("*/5 * * * *");
        assert!(!results.is_empty());
        assert!(results[0].confidence >= 0.8);
    }
}
