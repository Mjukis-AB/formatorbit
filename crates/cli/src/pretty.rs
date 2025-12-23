//! Pretty-printing for structured data with jq-style colors.
//!
//! Colors follow jq conventions:
//! - Strings: green
//! - Numbers: cyan (jq uses no color, but cyan is nice)
//! - Booleans: yellow
//! - Null: bright black (dimmed)
//! - Keys: blue
//! - Punctuation: white/default

use colored::{Color, Colorize};
use formatorbit_core::{ProtoField, ProtoValue};

/// Configuration for pretty printing.
#[derive(Debug, Clone, Copy)]
pub struct PrettyConfig {
    /// Enable colored output.
    pub color: bool,
    /// Indent string (usually 2 spaces).
    pub indent: &'static str,
    /// Compact mode (single line, no extra whitespace).
    pub compact: bool,
}

impl Default for PrettyConfig {
    fn default() -> Self {
        Self {
            color: true,
            indent: "  ",
            compact: false,
        }
    }
}

/// Pretty-print a JSON value with colors.
pub fn pretty_json(value: &serde_json::Value, config: &PrettyConfig) -> String {
    let mut output = String::new();
    format_json_value(value, config, 0, &mut output);
    output
}

fn format_json_value(
    value: &serde_json::Value,
    config: &PrettyConfig,
    depth: usize,
    output: &mut String,
) {
    match value {
        serde_json::Value::Null => {
            output.push_str(&colorize("null", Color::BrightBlack, config.color));
        }
        serde_json::Value::Bool(b) => {
            let s = if *b { "true" } else { "false" };
            output.push_str(&colorize(s, Color::Yellow, config.color));
        }
        serde_json::Value::Number(n) => {
            output.push_str(&colorize(&n.to_string(), Color::Cyan, config.color));
        }
        serde_json::Value::String(s) => {
            let escaped = escape_json_string(s);
            output.push_str(&colorize(
                &format!("\"{}\"", escaped),
                Color::Green,
                config.color,
            ));
        }
        serde_json::Value::Array(arr) => {
            format_json_array(arr, config, depth, output);
        }
        serde_json::Value::Object(obj) => {
            format_json_object(obj, config, depth, output);
        }
    }
}

