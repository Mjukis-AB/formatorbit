//! Color format (hex RGB/RGBA/ARGB).

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionPriority, ConversionStep, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

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
    /// Parse rgb(r, g, b) or rgba(r, g, b, a) CSS function.
    fn parse_rgb_function(s: &str) -> Option<(Rgba, &'static str)> {
        let trimmed = s.trim();

        // Try rgba() first
        if let Some(inner) = trimmed
            .strip_prefix("rgba(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 4 {
                let r = parts[0].parse::<u8>().ok()?;
                let g = parts[1].parse::<u8>().ok()?;
                let b = parts[2].parse::<u8>().ok()?;
                // Alpha can be 0-1 float or 0-255 int
                let a = Self::parse_alpha(parts[3])?;
                return Some((
                    Rgba {
                        r,
                        g,
                        b,
                        a: Some(a),
                    },
                    "rgba()",
                ));
            }
        }

        // Try rgb()
        if let Some(inner) = trimmed
            .strip_prefix("rgb(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 3 {
                let r = parts[0].parse::<u8>().ok()?;
                let g = parts[1].parse::<u8>().ok()?;
                let b = parts[2].parse::<u8>().ok()?;
                return Some((Rgba { r, g, b, a: None }, "rgb()"));
            }
        }

        None
    }

    /// Parse hsl(h, s%, l%) or hsla(h, s%, l%, a) CSS function.
    fn parse_hsl_function(s: &str) -> Option<(Rgba, &'static str)> {
        let trimmed = s.trim();

        // Try hsla() first
        if let Some(inner) = trimmed
            .strip_prefix("hsla(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 4 {
                let h = Self::parse_hue(parts[0])?;
                let s = Self::parse_percent(parts[1])?;
                let l = Self::parse_percent(parts[2])?;
                let a = Self::parse_alpha(parts[3])?;
                let (r, g, b) = Self::hsl_to_rgb(h, s, l);
                return Some((
                    Rgba {
                        r,
                        g,
                        b,
                        a: Some(a),
                    },
                    "hsla()",
                ));
            }
        }

        // Try hsl()
        if let Some(inner) = trimmed
            .strip_prefix("hsl(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 3 {
                let h = Self::parse_hue(parts[0])?;
                let s = Self::parse_percent(parts[1])?;
                let l = Self::parse_percent(parts[2])?;
                let (r, g, b) = Self::hsl_to_rgb(h, s, l);
                return Some((Rgba { r, g, b, a: None }, "hsl()"));
            }
        }

        None
    }

    /// Parse hue value (0-360, with optional "deg" suffix).
    fn parse_hue(s: &str) -> Option<f64> {
        let s = s.strip_suffix("deg").unwrap_or(s).trim();
        let h: f64 = s.parse().ok()?;
        if (0.0..=360.0).contains(&h) {
            Some(h)
        } else {
            None
        }
    }

    /// Parse percentage value (0-100, with optional "%" suffix).
    fn parse_percent(s: &str) -> Option<f64> {
        let s = s.strip_suffix('%').unwrap_or(s).trim();
        let p: f64 = s.parse().ok()?;
        if (0.0..=100.0).contains(&p) {
            Some(p)
        } else {
            None
        }
    }

    /// Parse alpha value (0-1 float or 0-255 int or percentage).
    fn parse_alpha(s: &str) -> Option<u8> {
        let s = s.trim();
        // Try percentage first
        if let Some(p) = s.strip_suffix('%') {
            let pct: f64 = p.trim().parse().ok()?;
            return Some((pct / 100.0 * 255.0) as u8);
        }
        // Try float (0-1)
        if let Ok(f) = s.parse::<f64>() {
            if (0.0..=1.0).contains(&f) {
                return Some((f * 255.0) as u8);
            }
        }
        // Try int (0-255)
        s.parse::<u8>().ok()
    }

    /// Convert HSL to RGB.
    fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
        let s = s / 100.0;
        let l = l / 100.0;

        if s == 0.0 {
            let v = (l * 255.0) as u8;
            return (v, v, v);
        }

        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let h = h / 360.0;

        let r = Self::hue_to_rgb(p, q, h + 1.0 / 3.0);
        let g = Self::hue_to_rgb(p, q, h);
        let b = Self::hue_to_rgb(p, q, h - 1.0 / 3.0);

        ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
    }

    fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    }

    /// Parse a hex color string like #RGB, #RRGGBB, #RRGGBBAA, or #AARRGGBB.
    /// For 6-char and 8-char hex, the # prefix is REQUIRED to avoid false positives
    /// on strings like "DEADBEEF" or "CAFEBABE".
    fn parse_hex_color(s: &str) -> Option<(Rgba, &'static str)> {
        let has_prefix = s.starts_with('#');
        let hex = s.strip_prefix('#').unwrap_or(s);

        // Hex colors must be ASCII - early exit if not to avoid panics on non-ASCII slicing
        if !hex.is_ascii() {
            return None;
        }

        match hex.len() {
            // #RGB -> expand to #RRGGBB (3-char is common enough to allow without prefix)
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some((Rgba { r, g, b, a: None }, "RGB"))
            }
            // #RRGGBB - require # prefix to avoid matching hex like "DEADBE"
            6 => {
                if !has_prefix {
                    return None;
                }
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some((Rgba { r, g, b, a: None }, "RGB"))
            }
            // #RRGGBBAA or #AARRGGBB - require # prefix to avoid matching "DEADBEEF"
            8 => {
                if !has_prefix {
                    return None;
                }
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

    fn make_interpretation(
        rgba: Rgba,
        format_hint: &str,
        high_confidence: bool,
        source_format: &str,
    ) -> Interpretation {
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
            source_format: source_format.to_string(),
            confidence: if high_confidence { 0.95 } else { 0.6 },
            description,
            rich_display: vec![],
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
            description: "Color parsing (hex, rgb(), hsl()) with conversions",
            examples: &[
                "#FF5733",
                "rgb(255, 87, 51)",
                "hsl(120, 100%, 50%)",
                "rgba(255, 128, 0, 0.5)",
            ],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try #RGB / #RRGGBB / #RRGGBBAA
        if let Some((rgba, format_hint)) = Self::parse_hex_color(input) {
            return vec![Self::make_interpretation(
                rgba,
                format_hint,
                input.starts_with('#'),
                "color-hex",
            )];
        }

        // Try 0xRRGGBB / 0xAARRGGBB (Android style)
        if let Some((rgba, format_hint)) = Self::parse_0x_color(input) {
            return vec![Self::make_interpretation(
                rgba,
                format_hint,
                true,
                "color-hex",
            )];
        }

        // Try rgb() / rgba()
        if let Some((rgba, format_hint)) = Self::parse_rgb_function(input) {
            return vec![Self::make_interpretation(
                rgba,
                format_hint,
                true,
                "color-rgb",
            )];
        }

        // Try hsl() / hsla()
        if let Some((rgba, format_hint)) = Self::parse_hsl_function(input) {
            return vec![Self::make_interpretation(
                rgba,
                format_hint,
                true,
                "color-hsl",
            )];
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

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        // Only convert 3 or 4 byte values (RGB or RGBA)
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

        let mut conversions = Vec::new();

        // Color rich display for all conversions
        let color_display = vec![RichDisplayOption::new(RichDisplay::Color {
            r,
            g,
            b,
            a: a.unwrap_or(255),
        })];

        // Hex format
        let hex_display = if let Some(alpha) = a {
            format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, alpha)
        } else {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        };
        conversions.push(Conversion {
            value: CoreValue::String(hex_display.clone()),
            target_format: "color-hex".to_string(),
            display: hex_display.clone(),
            path: vec!["color-hex".to_string()],
            steps: vec![ConversionStep {
                format: "color-hex".to_string(),
                value: CoreValue::String(hex_display.clone()),
                display: hex_display,
            }],
            priority: ConversionPriority::Semantic,
            display_only: true,
            rich_display: color_display.clone(),
            ..Default::default()
        });

        // rgb()/rgba() format
        let rgb_display = if let Some(alpha) = a {
            let alpha_f = alpha as f64 / 255.0;
            format!("rgba({}, {}, {}, {:.2})", r, g, b, alpha_f)
        } else {
            format!("rgb({}, {}, {})", r, g, b)
        };
        conversions.push(Conversion {
            value: CoreValue::String(rgb_display.clone()),
            target_format: "color-rgb".to_string(),
            display: rgb_display.clone(),
            path: vec!["color-rgb".to_string()],
            steps: vec![ConversionStep {
                format: "color-rgb".to_string(),
                value: CoreValue::String(rgb_display.clone()),
                display: rgb_display,
            }],
            priority: ConversionPriority::Semantic,
            display_only: true,
            rich_display: color_display.clone(),
            ..Default::default()
        });

        // hsl()/hsla() format
        let (h, s, l) = Self::rgb_to_hsl(r, g, b);
        let hsl_display = if let Some(alpha) = a {
            let alpha_f = alpha as f64 / 255.0;
            format!("hsla({}, {}%, {}%, {:.2})", h, s, l, alpha_f)
        } else {
            format!("hsl({}, {}%, {}%)", h, s, l)
        };
        conversions.push(Conversion {
            value: CoreValue::String(hsl_display.clone()),
            target_format: "color-hsl".to_string(),
            display: hsl_display.clone(),
            path: vec!["color-hsl".to_string()],
            steps: vec![ConversionStep {
                format: "color-hsl".to_string(),
                value: CoreValue::String(hsl_display.clone()),
                display: hsl_display,
            }],
            priority: ConversionPriority::Semantic,
            display_only: true,
            rich_display: color_display,
            ..Default::default()
        });

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["col", "rgb", "argb", "hsl"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        let trimmed = input.trim();

        // Check for various color formats and give specific errors
        if let Some(hex) = trimmed.strip_prefix('#') {
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some("invalid hex color: contains non-hex characters".to_string());
            }
            if hex.len() != 3 && hex.len() != 6 && hex.len() != 8 {
                return Some(format!(
                    "invalid hex color length: expected 3, 6, or 8 digits, got {}",
                    hex.len()
                ));
            }
            return None;
        }

        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some("invalid hex color: contains non-hex characters".to_string());
            }
            if hex.len() != 6 && hex.len() != 8 {
                return Some(format!(
                    "invalid 0x color length: expected 6 or 8 digits, got {}",
                    hex.len()
                ));
            }
            return None;
        }

        if trimmed.starts_with("rgb(") || trimmed.starts_with("rgba(") {
            if !trimmed.ends_with(')') {
                return Some("missing closing parenthesis in rgb()/rgba()".to_string());
            }
            return Some("invalid rgb()/rgba() format: expected rgb(r, g, b) or rgba(r, g, b, a) with values 0-255".to_string());
        }

        if trimmed.starts_with("hsl(") || trimmed.starts_with("hsla(") {
            if !trimmed.ends_with(')') {
                return Some("missing closing parenthesis in hsl()/hsla()".to_string());
            }
            return Some(
                "invalid hsl()/hsla() format: expected hsl(h, s%, l%) or hsla(h, s%, l%, a)"
                    .to_string(),
            );
        }

        Some("invalid color format: expected #RGB, #RRGGBB, rgb(), hsl(), or 0xRRGGBB".to_string())
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
        assert!(results[0].description.contains("RGB(255, 87, 51)"));

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
    fn test_parse_rgb_function() {
        let format = ColorFormat;
        let results = format.parse("rgb(35, 50, 35)");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("rgb()"));

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[35, 50, 35]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_rgba_function() {
        let format = ColorFormat;
        let results = format.parse("rgba(255, 128, 0, 0.5)");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[255, 128, 0, 127]); // 0.5 * 255 ≈ 127
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_hsl_function() {
        let format = ColorFormat;
        let results = format.parse("hsl(120, 100%, 50%)");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            // Pure green: hsl(120, 100%, 50%) = rgb(0, 255, 0)
            assert_eq!(bytes, &[0, 255, 0]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_hsla_function() {
        let format = ColorFormat;
        let results = format.parse("hsla(0, 100%, 50%, 0.5)");

        assert_eq!(results.len(), 1);
        if let CoreValue::Bytes(bytes) = &results[0].value {
            // Pure red with 50% alpha
            assert_eq!(bytes, &[255, 0, 0, 127]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_conversions_show_hex() {
        let format = ColorFormat;
        let value = CoreValue::Bytes(vec![35, 50, 35]);
        let conversions = format.conversions(&value);

        let hex = conversions
            .iter()
            .find(|c| c.target_format == "color-hex")
            .unwrap();
        assert_eq!(hex.display, "#233223");

        let rgb = conversions
            .iter()
            .find(|c| c.target_format == "color-rgb")
            .unwrap();
        assert_eq!(rgb.display, "rgb(35, 50, 35)");
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
    fn test_hex_without_prefix_not_color() {
        // 6-char and 8-char hex without # prefix should NOT be parsed as color
        // This avoids false positives like "DEADBEEF" being a color
        let format = ColorFormat;

        // 8-char hex like DEADBEEF - not a color without #
        assert!(format.parse("DEADBEEF").is_empty());
        assert!(format.parse("CAFEBABE").is_empty());

        // 6-char hex like FF5733 - not a color without #
        assert!(format.parse("FF5733").is_empty());

        // But WITH # prefix, they ARE colors
        assert!(!format.parse("#DEADBEEF").is_empty());
        assert!(!format.parse("#FF5733").is_empty());
    }
}
