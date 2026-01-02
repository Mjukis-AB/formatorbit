//! Natural language date/time parsing.
//!
//! Parses human-friendly date/time expressions like:
//! - Time of day: `15:00`, `3:30pm`, `9am`
//! - Relative words: `now`, `today`, `tomorrow`, `yesterday`
//! - Relative periods: `next week`, `last month`, `next year`
//! - Weekdays: `monday`, `next friday`, `last tuesday`
//! - Relative offsets: `in 2 days`, `3 weeks ago`
//! - Month + day: `15 dec`, `march 15th`
//! - Special dates: `christmas`, `halloween`
//! - Period boundaries: `end of month`, `eom`, `start of year`
//! - Quarters: `q1`, `next quarter`

use chrono::{Datelike, Duration, Local, NaiveTime, TimeZone, Utc, Weekday};
use regex::Regex;
use std::sync::OnceLock;

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation, RichDisplay, RichDisplayOption};

pub struct NaturalDateFormat;

/// Result of parsing a natural date expression
struct ParseResult {
    datetime: chrono::DateTime<Utc>,
    confidence: f32,
    description: String,
    is_now: bool, // Special flag for "now" to use LiveClock
}

/// Regex patterns for natural date parsing
fn patterns() -> &'static DatePatterns {
    static PATTERNS: OnceLock<DatePatterns> = OnceLock::new();
    PATTERNS.get_or_init(DatePatterns::new)
}

struct DatePatterns {
    // Time of day: 15:00, 15:00:30, 3:30pm, 9am
    time_24h: Regex,
    time_24h_seconds: Regex,
    time_12h: Regex,
    time_12h_bare: Regex,

    // Relative offsets: "in 2 days", "3 weeks ago", "a month from now"
    in_n_units: Regex,
    n_units_ago: Regex,
    a_unit_ago: Regex,
    a_unit_from_now: Regex,

    // Month + day: "15 dec", "dec 15", "march 15th"
    day_month: Regex,
    month_day: Regex,
}

impl DatePatterns {
    fn new() -> Self {
        Self {
            // 15:00 or 15:30
            time_24h: Regex::new(r"^(\d{1,2}):(\d{2})$").unwrap(),
            // 15:00:30
            time_24h_seconds: Regex::new(r"^(\d{1,2}):(\d{2}):(\d{2})$").unwrap(),
            // 3:30pm, 3:30 pm, 3:30PM
            time_12h: Regex::new(r"^(\d{1,2}):(\d{2})\s*(am|pm|AM|PM)$").unwrap(),
            // 9am, 9 am, 9AM, 12pm
            time_12h_bare: Regex::new(r"^(\d{1,2})\s*(am|pm|AM|PM)$").unwrap(),

            // "in 2 days", "in 3 weeks"
            in_n_units: Regex::new(
                r"(?i)^in\s+(\d+)\s+(days?|weeks?|months?|years?|hours?|minutes?)$",
            )
            .unwrap(),
            // "2 days ago", "3 weeks ago"
            n_units_ago: Regex::new(
                r"(?i)^(\d+)\s+(days?|weeks?|months?|years?|hours?|minutes?)\s+ago$",
            )
            .unwrap(),
            // "a week ago", "one month ago"
            a_unit_ago: Regex::new(
                r"(?i)^(a|an|one)\s+(day|week|month|year|hour|minute)\s+ago$",
            )
            .unwrap(),
            // "a month from now", "one week from now"
            a_unit_from_now: Regex::new(
                r"(?i)^(a|an|one)\s+(day|week|month|year|hour|minute)\s+from\s+now$",
            )
            .unwrap(),

            // "15 dec", "15 december", "15th dec"
            day_month: Regex::new(
                r"(?i)^(\d{1,2})(?:st|nd|rd|th)?\s+(jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|june?|july?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)$"
            ).unwrap(),
            // "dec 15", "december 15", "dec 15th"
            month_day: Regex::new(
                r"(?i)^(jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|june?|july?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)\s+(\d{1,2})(?:st|nd|rd|th)?$"
            ).unwrap(),
        }
    }
}

