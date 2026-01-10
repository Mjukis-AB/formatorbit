//! Archive file format detection and metadata extraction.
//!
//! Detects archive files and extracts:
//! - File count
//! - Total uncompressed size
//! - Compression ratio (for ZIP)
//! - File listing (first N files)

use std::io::{Cursor, Read};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct ArchiveFormat;

/// Archive metadata extracted from file.
#[derive(Debug, Clone, Default)]
struct ArchiveMetadata {
    archive_type: String,
    file_count: usize,
    total_size: u64,
    compressed_size: Option<u64>,
    files: Vec<String>,
}

impl ArchiveMetadata {
    fn compression_ratio(&self) -> Option<f64> {
        self.compressed_size.map(|c| {
            if self.total_size > 0 {
                1.0 - (c as f64 / self.total_size as f64)
            } else {
                0.0
            }
        })
    }
}

impl ArchiveFormat {
    /// Check if data is a ZIP archive.
    fn is_zip(data: &[u8]) -> bool {
        data.len() >= 4 && data.starts_with(&[0x50, 0x4B, 0x03, 0x04])
    }

    /// Check if data is a GZip file.
    fn is_gzip(data: &[u8]) -> bool {
        data.len() >= 2 && data.starts_with(&[0x1F, 0x8B])
    }

    /// Check if data is a TAR archive (check for ustar magic at offset 257).
    fn is_tar(data: &[u8]) -> bool {
        if data.len() < 265 {
            return false;
        }
        // Check for "ustar" magic at offset 257
        &data[257..262] == b"ustar"
    }

    /// Detect archive type from magic bytes.
    #[cfg(test)]
    fn detect_archive_type(data: &[u8]) -> Option<&'static str> {
        if Self::is_zip(data) {
            Some("ZIP")
        } else if Self::is_gzip(data) {
            Some("GZIP")
        } else if Self::is_tar(data) {
            Some("TAR")
        } else {
            None
        }
    }

    /// Parse ZIP archive and extract metadata.
    fn parse_zip(data: &[u8]) -> Option<ArchiveMetadata> {
        let cursor = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).ok()?;

        let mut meta = ArchiveMetadata {
            archive_type: "ZIP".to_string(),
            file_count: archive.len(),
            ..Default::default()
        };

        let mut total_size: u64 = 0;
        let mut compressed_size: u64 = 0;

        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                total_size += file.size();
                compressed_size += file.compressed_size();

                // Collect first 10 file names
                if meta.files.len() < 10 && !file.is_dir() {
                    meta.files.push(file.name().to_string());
                }
            }
        }

        meta.total_size = total_size;
        meta.compressed_size = Some(compressed_size);

        Some(meta)
    }

    /// Parse TAR archive and extract metadata.
    fn parse_tar(data: &[u8]) -> Option<ArchiveMetadata> {
        let cursor = Cursor::new(data);
        let mut archive = tar::Archive::new(cursor);

        let mut meta = ArchiveMetadata {
            archive_type: "TAR".to_string(),
            ..Default::default()
        };

        let entries = archive.entries().ok()?;

        for entry in entries.flatten() {
            meta.file_count += 1;
            meta.total_size += entry.size();

            // Collect first 10 file names
            if meta.files.len() < 10 {
                if let Ok(path) = entry.path() {
                    let path_str = path.to_string_lossy().to_string();
                    if !path_str.is_empty() {
                        meta.files.push(path_str);
                    }
                }
            }
        }

        Some(meta)
    }

    /// Parse GZip file and extract metadata.
    fn parse_gzip(data: &[u8]) -> Option<ArchiveMetadata> {
        let cursor = Cursor::new(data);
        let mut decoder = flate2::read::GzDecoder::new(cursor);

        // Try to decompress to get uncompressed size
        let mut decompressed = Vec::new();
        let uncompressed_size = match decoder.read_to_end(&mut decompressed) {
            Ok(size) => size as u64,
            Err(_) => 0,
        };

        // Check if it's a tar.gz
        let (archive_type, inner_meta) = if Self::is_tar(&decompressed) {
            if let Some(tar_meta) = Self::parse_tar(&decompressed) {
                ("TAR.GZ", Some(tar_meta))
            } else {
                ("GZIP", None)
            }
        } else {
            ("GZIP", None)
        };

        let meta = if let Some(mut inner) = inner_meta {
            inner.archive_type = archive_type.to_string();
            inner.compressed_size = Some(data.len() as u64);
            inner
        } else {
            ArchiveMetadata {
                archive_type: archive_type.to_string(),
                file_count: 1,
                total_size: uncompressed_size,
                compressed_size: Some(data.len() as u64),
                files: vec![],
            }
        };

        Some(meta)
    }

    /// Parse archive and extract metadata.
    fn parse_archive(data: &[u8]) -> Option<ArchiveMetadata> {
        if Self::is_zip(data) {
            Self::parse_zip(data)
        } else if Self::is_gzip(data) {
            Self::parse_gzip(data)
        } else if Self::is_tar(data) {
            Self::parse_tar(data)
        } else {
            None
        }
    }

    /// Format bytes as human-readable size.
    fn format_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    /// Format metadata into human-readable description.
    fn format_description(meta: &ArchiveMetadata) -> String {
        let mut parts = vec![format!("{} archive", meta.archive_type)];

        parts.push(format!(
            "{} file{}",
            meta.file_count,
            if meta.file_count == 1 { "" } else { "s" }
        ));

        parts.push(Self::format_size(meta.total_size));

        if let Some(ratio) = meta.compression_ratio() {
            if ratio > 0.0 {
                parts.push(format!("{:.0}% compression", ratio * 100.0));
            }
        }

        parts.join(", ")
    }

    /// Build RichDisplay options for UI rendering.
    fn build_rich_display(meta: &ArchiveMetadata) -> Vec<RichDisplayOption> {
        let mut displays = vec![];
        let mut pairs = vec![];

        pairs.push(("Format".to_string(), meta.archive_type.clone()));
        pairs.push((
            "Files".to_string(),
            format!(
                "{} file{}",
                meta.file_count,
                if meta.file_count == 1 { "" } else { "s" }
            ),
        ));
        pairs.push((
            "Uncompressed".to_string(),
            Self::format_size(meta.total_size),
        ));

        if let Some(compressed) = meta.compressed_size {
            pairs.push(("Compressed".to_string(), Self::format_size(compressed)));
        }

        if let Some(ratio) = meta.compression_ratio() {
            if ratio > 0.0 {
                pairs.push(("Compression".to_string(), format!("{:.1}%", ratio * 100.0)));
            }
        }

        displays.push(RichDisplayOption::new(RichDisplay::KeyValue { pairs }));

        // Add file listing as table if we have files
        if !meta.files.is_empty() {
            let headers = vec!["File".to_string()];
            let rows: Vec<Vec<String>> = meta.files.iter().map(|f| vec![f.clone()]).collect();

            displays.push(RichDisplayOption::new(RichDisplay::Table { headers, rows }));
        }

        displays
    }
}

