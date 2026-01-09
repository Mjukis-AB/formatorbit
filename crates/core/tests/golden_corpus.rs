//! Golden corpus tests for format interpretation confidence.
//!
//! This module generates test inputs for each format and validates that:
//! 1. The expected format is recognized
//! 2. The expected format has the highest (or acceptably high) confidence
//!
//! The goal is to catch regressions where unrelated formats "steal" confidence
//! from the intended interpretation (e.g., timestamps detected as ISBN-10).

use formatorbit_core::Formatorbit;

/// A golden test case: input string and expected top interpretation.
struct GoldenCase {
    input: &'static str,
    expected_format: &'static str,
    description: &'static str,
    /// Minimum acceptable confidence for the expected format
    min_confidence: f32,
    /// If true, expected_format must be THE top result
    /// If false, it just needs to be present with min_confidence
    must_be_top: bool,
}

impl GoldenCase {
    const fn top(input: &'static str, format: &'static str, desc: &'static str) -> Self {
        Self {
            input,
            expected_format: format,
            description: desc,
            min_confidence: 0.5,
            must_be_top: true,
        }
    }

    const fn present(
        input: &'static str,
        format: &'static str,
        desc: &'static str,
        min_conf: f32,
    ) -> Self {
        Self {
            input,
            expected_format: format,
            description: desc,
            min_confidence: min_conf,
            must_be_top: false,
        }
    }
}

// =============================================================================
// Golden Corpus: Timestamps / Epochs
// =============================================================================

const EPOCH_CASES: &[GoldenCase] = &[
    // Recent timestamps (should NOT be detected as ISBN, geohash, etc.)
    GoldenCase::top("1704067200", "epoch-seconds", "Jan 1, 2024 00:00:00 UTC"),
    GoldenCase::top("1700000000", "epoch-seconds", "Nov 14, 2023 (round number)"),
    GoldenCase::top("1609459200", "epoch-seconds", "Jan 1, 2021 00:00:00 UTC"),
    GoldenCase::top("1577836800", "epoch-seconds", "Jan 1, 2020 00:00:00 UTC"),
    GoldenCase::top("1546300800", "epoch-seconds", "Jan 1, 2019 00:00:00 UTC"),
    GoldenCase::top("1672531200", "epoch-seconds", "Jan 1, 2023 00:00:00 UTC"),
    GoldenCase::top("1735689600", "epoch-seconds", "Jan 1, 2025 00:00:00 UTC"),
    // Millisecond timestamps - decimal wins because it's more general, but epoch-millis should be present
    GoldenCase::present(
        "1704067200000",
        "epoch-millis",
        "Jan 1, 2024 in milliseconds",
        0.80,
    ),
    GoldenCase::present(
        "1700000000000",
        "epoch-millis",
        "Nov 14, 2023 in milliseconds",
        0.80,
    ),
    // Edge: older timestamps
    GoldenCase::top(
        "1000000000",
        "epoch-seconds",
        "Sep 9, 2001 (first 10-digit)",
    ),
    GoldenCase::top("946684800", "epoch-seconds", "Jan 1, 2000 00:00:00 UTC"),
];

// =============================================================================
// Golden Corpus: IP Addresses
// =============================================================================

const IP_CASES: &[GoldenCase] = &[
    // IPv4
    GoldenCase::top("192.168.1.1", "ipv4", "Private IP (common router)"),
    GoldenCase::top("10.0.0.1", "ipv4", "Private IP (10.x range)"),
    GoldenCase::top("172.16.0.1", "ipv4", "Private IP (172.16.x range)"),
    GoldenCase::top("8.8.8.8", "ipv4", "Google DNS"),
    GoldenCase::top("1.1.1.1", "ipv4", "Cloudflare DNS"),
    GoldenCase::top("127.0.0.1", "ipv4", "Localhost"),
    GoldenCase::top("255.255.255.255", "ipv4", "Broadcast"),
    GoldenCase::top("0.0.0.0", "ipv4", "Any address"),
    // IPv6
    GoldenCase::top("::1", "ipv6", "IPv6 localhost"),
    GoldenCase::top("fe80::1", "ipv6", "IPv6 link-local"),
    GoldenCase::top("2001:4860:4860::8888", "ipv6", "Google IPv6 DNS"),
    GoldenCase::top(
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
        "ipv6",
        "Full IPv6",
    ),
];