impl NaturalDateFormat {
    /// Parse time of day: 15:00, 15:00:30, 3:30pm, 9am
    fn parse_time_of_day(input: &str) -> Option<ParseResult> {
        let patterns = patterns();
        let trimmed = input.trim();
        let local_now = Local::now();
        let today = local_now.date_naive();

        // Try 24-hour format with seconds: 15:00:30
        if let Some(caps) = patterns.time_24h_seconds.captures(trimmed) {
            let hour: u32 = caps[1].parse().ok()?;
            let min: u32 = caps[2].parse().ok()?;
            let sec: u32 = caps[3].parse().ok()?;
            if hour < 24 && min < 60 && sec < 60 {
                let time = NaiveTime::from_hms_opt(hour, min, sec)?;
                let naive_dt = today.and_time(time);
                let local_dt = Local.from_local_datetime(&naive_dt).single()?;
                return Some(ParseResult {
                    datetime: local_dt.with_timezone(&Utc),
                    confidence: 0.92,
                    description: format!("Time: {:02}:{:02}:{:02}", hour, min, sec),
                    is_now: false,
                });
            }
        }

        // Try 24-hour format: 15:00
        if let Some(caps) = patterns.time_24h.captures(trimmed) {
            let hour: u32 = caps[1].parse().ok()?;
            let min: u32 = caps[2].parse().ok()?;
            if hour < 24 && min < 60 {
                let time = NaiveTime::from_hms_opt(hour, min, 0)?;
                let naive_dt = today.and_time(time);
                let local_dt = Local.from_local_datetime(&naive_dt).single()?;
                return Some(ParseResult {
                    datetime: local_dt.with_timezone(&Utc),
                    confidence: 0.90,
                    description: format!("Time: {:02}:{:02}", hour, min),
                    is_now: false,
                });
            }
        }

        // Try 12-hour format with minutes: 3:30pm
        if let Some(caps) = patterns.time_12h.captures(trimmed) {
            let mut hour: u32 = caps[1].parse().ok()?;
            let min: u32 = caps[2].parse().ok()?;
            let ampm = caps[3].to_lowercase();

            if (1..=12).contains(&hour) && min < 60 {
                // Convert to 24-hour
                if ampm == "pm" && hour != 12 {
                    hour += 12;
                } else if ampm == "am" && hour == 12 {
                    hour = 0;
                }

                let time = NaiveTime::from_hms_opt(hour, min, 0)?;
                let naive_dt = today.and_time(time);
                let local_dt = Local.from_local_datetime(&naive_dt).single()?;
                return Some(ParseResult {
                    datetime: local_dt.with_timezone(&Utc),
                    confidence: 0.90,
                    description: format!("Time: {}", trimmed),
                    is_now: false,
                });
            }
        }

        // Try bare 12-hour format: 9am, 12pm
        if let Some(caps) = patterns.time_12h_bare.captures(trimmed) {
            let mut hour: u32 = caps[1].parse().ok()?;
            let ampm = caps[2].to_lowercase();

            if (1..=12).contains(&hour) {
                if ampm == "pm" && hour != 12 {
                    hour += 12;
                } else if ampm == "am" && hour == 12 {
                    hour = 0;
                }

                let time = NaiveTime::from_hms_opt(hour, 0, 0)?;
                let naive_dt = today.and_time(time);
                let local_dt = Local.from_local_datetime(&naive_dt).single()?;
                return Some(ParseResult {
                    datetime: local_dt.with_timezone(&Utc),
                    confidence: 0.88,
                    description: format!("Time: {}", trimmed),
                    is_now: false,
                });
            }
        }

        None
    }

