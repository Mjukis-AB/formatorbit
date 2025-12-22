//! Color format (hex RGB/RGBA/ARGB).

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

/// Represents a parsed color with RGBA components.
#[derive(Debug, Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: Option<u8>,
}

pub struct ColorFormat;

impl ColorFormat {
    /// Parse a hex color string like #RGB, #RRGGBB, #RRGGBBAA, or #AARRGGBB.
    fn parse_hex_color(s: &str) -> Option<(Rgba, &'static str)> {
        let hex = s.strip_prefix('#').unwrap_or(s);

        match hex.len() {
            // #RGB -> expand to #RRGGBB
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some((Rgba { r, g, b, a: None }, "RGB"))
            }
            // #RRGGBB
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some((Rgba { r, g, b, a: None }, "RGB"))
            }
            // #RRGGBBAA or #AARRGGBB - we'll parse as RGBA but note it could be ARGB
            8 => {
                let b0 = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let b1 = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b2 = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let b3 = u8::from_str_radix(&hex[6..8], 16).ok()?;
                // Return as RGBA, conversions will show ARGB alternative
                Some((
                    Rgba {
                        r: b0,
                        g: b1,
                        b: b2,
                        a: Some(b3),
                    },
                    "RGBA",
                ))
            }
            _ => None,
        }
    }

    /// Parse 0xRRGGBB or 0xAARRGGBB format (common in Android/code).
    fn parse_0x_color(s: &str) -> Option<(Rgba, &'static str)> {
        let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;

        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some((Rgba { r, g, b, a: None }, "0xRRGGBB"))
            }
            8 => {
                // 0xAARRGGBB format (Android style)
                let a = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some((
                    Rgba {
                        r,
                        g,
                        b,
                        a: Some(a),
                    },
                    "0xAARRGGBB",
                ))
            }
            _ => None,
        }
    }

    fn make_interpretation(rgba: Rgba, format_hint: &str, high_confidence: bool) -> Interpretation {
        let Rgba { r, g, b, a } = rgba;
        let bytes = if let Some(alpha) = a {
            vec![r, g, b, alpha]
        } else {
            vec![r, g, b]
        };

        let (h, s, l) = Self::rgb_to_hsl(r, g, b);

        let description = if let Some(alpha) = a {
            format!("{format_hint}: RGBA({r}, {g}, {b}, {alpha}) / HSL({h}°, {s}%, {l}%)")
        } else {
            format!("{format_hint}: RGB({r}, {g}, {b}) / HSL({h}°, {s}%, {l}%)")
        };

        Interpretation {
            value: CoreValue::Bytes(bytes),
            source_format: "color-hex".to_string(),
            confidence: if high_confidence { 0.95 } else { 0.6 },
            description,
        }
    }

    /// Convert RGB to HSL.
    fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
        let r = r as f64 / 255.0;
        let g = g as f64 / 255.0;
        let b = b as f64 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if (max - min).abs() < f64::EPSILON {
            return (0, 0, (l * 100.0) as u8);
        }

        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        let h = if (max - r).abs() < f64::EPSILON {
            (g - b) / d + (if g < b { 6.0 } else { 0.0 })
        } else if (max - g).abs() < f64::EPSILON {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };

        ((h * 60.0) as u16, (s * 100.0) as u8, (l * 100.0) as u8)
    }
}

impl Format for ColorFormat {
    fn id(&self) -> &'static str {
        "color"
    }

    fn name(&self) -> &'static str {
        "Color"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Colors",
            description: "Color parsing (hex, RGB, ARGB) with HSL conversion",
            examples: &["#FF5733", "#F00", "#FF573380", "0x80FF5733"],
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try #RGB / #RRGGBB / #RRGGBBAA
        if let Some((rgba, format_hint)) = Self::parse_hex_color(input) {
            return vec![Self::make_interpretation(
                rgba,
                format_hint,
                input.starts_with('#'),
            )];
        }

        // Try 0xRRGGBB / 0xAARRGGBB (Android style)
        if let Some((rgba, format_hint)) = Self::parse_0x_color(input) {
            return vec![Self::make_interpretation(rgba, format_hint, true)];
        }

        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Don't format arbitrary bytes as color - too noisy for 3-4 byte values
        // Color output is only meaningful when input was parsed as a color
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    // Note: No conversions() either - color info is shown in parse description.
    // We don't want arbitrary 3-4 byte values (like IPs or small hex) to show color conversions.
    // Color conversions are only meaningful when the input was actually parsed as a color.

    fn aliases(&self) -> &'static [&'static str] {
        &["col", "rgb", "argb"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color_with_hash() {
        let format = ColorFormat;
        let results = format.parse("#FF5733");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "color-hex");
        assert!(results[0].confidence > 0.9);

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[255, 87, 51]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_short_hex() {
        let format = ColorFormat;
        let results = format.parse("#F00");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[255, 0, 0]); // #F00 expands to #FF0000
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_rgba() {
        let format = ColorFormat;
        let results = format.parse("#FF573380");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[255, 87, 51, 128]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_rgb_to_hsl() {
        // Red
        let (h, s, l) = ColorFormat::rgb_to_hsl(255, 0, 0);
        assert_eq!(h, 0);
        assert_eq!(s, 100);
        assert_eq!(l, 50);

        // White
        let (_h, s, l) = ColorFormat::rgb_to_hsl(255, 255, 255);
        assert_eq!(s, 0);
        assert_eq!(l, 100);
    }

    // Note: format() and conversions() tests removed because those methods
    // are now disabled to avoid noise from arbitrary bytes→color conversions.

    #[test]
    fn test_parse_android_argb() {
        let format = ColorFormat;
        // Android style: 0xAARRGGBB (80 = 50% alpha, FF5733 = orange)
        let results = format.parse("0x80FF5733");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("0xAARRGGBB"));

        if let CoreValue::Bytes(bytes) = &results[0].value {
            // Parsed as ARGB, stored as RGBA
            assert_eq!(bytes, &[255, 87, 51, 128]); // R, G, B, A
        } else {
            panic!("Expected Bytes");
        }
    }

}
