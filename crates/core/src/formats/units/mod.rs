//! Unit conversion formats.
//!
//! Provides parsing and conversion for physical units:
//! length, weight, volume, speed, pressure, angle, area, energy.

pub mod angle;
pub mod area;
pub mod energy;
pub mod length;
pub mod pressure;
pub mod speed;
pub mod volume;
pub mod weight;

pub use angle::AngleFormat;
pub use area::AreaFormat;
pub use energy::EnergyFormat;
pub use length::LengthFormat;
pub use pressure::PressureFormat;
pub use speed::SpeedFormat;
pub use volume::VolumeFormat;
pub use weight::WeightFormat;

/// Parse a number with decimal separator heuristics.
///
/// - Default: `.` is decimal
/// - `,` is decimal if followed by 1-2 digits (not 3)
/// - When both present, the last separator is decimal
pub fn parse_number(s: &str) -> Option<f64> {
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

/// Format a value with appropriate precision.
/// Shows integers without decimals, others with up to 2 decimal places.
pub fn format_value(value: f64) -> String {
    if (value - value.round()).abs() < 0.005 {
        format!("{}", value.round() as i64)
    } else {
        format!("{:.2}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_number_simple() {
        assert_eq!(parse_number("42"), Some(42.0));
        assert_eq!(parse_number("3.25"), Some(3.25));
        assert_eq!(parse_number("-10"), Some(-10.0));
    }

    #[test]
    fn test_parse_number_comma_decimal() {
        assert_eq!(parse_number("3,25"), Some(3.25));
        assert_eq!(parse_number("4,5"), Some(4.5));
    }

    #[test]
    fn test_parse_number_comma_thousands() {
        assert_eq!(parse_number("1,000"), Some(1000.0));
        assert_eq!(parse_number("1,234,567"), Some(1234567.0));
    }

    #[test]
    fn test_parse_number_mixed() {
        assert_eq!(parse_number("1,000.5"), Some(1000.5));
        assert_eq!(parse_number("1.000,5"), Some(1000.5));
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value(100.0), "100");
        assert_eq!(format_value(3.25159), "3.25");
        assert_eq!(format_value(0.5), "0.50");
    }
}