    /// Parse relative words: now, today, tomorrow, yesterday
    fn parse_relative_word(input: &str) -> Option<ParseResult> {
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today_start = local_now.date_naive().and_hms_opt(0, 0, 0)?;

        match lower.as_str() {
            "now" => Some(ParseResult {
                datetime: Utc::now(),
                confidence: 0.98,
                description: "Current time".to_string(),
                is_now: true,
            }),
            "today" => {
                let dt = Local.from_local_datetime(&today_start).single()?;
                Some(ParseResult {
                    datetime: dt.with_timezone(&Utc),
                    confidence: 0.95,
                    description: "Today (start of day)".to_string(),
                    is_now: false,
                })
            }
            "tomorrow" => {
                let tomorrow = today_start + Duration::days(1);
                let dt = Local.from_local_datetime(&tomorrow).single()?;
                Some(ParseResult {
                    datetime: dt.with_timezone(&Utc),
                    confidence: 0.95,
                    description: "Tomorrow (start of day)".to_string(),
                    is_now: false,
                })
            }
            "yesterday" => {
                let yesterday = today_start - Duration::days(1);
                let dt = Local.from_local_datetime(&yesterday).single()?;
                Some(ParseResult {
                    datetime: dt.with_timezone(&Utc),
                    confidence: 0.95,
                    description: "Yesterday (start of day)".to_string(),
                    is_now: false,
                })
            }
            _ => None,
        }
    }

    /// Parse relative periods: next week, last month, next year
    fn parse_relative_period(input: &str) -> Option<ParseResult> {
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today_start = local_now.date_naive().and_hms_opt(0, 0, 0)?;

        let (offset, desc) = match lower.as_str() {
            "next week" => (Duration::weeks(1), "Next week"),
            "last week" => (Duration::weeks(-1), "Last week"),
            "next month" => (Duration::days(30), "Next month"), // Approximate
            "last month" => (Duration::days(-30), "Last month"),
            "next year" => (Duration::days(365), "Next year"),
            "last year" => (Duration::days(-365), "Last year"),
            _ => return None,
        };

        let target = today_start + offset;
        let dt = Local.from_local_datetime(&target).single()?;
        Some(ParseResult {
            datetime: dt.with_timezone(&Utc),
            confidence: 0.90,
            description: desc.to_string(),
            is_now: false,
        })
    }

    /// Parse weekdays: monday, next friday, last tuesday
    fn parse_weekday(input: &str) -> Option<ParseResult> {
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today = local_now.date_naive();
        let current_weekday = today.weekday();

        // Parse the weekday name
        let (prefix, weekday_str) = if let Some(rest) = lower.strip_prefix("next ") {
            ("next", rest)
        } else if let Some(rest) = lower.strip_prefix("last ") {
            ("last", rest)
        } else if let Some(rest) = lower.strip_prefix("this ") {
            ("this", rest)
        } else {
            ("", lower.as_str())
        };

        let target_weekday = match weekday_str.trim() {
            "monday" | "mon" => Weekday::Mon,
            "tuesday" | "tue" | "tues" => Weekday::Tue,
            "wednesday" | "wed" => Weekday::Wed,
            "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
            "friday" | "fri" => Weekday::Fri,
            "saturday" | "sat" => Weekday::Sat,
            "sunday" | "sun" => Weekday::Sun,
            _ => return None,
        };

        // Calculate days until target weekday
        let current_num = current_weekday.num_days_from_monday() as i64;
        let target_num = target_weekday.num_days_from_monday() as i64;

        let days_offset = match prefix {
            "next" => {
                // Next week's occurrence
                let diff = target_num - current_num;
                (if diff <= 0 { diff + 7 } else { diff }) + 7
            }
            "last" => {
                // Previous occurrence
                let diff = target_num - current_num;
                if diff >= 0 {
                    diff - 7
                } else {
                    diff
                }
            }
            "this" => {
                // This week (past or future)
                target_num - current_num
            }
            "" => {
                // Next occurrence (including today if it's that day)
                let diff = target_num - current_num;
                if diff < 0 {
                    diff + 7
                } else if diff == 0 {
                    7
                } else {
                    diff
                }
            }
            _ => return None,
        };

        let target_date = today + Duration::days(days_offset);
        let target_dt = target_date.and_hms_opt(0, 0, 0)?;
        let dt = Local.from_local_datetime(&target_dt).single()?;

        let prefix_display = if prefix.is_empty() {
            "Next".to_string()
        } else {
            // Capitalize first letter
            let mut chars = prefix.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => prefix.to_string(),
            }
        };