fn format_json_array(
    arr: &[serde_json::Value],
    config: &PrettyConfig,
    depth: usize,
    output: &mut String,
) {
    if arr.is_empty() {
        output.push_str("[]");
        return;
    }

    // Check if array is simple (all primitives, short total length)
    let is_simple = arr.iter().all(|v| {
        matches!(
            v,
            serde_json::Value::Null
                | serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_)
        )
    });

    let inline = config.compact || (is_simple && arr.len() <= 4 && estimate_array_len(arr) < 60);

    if inline {
        output.push('[');
        for (i, item) in arr.iter().enumerate() {
            if i > 0 {
                output.push_str(", ");
            }
            format_json_value(item, config, depth + 1, output);
        }
        output.push(']');
    } else {
        output.push_str("[\n");
        for (i, item) in arr.iter().enumerate() {
            output.push_str(&config.indent.repeat(depth + 1));
            format_json_value(item, config, depth + 1, output);
            if i < arr.len() - 1 {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str(&config.indent.repeat(depth));
        output.push(']');
    }
}

fn format_json_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    config: &PrettyConfig,
    depth: usize,
    output: &mut String,
) {
    if obj.is_empty() {
        output.push_str("{}");
        return;
    }

    if config.compact {
        output.push('{');
        for (i, (key, value)) in obj.iter().enumerate() {
            if i > 0 {
                output.push_str(", ");
            }
            output.push_str(&colorize(
                &format!("\"{}\"", key),
                Color::Blue,
                config.color,
            ));
            output.push_str(": ");
            format_json_value(value, config, depth + 1, output);
        }
        output.push('}');
    } else {
        output.push_str("{\n");
        let entries: Vec<_> = obj.iter().collect();
        for (i, (key, value)) in entries.iter().enumerate() {
            output.push_str(&config.indent.repeat(depth + 1));
            output.push_str(&colorize(
                &format!("\"{}\"", key),
                Color::Blue,
                config.color,
            ));
            output.push_str(": ");
            format_json_value(value, config, depth + 1, output);
            if i < entries.len() - 1 {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str(&config.indent.repeat(depth));
        output.push('}');
    }
}

fn estimate_array_len(arr: &[serde_json::Value]) -> usize {
    arr.iter()
        .map(|v| match v {
            serde_json::Value::Null => 4,
            serde_json::Value::Bool(b) => {
                if *b {
                    4
                } else {
                    5
                }
            }
            serde_json::Value::Number(n) => n.to_string().len(),
            serde_json::Value::String(s) => s.len() + 2,
            _ => 20, // Nested structures get high estimate
        })
        .sum::<usize>()
        + arr.len() * 2 // commas and spaces
}

fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

fn colorize(s: &str, color: Color, enabled: bool) -> String {
    if enabled {
        s.color(color).to_string()
    } else {
        s.to_string()
    }
}

/// Pretty-print a protobuf message with colors.
pub fn pretty_protobuf(fields: &[ProtoField], config: &PrettyConfig) -> String {
    let mut output = String::new();
    format_proto_fields(fields, config, 0, &mut output);
    output
}

fn format_proto_fields(
    fields: &[ProtoField],
    config: &PrettyConfig,
    depth: usize,
    output: &mut String,
) {
    if config.compact {
        output.push('{');
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                output.push_str(", ");
            }
            output.push_str(&colorize(
                &field.field_number.to_string(),
                Color::Blue,
                config.color,
            ));
            output.push_str(": ");
            format_proto_value(&field.value, config, depth + 1, output);
            output.push_str(&colorize(
                &format!(" [{}]", wire_type_name(field.wire_type)),
                Color::BrightBlack,
                config.color,
            ));
        }
        output.push('}');
    } else {
        output.push_str("{\n");
        for (i, field) in fields.iter().enumerate() {
            output.push_str(&config.indent.repeat(depth + 1));
            output.push_str(&colorize(
                &field.field_number.to_string(),
                Color::Blue,
                config.color,
            ));
            output.push_str(": ");
            format_proto_value(&field.value, config, depth + 1, output);
            output.push_str(&colorize(
                &format!(" [{}]", wire_type_name(field.wire_type)),
                Color::BrightBlack,
                config.color,
            ));
            if i < fields.len() - 1 {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str(&config.indent.repeat(depth));
        output.push('}');
    }
}

fn format_proto_value(
    value: &ProtoValue,
    config: &PrettyConfig,
    depth: usize,
    output: &mut String,
) {
    match value {
        ProtoValue::Varint(v) => {
            output.push_str(&colorize(&v.to_string(), Color::Cyan, config.color));
            // Show bool hint for 0/1
            if *v <= 1 {
                let bool_str = if *v != 0 { "true" } else { "false" };
                output.push_str(&colorize(
                    &format!(" ({})", bool_str),
                    Color::Yellow,
                    config.color,
                ));
            } else {
                // Show signed interpretation if it would be smaller
                let signed = decode_zigzag(*v);
                if signed.abs() < (*v as i64).abs() / 2 {
                    output.push_str(&colorize(
                        &format!(" (signed: {})", signed),
                        Color::BrightBlack,
                        config.color,
                    ));
                }
            }
        }
        ProtoValue::Fixed64(v) => {
            output.push_str(&colorize(&v.to_string(), Color::Cyan, config.color));
            // Show double hint if reasonable
            let as_double = f64::from_bits(*v);
            if as_double.is_finite() && as_double.abs() > 1e-100 && as_double.abs() < 1e100 {
                output.push_str(&colorize(
                    &format!(" (double: {})", as_double),
                    Color::BrightBlack,
                    config.color,
                ));
            }
        }
        ProtoValue::Fixed32(v) => {
            output.push_str(&colorize(&v.to_string(), Color::Cyan, config.color));
            // Show float hint if reasonable
            let as_float = f32::from_bits(*v);
            if as_float.is_finite() && as_float.abs() > 1e-30 && as_float.abs() < 1e30 {
                output.push_str(&colorize(
                    &format!(" (float: {})", as_float),
                    Color::BrightBlack,
                    config.color,
                ));
            }
        }
        ProtoValue::String(s) => {
            output.push_str(&colorize(&format!("\"{}\"", s), Color::Green, config.color));
        }
        ProtoValue::Bytes(data) => {
            if data.len() <= 32 {
                let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
                output.push_str(&colorize(
                    &format!("<{}>", hex),
                    Color::Magenta,
                    config.color,
                ));
            } else {
                output.push_str(&colorize(
                    &format!("<{} bytes>", data.len()),
                    Color::Magenta,
                    config.color,
                ));
            }
        }
        ProtoValue::Message(fields) => {
            format_proto_fields(fields, config, depth, output);
        }
    }
}

fn wire_type_name(wire_type: u8) -> &'static str {
    match wire_type {
        0 => "varint",
        1 => "i64",
        2 => "len",
        5 => "i32",
        _ => "?",
    }
}

/// Decode zigzag-encoded signed integer.
fn decode_zigzag(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 1) as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn no_color_config() -> PrettyConfig {
        PrettyConfig {
            color: false,
            ..Default::default()
        }
    }

    fn compact_config() -> PrettyConfig {
        PrettyConfig {
            compact: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_pretty_json_simple() {
        let config = no_color_config();
        let value = json!({"name": "John", "age": 30});
        let output = pretty_json(&value, &config);
        assert!(output.contains("\"name\""));
        assert!(output.contains("\"John\""));
        assert!(output.contains("30"));
    }

    #[test]
    fn test_pretty_json_nested() {
        let config = no_color_config();
        let value = json!({
            "user": {
                "name": "John",
                "tags": ["admin", "user"]
            }
        });
        let output = pretty_json(&value, &config);
        assert!(output.contains("  \"user\""));
        assert!(output.contains("    \"name\""));
    }

    #[test]
    fn test_pretty_json_compact() {
        let config = compact_config();
        let value = json!({"name": "John", "age": 30});
        let output = pretty_json(&value, &config);
        assert!(!output.contains('\n'));
    }

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string("hello"), "hello");
        assert_eq!(escape_json_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_json_string("say \"hi\""), "say \\\"hi\\\"");
    }
}
