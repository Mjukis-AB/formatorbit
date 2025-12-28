//! Snapshot tests for format outputs.
//!
//! These tests ensure that format parsing and conversion outputs remain stable.
//! If a change is intentional, update snapshots with: `cargo insta review`

use formatorbit_core::{ConversionResult, Formatorbit};
use insta::{assert_json_snapshot, with_settings};

/// Helper to get the first interpretation for a given format.
fn first_result_for_format<'a>(
    results: &'a [ConversionResult],
    format: &str,
) -> Option<&'a ConversionResult> {
    results
        .iter()
        .find(|r| r.interpretation.source_format == format)
}

/// Snapshot settings that redact volatile fields.
macro_rules! stable_snapshot {
    ($name:expr, $value:expr) => {
        with_settings!({
            // Sort maps for deterministic output
            sort_maps => true,
        }, {
            assert_json_snapshot!($name, $value);
        });
    };
}

// =============================================================================
// IP Address Tests
// =============================================================================

#[test]
fn test_snapshot_ipv4_private() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("192.168.1.1");
    let result = first_result_for_format(&results, "ipv4").unwrap();

    stable_snapshot!("ipv4_private_interpretation", &result.interpretation);
}

#[test]
fn test_snapshot_ipv4_public() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("8.8.8.8");
    let result = first_result_for_format(&results, "ipv4").unwrap();

    stable_snapshot!("ipv4_public_interpretation", &result.interpretation);
}

#[test]
fn test_snapshot_ipv4_loopback() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("127.0.0.1");
    let result = first_result_for_format(&results, "ipv4").unwrap();

    stable_snapshot!("ipv4_loopback_interpretation", &result.interpretation);
}

#[test]
fn test_snapshot_ipv6_loopback() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("::1");
    let result = first_result_for_format(&results, "ipv6").unwrap();

    stable_snapshot!("ipv6_loopback_interpretation", &result.interpretation);
}

// =============================================================================
// Color Tests
// =============================================================================

#[test]
fn test_snapshot_color_hex() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("#FF5500");
    let result = first_result_for_format(&results, "color-hex").unwrap();

    stable_snapshot!("color_hex_interpretation", &result.interpretation);
    stable_snapshot!("color_hex_conversions", &result.conversions);
}

#[test]
fn test_snapshot_color_rgb() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("rgb(255, 128, 0)");
    let result = first_result_for_format(&results, "color-rgb").unwrap();

    stable_snapshot!("color_rgb_interpretation", &result.interpretation);
}

// =============================================================================
// UUID Tests
// =============================================================================

#[test]
fn test_snapshot_uuid() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("550e8400-e29b-41d4-a716-446655440000");
    let result = first_result_for_format(&results, "uuid").unwrap();

    stable_snapshot!("uuid_interpretation", &result.interpretation);
}

// =============================================================================
// Hex Tests
// =============================================================================

#[test]
fn test_snapshot_hex_with_prefix() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("0xDEADBEEF");
    let result = first_result_for_format(&results, "hex").unwrap();

    stable_snapshot!("hex_with_prefix_interpretation", &result.interpretation);
}

#[test]
fn test_snapshot_hex_bytes() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("48656c6c6f");
    let result = first_result_for_format(&results, "hex").unwrap();

    stable_snapshot!("hex_bytes_interpretation", &result.interpretation);
    // Check some key conversions
    let utf8_conv = result
        .conversions
        .iter()
        .find(|c| c.target_format == "utf8");
    stable_snapshot!("hex_to_utf8_conversion", utf8_conv);
}

// =============================================================================
// Base64 Tests
// =============================================================================

#[test]
fn test_snapshot_base64() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("SGVsbG8gV29ybGQ=");
    let result = first_result_for_format(&results, "base64").unwrap();

    stable_snapshot!("base64_interpretation", &result.interpretation);
}

// =============================================================================
// DateTime/Epoch Tests
// =============================================================================