        Some(ParseResult {
            datetime: dt.with_timezone(&Utc),
            confidence: 0.90,
            description: format!("{} {}", prefix_display, weekday_str),
            is_now: false,
        })
    }

    /// Parse relative offsets: "in 2 days", "3 weeks ago"
    fn parse_relative_offset(input: &str) -> Option<ParseResult> {
        let patterns = patterns();
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();

        // "in N units"
        if let Some(caps) = patterns.in_n_units.captures(&lower) {
            let n: i64 = caps[1].parse().ok()?;
            let unit = &caps[2];
            let offset = Self::unit_to_duration(unit, n)?;
            let target = local_now + offset;
            return Some(ParseResult {
                datetime: target.with_timezone(&Utc),
                confidence: 0.88,
                description: format!("In {} {}", n, unit),
                is_now: false,
            });
        }

        // "N units ago"
        if let Some(caps) = patterns.n_units_ago.captures(&lower) {
            let n: i64 = caps[1].parse().ok()?;
            let unit = &caps[2];
            let offset = Self::unit_to_duration(unit, n)?;
            let target = local_now - offset;
            return Some(ParseResult {
                datetime: target.with_timezone(&Utc),
                confidence: 0.88,
                description: format!("{} {} ago", n, unit),
                is_now: false,
            });
        }

        // "a/one unit ago"
        if let Some(caps) = patterns.a_unit_ago.captures(&lower) {
            let unit = &caps[2];
            let offset = Self::unit_to_duration(unit, 1)?;
            let target = local_now - offset;
            return Some(ParseResult {
                datetime: target.with_timezone(&Utc),
                confidence: 0.88,
                description: format!("A {} ago", unit),
                is_now: false,
            });
        }

        // "a/one unit from now"
        if let Some(caps) = patterns.a_unit_from_now.captures(&lower) {
            let unit = &caps[2];
            let offset = Self::unit_to_duration(unit, 1)?;
            let target = local_now + offset;
            return Some(ParseResult {
                datetime: target.with_timezone(&Utc),
                confidence: 0.88,
                description: format!("A {} from now", unit),
                is_now: false,
            });
        }

        None
    }

    /// Convert unit string to Duration
    fn unit_to_duration(unit: &str, n: i64) -> Option<Duration> {
        let unit_lower = unit.to_lowercase();
        let base = if unit_lower.starts_with("day") {
            Duration::days(1)
        } else if unit_lower.starts_with("week") {
            Duration::weeks(1)
        } else if unit_lower.starts_with("month") {
            Duration::days(30) // Approximate
        } else if unit_lower.starts_with("year") {
            Duration::days(365)
        } else if unit_lower.starts_with("hour") {
            Duration::hours(1)
        } else if unit_lower.starts_with("minute") {
            Duration::minutes(1)
        } else {
            return None;
        };

        Some(base * n as i32)
    }

    /// Parse month + day: "15 dec", "dec 15", "march 15th"
    fn parse_month_day(input: &str) -> Option<ParseResult> {
        let patterns = patterns();
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today = local_now.date_naive();

        let (day, month_str) = if let Some(caps) = patterns.day_month.captures(&lower) {
            (caps[1].parse::<u32>().ok()?, caps[2].to_string())
        } else if let Some(caps) = patterns.month_day.captures(&lower) {
            (caps[2].parse::<u32>().ok()?, caps[1].to_string())
        } else {
            return None;
        };

        // Validate day
        if !(1..=31).contains(&day) {
            return None;
        }

        let month = Self::month_name_to_number(&month_str)?;

        // Determine year: next occurrence
        let mut year = today.year();
        let target_date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;

        // If the date has passed this year, use next year
        if target_date < today {
            year += 1;
        }

        let target_date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
        let target_dt = target_date.and_hms_opt(0, 0, 0)?;
        let dt = Local.from_local_datetime(&target_dt).single()?;

        Some(ParseResult {
            datetime: dt.with_timezone(&Utc),
            confidence: 0.85,
            description: format!("{} {}", Self::month_number_to_name(month), day),
            is_now: false,
        })
    }

    /// Convert month name to number (1-12)
    fn month_name_to_number(name: &str) -> Option<u32> {
        let lower = name.to_lowercase();
        match lower.as_str() {
            s if s.starts_with("jan") => Some(1),
            s if s.starts_with("feb") => Some(2),
            s if s.starts_with("mar") => Some(3),
            s if s.starts_with("apr") => Some(4),
            "may" => Some(5),
            s if s.starts_with("jun") => Some(6),
            s if s.starts_with("jul") => Some(7),
            s if s.starts_with("aug") => Some(8),
            s if s.starts_with("sep") => Some(9),
            s if s.starts_with("oct") => Some(10),
            s if s.starts_with("nov") => Some(11),
            s if s.starts_with("dec") => Some(12),
            _ => None,
        }
    }

    /// Convert month number to short name
    fn month_number_to_name(month: u32) -> &'static str {
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

    /// Parse special dates: christmas, halloween, etc.
    fn parse_special_date(input: &str) -> Option<ParseResult> {
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today = local_now.date_naive();
        let year = today.year();

        let (month, day, name) = match lower.as_str() {
            "christmas" | "xmas" => (12, 25, "Christmas"),
            "new years" | "new year" | "nye" | "new years day" | "new year's day" => {
                (1, 1, "New Year's Day")
            }
            "halloween" => (10, 31, "Halloween"),
            "valentines" | "valentine's day" | "valentines day" => (2, 14, "Valentine's Day"),
            "independence day" | "4th of july" | "july 4th" => (7, 4, "Independence Day"),
            "thanksgiving" => {
                // 4th Thursday of November - complex to calculate
                // For simplicity, approximate as Nov 25
                (11, 25, "Thanksgiving (approx)")
            }
            _ => return None,
        };

        // Next occurrence
        let mut target_year = year;
        let target_date = chrono::NaiveDate::from_ymd_opt(target_year, month, day)?;
        if target_date < today {
            target_year += 1;
        }

        let target_date = chrono::NaiveDate::from_ymd_opt(target_year, month, day)?;
        let target_dt = target_date.and_hms_opt(0, 0, 0)?;
        let dt = Local.from_local_datetime(&target_dt).single()?;

        Some(ParseResult {
            datetime: dt.with_timezone(&Utc),
            confidence: 0.92,
            description: name.to_string(),
            is_now: false,
        })
    }

    /// Parse period boundaries: end of month, start of year, etc.
    fn parse_period_boundary(input: &str) -> Option<ParseResult> {
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today = local_now.date_naive();

        let (target_date, desc) = match lower.as_str() {
            "end of month" | "eom" => {
                let next_month = if today.month() == 12 {
                    chrono::NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)?
                } else {
                    chrono::NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)?
                };
                let last_day = next_month - Duration::days(1);
                (last_day, "End of month")
            }
            "start of month" | "som" => {
                let first_day = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1)?;
                (first_day, "Start of month")
            }
            "end of year" | "eoy" => {
                let last_day = chrono::NaiveDate::from_ymd_opt(today.year(), 12, 31)?;
                (last_day, "End of year")
            }
            "start of year" | "soy" => {
                let first_day = chrono::NaiveDate::from_ymd_opt(today.year(), 1, 1)?;
                (first_day, "Start of year")
            }
            "end of week" | "eow" => {
                // End of week = Sunday
                let days_until_sunday = 6 - today.weekday().num_days_from_monday() as i64;
                let sunday = today + Duration::days(days_until_sunday);
                (sunday, "End of week (Sunday)")
            }
            "start of week" | "sow" => {
                // Start of week = Monday
                let days_since_monday = today.weekday().num_days_from_monday() as i64;
                let monday = today - Duration::days(days_since_monday);
                (monday, "Start of week (Monday)")
            }
            _ => return None,
        };

        let target_dt = target_date.and_hms_opt(0, 0, 0)?;
        let dt = Local.from_local_datetime(&target_dt).single()?;

        Some(ParseResult {
            datetime: dt.with_timezone(&Utc),
            confidence: 0.88,
            description: desc.to_string(),
            is_now: false,
        })
    }

    /// Parse quarters: q1, q2, next quarter
    fn parse_quarter(input: &str) -> Option<ParseResult> {
        let lower = input.trim().to_lowercase();
        let local_now = Local::now();
        let today = local_now.date_naive();
        let year = today.year();
        let current_quarter = (today.month() - 1) / 3 + 1;

        let (target_year, quarter, desc) = match lower.as_str() {
            "q1" => (year, 1, "Q1 (January 1)"),
            "q2" => (year, 2, "Q2 (April 1)"),
            "q3" => (year, 3, "Q3 (July 1)"),
            "q4" => (year, 4, "Q4 (October 1)"),
            "next quarter" => {
                let next_q = if current_quarter == 4 {
                    1
                } else {
                    current_quarter + 1
                };
                let next_year = if current_quarter == 4 { year + 1 } else { year };
                (next_year, next_q, "Next quarter")
            }
            "last quarter" => {
                let prev_q = if current_quarter == 1 {
                    4
                } else {
                    current_quarter - 1
                };
                let prev_year = if current_quarter == 1 { year - 1 } else { year };
                (prev_year, prev_q, "Last quarter")
            }
            _ => return None,
        };

        let month = match quarter {
            1 => 1,
            2 => 4,
            3 => 7,
            4 => 10,
            _ => return None,
        };

        let target_date = chrono::NaiveDate::from_ymd_opt(target_year, month, 1)?;
        let target_dt = target_date.and_hms_opt(0, 0, 0)?;
        let dt = Local.from_local_datetime(&target_dt).single()?;

        Some(ParseResult {
            datetime: dt.with_timezone(&Utc),
            confidence: 0.85,
            description: desc.to_string(),
            is_now: false,
        })
    }

    /// Format a datetime relative to now
    fn format_relative(dt: chrono::DateTime<Utc>) -> String {
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
}

