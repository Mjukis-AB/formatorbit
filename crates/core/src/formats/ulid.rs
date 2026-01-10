//! ULID (Universally Unique Lexicographically Sortable Identifier) format.
//!
//! ULIDs are 26-character Crockford base32 encoded identifiers with an embedded timestamp.
//! Structure: TTTTTTTTTTRRRRRRRRRRRRRRR (10 chars timestamp + 16 chars randomness)

use chrono::{DateTime, TimeZone, Utc};

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct UlidFormat;

/// Crockford's Base32 alphabet (excludes I, L, O, U to avoid confusion)
const CROCKFORD_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

impl UlidFormat {
    /// Check if a character is valid in Crockford base32.
    fn is_valid_crockford_char(c: char) -> bool {
        let c = c.to_ascii_uppercase();
        // Valid: 0-9, A-H, J-K, M-N, P-T, V-Z
        // Invalid: I, L, O, U
        matches!(c, '0'..='9' | 'A'..='H' | 'J'..='K' | 'M'..='N' | 'P'..='T' | 'V'..='Z')
    }

    /// Normalize a character (handle common substitutions).
    fn normalize_char(c: char) -> Option<u8> {
        let c = c.to_ascii_uppercase();
        match c {
            'O' => Some(0),       // O -> 0
            'I' | 'L' => Some(1), // I, L -> 1
            _ => CROCKFORD_ALPHABET
                .iter()
                .position(|&x| x == c as u8)
                .map(|p| p as u8),
        }
    }

    /// Decode the timestamp portion (first 10 characters) to milliseconds since epoch.
    fn decode_timestamp(s: &str) -> Option<u64> {
        if s.len() < 10 {
            return None;
        }

        let timestamp_chars = &s[..10];
        let mut timestamp: u64 = 0;

        for c in timestamp_chars.chars() {
            let value = Self::normalize_char(c)? as u64;
            timestamp = timestamp.checked_mul(32)?.checked_add(value)?;
        }

        // Overflow check: max valid timestamp is 7ZZZZZZZZZ = 281474976710655
        if timestamp > 281474976710655 {
            return None;
        }

        Some(timestamp)
    }

    /// Decode ULID to bytes (128 bits = 16 bytes).
    fn decode_to_bytes(s: &str) -> Option<Vec<u8>> {
        if s.len() != 26 {
            return None;
        }

        // Decode as 128-bit number, then convert to bytes
        let mut value: u128 = 0;
        for c in s.chars() {
            let digit = Self::normalize_char(c)? as u128;
            value = value.checked_mul(32)?.checked_add(digit)?;
        }

        Some(value.to_be_bytes().to_vec())
    }

    /// Format timestamp as human-readable string.
    fn format_timestamp(millis: u64) -> Option<String> {
        let secs = (millis / 1000) as i64;
        let nanos = ((millis % 1000) * 1_000_000) as u32;
        Utc.timestamp_opt(secs, nanos)
            .single()
            .map(|dt: DateTime<Utc>| dt.to_rfc3339())
    }
}

impl Format for UlidFormat {
    fn id(&self) -> &'static str {
        "ulid"
    }

    fn name(&self) -> &'static str {
        "ULID"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Identifiers",
            description: "Universally Unique Lexicographically Sortable Identifier",
            examples: &["01ARZ3NDEKTSV4RRFFQ69G5FAV", "01H5S5JXMEZ7YQ5DBVTZ1Z5Q4T"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Must be exactly 26 characters
        if trimmed.len() != 26 {
            return vec![];
        }

        // All characters must be valid Crockford base32
        if !trimmed.chars().all(Self::is_valid_crockford_char) {
            return vec![];
        }

        // First character must be 0-7 (to fit in 128 bits)
        let Some(first) = trimmed.chars().next() else {
            return vec![];
        };
        if !matches!(first, '0'..='7') {
            return vec![];
        }

        // Try to decode timestamp
        let Some(timestamp_millis) = Self::decode_timestamp(trimmed) else {
            return vec![];
        };
        let Some(timestamp_str) = Self::format_timestamp(timestamp_millis) else {
            return vec![];
        };

        // Decode to bytes
        let Some(bytes) = Self::decode_to_bytes(trimmed) else {
            return vec![];
        };

        // High confidence if timestamp is reasonable (after 2000, before 2100)
        let year_2000_millis: u64 = 946684800000;
        let year_2100_millis: u64 = 4102444800000;
        let confidence =
            if timestamp_millis >= year_2000_millis && timestamp_millis <= year_2100_millis {
                0.92
            } else if timestamp_millis > 0 {
                0.70
            } else {
                0.50
            };

        let description = format!("ULID (created: {})", timestamp_str);

        vec![Interpretation {
            value: CoreValue::Bytes(bytes),
            source_format: "ulid".to_string(),
            confidence,
            description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Don't format arbitrary bytes as ULID - only values originally parsed as ULID
        // should be displayed as ULID. Otherwise we get noise like UUID→ULID.
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    // Note: No conversions() - ULID timestamp is shown in parse description.
    // We don't want arbitrary 16-byte values (like UUIDs) to show ULID timestamps.

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ulid() {
        let format = UlidFormat;
        // Example ULID
        let results = format.parse("01ARZ3NDEKTSV4RRFFQ69G5FAV");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "ulid");
        assert!(results[0].description.contains("ULID"));
    }

    #[test]
    fn test_decode_timestamp() {
        // 01ARZ3NDEK = specific timestamp
        let millis = UlidFormat::decode_timestamp("01ARZ3NDEK").unwrap();
        assert!(millis > 0);
    }

    #[test]
    fn test_invalid_ulid() {
        let format = UlidFormat;

        // Wrong length
        assert!(format.parse("01ARZ3NDEK").is_empty());

        // Invalid characters (contains I, L, O, U)
        assert!(format.parse("01ARZ3NDEKTSV4RRFFQ69G5FAI").is_empty());
        assert!(format.parse("01ARZ3NDEKTSV4RRFFQ69G5FAL").is_empty());
        assert!(format.parse("01ARZ3NDEKTSV4RRFFQ69G5FAO").is_empty());
        assert!(format.parse("01ARZ3NDEKTSV4RRFFQ69G5FAU").is_empty());
    }

    #[test]
    fn test_first_char_overflow() {
        let format = UlidFormat;
        // First char > 7 would overflow 128 bits
        assert!(format.parse("8ZZZZZZZZZZZZZZZZZZZZZZZZ").is_empty());
    }

    // Note: roundtrip test removed because format() is now disabled
    // to avoid noise from arbitrary bytes→ULID conversions.
}
