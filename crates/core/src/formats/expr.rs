//! Math expression format.
//!
//! Evaluates mathematical expressions like:
//! - `2 + 2` → 4
//! - `0xFF + 1` → 256
//! - `0b1010 | 0b0101` → 15 (using bitor function)
//! - `2 ^ 16` → 65536

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation};

pub struct ExprFormat;

impl ExprFormat {
    /// Check if input looks like an expression (has operators or function calls).
    fn looks_like_expression(input: &str) -> bool {
        // Must contain at least one operator character
        // But not be ONLY operators (like "++")
        let has_operator = input
            .chars()
            .any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '|' | '&' | '<' | '>'));
        let has_alphanumeric = input.chars().any(|c| c.is_alphanumeric());

        // Check for function call syntax: identifier followed by parentheses
        let has_function_call = input.contains('(') && input.contains(')') && has_alphanumeric;

        // Exclude things that are clearly not expressions
        // - UUIDs (has dashes but also specific pattern)
        // - Dates (has dashes/slashes)
        // - URLs (has :// or starts with http)
        let looks_like_uuid = input.len() == 36 && input.chars().filter(|c| *c == '-').count() == 4;
        let looks_like_url = input.contains("://") || input.starts_with("http");
        let looks_like_date =
            input.contains('/') && input.chars().filter(|c| *c == '/').count() >= 2;

        (has_operator || has_function_call)
            && has_alphanumeric
            && !looks_like_uuid
            && !looks_like_url
            && !looks_like_date
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
            has_validation: false,
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

        // Try to evaluate using the global context (which may have plugin vars/funcs)
        let eval_result = match crate::expr_context::eval(&processed) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        // Convert evalexpr Value to our CoreValue
        // If currency was used, wrap in CoreValue::Currency for proper formatting
        let (core_value, description) = match eval_result.value {
            evalexpr::Value::Int(i) => {
                let desc = if let Some(ref currency) = eval_result.result_currency {
                    format!("{} = {} {}", trimmed, i, currency)
                } else {
                    format!("{} = {}", trimmed, i)
                };
                let value = if let Some(ref currency) = eval_result.result_currency {
                    CoreValue::Currency {
                        amount: i as f64,
                        code: currency.clone(),
                    }
                } else {
                    CoreValue::Int {
                        value: i as i128,
                        original_bytes: None,
                    }
                };
                (value, desc)
            }
            evalexpr::Value::Float(f) => {
                // Only accept if it's a "clean" result (not NaN/Inf)
                if !f.is_finite() {
                    return vec![];
                }
                let desc = if let Some(ref currency) = eval_result.result_currency {
                    // Format currency amounts nicely (2 decimal places)
                    format!("{} = {:.2} {}", trimmed, f, currency)
                } else {
                    format!("{} = {}", trimmed, f)
                };
                let value = if let Some(ref currency) = eval_result.result_currency {
                    CoreValue::Currency {
                        amount: f,
                        code: currency.clone(),
                    }
                } else {
                    CoreValue::Float(f)
                };
                (value, desc)
            }
            // evalexpr can return other types (bool, string, tuple) but we don't care about those
            _ => return vec![],
        };

        // Dynamic confidence based on expression complexity
        // More operators = more likely to be an intentional expression
        let mult_div_count = trimmed
            .chars()
            .filter(|c| matches!(c, '*' | '/' | '%' | '^'))
            .count();
        let has_bitwise = trimmed.contains('|') || trimmed.contains('&');
        let has_shift = trimmed.contains("<<") || trimmed.contains(">>");
        let add_sub_count = trimmed.chars().filter(|c| matches!(c, '+' | '-')).count();

        let confidence = if mult_div_count >= 2 || has_shift || has_bitwise {
            // Complex expression: 5*9*3*9/23, 1<<8, a|b
            0.95
        } else if mult_div_count == 1 {
            // Single multiply/divide: 5*9, 10/2
            0.85
        } else if add_sub_count >= 1 {
            // Addition/subtraction only - lower because +/- appear in dates, UUIDs
            0.75
        } else {
            // Bare literal that evaluates (0xFF) - let hex/bin/oct win
            0.5
        };

        vec![Interpretation {
            value: core_value,
            source_format: "expr".to_string(),
            confidence,
            description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // Expressions don't format values, they parse them
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        // Don't emit result conversions here - they would pollute other interpretations.
        // Use source_conversions() instead for expr-specific results.
        vec![]
    }

    fn source_conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        // Emit a Primary priority conversion for the expression result.
        // This is only called when expr was the source parser, so it won't
        // pollute other interpretations like color-hex or datasize.
        match value {
            CoreValue::Int { value: i, .. } => vec![Conversion {
                value: CoreValue::Int {
                    value: *i,
                    original_bytes: None,
                },
                target_format: "result".to_string(),
                display: i.to_string(),
                path: vec![], // Will be set by BFS
                is_lossy: false,
                steps: vec![], // Will be set by BFS
                priority: ConversionPriority::Primary,
                kind: ConversionKind::Conversion,
                display_only: true, // Don't explore further from result
                hidden: false,
                rich_display: vec![],
            }],
            CoreValue::Float(f) => vec![Conversion {
                value: CoreValue::Float(*f),
                target_format: "result".to_string(),
                display: f.to_string(),
                path: vec![], // Will be set by BFS
                is_lossy: false,
                steps: vec![], // Will be set by BFS
                priority: ConversionPriority::Primary,
                kind: ConversionKind::Conversion,
                display_only: true, // Don't explore further from result
                hidden: false,
                rich_display: vec![],
            }],
            CoreValue::Currency { amount, code } => vec![Conversion {
                value: CoreValue::Currency {
                    amount: *amount,
                    code: code.clone(),
                },
                target_format: "result".to_string(),
                display: format!("{:.2} {}", amount, code),
                path: vec![], // Will be set by BFS
                is_lossy: false,
                steps: vec![], // Will be set by BFS
                priority: ConversionPriority::Primary,
                kind: ConversionKind::Conversion,
                display_only: true, // Don't explore further from result
                hidden: false,
                rich_display: vec![],
            }],
            _ => vec![],
        }
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

    #[test]
    fn test_to_currency_function_exists() {
        // Verify that toEUR/inEUR functions are registered and callable
        let format = ExprFormat;

        // toEUR(100) should parse as an expression (has function call)
        let results = format.parse("toEUR(100)");
        assert_eq!(results.len(), 1);

        // Result should be a float (currency conversions return floats)
        if let CoreValue::Float(f) = &results[0].value {
            // Should be some positive value (100 in user's currency converted to EUR)
            assert!(*f > 0.0);
        } else {
            panic!("Expected Float from toEUR");
        }
    }

    #[test]
    fn test_in_currency_function_exists() {
        // Verify that inEUR is an alias for toEUR
        let format = ExprFormat;

        let results = format.parse("inEUR(100)");
        assert_eq!(results.len(), 1);

        if let CoreValue::Float(f) = &results[0].value {
            assert!(*f > 0.0);
        } else {
            panic!("Expected Float from inEUR");
        }
    }
}