impl Format for ArchiveFormat {
    fn id(&self) -> &'static str {
        "archive"
    }

    fn name(&self) -> &'static str {
        "Archive File"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Archives",
            description: "Archive file with metadata extraction",
            examples: &["[binary archive data]"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to decode as base64
        if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, input)
        {
            if let Some(meta) = Self::parse_archive(&bytes) {
                let description = Self::format_description(&meta);
                let rich_display = Self::build_rich_display(&meta);

                return vec![Interpretation {
                    value: CoreValue::Bytes(bytes),
                    source_format: "archive".to_string(),
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

        let Some(meta) = Self::parse_archive(bytes) else {
            return vec![];
        };

        let description = Self::format_description(&meta);
        let rich_display = Self::build_rich_display(&meta);

        vec![Conversion {
            value: CoreValue::String(description.clone()),
            target_format: "archive-info".to_string(),
            display: description,
            path: vec!["archive-info".to_string()],
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
        &["archive", "zip", "tar", "gz", "gzip", "tar.gz", "tgz"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_archive_type() {
        assert_eq!(
            ArchiveFormat::detect_archive_type(&[0x50, 0x4B, 0x03, 0x04]),
            Some("ZIP")
        );
        assert_eq!(
            ArchiveFormat::detect_archive_type(&[0x1F, 0x8B]),
            Some("GZIP")
        );
        assert_eq!(ArchiveFormat::detect_archive_type(b"not an archive"), None);
    }

    #[test]
    fn test_is_zip() {
        assert!(ArchiveFormat::is_zip(&[0x50, 0x4B, 0x03, 0x04, 0xFF]));
        assert!(!ArchiveFormat::is_zip(&[0x1F, 0x8B]));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(ArchiveFormat::format_size(512), "512 bytes");
        assert_eq!(ArchiveFormat::format_size(1024), "1.0 KB");
        assert_eq!(ArchiveFormat::format_size(1536), "1.5 KB");
        assert_eq!(ArchiveFormat::format_size(1048576), "1.0 MB");
        assert_eq!(ArchiveFormat::format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_compression_ratio() {
        let meta = ArchiveMetadata {
            total_size: 1000,
            compressed_size: Some(500),
            ..Default::default()
        };
        assert!((meta.compression_ratio().unwrap() - 0.5).abs() < 0.001);
    }
}
