//! Math expression format.
//!
//! Evaluates mathematical expressions like:
//! - `2 + 2` → 4
//! - `0xFF + 1` → 256
//! - `0b1010 | 0b0101` → 15 (using bitor function)
//! - `2 ^ 16` → 65536

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, CoreValue, Interpretation};

pub struct ExprFormat;

impl ExprFormat {
    /// Check if input looks like an expression (has operators).
    fn looks_like_expression(input: &str) -> bool {
        // Must contain at least one operator character
        // But not be ONLY operators (like "++")
        let has_operator = input
            .chars()
            .any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '|' | '&' | '<' | '>'));
        let has_alphanumeric = input.chars().any(|c| c.is_alphanumeric());

        // Exclude things that are clearly not expressions
        // - UUIDs (has dashes but also specific pattern)
        // - Dates (has dashes/slashes)
        // - URLs (has :// or starts with http)
        let looks_like_uuid = input.len() == 36 && input.chars().filter(|c| *c == '-').count() == 4;
        let looks_like_url = input.contains("://") || input.starts_with("http");
        let looks_like_date =
            input.contains('/') && input.chars().filter(|c| *c == '/').count() >= 2;

        has_operator && has_alphanumeric && !looks_like_uuid && !looks_like_url && !looks_like_date
    }

    /// Preprocess input to convert common operator syntax to evalexpr functions.
    /// evalexpr uses function syntax for bitwise: bitor(a, b) instead of a | b
    fn preprocess(input: &str) -> String {
        let mut result = input.to_string();

        // Convert bitwise operators to function calls
        // This is a simple approach - won't handle complex nested expressions perfectly
        // but covers common cases like "0b1010 | 0b0101"

        // Handle | (bitwise or) - but not ||
        if result.contains('|') && !result.contains("||") {
            if let Some((left, right)) = result.split_once('|') {
                let left = left.trim();
                let right = right.trim();
                if !left.is_empty() && !right.is_empty() {
                    result = format!("bitor({}, {})", left, right);
                }
            }
        }

        // Handle & (bitwise and) - but not &&
        if result.contains('&') && !result.contains("&&") {
            if let Some((left, right)) = result.split_once('&') {
                let left = left.trim();
                let right = right.trim();
                if !left.is_empty() && !right.is_empty() {
                    result = format!("bitand({}, {})", left, right);
                }
            }
        }

        // Handle << (left shift)
        if result.contains("<<") {
            if let Some((left, right)) = result.split_once("<<") {
                let left = left.trim();
                let right = right.trim();
                if !left.is_empty() && !right.is_empty() {
                    result = format!("shl({}, {})", left, right);
                }
            }
        }

        // Handle >> (right shift)
        if result.contains(">>") {
            if let Some((left, right)) = result.split_once(">>") {
                let left = left.trim();
                let right = right.trim();
                if !left.is_empty() && !right.is_empty() {
                    result = format!("shr({}, {})", left, right);
                }
            }
        }

        result
    }
}

impl Format for ExprFormat {
    fn id(&self) -> &'static str {
        "expr"
    }

    fn name(&self) -> &'static str {
        "Expression"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Math",
            description: "Mathematical expressions with hex/binary/octal support",
            examples: &["2 + 2", "0xFF + 1", "1 << 8", "0b1010 | 0b0101"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Quick check - does this look like an expression?
        if !Self::looks_like_expression(trimmed) {
            return vec![];
        }

        // Preprocess to handle bitwise operators
        let processed = Self::preprocess(trimmed);

        // Try to evaluate
        let result = match evalexpr::eval(&processed) {
            Ok(value) => value,
            Err(_) => return vec![],
        };

        // Convert evalexpr Value to our CoreValue
        let (core_value, description) = match result {
            evalexpr::Value::Int(i) => (
                CoreValue::Int {
                    value: i as i128,
                    original_bytes: None,
                },
                format!("{} = {}", trimmed, i),
            ),
            evalexpr::Value::Float(f) => {
                // Only accept if it's a "clean" result (not NaN/Inf)
                if !f.is_finite() {
                    return vec![];
                }
                (CoreValue::Float(f), format!("{} = {}", trimmed, f))
            }
            // evalexpr can return other types (bool, string, tuple) but we don't care about those
            _ => return vec![],
        };

        // Confidence: lower than direct format detection
        // We want "0xFF" to be detected as hex first, not as an expression
        let confidence = 0.6;

        vec![Interpretation {
            value: core_value,
            source_format: "expr".to_string(),
            confidence,
            description,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // Expressions don't format values, they parse them
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        vec![]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["expression", "math", "calc"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_addition() {
        let format = ExprFormat;
        let results = format.parse("2 + 2");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "expr");

        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 4);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_hex_arithmetic() {
        let format = ExprFormat;
        let results = format.parse("0xFF + 1");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 256);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_binary_literals() {
        let format = ExprFormat;
        let results = format.parse("0b1000 + 0b0001");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 9);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_bitwise_or() {
        let format = ExprFormat;
        let results = format.parse("0b1010 | 0b0101");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 15); // 0b1111
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_bitwise_and() {
        let format = ExprFormat;
        let results = format.parse("0xFF & 0x0F");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 15);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_left_shift() {
        let format = ExprFormat;
        let results = format.parse("1 << 8");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 256);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_right_shift() {
        let format = ExprFormat;
        let results = format.parse("256 >> 4");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 16);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_power() {
        let format = ExprFormat;
        let results = format.parse("2 ^ 16");

        assert_eq!(results.len(), 1);
        // evalexpr returns float for power
        if let CoreValue::Float(value) = &results[0].value {
            assert_eq!(*value, 65536.0);
        } else {
            panic!("Expected Float, got {:?}", results[0].value);
        }
    }

    #[test]
    fn test_not_expression_plain_hex() {
        let format = ExprFormat;
        // Plain hex without operator should not be parsed as expression
        let results = format.parse("0xFF");
        assert!(results.is_empty());
    }

    #[test]
    fn test_not_expression_uuid() {
        let format = ExprFormat;
        // UUID has dashes but is not an expression
        let results = format.parse("550e8400-e29b-41d4-a716-446655440000");
        assert!(results.is_empty());
    }

    #[test]
    fn test_complex_expression() {
        let format = ExprFormat;
        let results = format.parse("(10 + 5) * 2");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 30);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_modulo() {
        let format = ExprFormat;
        let results = format.parse("17 % 5");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 2);
        } else {
            panic!("Expected Int");
        }
    }
}