// =============================================================================
// Golden Corpus: UUIDs
// =============================================================================

const UUID_CASES: &[GoldenCase] = &[
    GoldenCase::top(
        "550e8400-e29b-41d4-a716-446655440000",
        "uuid",
        "Standard UUID v4",
    ),
    GoldenCase::top("00000000-0000-0000-0000-000000000000", "uuid", "Nil UUID"),
    GoldenCase::top("ffffffff-ffff-ffff-ffff-ffffffffffff", "uuid", "Max UUID"),
    GoldenCase::top(
        "123e4567-e89b-12d3-a456-426614174000",
        "uuid",
        "Example UUID",
    ),
    // Without dashes (should still be recognized)
    GoldenCase::top(
        "550e8400e29b41d4a716446655440000",
        "uuid",
        "UUID without dashes",
    ),
];

// =============================================================================
// Golden Corpus: Hex Strings
// =============================================================================

const HEX_CASES: &[GoldenCase] = &[
    // With prefix - but 8-char hex with 0x prefix looks like color #RRGGBBAA
    // This is debatable, but color-hex wins for now
    GoldenCase::present("0xDEADBEEF", "hex", "Hex with 0x prefix", 0.90),
    GoldenCase::present("0xCAFEBABE", "hex", "Java class magic with prefix", 0.90),
    GoldenCase::top("0xFF", "hex", "Single byte with prefix"),
    GoldenCase::top("0xFFFF", "hex", "Two bytes with prefix"),
    // Without prefix (should still prefer hex over base64/hash for magic numbers)
    GoldenCase::top("DEADBEEF", "hex", "Debug marker (no prefix)"),
    GoldenCase::top("CAFEBABE", "hex", "Java class magic (no prefix)"),
    GoldenCase::top("FEEDFACE", "hex", "Mach-O magic (no prefix)"),
    GoldenCase::top("BAADF00D", "hex", "Debug marker (no prefix)"),
    // Longer hex strings
    GoldenCase::top("48656c6c6f", "hex", "Hello in hex"),
    // 16 bytes looks like UUID - that's actually correct behavior
    GoldenCase::present(
        "00112233445566778899AABBCCDDEEFF",
        "hex",
        "16 bytes hex (uuid wins)",
        0.90,
    ),
];

// =============================================================================
// Golden Corpus: Base64
// =============================================================================

const BASE64_CASES: &[GoldenCase] = &[
    GoldenCase::top("SGVsbG8gV29ybGQ=", "base64", "Hello World"),
    GoldenCase::top("dGVzdA==", "base64", "test with padding"),
    GoldenCase::top("YWJj", "base64", "abc no padding"),
    GoldenCase::top(
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
        "base64",
        "JWT header-like",
    ),
    // Should NOT match pure hex as base64
    // (these should be hex, not base64)
];

// =============================================================================
// Golden Corpus: Colors
// =============================================================================

const COLOR_CASES: &[GoldenCase] = &[
    // Hex colors (require # prefix)
    GoldenCase::top("#FF5500", "color-hex", "Orange hex color"),
    GoldenCase::top("#000000", "color-hex", "Black"),
    GoldenCase::top("#FFFFFF", "color-hex", "White"),
    GoldenCase::top("#F00", "color-hex", "Short hex red"),
    GoldenCase::top("#ff5500", "color-hex", "Lowercase hex color"),
    GoldenCase::top("#DEADBE", "color-hex", "Hex color (not magic number)"),
    // RGB functions
    GoldenCase::top("rgb(255, 128, 0)", "color-rgb", "RGB function"),
    GoldenCase::top("rgba(255, 128, 0, 0.5)", "color-rgb", "RGBA function"),
    // HSL functions
    GoldenCase::top("hsl(120, 100%, 50%)", "color-hsl", "HSL green"),
    GoldenCase::top("hsla(120, 100%, 50%, 0.5)", "color-hsl", "HSLA green"),
];

