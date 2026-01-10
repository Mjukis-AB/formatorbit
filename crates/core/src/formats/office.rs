//! Microsoft Office document format detection and metadata extraction.
//!
//! Detects Office Open XML files (.docx, .xlsx, .pptx) and extracts:
//! - Document type (Word, Excel, PowerPoint)
//! - Title, author, subject
//! - Creation and modification dates
//! - Page/sheet/slide count

use std::io::{Cursor, Read};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct OfficeFormat;

/// Office document type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfficeType {
    Word,
    Excel,
    PowerPoint,
}

impl OfficeType {
    fn name(&self) -> &'static str {
        match self {
            Self::Word => "Word",
            Self::Excel => "Excel",
            Self::PowerPoint => "PowerPoint",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Self::Word => "docx",
            Self::Excel => "xlsx",
            Self::PowerPoint => "pptx",
        }
    }

    fn count_label(&self) -> &'static str {
        match self {
            Self::Word => "pages",
            Self::Excel => "sheets",
            Self::PowerPoint => "slides",
        }
    }
}

/// Office document metadata extracted from file.
#[derive(Debug, Clone, Default)]
struct OfficeMetadata {
    doc_type: Option<OfficeType>,
    title: Option<String>,
    creator: Option<String>,
    subject: Option<String>,
    description: Option<String>,
    keywords: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    item_count: Option<usize>,
}

impl OfficeFormat {
    /// Check if data is a ZIP file (Office docs are ZIP-based).
    fn is_zip(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(&[0x50, 0x4B, 0x03, 0x04])
    }

    /// Detect Office document type by checking for specific files inside.
    fn detect_office_type(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<OfficeType> {
        // Check for Word
        if archive.by_name("word/document.xml").is_ok() {
            return Some(OfficeType::Word);
        }

        // Check for Excel
        if archive.by_name("xl/workbook.xml").is_ok() {
            return Some(OfficeType::Excel);
        }

        // Check for PowerPoint
        if archive.by_name("ppt/presentation.xml").is_ok() {
            return Some(OfficeType::PowerPoint);
        }

        None
    }

    /// Parse XML to extract text content of an element.
    fn extract_xml_element(xml: &str, tag: &str) -> Option<String> {
        // Simple XML parsing - look for <tag>content</tag>
        // Handle both with and without namespace prefix
        let patterns = [
            format!("<{}>", tag),
            format!("<dc:{}>", tag),
            format!("<cp:{}>", tag),
            format!("<dcterms:{}>", tag),
        ];

        for pattern in &patterns {
            if let Some(start) = xml.find(pattern) {
                let content_start = start + pattern.len();
                let end_tag = format!("</{}", &pattern[1..]);
                if let Some(end) = xml[content_start..].find(&end_tag) {
                    let content = &xml[content_start..content_start + end];
                    if !content.is_empty() {
                        return Some(content.to_string());
                    }
                }
            }
        }

        None
    }

    /// Parse core.xml to extract document properties.
    fn parse_core_xml(xml: &str) -> OfficeMetadata {
        OfficeMetadata {
            title: Self::extract_xml_element(xml, "title"),
            creator: Self::extract_xml_element(xml, "creator"),
            subject: Self::extract_xml_element(xml, "subject"),
            description: Self::extract_xml_element(xml, "description"),
            keywords: Self::extract_xml_element(xml, "keywords"),
            created: Self::extract_xml_element(xml, "created"),
            modified: Self::extract_xml_element(xml, "modified"),
            ..Default::default()
        }
    }

    /// Count sheets in Excel workbook.
    fn count_excel_sheets(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<usize> {
        let mut file = archive.by_name("xl/workbook.xml").ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;

        // Count <sheet> elements
        Some(contents.matches("<sheet ").count())
    }

    /// Count slides in PowerPoint presentation.
    fn count_ppt_slides(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<usize> {
        // Count slideN.xml files in ppt/slides/
        let mut count = 0;
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let name = file.name();
                if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                    count += 1;
                }
            }
        }
        if count > 0 {
            Some(count)
        } else {
            None
        }
    }

    /// Count pages in Word document (from app.xml).
    fn count_word_pages(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<usize> {
        let mut file = archive.by_name("docProps/app.xml").ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;

        // Look for <Pages>N</Pages>
        Self::extract_xml_element(&contents, "Pages").and_then(|s| s.parse().ok())
    }

    /// Parse Office document and extract metadata.
    fn parse_office(data: &[u8]) -> Option<OfficeMetadata> {
        if !Self::is_zip(data) {
            return None;
        }

        let cursor = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).ok()?;

        // Detect document type
        let doc_type = Self::detect_office_type(&mut archive)?;

        // Parse core.xml for metadata
        let mut meta = if let Ok(mut file) = archive.by_name("docProps/core.xml") {
            let mut contents = String::new();
            file.read_to_string(&mut contents).ok()?;
            Self::parse_core_xml(&contents)
        } else {
            OfficeMetadata::default()
        };

        meta.doc_type = Some(doc_type);

        // Get item count based on document type
        meta.item_count = match doc_type {
            OfficeType::Word => Self::count_word_pages(&mut archive),
            OfficeType::Excel => Self::count_excel_sheets(&mut archive),
            OfficeType::PowerPoint => Self::count_ppt_slides(&mut archive),
        };

        Some(meta)
    }

