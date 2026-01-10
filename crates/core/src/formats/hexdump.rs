//! Hexdump (xxd-style) format for viewing raw bytes.

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct HexdumpFormat;

impl HexdumpFormat {
    /// Format bytes as xxd-style hexdump.
    ///
    /// Example output:
    /// ```text
    /// 00000000: 6869 7420 7072 6f74 6f62 7566 0a0a 0a0a  hit protobuf....
    /// 00000010: 0a0a 0a0a 0a0a                           ......
    /// ```
    fn format_hexdump(bytes: &[u8], max_lines: usize) -> String {
        let mut output = String::new();
        let bytes_per_line = 16;

        for (line_idx, chunk) in bytes.chunks(bytes_per_line).enumerate() {
            if line_idx >= max_lines {
                let remaining = bytes.len() - (line_idx * bytes_per_line);
                output.push_str(&format!("... ({} more bytes)\n", remaining));
                break;
            }

            let offset = line_idx * bytes_per_line;

            // Offset
            output.push_str(&format!("{:08x}: ", offset));

            // Hex bytes (grouped in pairs)
            for (i, byte) in chunk.iter().enumerate() {
                output.push_str(&format!("{:02x}", byte));
                if i % 2 == 1 && i < chunk.len() - 1 {
                    output.push(' ');
                }
            }

            // Padding if line is short
            let hex_chars = chunk.len() * 2 + chunk.len() / 2;
            let full_hex_chars = bytes_per_line * 2 + bytes_per_line / 2;
            for _ in hex_chars..full_hex_chars {
                output.push(' ');
            }

            // ASCII representation
            output.push_str("  ");
            for byte in chunk {
                let c = if byte.is_ascii_graphic() || *byte == b' ' {
                    *byte as char
                } else {
                    '.'
                };
                output.push(c);
            }

            output.push('\n');
        }

        // Remove trailing newline
        output.pop();
        output
    }
}

impl Format for HexdumpFormat {
    fn id(&self) -> &'static str {
        "hexdump"
    }

    fn name(&self) -> &'static str {
        "Hexdump"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Encoding",
            description: "xxd-style hex dump with ASCII",
            examples: &[],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // Hexdump is output-only, not a parseable input format
        vec![]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(_))
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) => Some(Self::format_hexdump(bytes, 16)),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Only show hexdump for reasonably sized data (at least 4 bytes)
        if bytes.len() < 4 {
            return vec![];
        }

        let display = Self::format_hexdump(bytes, 8); // Show up to 8 lines in conversion

        vec![Conversion {
            value: CoreValue::String(display.clone()),
            target_format: "hexdump".to_string(),
            display: display.clone(),
            path: vec!["hexdump".to_string()],
            steps: vec![ConversionStep {
                format: "hexdump".to_string(),
                value: CoreValue::String(display.clone()),
                display,
            }],
            is_lossy: false,
            // Priority between Encoding and Raw - shows when no structured data found
            priority: ConversionPriority::Encoding,
            display_only: true, // Terminal format - don't re-encode the hexdump string
            kind: ConversionKind::Representation,
            hidden: false,
            rich_display: vec![],
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["xxd", "dump"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hexdump_short() {
        let bytes = vec![0x68, 0x65, 0x6c, 0x6c, 0x6f]; // "hello"
        let output = HexdumpFormat::format_hexdump(&bytes, 16);
        // Format is "6865 6c6c 6f" (space every 2 bytes)
        assert!(output.contains("6865 6c6c 6f"));
        assert!(output.contains("hello"));
    }

    #[test]
    fn test_format_hexdump_multiline() {
        let bytes: Vec<u8> = (0..32).collect();
        let output = HexdumpFormat::format_hexdump(&bytes, 16);
        assert!(output.contains("00000000:"));
        assert!(output.contains("00000010:"));
    }

    #[test]
    fn test_format_hexdump_truncation() {
        let bytes: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let output = HexdumpFormat::format_hexdump(&bytes, 4);
        assert!(output.contains("... (192 more bytes)"));
    }

    #[test]
    fn test_conversions() {
        let format = HexdumpFormat;
        let value = CoreValue::Bytes(vec![0x68, 0x65, 0x6c, 0x6c, 0x6f]);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "hexdump");
        assert!(conversions[0].display.contains("hello"));
    }

    #[test]
    fn test_no_conversion_for_small_bytes() {
        let format = HexdumpFormat;
        let value = CoreValue::Bytes(vec![0x01, 0x02]); // Only 2 bytes
        let conversions = format.conversions(&value);

        assert!(conversions.is_empty());
    }
}