#[test]
fn test_snapshot_datetime_rfc3339() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("2025-01-15T10:30:00Z");
    let result = first_result_for_format(&results, "datetime").unwrap();

    stable_snapshot!("datetime_rfc3339_interpretation", &result.interpretation);
    stable_snapshot!("datetime_rfc3339_conversions", &result.conversions);
}

// =============================================================================
// Unit Conversion Tests
// =============================================================================

#[test]
fn test_snapshot_length_meters() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("5m");
    let result = first_result_for_format(&results, "length").unwrap();

    stable_snapshot!("length_meters_interpretation", &result.interpretation);
    stable_snapshot!("length_meters_conversions", &result.conversions);
}

#[test]
fn test_snapshot_temperature_celsius() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("100C");
    let result = first_result_for_format(&results, "temperature").unwrap();

    stable_snapshot!("temperature_celsius_interpretation", &result.interpretation);
    stable_snapshot!("temperature_celsius_conversions", &result.conversions);
}

#[test]
fn test_snapshot_datasize() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("1024 bytes");
    let result = first_result_for_format(&results, "datasize").unwrap();

    stable_snapshot!("datasize_interpretation", &result.interpretation);
    stable_snapshot!("datasize_conversions", &result.conversions);
}

// =============================================================================
// Duration Tests
// =============================================================================

#[test]
fn test_snapshot_duration() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("2h30m");
    let result = first_result_for_format(&results, "duration").unwrap();

    // Duration description includes current timestamp, so we redact it
    assert_json_snapshot!("duration_interpretation", &result.interpretation, {
        ".description" => "[description with dynamic timestamp]"
    });
    assert_json_snapshot!("duration_conversions", &result.conversions, {
        "[].display" => insta::dynamic_redaction(|value, _path| {
            // Redact any display that contains a timestamp
            if let Some(s) = value.as_str() {
                if s.contains("T") && s.contains("Z") {
                    return insta::internals::Content::String("[dynamic timestamp]".into());
                }
            }
            value.clone()
        })
    });
}

// =============================================================================
// JSON Tests
// =============================================================================

#[test]
fn test_snapshot_json_object() {
    let forb = Formatorbit::new();
    let results = forb.convert_all(r#"{"name": "test", "value": 42}"#);
    let result = first_result_for_format(&results, "json").unwrap();

    stable_snapshot!("json_object_interpretation", &result.interpretation);
}

// =============================================================================
// Hash Tests
// =============================================================================

#[test]
fn test_snapshot_sha256() {
    let forb = Formatorbit::new();
    // SHA-256 of "hello"
    let results =
        forb.convert_all("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    let result = first_result_for_format(&results, "hash").unwrap();

    stable_snapshot!("sha256_interpretation", &result.interpretation);
}

// =============================================================================
// Expression Tests
// =============================================================================

#[test]
fn test_snapshot_expression() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("2 + 2");
    let result = first_result_for_format(&results, "expr").unwrap();

    stable_snapshot!("expression_interpretation", &result.interpretation);
    stable_snapshot!("expression_conversions", &result.conversions);
}

// =============================================================================
// Binary/Octal Tests
// =============================================================================

#[test]
fn test_snapshot_binary() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("0b11111111");
    let result = first_result_for_format(&results, "binary").unwrap();

    stable_snapshot!("binary_interpretation", &result.interpretation);
}

#[test]
fn test_snapshot_octal() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("0o755");
    let result = first_result_for_format(&results, "octal").unwrap();

    stable_snapshot!("octal_interpretation", &result.interpretation);
}

// =============================================================================
// Currency Tests
// =============================================================================

#[test]
fn test_snapshot_currency() {
    let forb = Formatorbit::new();
    let results = forb.convert_all("$100");
    let result = first_result_for_format(&results, "currency").unwrap();

    stable_snapshot!("currency_interpretation", &result.interpretation);
}
