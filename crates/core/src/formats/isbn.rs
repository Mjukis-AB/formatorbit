//! ISBN format for string-based ISBN parsing.
//!
//! Handles ISBN-10 with X check digit and hyphenated formats like:
//! - `0-306-40615-2`
//! - `978-0-306-40615-7`
//! - `019853453X`

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct IsbnFormat;

impl IsbnFormat {
    /// Extract digits from ISBN string (ignoring hyphens/spaces).
    /// Returns digits and whether it ends with X.
    fn extract_digits(input: &str) -> Option<(Vec<u8>, bool)> {
        let input = input.trim();
        let mut digits = Vec::new();
        let mut has_x = false;

        for (i, c) in input.chars().enumerate() {
            match c {
                '0'..='9' => digits.push(c as u8 - b'0'),
                'X' | 'x' => {
                    // X only valid as last character of ISBN-10
                    if i == input.len() - 1 {
                        has_x = true;
                    } else {
                        return None;
                    }
                }
                '-' | ' ' => {} // Ignore separators
                _ => return None,
            }
        }

        Some((digits, has_x))
    }

    /// Validate ISBN-10 checksum.
    /// Weights are 10,9,8,7,6,5,4,3,2,1 from left to right.
    /// X represents 10 as check digit.
    fn validate_isbn10(digits: &[u8], has_x: bool) -> bool {
        // With X: 9 digits + X = 10 chars
        // Without X: 10 digits
        let expected_len = if has_x { 9 } else { 10 };
        if digits.len() != expected_len {
            return false;
        }

        let mut sum: u32 = 0;
        for (i, &d) in digits.iter().enumerate() {
            sum += (d as u32) * (10 - i as u32);
        }

        if has_x {
            // X = 10 as check digit, weight = 1
            sum += 10;
        }

        sum.is_multiple_of(11)
    }

    /// Validate ISBN-13 / EAN-13 checksum.
    fn validate_isbn13(digits: &[u8]) -> bool {
        if digits.len() != 13 {
            return false;
        }

        let mut sum: u32 = 0;
        for (i, &d) in digits.iter().enumerate() {
            let weight = if i % 2 == 0 { 1 } else { 3 };
            sum += (d as u32) * weight;
        }

        sum.is_multiple_of(10)
    }

    /// Format ISBN with standard hyphenation (simplified).
    fn format_isbn10(digits: &[u8], has_x: bool) -> String {
        let mut s: String = digits.iter().map(|d| (b'0' + d) as char).collect();
        if has_x {
            s.push('X');
        }
        s
    }

    fn format_isbn13(digits: &[u8]) -> String {
        digits.iter().map(|d| (b'0' + d) as char).collect()
    }

    /// Check if input has ISBN-style formatting (hyphens or spaces).
    /// Formatted ISBNs are more likely to be intentional.
    fn has_isbn_formatting(input: &str) -> bool {
        input.contains('-') || input.contains(' ')
    }

    /// Check if a 10-digit number looks like a Unix epoch timestamp.
    /// Timestamps from ~2001 to ~2286 are 10 digits starting with 1.
    fn looks_like_epoch(digits: &[u8]) -> bool {
        if digits.len() != 10 {
            return false;
        }

        // Convert digits to number
        let mut value: u64 = 0;
        for &d in digits {
            value = value * 10 + d as u64;
        }

        // Reasonable epoch range: 2001-01-01 to 2100-01-01
        // (978000000 to 4102444800)
        // Most 10-digit numbers starting with 1 in range 1000000000-2100000000
        // are plausible timestamps
        (1_000_000_000..=2_100_000_000).contains(&value)
    }
}