    /// Format date string (ISO 8601 to more readable format).
    fn format_date(iso: &str) -> String {
        // Convert 2023-12-25T12:00:00Z to 2023-12-25 12:00:00
        iso.replace('T', " ").trim_end_matches('Z').to_string()
    }

    /// Format metadata into human-readable description.
    fn format_description(meta: &OfficeMetadata) -> String {
        let doc_type = meta.doc_type.unwrap_or(OfficeType::Word);
        let mut parts = vec![format!("{} document", doc_type.name())];

        if let Some(ref title) = meta.title {
            if !title.is_empty() {
                parts.push(format!("\"{}\"", title));
            }
        }

        if let Some(ref creator) = meta.creator {
            if !creator.is_empty() {
                parts.push(format!("by {}", creator));
            }
        }

        if let Some(count) = meta.item_count {
            parts.push(format!("{} {}", count, doc_type.count_label()));
        }

        parts.join(", ")
    }

    /// Build RichDisplay options for UI rendering.
    fn build_rich_display(meta: &OfficeMetadata) -> Vec<RichDisplayOption> {
        let doc_type = meta.doc_type.unwrap_or(OfficeType::Word);
        let mut pairs = vec![];

        pairs.push((
            "Format".to_string(),
            format!("{} ({})", doc_type.name(), doc_type.extension()),
        ));

        if let Some(ref title) = meta.title {
            if !title.is_empty() {
                pairs.push(("Title".to_string(), title.clone()));
            }
        }

        if let Some(ref creator) = meta.creator {
            if !creator.is_empty() {
                pairs.push(("Author".to_string(), creator.clone()));
            }
        }

        if let Some(ref subject) = meta.subject {
            if !subject.is_empty() {
                pairs.push(("Subject".to_string(), subject.clone()));
            }
        }

        if let Some(ref desc) = meta.description {
            if !desc.is_empty() {
                pairs.push(("Description".to_string(), desc.clone()));
            }
        }

        if let Some(ref keywords) = meta.keywords {
            if !keywords.is_empty() {
                pairs.push(("Keywords".to_string(), keywords.clone()));
            }
        }

        if let Some(count) = meta.item_count {
            let label = match doc_type {
                OfficeType::Word => "Pages",
                OfficeType::Excel => "Sheets",
                OfficeType::PowerPoint => "Slides",
            };
            pairs.push((label.to_string(), count.to_string()));
        }

        if let Some(ref created) = meta.created {
            pairs.push(("Created".to_string(), Self::format_date(created)));
        }

        if let Some(ref modified) = meta.modified {
            pairs.push(("Modified".to_string(), Self::format_date(modified)));
        }

        vec![RichDisplayOption::new(RichDisplay::KeyValue { pairs })]
    }
}

impl Format for OfficeFormat {
    fn id(&self) -> &'static str {
        "office"
    }

    fn name(&self) -> &'static str {
        "Office Document"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Documents",
            description: "Microsoft Office document with metadata extraction",
            examples: &["[binary Office document data]"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_office(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "office".to_string(),
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

        let Some(meta) = Self::parse_office(bytes) else {
            return vec![];
        };

        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        vec![Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "office-info".to_string(),
            display: description,
            path: vec!["office-info".to_string()],
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
        &["office", "docx", "xlsx", "pptx"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_zip() {
        assert!(OfficeFormat::is_zip(&[0x50, 0x4B, 0x03, 0x04, 0xFF]));
        assert!(!OfficeFormat::is_zip(&[0x1F, 0x8B]));
    }

    #[test]
    fn test_extract_xml_element() {
        let xml = r#"<cp:coreProperties><dc:title>Test Doc</dc:title></cp:coreProperties>"#;
        assert_eq!(
            OfficeFormat::extract_xml_element(xml, "title"),
            Some("Test Doc".to_string())
        );
    }

    #[test]
    fn test_format_date() {
        assert_eq!(
            OfficeFormat::format_date("2023-12-25T12:00:00Z"),
            "2023-12-25 12:00:00"
        );
    }

    #[test]
    fn test_office_type() {
        assert_eq!(OfficeType::Word.name(), "Word");
        assert_eq!(OfficeType::Excel.extension(), "xlsx");
        assert_eq!(OfficeType::PowerPoint.count_label(), "slides");
    }
}
