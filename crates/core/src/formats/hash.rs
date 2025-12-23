//! Hash detection format.
//!
//! Identifies potential hash values by their length and character set.

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct HashFormat;

/// Known hash types with their hex lengths.
const HASH_TYPES: &[(&str, usize, &str)] = &[
    ("MD5", 32, "128-bit"),
    ("SHA-1", 40, "160-bit"),
    ("SHA-224", 56, "224-bit"),
    ("SHA-256", 64, "256-bit"),
    ("SHA-384", 96, "384-bit"),
    ("SHA-512", 128, "512-bit"),
    ("RIPEMD-160", 40, "160-bit"),
    ("CRC-32", 8, "32-bit"),
    ("MD4", 32, "128-bit"),
    ("SHA-512/256", 64, "256-bit"),
    ("BLAKE2s-256", 64, "256-bit"),
    ("BLAKE2b-512", 128, "512-bit"),
];

impl HashFormat {
    /// Check if a string looks like a hex hash.
    fn is_hex_hash(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Get possible hash types for a given hex length.
    fn identify_hash(len: usize) -> Vec<(&'static str, &'static str)> {
        HASH_TYPES
            .iter()
            .filter(|(_, l, _)| *l == len)
            .map(|(name, _, bits)| (*name, *bits))
            .collect()
    }
}

impl Format for HashFormat {
    fn id(&self) -> &'static str {
        "hash"
    }

    fn name(&self) -> &'static str {
        "Hash"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Hashing",
            description: "Hash detection by length (MD5, SHA-1, SHA-256, etc.)",
            examples: &[
                "d41d8cd98f00b204e9800998ecf8427e",
                "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            ],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Must be valid hex
        if !Self::is_hex_hash(trimmed) {
            return vec![];
        }

        let len = trimmed.len();
        let matches = Self::identify_hash(len);

        if matches.is_empty() {
            return vec![];
        }

        // Build description
        let hash_names: Vec<_> = matches.iter().map(|(name, _)| *name).collect();
        let bits = matches[0].1;

        let description = if hash_names.len() == 1 {
            format!("Possible {} hash ({})", hash_names[0], bits)
        } else {
            format!("Possible {} hash ({})", hash_names.join(" or "), bits)
        };

        // Confidence based on how unique the length is
        let confidence = match matches.len() {
            1 => 0.70, // Unique length (e.g., SHA-224, SHA-384)
            2 => 0.60, // Two possibilities (e.g., MD5/MD4, SHA-256/BLAKE2s)
            _ => 0.50, // Multiple possibilities
        };

        // Store as bytes for potential further conversion
        let bytes: Vec<u8> = (0..len)
            .step_by(2)
            .filter_map(|i| {
                if i + 2 <= len {
                    u8::from_str_radix(&trimmed[i..i + 2], 16).ok()
                } else {
                    None
                }
            })
            .collect();

        vec![Interpretation {
            value: CoreValue::Bytes(bytes),
            source_format: "hash".to_string(),
            confidence,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // We don't generate hashes
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["md5", "sha1", "sha256", "sha512"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_md5() {
        let format = HashFormat;
        // MD5 of empty string
        let results = format.parse("d41d8cd98f00b204e9800998ecf8427e");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("MD5"));
        assert!(results[0].description.contains("128-bit"));
    }

    #[test]
    fn test_detect_sha1() {
        let format = HashFormat;
        // SHA-1 of empty string
        let results = format.parse("da39a3ee5e6b4b0d3255bfef95601890afd80709");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("SHA-1"));
        assert!(results[0].description.contains("160-bit"));
    }

    #[test]
    fn test_detect_sha256() {
        let format = HashFormat;
        // SHA-256 of empty string
        let results =
            format.parse("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("SHA-256"));
        assert!(results[0].description.contains("256-bit"));
    }

    #[test]
    fn test_detect_sha512() {
        let format = HashFormat;
        // SHA-512 of empty string
        let results = format.parse("cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("SHA-512"));
        assert!(results[0].description.contains("512-bit"));
    }

    #[test]
    fn test_detect_crc32() {
        let format = HashFormat;
        let results = format.parse("DEADBEEF");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("CRC-32"));
    }

    #[test]
    fn test_not_hash() {
        let format = HashFormat;

        // Not valid hex
        assert!(format.parse("not-a-hash").is_empty());

        // Wrong length (no matching hash type)
        assert!(format.parse("abc").is_empty());
        assert!(format.parse("abcde").is_empty());
    }

    #[test]
    fn test_case_insensitive() {
        let format = HashFormat;
        let results = format.parse("D41D8CD98F00B204E9800998ECF8427E");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("MD5"));
    }

    #[test]
    fn test_confidence_unique_length() {
        let format = HashFormat;

        // SHA-224 has unique length (56)
        let sha224 = format.parse("d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f");
        assert!(sha224[0].confidence > 0.65);

        // MD5/MD4 share length (32) - lower confidence
        let md5 = format.parse("d41d8cd98f00b204e9800998ecf8427e");
        assert!(md5[0].confidence < sha224[0].confidence);
    }
}
