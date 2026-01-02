//! Epoch timestamp format.
//!
//! Parses numeric strings as Unix epoch timestamps (seconds or milliseconds since 1970-01-01).
//! Uses dynamic confidence scoring based on proximity to current time.

use chrono::{DateTime, TimeZone, Utc};
use tracing::{debug, trace};

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation, RichDisplay, RichDisplayOption};

use super::datetime::{
    MAX_EPOCH_MICROS, MAX_EPOCH_MILLIS, MAX_EPOCH_NANOS, MAX_EPOCH_SECONDS, MIN_EPOCH_MICROS,
    MIN_EPOCH_MILLIS, MIN_EPOCH_NANOS, MIN_EPOCH_SECONDS,
};

pub struct EpochFormat;

impl EpochFormat {
    /// Calculate dynamic confidence based on proximity to current time.
    /// Timestamps closer to "now" are more likely to be intentional.
    fn calculate_confidence(dt: DateTime<Utc>) -> f32 {
        let now = Utc::now();
        let diff_secs = (dt.timestamp() - now.timestamp()).abs();

        const WEEK: i64 = 7 * 24 * 3600;
        const YEAR: i64 = 365 * 24 * 3600;
        const THIRTY_YEARS: i64 = 30 * YEAR;

        if diff_secs < WEEK {
            0.95 // Within a week - almost certainly intentional
        } else if diff_secs < YEAR {
            0.90 // Within a year - very likely a timestamp
        } else if diff_secs < THIRTY_YEARS {
            0.87 // Within 30 years - probably a timestamp (beats decimal's 0.85)
        } else {
            0.75 // Distant (but still valid range) - less certain
        }
    }

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
}