// =============================================================================
// Golden Corpus: Dates and Times
// =============================================================================

const DATETIME_CASES: &[GoldenCase] = &[
    GoldenCase::top("2024-01-15T10:30:00Z", "datetime", "ISO 8601 / RFC 3339"),
    // Date-only isn't currently parsed as datetime, just text/nanoid
    // This could be added as a feature later
    GoldenCase::top(
        "2024-01-15T10:30:00+05:30",
        "datetime",
        "ISO with timezone offset",
    ),
    GoldenCase::top(
        "2024-01-15T10:30:00.123Z",
        "datetime",
        "ISO with milliseconds",
    ),
];

// =============================================================================
// Golden Corpus: Natural Language Dates
// =============================================================================

const NATURAL_DATE_CASES: &[GoldenCase] = &[
    // Time of day - should beat hex for "15:00"
    GoldenCase::top("15:00", "natural-date", "24-hour time"),
    GoldenCase::top("09:30", "natural-date", "Morning time"),
    GoldenCase::top("3:30pm", "natural-date", "12-hour with pm"),
    GoldenCase::top("9am", "natural-date", "Hour with am"),
    GoldenCase::top("12:00:00", "natural-date", "Noon with seconds"),
    // Relative words
    GoldenCase::top("now", "natural-date", "Current time"),
    GoldenCase::top("today", "natural-date", "Today"),
    GoldenCase::top("tomorrow", "natural-date", "Tomorrow"),
    GoldenCase::top("yesterday", "natural-date", "Yesterday"),
    // Relative periods
    GoldenCase::top("next week", "natural-date", "Next week"),
    GoldenCase::top("last month", "natural-date", "Last month"),
    GoldenCase::top("next year", "natural-date", "Next year"),
    // Weekdays
    GoldenCase::top("monday", "natural-date", "Weekday name"),
    GoldenCase::top("next friday", "natural-date", "Next weekday"),
    GoldenCase::top("last tuesday", "natural-date", "Last weekday"),
    // Relative offsets
    GoldenCase::top("in 2 days", "natural-date", "In N days"),
    GoldenCase::top("3 weeks ago", "natural-date", "N weeks ago"),
    GoldenCase::top("a month from now", "natural-date", "A month from now"),
    // Month + day
    GoldenCase::top("dec 15", "natural-date", "Month day (short)"),
    GoldenCase::top("15 december", "natural-date", "Day month (long)"),
    GoldenCase::top("march 15th", "natural-date", "Month day with ordinal"),
    // Special dates
    GoldenCase::top("christmas", "natural-date", "Christmas"),
    GoldenCase::top("halloween", "natural-date", "Halloween"),
    GoldenCase::top("new years", "natural-date", "New Year's"),
    // Period boundaries
    GoldenCase::top("end of month", "natural-date", "End of month"),
    GoldenCase::top("eom", "natural-date", "EOM abbreviation"),
    GoldenCase::top("start of year", "natural-date", "Start of year"),
    // Quarters
    GoldenCase::top("q1", "natural-date", "Quarter 1"),
    GoldenCase::top("q2", "natural-date", "Quarter 2"),
    GoldenCase::top("next quarter", "natural-date", "Next quarter"),
];

// =============================================================================
// Golden Corpus: ISBNs (formatted should win, unformatted may not)
// =============================================================================

const ISBN_CASES: &[GoldenCase] = &[
    // Formatted ISBNs should always be recognized
    GoldenCase::top("0-306-40615-2", "isbn-10", "ISBN-10 with hyphens"),
    GoldenCase::top("978-0-306-40615-7", "isbn-13", "ISBN-13 with hyphens"),
    GoldenCase::top("0 306 40615 2", "isbn-10", "ISBN-10 with spaces"),
    // X check digit is strong signal
    GoldenCase::top("155860832X", "isbn-10", "ISBN-10 with X checkdigit"),
    GoldenCase::top("080442957X", "isbn-10", "ISBN-10 with X checkdigit 2"),
];

