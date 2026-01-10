//! Font file format detection and metadata extraction.
//!
//! Detects font files and extracts:
//! - Font family name and subfamily (style)
//! - Version string
//! - Glyph count
//! - Font type (TrueType, OpenType, WOFF, WOFF2)

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct FontFormat;

/// Font metadata extracted from file.
#[derive(Debug, Clone, Default)]
struct FontMetadata {
    font_type: String,
    family_name: Option<String>,
    subfamily: Option<String>,
    full_name: Option<String>,
    version: Option<String>,
    glyph_count: usize,
    units_per_em: u16,
    is_variable: bool,
}

impl FontFormat {
    /// Check if data is a TrueType font (TTF).
    fn is_ttf(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(&[0x00, 0x01, 0x00, 0x00])
    }

    /// Check if data is an OpenType font (OTF).
    fn is_otf(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(b"OTTO")
    }

    /// Check if data is a WOFF font.
    fn is_woff(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(b"wOFF")
    }

    /// Check if data is a WOFF2 font.
    fn is_woff2(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(b"wOF2")
    }

    /// Detect font type from magic bytes.
    fn detect_font_type(data: &[u8]) -> Option<&'static str> {
        if Self::is_ttf(data) {
            Some("TrueType")
        } else if Self::is_otf(data) {
            Some("OpenType")
        } else if Self::is_woff(data) {
            Some("WOFF")
        } else if Self::is_woff2(data) {
            Some("WOFF2")
        } else {
            None
        }
    }

    /// Parse font and extract metadata using ttf-parser.
    fn parse_font(data: &[u8]) -> Option<FontMetadata> {
        let font_type = Self::detect_font_type(data)?;

        // WOFF/WOFF2 are compressed and need special handling
        // ttf-parser doesn't support WOFF directly, so we only extract basic info
        if font_type == "WOFF" || font_type == "WOFF2" {
            return Some(FontMetadata {
                font_type: font_type.to_string(),
                ..Default::default()
            });
        }

        // Parse TTF/OTF with ttf-parser
        let face = ttf_parser::Face::parse(data, 0).ok()?;

        let mut meta = FontMetadata {
            font_type: font_type.to_string(),
            glyph_count: face.number_of_glyphs() as usize,
            units_per_em: face.units_per_em(),
            is_variable: face.is_variable(),
            ..Default::default()
        };

        // Extract name table entries
        for name in face.names() {
            // Prefer English (language ID 0x0409/1033 for Windows, 0 for Mac)
            let is_english = name.language_id == 0x0409 || name.language_id == 0;

            if !is_english && meta.family_name.is_some() {
                continue;
            }

            if let Some(name_str) = name.to_string() {
                match name.name_id {
                    ttf_parser::name_id::FAMILY => {
                        meta.family_name = Some(name_str);
                    }
                    ttf_parser::name_id::SUBFAMILY => {
                        meta.subfamily = Some(name_str);
                    }
                    ttf_parser::name_id::FULL_NAME => {
                        meta.full_name = Some(name_str);
                    }
                    // Version string is name_id 5
                    5 => {
                        meta.version = Some(name_str);
                    }
                    _ => {}
                }
            }
        }

        Some(meta)
    }

    /// Format metadata into human-readable description.
    fn format_description(meta: &FontMetadata) -> String {
        let mut parts = vec![meta.font_type.clone()];

        if let Some(ref name) = meta.full_name {
            parts.push(format!("\"{}\"", name));
        } else if let Some(ref family) = meta.family_name {
            if let Some(ref style) = meta.subfamily {
                parts.push(format!("\"{}\" {}", family, style));
            } else {
                parts.push(format!("\"{}\"", family));
            }
        }

        if meta.glyph_count > 0 {
            parts.push(format!("{} glyphs", meta.glyph_count));
        }

        if meta.is_variable {
            parts.push("variable".to_string());
        }

        parts.join(", ")
    }

    /// Build RichDisplay options for UI rendering.
    fn build_rich_display(meta: &FontMetadata) -> Vec<RichDisplayOption> {
        let mut pairs = vec![];

        pairs.push(("Format".to_string(), meta.font_type.clone()));

        if let Some(ref name) = meta.family_name {
            pairs.push(("Family".to_string(), name.clone()));
        }

        if let Some(ref style) = meta.subfamily {
            pairs.push(("Style".to_string(), style.clone()));
        }

        if let Some(ref full) = meta.full_name {
            pairs.push(("Full Name".to_string(), full.clone()));
        }

        if let Some(ref version) = meta.version {
            pairs.push(("Version".to_string(), version.clone()));
        }

        if meta.glyph_count > 0 {
            pairs.push(("Glyphs".to_string(), meta.glyph_count.to_string()));
        }

        if meta.units_per_em > 0 {
            pairs.push(("Units/Em".to_string(), meta.units_per_em.to_string()));
        }

        if meta.is_variable {
            pairs.push(("Variable".to_string(), "Yes".to_string()));
        }

        vec![RichDisplayOption::new(RichDisplay::KeyValue { pairs })]
    }
}

impl Format for FontFormat {
    fn id(&self) -> &'static str {
        "font"
    }

    fn name(&self) -> &'static str {
        "Font File"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Documents",
            description: "Font file with metadata extraction",
            examples: &["[binary font data]"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_font(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "font".to_string(),
                    confidence: 0.95,
                    description,
                    rich_display,
                }];
            }
        }

        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        let Some(meta) = Self::parse_font(bytes) else {
            return vec![];
        };

        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        vec![Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "font-info".to_string(),
            display: description,
            path: vec!["font-info".to_string()],
            steps: vec![],
            is_lossy: false,
            priority: ConversionPriority::Structured,
            display_only: true,
            kind: ConversionKind::Representation,
            hidden: false,
            rich_display,
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["font", "ttf", "otf", "woff", "woff2"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_font_type() {
        assert_eq!(
            FontFormat::detect_font_type(&[0x00, 0x01, 0x00, 0x00]),
            Some("TrueType")
        );
        assert_eq!(FontFormat::detect_font_type(b"OTTO"), Some("OpenType"));
        assert_eq!(FontFormat::detect_font_type(b"wOFF"), Some("WOFF"));
        assert_eq!(FontFormat::detect_font_type(b"wOF2"), Some("WOFF2"));
        assert_eq!(FontFormat::detect_font_type(b"not a font"), None);
    }

    #[test]
    fn test_is_ttf() {
        assert!(FontFormat::is_ttf(&[0x00, 0x01, 0x00, 0x00, 0xFF]));
        assert!(!FontFormat::is_ttf(b"OTTO"));
    }

    #[test]
    fn test_is_otf() {
        assert!(FontFormat::is_otf(b"OTTO"));
        assert!(!FontFormat::is_otf(&[0x00, 0x01, 0x00, 0x00]));
    }
}
