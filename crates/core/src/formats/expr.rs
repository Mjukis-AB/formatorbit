//! Math expression format.
//!
//! Evaluates mathematical expressions like:
//! - `2 + 2` → 4
//! - `0xFF + 1` → 256
//! - `0b1010 | 0b0101` → 15 (using bitor function)
//! - `2 ^ 16` → 65536

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

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
    ///
    /// evalexpr has no infix bitwise/shift operators, so `a | b` must become
    /// `bitor(a, b)`, etc. The rewrite is precedence-aware and handles chained
    /// expressions correctly:
    ///
    /// - `1 | 2 | 4` → `bitor(bitor(1, 2), 4)` = 7
    /// - `8 >> 1 >> 1` → `shr(shr(8, 1), 1)` = 2
    /// - `1 | 2 & 3` → `bitor(1, bitand(2, 3))` = 3 (C precedence: `&` binds
    ///   tighter than `|`; shifts bind tighter than both)
    ///
    /// Operators are split at the lowest precedence level first, scanning for the
    /// *rightmost* top-level occurrence so chains stay left-associative. Content
    /// inside parentheses is preserved and recursed into. `||` / `&&` (logical
    /// operators, which evalexpr handles natively) are left untouched.
    fn preprocess(input: &str) -> String {
        Self::rewrite_bitwise(input.trim())
    }

    /// Recursively rewrite bitwise/shift operators in one expression fragment.
    ///
    /// Precedence, lowest to highest: `|` < `&` < (`<<` | `>>`). Each level splits
    /// at its rightmost top-level operator (left-associative), rewrites the right
    /// operand at the same level and the left operand at the next-lower level.
    fn rewrite_bitwise(expr: &str) -> String {
        let expr = expr.trim();
        if expr.is_empty() {
            return String::new();
        }

        // Precedence ladder, lowest first. Each entry maps an operator token to
        // the evalexpr function that replaces it.
        const LEVELS: &[&[(&str, &str)]] = &[
            &[("|", "bitor")],
            &[("&", "bitand")],
            &[("<<", "shl"), (">>", "shr")],
        ];

        Self::rewrite_level(expr, LEVELS)
    }

    fn rewrite_level(expr: &str, levels: &[&[(&str, &str)]]) -> String {
        let expr = expr.trim();
        let Some((ops, rest)) = levels.split_first() else {
            // Below the lowest-precedence level: only parenthesised groups may
            // still contain bitwise operators to rewrite.
            return Self::rewrite_parens(expr);
        };

        // Find the rightmost top-level (paren-depth 0) occurrence of any operator
        // at this precedence level. Rightmost split keeps left-associativity:
        // `a | b | c` → bitor(`a | b`, `c`).
        if let Some((idx, op_tok, func)) = Self::find_rightmost_op(expr, ops) {
            let left = expr[..idx].trim();
            let right = expr[idx + op_tok.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                // Left operand: same precedence level (absorbs the rest of the chain).
                // Right operand: strictly higher precedence (already fully reduced here).
                let left_r = Self::rewrite_level(left, levels);
                let right_r = Self::rewrite_level(right, rest);
                return format!("{}({}, {})", func, left_r, right_r);
            }
        }

        // No operator at this level; descend to the next higher-precedence level.
        Self::rewrite_level(expr, rest)
    }

    /// Rewrite bitwise operators that only appear inside parenthesised groups.
    fn rewrite_parens(expr: &str) -> String {
        if !expr.contains('(') {
            return expr.to_string();
        }
        let mut out = String::with_capacity(expr.len());
        let bytes = expr.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'(' {
                // Find the matching close paren.
                let mut depth = 1;
                let mut j = i + 1;
                while j < bytes.len() && depth > 0 {
                    match bytes[j] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                if depth == 0 {
                    let inner = &expr[i + 1..j - 1];
                    out.push('(');
                    out.push_str(&Self::rewrite_bitwise(inner));
                    out.push(')');
                    i = j;
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }

    /// Find the rightmost top-level occurrence of any operator in `ops`.
    ///
    /// Skips positions inside parentheses and the doubled logical forms `||`/`&&`.
    /// Returns (byte index, matched operator token, replacement function name).
    fn find_rightmost_op<'a>(
        expr: &str,
        ops: &'a [(&str, &str)],
    ) -> Option<(usize, &'a str, &'a str)> {
        let bytes = expr.as_bytes();
        let mut depth = 0i32;
        let mut found: Option<(usize, &'a str, &'a str)> = None;
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'(' => {
                    depth += 1;
                    i += 1;
                }
                b')' => {
                    depth -= 1;
                    i += 1;
                }
                _ if depth == 0 => {
                    let mut matched = None;
                    for (tok, func) in ops {
                        let tb = tok.as_bytes();
                        if bytes[i..].starts_with(tb) {
                            // Skip the doubled logical operators || and &&.
                            let is_logical_double = tb.len() == 1
                                && (*tok == "|" || *tok == "&")
                                && (bytes.get(i + 1) == Some(&tb[0])
                                    || (i > 0 && bytes[i - 1] == tb[0]));
                            if !is_logical_double {
                                matched = Some((*tok, *func, tb.len()));
                                break;
                            }
                        }
                    }
                    if let Some((tok, func, len)) = matched {
                        // Record this occurrence and keep scanning for one
                        // further right (left-associative split point).
                        found = Some((i, tok, func));
                        i += len;
                    } else {
                        i += 1;
                    }
                }
                _ => i += 1,
            }
        }
        found
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
            CoreValue::Int { value: i, .. } => vec![
                Conversion::new(
                    CoreValue::Int {
                        value: *i,
                        original_bytes: None,
                    },
                    "result",
                    i.to_string(),
                )
                .priority(ConversionPriority::Primary)
                .display_only(true), // Don't explore further from result
            ],
            CoreValue::Float(f) => vec![
                Conversion::new(CoreValue::Float(*f), "result", f.to_string())
                    .priority(ConversionPriority::Primary)
                    .display_only(true), // Don't explore further from result
            ],
            CoreValue::Currency { amount, code } => vec![
                Conversion::new(
                    CoreValue::Currency {
                        amount: *amount,
                        code: code.clone(),
                    },
                    "result",
                    format!("{:.2} {}", amount, code),
                )
                .priority(ConversionPriority::Primary)
                .display_only(true), // Don't explore further from result
            ],
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

    /// Helper: parse an expression and assert it evaluates to the given integer.
    fn assert_int(expr: &str, expected: i128) {
        let format = ExprFormat;
        let results = format.parse(expr);
        assert_eq!(results.len(), 1, "expected one interpretation for {expr:?}");
        match &results[0].value {
            CoreValue::Int { value, .. } => {
                assert_eq!(*value, expected, "wrong value for {expr:?}");
            }
            other => panic!("expected Int for {expr:?}, got {other:?}"),
        }
    }

    #[test]
    fn test_chained_bitwise_or() {
        // 1 | 2 | 4 == 7 (was mishandled: bitor(1, 2 | 4) left a raw `|`)
        assert_int("1 | 2 | 4", 7);
    }

    #[test]
    fn test_chained_right_shift() {
        // 8 >> 1 >> 1 == 2 (left-associative: (8 >> 1) >> 1)
        assert_int("8 >> 1 >> 1", 2);
    }

    #[test]
    fn test_chained_left_shift() {
        // 1 << 2 << 3 == 32 (left-associative: (1 << 2) << 3 = 4 << 3)
        assert_int("1 << 2 << 3", 32);
    }

    #[test]
    fn test_chained_bitwise_and() {
        // 15 & 6 & 3 == 2
        assert_int("15 & 6 & 3", 2);
    }

    #[test]
    fn test_bitwise_precedence_and_binds_tighter_than_or() {
        // C precedence: & binds tighter than |, so 1 | 2 & 3 == 1 | (2 & 3) == 1 | 2 == 3
        assert_int("1 | 2 & 3", 3);
    }

    #[test]
    fn test_bitwise_precedence_shift_binds_tighter() {
        // Shifts bind tighter than & and |: 1 | 1 << 4 == 1 | (1 << 4) == 1 | 16 == 17
        assert_int("1 | 1 << 4", 17);
        // & vs <<: 3 & 1 << 2 == 3 & (1 << 2) == 3 & 4 == 0
        assert_int("3 & 1 << 2", 0);
    }

    #[test]
    fn test_bitwise_mixed_chain() {
        // 8 >> 1 | 1 == (8 >> 1) | 1 == 4 | 1 == 5
        assert_int("8 >> 1 | 1", 5);
    }

    #[test]
    fn test_bitwise_with_parens() {
        // Parens override precedence: (1 | 2) & 4 == 3 & 4 == 0
        assert_int("(1 | 2) & 4", 0);
        // Nested chain inside parens still reduces: (1 | 2 | 4) == 7
        assert_int("(1 | 2 | 4)", 7);
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
