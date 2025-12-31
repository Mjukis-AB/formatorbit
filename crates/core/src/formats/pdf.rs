//! PDF document format detection and metadata extraction.
//!
//! Detects PDF files and extracts:
//! - Basic properties: page count, PDF version
//! - Document info: title, author, subject, creator
//! - Dates: creation date, modification date
//! - Security: encryption status

use std::io::Cursor;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct PdfFormat;

/// PDF metadata extracted from file.
#[derive(Debug, Clone, Default)]
struct PdfMetadata {
    // Basic properties
    page_count: usize,
    version: String,

    // Document info
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    creator: Option<String>,
    producer: Option<String>,
    keywords: Option<String>,

    // Dates
    creation_date: Option<String>,
    mod_date: Option<String>,

    // Security
    is_encrypted: bool,
}

impl PdfFormat {
    /// Check if data starts with PDF magic bytes.
    fn is_pdf(data: &[u8]) -> bool {
        data.len() >= 5 && data.starts_with(b"%PDF-")
    }

    /// Extract PDF version from header.
    fn extract_version(data: &[u8]) -> Option<String> {
        // PDF header format: %PDF-X.Y
        if data.len() < 8 {
            return None;
        }

        // Find the version number after %PDF-
        let header = std::str::from_utf8(&data[5..8.min(data.len())]).ok()?;
        let version = header.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
        if version.is_empty() {
            None
        } else {
            Some(version.to_string())
        }
    }

    /// Parse PDF and extract metadata using lopdf.
    fn parse_pdf(data: &[u8]) -> Option<PdfMetadata> {
        if !Self::is_pdf(data) {
            return None;
        }

        let mut meta = PdfMetadata {
            version: Self::extract_version(data).unwrap_or_else(|| "unknown".to_string()),
            ..Default::default()
        };

        // Try to parse with lopdf
        let cursor = Cursor::new(data);
        let doc = match lopdf::Document::load_from(cursor) {
            Ok(doc) => doc,
            Err(_) => {
                // Even if parsing fails, we detected PDF magic
                return Some(meta);
            }
        };

        // Get page count
        meta.page_count = doc.get_pages().len();

        // Check encryption
        meta.is_encrypted = doc.is_encrypted();

        // Extract document info dictionary
        if let Ok(info_ref) = doc.trailer.get(b"Info") {
            if let Ok(info_ref) = info_ref.as_reference() {
                if let Ok(info) = doc.get_object(info_ref) {
                    if let Ok(dict) = info.as_dict() {
                        meta.title = Self::get_string_from_dict(&doc, dict, b"Title");
                        meta.author = Self::get_string_from_dict(&doc, dict, b"Author");
                        meta.subject = Self::get_string_from_dict(&doc, dict, b"Subject");
                        meta.creator = Self::get_string_from_dict(&doc, dict, b"Creator");
                        meta.producer = Self::get_string_from_dict(&doc, dict, b"Producer");
                        meta.keywords = Self::get_string_from_dict(&doc, dict, b"Keywords");
                        meta.creation_date = Self::get_date_from_dict(&doc, dict, b"CreationDate");
                        meta.mod_date = Self::get_date_from_dict(&doc, dict, b"ModDate");
                    }
                }
            }
        }

        Some(meta)
    }

    /// Get a string value from PDF dictionary.
    fn get_string_from_dict(
        doc: &lopdf::Document,
        dict: &lopdf::Dictionary,
        key: &[u8],
    ) -> Option<String> {
        let obj = dict.get(key).ok()?;
        Self::object_to_string(doc, obj)
    }