impl Format for IsbnFormat {
    fn id(&self) -> &'static str {
        "isbn"
    }

    fn name(&self) -> &'static str {
        "ISBN"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Identifiers",
            description: "International Standard Book Number (ISBN-10/ISBN-13)",
            examples: &["978-0-306-40615-7", "0-19-853453-X", "9780306406157"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((digits, has_x)) = Self::extract_digits(input) else {
            return vec![];
        };

        let total_len = digits.len() + if has_x { 1 } else { 0 };

        // ISBN-10 (10 characters, may end with X)
        if total_len == 10 && Self::validate_isbn10(&digits, has_x) {
            let formatted = Self::format_isbn10(&digits, has_x);

            // Determine confidence based on formatting and content
            let confidence = if has_x {
                // X check digit is strong ISBN signal
                0.95
            } else if Self::has_isbn_formatting(input) {
                // Formatted with hyphens/spaces - likely intentional
                0.90
            } else if Self::looks_like_epoch(&digits) {
                // Pure numeric that looks like a timestamp - probably not ISBN
                0.40
            } else {
                // Pure numeric, not timestamp-like
                0.70
            };

            return vec![Interpretation {
                value: CoreValue::String(formatted.clone()),
                source_format: "isbn-10".to_string(),
                confidence,
                description: format!("ISBN-10: {}", formatted),
                rich_display: vec![],
            }];
        }

        // ISBN-13 (13 digits, no X allowed)
        if total_len == 13 && !has_x && Self::validate_isbn13(&digits) {
            let formatted = Self::format_isbn13(&digits);
            // Check if it's a book ISBN (978/979 prefix) or regular EAN
            let is_book = digits.starts_with(&[9, 7, 8]) || digits.starts_with(&[9, 7, 9]);
            let format_name = if is_book { "isbn-13" } else { "ean-13" };
            let desc = if is_book {
                format!("ISBN-13: {}", formatted)
            } else {
                format!("EAN-13: {}", formatted)
            };

            return vec![Interpretation {
                value: CoreValue::String(formatted),
                source_format: format_name.to_string(),
                confidence: 0.90,
                description: desc,
                rich_display: vec![],
            }];
        }

        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["isbn10", "isbn13", "ean"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isbn10_with_x() {
        let format = IsbnFormat;
        // 155860832X is a valid ISBN-10 ending with X
        let results = format.parse("1-55860-832-X");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "isbn-10");
        assert!(results[0].description.contains("155860832X"));
    }

    #[test]
    fn test_isbn10_lowercase_x() {
        let format = IsbnFormat;
        let results = format.parse("155860832x");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "isbn-10");
    }

    #[test]
    fn test_isbn10_numeric() {
        let format = IsbnFormat;
        let results = format.parse("0-306-40615-2");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "isbn-10");
    }

    #[test]
    fn test_isbn13() {
        let format = IsbnFormat;
        let results = format.parse("978-0-306-40615-7");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "isbn-13");
    }

    #[test]
    fn test_isbn13_no_hyphens() {
        let format = IsbnFormat;
        let results = format.parse("9780306406157");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "isbn-13");
    }

    #[test]
    fn test_ean13_non_book() {
        let format = IsbnFormat;
        let results = format.parse("5901234123457");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "ean-13");
    }

    #[test]
    fn test_invalid_checksum() {
        let format = IsbnFormat;
        assert!(format.parse("978-0-306-40615-8").is_empty()); // Wrong check digit
        assert!(format.parse("155860832-1").is_empty()); // Wrong check digit (should be X)
    }

    #[test]
    fn test_invalid_format() {
        let format = IsbnFormat;
        assert!(format.parse("hello").is_empty());
        assert!(format.parse("12345").is_empty()); // Too short
        assert!(format.parse("1234567890123456").is_empty()); // Too long
    }

    #[test]
    fn test_confidence_with_x_checkdigit() {
        let format = IsbnFormat;
        let results = format.parse("155860832X");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.95); // X gives highest confidence
    }

    #[test]
    fn test_confidence_with_hyphens() {
        let format = IsbnFormat;
        let results = format.parse("0-306-40615-2");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.90); // Formatted gets high confidence
    }

    #[test]
    fn test_confidence_epoch_like_number() {
        let format = IsbnFormat;
        // 1704067200 is Jan 1, 2024 - looks like epoch timestamp
        let results = format.parse("1704067200");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence <= 0.50); // Low confidence for epoch-like
    }

    #[test]
    fn test_confidence_pure_numeric_non_epoch() {
        let format = IsbnFormat;
        // 0306406152 is not in epoch range (too small)
        let results = format.parse("0306406152");
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.70); // Medium confidence for pure numeric
        assert!(results[0].confidence < 0.90); // But less than formatted
    }
}
