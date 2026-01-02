//! Hash digest calculation format.
//!
//! Calculates various hash digests from byte data.
//!
//! Performance note: Hash computation is expensive for large data.
//! We skip expensive algorithms (SHA-512, Blake) for data > 1MB.

use blake2::{digest::consts::U32, Blake2b, Digest as Blake2Digest};
use crc32fast::Hasher as Crc32Hasher;
use md5::{Digest as Md5Digest, Md5};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest as Sha2Digest, Sha256, Sha512};

/// Maximum size for computing all hashes (1 MB).
/// Above this, only fast hashes (CRC32, MD5) are computed.
const FAST_HASH_THRESHOLD: usize = 1024 * 1024;

/// Maximum size for computing any hashes (10 MB).
/// Above this, only CRC32 is computed.
const MAX_HASH_SIZE: usize = 10 * 1024 * 1024;

/// Blake2b with 256-bit output.
type Blake2b256 = Blake2b<U32>;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct DigestFormat;

impl DigestFormat {
    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn make_conversion(algorithm: &str, hex: String) -> Conversion {
        Conversion {
            value: CoreValue::String(hex.clone()),
            target_format: algorithm.to_string(),
            display: hex.clone(),
            path: vec![algorithm.to_string()],
            steps: vec![ConversionStep {
                format: algorithm.to_string(),
                value: CoreValue::String(hex.clone()),
                display: hex,
            }],
            is_lossy: true, // Hash is one-way
            priority: ConversionPriority::Encoding,
            display_only: true, // Don't explore hex strings further
            kind: ConversionKind::Conversion,
            hidden: false,
            rich_display: vec![],
        }
    }
}

impl Format for DigestFormat {
    fn id(&self) -> &'static str {
        "digest"
    }

    fn name(&self) -> &'static str {
        "Hash Digests"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Encoding",
            description: "Calculates hash digests (MD5, SHA, Blake, CRC32)",
            examples: &[],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // No parsing - this format only produces conversions from bytes
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

        let size = bytes.len();
        let mut conversions = Vec::new();

        // CRC32 - always compute (very fast)
        let mut crc = Crc32Hasher::new();
        crc.update(bytes);
        let crc_value = crc.finalize();
        conversions.push(Self::make_conversion("crc32", format!("{:08x}", crc_value)));

        // Skip all other hashes for very large data
        if size > MAX_HASH_SIZE {
            return conversions;
        }

        // MD5 - compute for data up to 10MB
        let md5_hash = <Md5 as Md5Digest>::digest(bytes);
        conversions.push(Self::make_conversion("md5", Self::hex_encode(&md5_hash)));

        // SHA-1 - compute for data up to 10MB
        let sha1_hash = <Sha1 as Sha1Digest>::digest(bytes);
        conversions.push(Self::make_conversion("sha1", Self::hex_encode(&sha1_hash)));

        // Skip expensive hashes for large data (> 1MB)
        if size > FAST_HASH_THRESHOLD {
            return conversions;
        }

        // SHA-256
        let sha256_hash = <Sha256 as Sha2Digest>::digest(bytes);
        conversions.push(Self::make_conversion(
            "sha256",
            Self::hex_encode(&sha256_hash),
        ));

        // SHA-512
        let sha512_hash = <Sha512 as Sha2Digest>::digest(bytes);
        conversions.push(Self::make_conversion(
            "sha512",
            Self::hex_encode(&sha512_hash),
        ));

        // Blake2b-256
        let blake2_hash = <Blake2b256 as Blake2Digest>::digest(bytes);
        conversions.push(Self::make_conversion(
            "blake2b-256",
            Self::hex_encode(&blake2_hash),
        ));

        // Blake3
        let blake3_hash = blake3::hash(bytes);
        conversions.push(Self::make_conversion(
            "blake3",
            blake3_hash.to_hex().to_string(),
        ));

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["hash", "checksum"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digest_hello() {
        let format = DigestFormat;
        let value = CoreValue::Bytes(b"Hello".to_vec());
        let conversions = format.conversions(&value);

        // Should have 7 digest types
        assert_eq!(conversions.len(), 7);

        // Check SHA-256 of "Hello"
        let sha256 = conversions
            .iter()
            .find(|c| c.target_format == "sha256")
            .unwrap();
        assert_eq!(
            sha256.display,
            "185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969"
        );
    }

    #[test]
    fn test_digest_empty() {
        let format = DigestFormat;
        let value = CoreValue::Bytes(vec![]);
        let conversions = format.conversions(&value);

        // SHA-256 of empty input
        let sha256 = conversions
            .iter()
            .find(|c| c.target_format == "sha256")
            .unwrap();
        assert_eq!(
            sha256.display,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_digest_only_from_bytes() {
        let format = DigestFormat;

        // String should produce no conversions
        let string_value = CoreValue::String("Hello".to_string());
        assert!(format.conversions(&string_value).is_empty());

        // Int should produce no conversions
        let int_value = CoreValue::Int {
            value: 12345,
            original_bytes: None,
        };
        assert!(format.conversions(&int_value).is_empty());
    }
}
