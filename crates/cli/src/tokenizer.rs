//! Tokenizer for pipe mode.
//!
//! Extracts tokens from log lines with position tracking and smart grouping
//! for hex-like sequences (e.g., "69 1E 01 B8" -> single token).

use unicode_width::UnicodeWidthStr;

/// A token extracted from a line with its position information.
#[derive(Debug, Clone)]
pub struct Token {
    /// The token text (may be normalized, e.g., grouped hex bytes)
    pub text: String,
    /// Byte offset in the original line (start)
    pub start: usize,
    /// Byte offset in the original line (end, exclusive)
    pub end: usize,
    /// Display column (0-indexed, accounting for unicode width)
    pub display_col: usize,
}

/// Tokenize a line with smart hex grouping.
///
/// Splits on whitespace but groups adjacent hex-like tokens
/// (e.g., "69 1E 01 B8" becomes a single token).
pub fn tokenize(line: &str) -> Vec<Token> {
    let raw_tokens = extract_raw_tokens(line);
    apply_hex_grouping(&raw_tokens, line)
}

/// Extract individual whitespace-separated tokens with positions.
fn extract_raw_tokens(line: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut display_col = 0;
    let mut byte_offset = 0;
    let mut in_token = false;
    let mut token_start_byte = 0;
    let mut token_start_col = 0;

    for (i, c) in line.char_indices() {
        let c_width = UnicodeWidthStr::width(c.to_string().as_str());

        if c.is_whitespace() {
            if in_token {
                // End of token
                let text = &line[token_start_byte..i];
                tokens.push(Token {
                    text: text.to_string(),
                    start: token_start_byte,
                    end: i,
                    display_col: token_start_col,
                });
                in_token = false;
            }
        } else if !in_token {
            // Start of new token
            in_token = true;
            token_start_byte = i;
            token_start_col = display_col;
        }

        display_col += c_width;
        byte_offset = i + c.len_utf8();
    }

    // Handle final token
    if in_token {
        let text = &line[token_start_byte..byte_offset];
        tokens.push(Token {
            text: text.to_string(),
            start: token_start_byte,
            end: byte_offset,
            display_col: token_start_col,
        });
    }

    tokens
}

/// Check if a token looks like a hex byte (1-2 hex digits).
fn looks_like_hex_byte(s: &str) -> bool {
    let len = s.len();
    (len == 1 || len == 2) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Try to group adjacent tokens that look like hex bytes.
fn apply_hex_grouping(tokens: &[Token], original_line: &str) -> Vec<Token> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        // Check if this starts a potential hex sequence
        if looks_like_hex_byte(&tokens[i].text) {
            // Try to extend the sequence
            let mut end = i + 1;
            while end < tokens.len() && looks_like_hex_byte(&tokens[end].text) {
                end += 1;
            }

            if end > i + 1 {
                // We have a sequence of 2+ hex-like tokens - group them
                let start_token = &tokens[i];
                let end_token = &tokens[end - 1];

                // Combine the text with spaces preserved from original
                let combined = &original_line[start_token.start..end_token.end];

                result.push(Token {
                    text: combined.to_string(),
                    start: start_token.start,
                    end: end_token.end,
                    display_col: start_token.display_col,
                });

                i = end;
                continue;
            }
        }

        // Not part of a group, add as-is
        result.push(tokens[i].clone());
        i += 1;
    }

    result
}

/// Quick check if a token is worth analyzing.
/// Returns false for common words that are unlikely to be interesting.
pub fn is_interesting_candidate(token: &str) -> bool {
    let len = token.len();

    // Skip very short tokens
    if len < 2 {
        return false;
    }

    // Skip very long tokens (unlikely to be a single value)
    if len > 128 {
        return false;
    }

    // Skip common log words
    let lower = token.to_lowercase();
    if matches!(
        lower.as_str(),
        "the"
            | "and"
            | "for"
            | "from"
            | "with"
            | "that"
            | "this"
            | "are"
            | "was"
            | "were"
            | "been"
            | "have"
            | "has"
            | "had"
            | "not"
            | "but"
            | "what"
            | "all"
            | "when"
            | "user"
            | "logged"
            | "error"
            | "warning"
            | "warn"
            | "info"
            | "debug"
            | "trace"
            | "received"
            | "sent"
            | "payload"
            | "request"
            | "response"
            | "status"
            | "message"
            | "data"
            | "value"
            | "null"
            | "true"
            | "false"
            | "none"
    ) {
        return false;
    }

    // Check for patterns that are likely interpretable
    let has_digits = token.chars().any(|c| c.is_ascii_digit());
    let has_format_marker = token.starts_with("0x")
        || token.starts_with('#')
        || token.contains('-')
        || token.contains(':')
        || token.contains('.');

    // Check if it looks hex-like (letters a-f plus digits)
    let is_hex_like = (4..=64).contains(&len) && token.chars().all(|c| c.is_ascii_hexdigit());

    has_digits || has_format_marker || is_hex_like
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("hello world");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "hello");
        assert_eq!(tokens[1].text, "world");
    }

    #[test]
    fn test_tokenize_positions() {
        // Use non-hex tokens to avoid grouping
        let tokens = tokenize("hi there");
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, 2);
        assert_eq!(tokens[0].display_col, 0);
        assert_eq!(tokens[1].start, 3);
        assert_eq!(tokens[1].end, 8);
        assert_eq!(tokens[1].display_col, 3);
    }

    #[test]
    fn test_hex_grouping() {
        let tokens = tokenize("payload: 69 1E 01 B8 end");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].text, "payload:");
        assert_eq!(tokens[1].text, "69 1E 01 B8");
        assert_eq!(tokens[2].text, "end");
    }

    #[test]
    fn test_hex_grouping_preserves_position() {
        let tokens = tokenize("data: AB CD EF");
        assert_eq!(tokens.len(), 2);
        let hex_token = &tokens[1];
        assert_eq!(hex_token.text, "AB CD EF");
        assert_eq!(hex_token.start, 6);
        assert_eq!(hex_token.end, 14);
    }

    #[test]
    fn test_single_hex_not_grouped() {
        let tokens = tokenize("value: FF");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[1].text, "FF");
    }

    #[test]
    fn test_is_interesting() {
        assert!(is_interesting_candidate(
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(is_interesting_candidate("192.168.1.1"));
        assert!(is_interesting_candidate("0x691E01B8"));
        assert!(is_interesting_candidate("#FF5733"));
        assert!(is_interesting_candidate("1763574200"));
        assert!(is_interesting_candidate("691E01B8"));

        assert!(!is_interesting_candidate("the"));
        assert!(!is_interesting_candidate("error"));
        assert!(!is_interesting_candidate("a")); // too short
    }

    #[test]
    fn test_mixed_line() {
        let tokens = tokenize("[2024-01-15] User abc123 sent 69 1E 01 B8");
        // Should have: [2024-01-15], User, abc123, sent, "69 1E 01 B8"
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[4].text, "69 1E 01 B8");
    }
}