impl Format for NaturalDateFormat {
    fn id(&self) -> &'static str {
        "natural-date"
    }

    fn name(&self) -> &'static str {
        "Natural Date"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Timestamps",
            description: "Natural language dates (15:00, tomorrow, next monday, dec 15, christmas)",
            examples: &["15:00", "tomorrow", "next friday", "dec 15", "christmas"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try each parser in order of specificity
        let result = Self::parse_relative_word(input)
            .or_else(|| Self::parse_time_of_day(input))
            .or_else(|| Self::parse_relative_period(input))
            .or_else(|| Self::parse_weekday(input))
            .or_else(|| Self::parse_relative_offset(input))
            .or_else(|| Self::parse_month_day(input))
            .or_else(|| Self::parse_special_date(input))
            .or_else(|| Self::parse_period_boundary(input))
            .or_else(|| Self::parse_quarter(input));

        let Some(result) = result else {
            return vec![];
        };

        let iso = result.datetime.to_rfc3339();
        let relative = Self::format_relative(result.datetime);

        let rich_display = if result.is_now {
            vec![RichDisplayOption::new(RichDisplay::LiveClock {
                label: "Now".to_string(),
            })]
        } else {
            vec![RichDisplayOption::new(RichDisplay::DateTime {
                epoch_millis: result.datetime.timestamp_millis(),
                iso: iso.clone(),
                relative: relative.clone(),
            })]
        };

        vec![Interpretation {
            value: CoreValue::DateTime(result.datetime),
            source_format: "natural-date".to_string(),
            confidence: result.confidence,
            description: format!("{} → {} ({})", result.description, iso, relative),
            rich_display,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // Formatting handled by datetime.rs
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["nat-date", "human-date"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_24h() {
        let format = NaturalDateFormat;
        let results = format.parse("15:00");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.90);
        assert!(results[0].description.contains("Time"));
    }

    #[test]
    fn test_parse_time_12h() {
        let format = NaturalDateFormat;
        let results = format.parse("3:30pm");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.88);
    }

    #[test]
    fn test_parse_now() {
        let format = NaturalDateFormat;
        let results = format.parse("now");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.98);
        // Should have LiveClock rich display
        assert!(!results[0].rich_display.is_empty());
    }

    #[test]
    fn test_parse_tomorrow() {
        let format = NaturalDateFormat;
        let results = format.parse("tomorrow");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.95);
    }

    #[test]
    fn test_parse_weekday() {
        let format = NaturalDateFormat;
        let results = format.parse("monday");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.90);
    }

    #[test]
    fn test_parse_month_day() {
        let format = NaturalDateFormat;
        let results = format.parse("dec 15");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.85);
    }

    #[test]
    fn test_parse_christmas() {
        let format = NaturalDateFormat;
        let results = format.parse("christmas");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.92);
    }
}