// =============================================================================
// Golden Corpus: JSON
// =============================================================================

const JSON_CASES: &[GoldenCase] = &[
    GoldenCase::top(r#"{"key": "value"}"#, "json", "Simple object"),
    GoldenCase::top(r#"[1, 2, 3]"#, "json", "Simple array"),
    GoldenCase::top(r#"{"nested": {"a": 1}}"#, "json", "Nested object"),
    // Note: bare true/false/null aren't parsed as JSON currently
    // (they're just keywords/text). Only objects/arrays are JSON.
];

// =============================================================================
// Golden Corpus: Plain Text (should NOT be misinterpreted)
// =============================================================================

const TEXT_CASES: &[GoldenCase] = &[
    // Text has low confidence (10%) by design - it's the fallback
    // We just verify it's present, not that it's top
    GoldenCase::present("hello world", "text", "Simple text", 0.10),
    GoldenCase::present("The quick brown fox", "text", "English sentence", 0.10),
    // Note: Some identifiers match other formats (nanoid, geohash, coords)
    // This is expected - we're testing text is at least recognized
    GoldenCase::present("camelCaseIdentifier", "text", "Camel case identifier", 0.10),
    GoldenCase::present("README", "text", "Uppercase word", 0.10),
];

// =============================================================================
// Golden Corpus: Numbers / Integers
// =============================================================================

const NUMBER_CASES: &[GoldenCase] = &[
    GoldenCase::top("42", "decimal", "Small integer"),
    GoldenCase::top("12345", "decimal", "Medium integer"),
    GoldenCase::top("-42", "decimal", "Negative integer"),
    // Note: floats aren't currently parsed as decimal
    GoldenCase::top("1000000", "decimal", "Million"),
    // Binary
    GoldenCase::top("0b11111111", "binary", "Binary with prefix"),
    GoldenCase::top("0b1010", "binary", "Binary 10"),
    // Octal - permissions format wins for 755/644 which is correct
    GoldenCase::present("0o755", "octal", "Octal permissions", 0.90),
    GoldenCase::present("0o644", "octal", "Octal file mode", 0.90),
];

// =============================================================================
// Golden Corpus: Durations
// =============================================================================

const DURATION_CASES: &[GoldenCase] = &[
    // Duration should be TOP for all these - geohash may also match but lower confidence
    GoldenCase::top("2h30m", "duration", "Hours and minutes"),
    GoldenCase::top("1d12h", "duration", "Day and hours"),
    GoldenCase::top("500ms", "duration", "Milliseconds"),
    GoldenCase::top("1h30m45s", "duration", "H:M:S format"),
    // ISO 8601 duration - very specific format = 95% confidence
    GoldenCase::top("PT2H30M", "duration", "ISO 8601 duration"),
    GoldenCase::top("P5D", "duration", "ISO 8601 5 days"),
];

// =============================================================================
// Golden Corpus: Data Sizes
// =============================================================================

const DATASIZE_CASES: &[GoldenCase] = &[
    GoldenCase::top("1024 bytes", "datasize", "Bytes"),
    GoldenCase::top("1.5 GB", "datasize", "Gigabytes"),
    GoldenCase::top("500 MB", "datasize", "Megabytes"),
    GoldenCase::top("4 KiB", "datasize", "Kibibytes (IEC)"),
    GoldenCase::top("1 TB", "datasize", "Terabytes"),
];

// =============================================================================
// Golden Corpus: Coordinates
// =============================================================================

const COORDS_CASES: &[GoldenCase] = &[
    // Note: Geohash is no longer parsed as input (too many false positives with words)
    // It's still available as conversion output from other coord formats
    // Decimal degrees
    GoldenCase::top("40.7128, -74.0060", "coords", "NYC coordinates"),
    GoldenCase::top("51.5074, -0.1278", "coords", "London coordinates"),
    // Plus codes
    GoldenCase::top("87G8P27Q+VF", "coords", "Plus code"),
];

// =============================================================================
// Golden Corpus: Units
// =============================================================================

const UNIT_CASES: &[GoldenCase] = &[
    // Length - "5m" conflicts with duration (5 minutes)
    GoldenCase::present("5m", "length", "5 meters", 0.80),
    GoldenCase::top("100km", "length", "100 kilometers"),
    GoldenCase::top("6ft", "length", "6 feet"),
    GoldenCase::top("2.5in", "length", "2.5 inches"),
    // Temperature - "100C" looks like hex (100C = 0x100C bytes)
    GoldenCase::present("100C", "temperature", "100 Celsius", 0.80),
    GoldenCase::top("72F", "temperature", "72 Fahrenheit"),
    GoldenCase::top("273K", "temperature", "273 Kelvin"),
    GoldenCase::top("-40C", "temperature", "Negative Celsius"),
    // Weight
    GoldenCase::top("5kg", "weight", "5 kilograms"),
    GoldenCase::top("100lb", "weight", "100 pounds"),
];

// =============================================================================
// Golden Corpus: Hashes (by length)
// =============================================================================

const HASH_CASES: &[GoldenCase] = &[
    // MD5 (32 hex chars)
    GoldenCase::present(
        "d41d8cd98f00b204e9800998ecf8427e",
        "hash",
        "MD5 of empty string",
        0.5,
    ),
    // SHA-1 (40 hex chars)
    GoldenCase::present(
        "da39a3ee5e6b4b0d3255bfef95601890afd80709",
        "hash",
        "SHA-1 of empty string",
        0.5,
    ),
    // SHA-256 (64 hex chars)
    GoldenCase::present(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "hash",
        "SHA-256 of empty string",
        0.5,
    ),
];

// =============================================================================
// Golden Corpus: CIDR Notation
// =============================================================================

const CIDR_CASES: &[GoldenCase] = &[
    GoldenCase::top("192.168.1.0/24", "cidr", "Class C network"),
    GoldenCase::top("10.0.0.0/8", "cidr", "Class A private"),
    GoldenCase::top("172.16.0.0/12", "cidr", "Class B private"),
    GoldenCase::top("0.0.0.0/0", "cidr", "Default route"),
];

// =============================================================================
// Golden Corpus: Expressions
// =============================================================================

const EXPR_CASES: &[GoldenCase] = &[
    GoldenCase::top("2 + 2", "expr", "Simple addition"),
    GoldenCase::top("10 * 5", "expr", "Multiplication"),
    GoldenCase::top("100 / 4", "expr", "Division"),
    // Note: ** power operator may not be supported
    GoldenCase::top("2 ^ 8", "expr", "XOR/Power"),
    GoldenCase::top("0xFF + 1", "expr", "Hex in expression"),
    GoldenCase::top("1 << 8", "expr", "Bit shift"),
];

// =============================================================================
// Golden Corpus: Permissions
// =============================================================================

const PERMISSION_CASES: &[GoldenCase] = &[
    GoldenCase::top("755", "permissions", "rwxr-xr-x"),
    GoldenCase::top("644", "permissions", "rw-r--r--"),
    GoldenCase::top("777", "permissions", "rwxrwxrwx"),
    GoldenCase::top("0755", "permissions", "With leading zero"),
    GoldenCase::top("rwxr-xr-x", "permissions", "Symbolic notation"),
    GoldenCase::top("-rw-r--r--", "permissions", "ls -l style"),
];

// =============================================================================
// Golden Corpus: URLs / URL Encoding
// =============================================================================

const URL_CASES: &[GoldenCase] = &[
    GoldenCase::top("Hello%20World", "url-encoded", "Space encoded"),
    GoldenCase::top("%E2%9C%93", "url-encoded", "Checkmark encoded"),
    GoldenCase::top(
        "foo%3Dbar%26baz%3Dqux",
        "url-encoded",
        "Query params encoded",
    ),
];

// =============================================================================
// Golden Corpus: Currency
// =============================================================================

const CURRENCY_CASES: &[GoldenCase] = &[
    GoldenCase::top("$100", "currency", "USD with symbol"),
    GoldenCase::top("€50", "currency", "EUR with symbol"),
    GoldenCase::top("£75", "currency", "GBP with symbol"),
    GoldenCase::top("100 USD", "currency", "USD with code"),
    GoldenCase::top("50 EUR", "currency", "EUR with code"),
];

// =============================================================================
// Golden Corpus: JWT
// =============================================================================

const JWT_CASES: &[GoldenCase] = &[
    GoldenCase::top(
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        "jwt",
        "Standard JWT"
    ),
];

// =============================================================================
// Adversarial Cases: Inputs that could confuse multiple formats
// =============================================================================

const ADVERSARIAL_CASES: &[GoldenCase] = &[
    // 10-digit numbers that look like epochs but also pass ISBN checksum
    GoldenCase::top(
        "1704067200",
        "epoch-seconds",
        "Epoch that passes ISBN checksum",
    ),
    // 8-char hex that could be CRC32 (but we don't detect CRC32 anymore)
    GoldenCase::top("DEADBEEF", "hex", "8-char hex (not CRC32)"),
    // Pure numeric geohash-valid strings - epoch should be present
    GoldenCase::present(
        "1234567890",
        "epoch-seconds",
        "10 digits - epoch not geohash",
        0.5,
    ),
    // Short hex that could be color without #
    GoldenCase::top("CAFE", "hex", "4-char hex (not color without #)"),
    // Word-like strings: geohash no longer parsed as input, so text wins
    GoldenCase::present("rustfmt", "text", "Tool name (geohash removed)", 0.10),
    GoldenCase::present("prettier", "text", "Tool name (text)", 0.10),
    // Numbers that could be many things
    GoldenCase::top("65535", "decimal", "Max uint16 (not port primarily)"),
];

// =============================================================================
// Test Runner
// =============================================================================

fn run_golden_tests(cases: &[GoldenCase], category: &str) {
    let forb = Formatorbit::new();
    let mut failures = Vec::new();

    for case in cases {
        let results = forb.convert_all(case.input);

        if results.is_empty() {
            failures.push(format!(
                "[{}] '{}' ({}): No interpretations found, expected {}",
                category, case.input, case.description, case.expected_format
            ));
            continue;
        }

        // Find the expected format in results
        let expected_result = results
            .iter()
            .find(|r| r.interpretation.source_format == case.expected_format);

        match expected_result {
            None => {
                let found: Vec<_> = results
                    .iter()
                    .map(|r| {
                        format!(
                            "{}({:.0}%)",
                            r.interpretation.source_format,
                            r.interpretation.confidence * 100.0
                        )
                    })
                    .collect();
                failures.push(format!(
                    "[{}] '{}' ({}): Expected {} not found. Found: {:?}",
                    category, case.input, case.description, case.expected_format, found
                ));
            }
            Some(result) => {
                let confidence = result.interpretation.confidence;

                // Check minimum confidence
                if confidence < case.min_confidence {
                    failures.push(format!(
                        "[{}] '{}' ({}): {} confidence {:.0}% < required {:.0}%",
                        category,
                        case.input,
                        case.description,
                        case.expected_format,
                        confidence * 100.0,
                        case.min_confidence * 100.0
                    ));
                }

                // Check if must be top result
                if case.must_be_top {
                    let top = &results[0];
                    if top.interpretation.source_format != case.expected_format {
                        failures.push(format!(
                            "[{}] '{}' ({}): Expected {} to be top, but {} ({:.0}%) is top. {} is at {:.0}%",
                            category,
                            case.input,
                            case.description,
                            case.expected_format,
                            top.interpretation.source_format,
                            top.interpretation.confidence * 100.0,
                            case.expected_format,
                            confidence * 100.0
                        ));
                    }
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} golden corpus failures:\n\n{}\n",
            failures.len(),
            failures.join("\n\n")
        );
    }
}

// =============================================================================
// Individual Test Functions
// =============================================================================

#[test]
fn test_golden_epochs() {
    run_golden_tests(EPOCH_CASES, "Epochs");
}

#[test]
fn test_golden_ips() {
    run_golden_tests(IP_CASES, "IPs");
}

#[test]
fn test_golden_uuids() {
    run_golden_tests(UUID_CASES, "UUIDs");
}

#[test]
fn test_golden_hex() {
    run_golden_tests(HEX_CASES, "Hex");
}

#[test]
fn test_golden_base64() {
    run_golden_tests(BASE64_CASES, "Base64");
}

#[test]
fn test_golden_colors() {
    run_golden_tests(COLOR_CASES, "Colors");
}

#[test]
fn test_golden_datetime() {
    run_golden_tests(DATETIME_CASES, "DateTime");
}

#[test]
fn test_golden_natural_dates() {
    run_golden_tests(NATURAL_DATE_CASES, "NaturalDate");
}

#[test]
fn test_golden_isbn() {
    run_golden_tests(ISBN_CASES, "ISBN");
}

#[test]
fn test_golden_json() {
    run_golden_tests(JSON_CASES, "JSON");
}

#[test]
fn test_golden_text() {
    run_golden_tests(TEXT_CASES, "Text");
}

#[test]
fn test_golden_numbers() {
    run_golden_tests(NUMBER_CASES, "Numbers");
}

#[test]
fn test_golden_durations() {
    run_golden_tests(DURATION_CASES, "Durations");
}

#[test]
fn test_golden_datasize() {
    run_golden_tests(DATASIZE_CASES, "DataSize");
}

#[test]
fn test_golden_coords() {
    run_golden_tests(COORDS_CASES, "Coords");
}

#[test]
fn test_golden_units() {
    run_golden_tests(UNIT_CASES, "Units");
}

#[test]
fn test_golden_hashes() {
    run_golden_tests(HASH_CASES, "Hashes");
}

#[test]
fn test_golden_cidr() {
    run_golden_tests(CIDR_CASES, "CIDR");
}

#[test]
fn test_golden_expressions() {
    run_golden_tests(EXPR_CASES, "Expressions");
}

#[test]
fn test_golden_permissions() {
    run_golden_tests(PERMISSION_CASES, "Permissions");
}

#[test]
fn test_golden_urls() {
    run_golden_tests(URL_CASES, "URLs");
}

#[test]
fn test_golden_currency() {
    run_golden_tests(CURRENCY_CASES, "Currency");
}

#[test]
fn test_golden_jwt() {
    run_golden_tests(JWT_CASES, "JWT");
}

#[test]
fn test_golden_adversarial() {
    run_golden_tests(ADVERSARIAL_CASES, "Adversarial");
}

// =============================================================================
// Summary Test (runs all and reports)
// =============================================================================

#[test]
fn test_golden_corpus_summary() {
    let all_cases: &[(&str, &[GoldenCase])] = &[
        ("Epochs", EPOCH_CASES),
        ("IPs", IP_CASES),
        ("UUIDs", UUID_CASES),
        ("Hex", HEX_CASES),
        ("Base64", BASE64_CASES),
        ("Colors", COLOR_CASES),
        ("DateTime", DATETIME_CASES),
        ("ISBN", ISBN_CASES),
        ("JSON", JSON_CASES),
        ("Text", TEXT_CASES),
        ("Numbers", NUMBER_CASES),
        ("Durations", DURATION_CASES),
        ("DataSize", DATASIZE_CASES),
        ("Coords", COORDS_CASES),
        ("Units", UNIT_CASES),
        ("Hashes", HASH_CASES),
        ("CIDR", CIDR_CASES),
        ("Expressions", EXPR_CASES),
        ("Permissions", PERMISSION_CASES),
        ("URLs", URL_CASES),
        ("Currency", CURRENCY_CASES),
        ("JWT", JWT_CASES),
        ("Adversarial", ADVERSARIAL_CASES),
    ];

    let total: usize = all_cases.iter().map(|(_, cases)| cases.len()).sum();
    eprintln!(
        "\nGolden corpus: {} test cases across {} categories",
        total,
        all_cases.len()
    );
}
