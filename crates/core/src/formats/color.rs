//! Color format (hex RGB/RGBA/ARGB).

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

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

    fn can_format(&self, value: &CoreValue) -> bool {
        match value {
            CoreValue::Bytes(bytes) => bytes.len() == 3 || bytes.len() == 4,
            _ => false,
        }
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) if bytes.len() == 3 => {
                Some(format!("#{:02X}{:02X}{:02X}", bytes[0], bytes[1], bytes[2]))
            }
            CoreValue::Bytes(bytes) if bytes.len() == 4 => Some(format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                bytes[0], bytes[1], bytes[2], bytes[3]
            )),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        if bytes.len() != 3 && bytes.len() != 4 {
            return vec![];
        }

        let r = bytes[0];
        let g = bytes[1];
        let b = bytes[2];
        let a = bytes.get(3).copied();
        let (h, s, l) = Self::rgb_to_hsl(r, g, b);

        let mut conversions = vec![];

        // CSS rgb()/rgba() format
        let rgb_str = if let Some(alpha) = a {
            format!("rgba({r}, {g}, {b}, {})", alpha)
        } else {
            format!("rgb({r}, {g}, {b})")
        };
        conversions.push(Conversion {
            value: CoreValue::String(rgb_str.clone()),
            target_format: "color-rgb".to_string(),
            display: rgb_str,
            path: vec!["color-rgb".to_string()],
            is_lossy: false,
            priority: ConversionPriority::Semantic,
        });

        // HSL format
        let hsl_str = format!("hsl({h}°, {s}%, {l}%)");
        conversions.push(Conversion {
            value: CoreValue::String(hsl_str.clone()),
            target_format: "color-hsl".to_string(),
            display: hsl_str,
            path: vec!["color-hsl".to_string()],
            is_lossy: false,
            priority: ConversionPriority::Semantic,
        });

        // 0xRRGGBB or 0xAARRGGBB (Android/code style)
        let hex_int = if let Some(alpha) = a {
            format!("0x{alpha:02X}{r:02X}{g:02X}{b:02X}")
        } else {
            format!("0x{r:02X}{g:02X}{b:02X}")
        };
        conversions.push(Conversion {
            value: CoreValue::String(hex_int.clone()),
            target_format: if a.is_some() {
                "color-argb"
            } else {
                "color-0x"
            }
            .to_string(),
            display: hex_int,
            path: vec!["color-argb".to_string()],
            is_lossy: false,
            priority: ConversionPriority::Semantic,
        });

        // If we have alpha, also show the ARGB interpretation
        // (user might have entered #RRGGBBAA but it could be #AARRGGBB)
        if let Some(alpha) = a {
            // Interpret bytes as ARGB instead of RGBA
            let argb_r = g; // bytes[1]
            let argb_g = b; // bytes[2]
            let argb_b = alpha; // bytes[3]
            let argb_a = r; // bytes[0]

            let argb_str = format!("ARGB: rgba({argb_r}, {argb_g}, {argb_b}, {argb_a})");
            conversions.push(Conversion {
                value: CoreValue::String(argb_str.clone()),
                target_format: "color-as-argb".to_string(),
                display: argb_str,
                path: vec!["color-as-argb".to_string()],
                is_lossy: false,
                priority: ConversionPriority::Semantic,
            });
        }

        conversions
    }

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

    #[test]
    fn test_format_to_hex() {
        let format = ColorFormat;
        let value = CoreValue::Bytes(vec![255, 87, 51]);
        assert_eq!(format.format(&value), Some("#FF5733".to_string()));
    }

    #[test]
    fn test_conversions_rgb_hsl() {
        let format = ColorFormat;
        let value = CoreValue::Bytes(vec![255, 0, 0]);
        let conversions = format.conversions(&value);

        assert!(conversions.iter().any(|c| c.target_format == "color-rgb"));
        assert!(conversions.iter().any(|c| c.target_format == "color-hsl"));

        let hsl = conversions
            .iter()
            .find(|c| c.target_format == "color-hsl")
            .unwrap();
        assert!(hsl.display.contains("0°")); // Red is 0 degrees
    }

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

    #[test]
    fn test_conversions_include_argb() {
        let format = ColorFormat;
        let value = CoreValue::Bytes(vec![255, 87, 51, 128]); // RGBA
        let conversions = format.conversions(&value);

        // Should have 0xAARRGGBB format
        let argb = conversions
            .iter()
            .find(|c| c.target_format == "color-argb")
            .unwrap();
        assert_eq!(argb.display, "0x80FF5733");

        // Should also show alternative ARGB interpretation
        assert!(conversions
            .iter()
            .any(|c| c.target_format == "color-as-argb"));
    }
}