impl Format for EpochFormat {
    fn id(&self) -> &'static str {
        "epoch"
    }

    fn name(&self) -> &'static str {
        "Epoch Timestamp"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Timestamps",
            description:
                "Unix epoch timestamp (seconds, milliseconds, microseconds, or nanoseconds)",
            examples: &["1735344000", "1735344000000", "1735344000000000"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();
        trace!(input_len = trimmed.len(), "epoch: checking input");

        // Must be a valid integer
        let Ok(value) = trimmed.parse::<i64>() else {
            trace!("epoch: rejected - not a valid integer");
            return vec![];
        };

        let mut results = vec![];

        // Check if valid epoch seconds
        if (MIN_EPOCH_SECONDS..=MAX_EPOCH_SECONDS).contains(&value) {
            if let Some(dt) = Utc.timestamp_opt(value, 0).single() {
                let confidence = Self::calculate_confidence(dt);
                let iso = dt.to_rfc3339();
                let relative = Self::format_relative(dt);

                debug!(epoch = value, confidence, iso, "epoch: matched as seconds");

                results.push(Interpretation {
                    value: CoreValue::DateTime(dt),
                    source_format: "epoch-seconds".to_string(),
                    confidence,
                    description: format!("{} ({})", iso, relative),
                    rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                        epoch_millis: value * 1000,
                        iso: iso.clone(),
                        relative,
                    })],
                });
            }
        }

        // Check if valid epoch milliseconds
        if (MIN_EPOCH_MILLIS..=MAX_EPOCH_MILLIS).contains(&value) {
            let secs = value / 1000;
            let nanos = ((value % 1000) * 1_000_000) as u32;
            if let Some(dt) = Utc.timestamp_opt(secs, nanos).single() {
                // Milliseconds get slightly lower confidence than seconds
                // to avoid both appearing at same confidence
                let base_confidence = Self::calculate_confidence(dt);
                let confidence = (base_confidence - 0.05).max(0.70);
                let iso = dt.to_rfc3339();
                let relative = Self::format_relative(dt);

                debug!(
                    epoch = value,
                    confidence, iso, "epoch: matched as milliseconds"
                );

                results.push(Interpretation {
                    value: CoreValue::DateTime(dt),
                    source_format: "epoch-millis".to_string(),
                    confidence,
                    description: format!("{} ({})", iso, relative),
                    rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                        epoch_millis: value,
                        iso: iso.clone(),
                        relative,
                    })],
                });
            }
        }

        // Check if valid epoch microseconds
        if (MIN_EPOCH_MICROS..=MAX_EPOCH_MICROS).contains(&value) {
            let secs = value / 1_000_000;
            let nanos = ((value % 1_000_000) * 1_000) as u32;
            if let Some(dt) = Utc.timestamp_opt(secs, nanos).single() {
                let base_confidence = Self::calculate_confidence(dt);
                let confidence = (base_confidence - 0.10).max(0.65);
                let iso = dt.to_rfc3339();
                let relative = Self::format_relative(dt);

                debug!(
                    epoch = value,
                    confidence, iso, "epoch: matched as microseconds"
                );

                results.push(Interpretation {
                    value: CoreValue::DateTime(dt),
                    source_format: "epoch-micros".to_string(),
                    confidence,
                    description: format!("{} ({})", iso, relative),
                    rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                        epoch_millis: value / 1000,
                        iso: iso.clone(),
                        relative,
                    })],
                });
            }
        }

        // Check if valid epoch nanoseconds
        if (MIN_EPOCH_NANOS..=MAX_EPOCH_NANOS).contains(&value) {
            let secs = value / 1_000_000_000;
            let nanos = (value % 1_000_000_000) as u32;
            if let Some(dt) = Utc.timestamp_opt(secs, nanos).single() {
                let base_confidence = Self::calculate_confidence(dt);
                let confidence = (base_confidence - 0.15).max(0.60);
                let iso = dt.to_rfc3339();
                let relative = Self::format_relative(dt);

                debug!(
                    epoch = value,
                    confidence, iso, "epoch: matched as nanoseconds"
                );

                results.push(Interpretation {
                    value: CoreValue::DateTime(dt),
                    source_format: "epoch-nanos".to_string(),
                    confidence,
                    description: format!("{} ({})", iso, relative),
                    rich_display: vec![RichDisplayOption::new(RichDisplay::DateTime {
                        epoch_millis: value / 1_000_000,
                        iso: iso.clone(),
                        relative,
                    })],
                });
            }
        }

        results
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // Formatting handled by datetime.rs conversions
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["unix", "timestamp"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_epoch_seconds() {
        let format = EpochFormat;
        // A timestamp that's "now" (approximately)
        let now = Utc::now().timestamp();
        let results = format.parse(&now.to_string());

        assert!(!results.is_empty());
        let epoch_secs = results
            .iter()
            .find(|i| i.source_format == "epoch-seconds")
            .expect("Should have epoch-seconds interpretation");
        assert!(
            epoch_secs.confidence >= 0.90,
            "Recent timestamp should have high confidence"
        );
    }

    #[test]
    fn test_parse_epoch_millis() {
        let format = EpochFormat;
        // A milliseconds timestamp that's "now"
        let now_millis = Utc::now().timestamp_millis();
        let results = format.parse(&now_millis.to_string());

        let epoch_millis = results
            .iter()
            .find(|i| i.source_format == "epoch-millis")
            .expect("Should have epoch-millis interpretation");
        assert!(epoch_millis.confidence >= 0.70);
    }

    #[test]
    fn test_old_timestamp_lower_confidence() {
        let format = EpochFormat;
        // Jan 1, 2000 - 25 years ago
        let results = format.parse("946684800");

        assert!(!results.is_empty());
        let epoch_secs = results
            .iter()
            .find(|i| i.source_format == "epoch-seconds")
            .unwrap();
        // Should be within 30 years, so 0.87
        assert!(epoch_secs.confidence >= 0.85);
        assert!(epoch_secs.confidence < 0.90);
    }

    #[test]
    fn test_invalid_input() {
        let format = EpochFormat;
        assert!(format.parse("not-a-number").is_empty());
        assert!(format.parse("").is_empty());
    }

    #[test]
    fn test_out_of_range() {
        let format = EpochFormat;
        // Before 2000
        assert!(format.parse("0").is_empty());
        // After 2100
        assert!(format.parse("5000000000").is_empty());
    }
}