    /// Convert PDF object to string.
    fn object_to_string(doc: &lopdf::Document, obj: &lopdf::Object) -> Option<String> {
        match obj {
            lopdf::Object::String(bytes, _) => {
                // Try UTF-16BE first (PDF standard for Unicode)
                if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                    // UTF-16BE BOM
                    let chars: Vec<u16> = bytes[2..]
                        .chunks(2)
                        .filter_map(|c| {
                            if c.len() == 2 {
                                Some(u16::from_be_bytes([c[0], c[1]]))
                            } else {
                                None
                            }
                        })
                        .collect();
                    String::from_utf16(&chars).ok()
                } else {
                    // Try as Latin-1/PDFDocEncoding
                    Some(bytes.iter().map(|&b| b as char).collect())
                }
            }
            lopdf::Object::Name(name) => String::from_utf8(name.clone()).ok(),
            lopdf::Object::Reference(r) => {
                let obj = doc.get_object(*r).ok()?;
                Self::object_to_string(doc, obj)
            }
            _ => None,
        }
    }

    /// Get a date value from PDF dictionary and format it.
    fn get_date_from_dict(
        doc: &lopdf::Document,
        dict: &lopdf::Dictionary,
        key: &[u8],
    ) -> Option<String> {
        let date_str = Self::get_string_from_dict(doc, dict, key)?;
        Self::parse_pdf_date(&date_str)
    }

    /// Parse PDF date format (D:YYYYMMDDHHmmSSOHH'mm').
    fn parse_pdf_date(s: &str) -> Option<String> {
        let s = s.trim_start_matches("D:");
        if s.len() < 4 {
            return None;
        }

        let year = s.get(0..4)?;
        let month = s.get(4..6).unwrap_or("01");
        let day = s.get(6..8).unwrap_or("01");
        let hour = s.get(8..10).unwrap_or("00");
        let min = s.get(10..12).unwrap_or("00");
        let sec = s.get(12..14).unwrap_or("00");

        Some(format!(
            "{}-{}-{} {}:{}:{}",
            year, month, day, hour, min, sec
        ))
    }

    /// Format metadata into human-readable description.
    fn format_description(meta: &PdfMetadata) -> String {
        let mut parts = vec![format!("PDF {} document", meta.version)];

        parts.push(format!(
            "{} page{}",
            meta.page_count,
            if meta.page_count == 1 { "" } else { "s" }
        ));

        if let Some(ref title) = meta.title {
            if !title.is_empty() {
                parts.push(format!("\"{}\"", title));
            }
        }

        if let Some(ref author) = meta.author {
            if !author.is_empty() {
                parts.push(format!("by {}", author));
            }
        }

        if meta.is_encrypted {
            parts.push("(encrypted)".to_string());
        }

        parts.join(", ")
    }

    /// Build RichDisplay options for UI rendering.
    fn build_rich_display(meta: &PdfMetadata) -> Vec<RichDisplayOption> {
        let mut displays = vec![];
        let mut pairs = vec![];

        // Basic info
        pairs.push(("Format".to_string(), format!("PDF {}", meta.version)));
        pairs.push((
            "Pages".to_string(),
            format!(
                "{} page{}",
                meta.page_count,
                if meta.page_count == 1 { "" } else { "s" }
            ),
        ));

        // Document info
        if let Some(ref title) = meta.title {
            if !title.is_empty() {
                pairs.push(("Title".to_string(), title.clone()));
            }
        }
        if let Some(ref author) = meta.author {
            if !author.is_empty() {
                pairs.push(("Author".to_string(), author.clone()));
            }
        }
        if let Some(ref subject) = meta.subject {
            if !subject.is_empty() {
                pairs.push(("Subject".to_string(), subject.clone()));
            }
        }
        if let Some(ref creator) = meta.creator {
            if !creator.is_empty() {
                pairs.push(("Creator".to_string(), creator.clone()));
            }
        }
        if let Some(ref producer) = meta.producer {
            if !producer.is_empty() {
                pairs.push(("Producer".to_string(), producer.clone()));
            }
        }
        if let Some(ref keywords) = meta.keywords {
            if !keywords.is_empty() {
                pairs.push(("Keywords".to_string(), keywords.clone()));
            }
        }

        // Dates
        if let Some(ref date) = meta.creation_date {
            pairs.push(("Created".to_string(), date.clone()));
        }
        if let Some(ref date) = meta.mod_date {
            pairs.push(("Modified".to_string(), date.clone()));
        }

        // Security
        if meta.is_encrypted {
            pairs.push(("Security".to_string(), "Encrypted".to_string()));
        }

        displays.push(RichDisplayOption::new(RichDisplay::KeyValue { pairs }));

        displays
    }
}

impl Format for PdfFormat {
    fn id(&self) -> &'static str {
        "pdf"
    }

    fn name(&self) -> &'static str {
        "PDF Document"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Documents",
            description: "PDF document with metadata extraction",
            examples: &["[binary PDF data]"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_pdf(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "pdf".to_string(),
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

        let Some(meta) = Self::parse_pdf(bytes) else {
            return vec![];
        };

        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        vec![Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "pdf-info".to_string(),
            display: description,
            path: vec!["pdf-info".to_string()],
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
        &["pdf"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pdf() {
        assert!(PdfFormat::is_pdf(b"%PDF-1.4"));
        assert!(PdfFormat::is_pdf(b"%PDF-2.0 extra stuff"));
        assert!(!PdfFormat::is_pdf(b"not a pdf"));
        assert!(!PdfFormat::is_pdf(b"%PDF")); // Too short
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            PdfFormat::extract_version(b"%PDF-1.4"),
            Some("1.4".to_string())
        );
        assert_eq!(
            PdfFormat::extract_version(b"%PDF-2.0"),
            Some("2.0".to_string())
        );
        assert_eq!(
            PdfFormat::extract_version(b"%PDF-1.7 extra"),
            Some("1.7".to_string())
        );
    }

    #[test]
    fn test_parse_pdf_date() {
        assert_eq!(
            PdfFormat::parse_pdf_date("D:20231225120000"),
            Some("2023-12-25 12:00:00".to_string())
        );
        assert_eq!(
            PdfFormat::parse_pdf_date("D:2023"),
            Some("2023-01-01 00:00:00".to_string())
        );
    }
}
