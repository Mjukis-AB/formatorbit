//! Constants format: bidirectional lookup for well-known constants.
//!
//! - Number → Name: Show as trait on integers (e.g., `443` → "HTTPS port")
//! - Name → Number: Parse strings (e.g., `ESC` → 27, `ssh` → 22)

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
    RichDisplay, RichDisplayOption,
};

/// A well-known constant with its value and display information.
struct Constant {
    /// Canonical name (for parsing)
    name: &'static str,
    /// Alternate names (for parsing)
    aliases: &'static [&'static str],
    /// The integer value
    value: i128,
    /// Display text when showing as trait (e.g., "HTTPS (port 443/tcp)")
    trait_display: &'static str,
    /// Display text when parsed from name (e.g., "port 443/tcp (HTTPS)")
    parse_display: &'static str,
}

// =============================================================================
// Constant Definitions
// =============================================================================

/// HTTP status codes - only the most commonly looked up ones.
const HTTP_STATUS: &[Constant] = &[
    Constant {
        name: "OK",
        aliases: &["200"],
        value: 200,
        trait_display: "HTTP 200 OK",
        parse_display: "HTTP 200 OK",
    },
    Constant {
        name: "Created",
        aliases: &["201"],
        value: 201,
        trait_display: "HTTP 201 Created",
        parse_display: "HTTP 201 Created",
    },
    Constant {
        name: "No Content",
        aliases: &["204"],
        value: 204,
        trait_display: "HTTP 204 No Content",
        parse_display: "HTTP 204 No Content",
    },
    Constant {
        name: "Moved Permanently",
        aliases: &["301"],
        value: 301,
        trait_display: "HTTP 301 Moved Permanently",
        parse_display: "HTTP 301 Moved Permanently",
    },
    Constant {
        name: "Found",
        aliases: &["302"],
        value: 302,
        trait_display: "HTTP 302 Found",
        parse_display: "HTTP 302 Found",
    },
    Constant {
        name: "Not Modified",
        aliases: &["304"],
        value: 304,
        trait_display: "HTTP 304 Not Modified",
        parse_display: "HTTP 304 Not Modified",
    },
    Constant {
        name: "Bad Request",
        aliases: &["400"],
        value: 400,
        trait_display: "HTTP 400 Bad Request",
        parse_display: "HTTP 400 Bad Request",
    },
    Constant {
        name: "Unauthorized",
        aliases: &["401"],
        value: 401,
        trait_display: "HTTP 401 Unauthorized",
        parse_display: "HTTP 401 Unauthorized",
    },
    Constant {
        name: "Forbidden",
        aliases: &["403"],
        value: 403,
        trait_display: "HTTP 403 Forbidden",
        parse_display: "HTTP 403 Forbidden",
    },
    Constant {
        name: "Not Found",
        aliases: &["404"],
        value: 404,
        trait_display: "HTTP 404 Not Found",
        parse_display: "HTTP 404 Not Found",
    },
    Constant {
        name: "Method Not Allowed",
        aliases: &["405"],
        value: 405,
        trait_display: "HTTP 405 Method Not Allowed",
        parse_display: "HTTP 405 Method Not Allowed",
    },
    Constant {
        name: "Internal Server Error",
        aliases: &["500"],
        value: 500,
        trait_display: "HTTP 500 Internal Server Error",
        parse_display: "HTTP 500 Internal Server Error",
    },
    Constant {
        name: "Bad Gateway",
        aliases: &["502"],
        value: 502,
        trait_display: "HTTP 502 Bad Gateway",
        parse_display: "HTTP 502 Bad Gateway",
    },
    Constant {
        name: "Service Unavailable",
        aliases: &["503"],
        value: 503,
        trait_display: "HTTP 503 Service Unavailable",
        parse_display: "HTTP 503 Service Unavailable",
    },
    Constant {
        name: "Gateway Timeout",
        aliases: &["504"],
        value: 504,
        trait_display: "HTTP 504 Gateway Timeout",
        parse_display: "HTTP 504 Gateway Timeout",
    },
];

/// Well-known TCP/UDP ports - the "greatest hits" that developers look up.
const PORTS: &[Constant] = &[
    Constant {
        name: "ftp",
        aliases: &[],
        value: 21,
        trait_display: "FTP (port 21/tcp)",
        parse_display: "port 21/tcp (FTP)",
    },
    Constant {
        name: "ssh",
        aliases: &[],
        value: 22,
        trait_display: "SSH (port 22/tcp)",
        parse_display: "port 22/tcp (SSH)",
    },
    Constant {
        name: "telnet",
        aliases: &[],
        value: 23,
        trait_display: "Telnet (port 23/tcp)",
        parse_display: "port 23/tcp (Telnet)",
    },
    Constant {
        name: "smtp",
        aliases: &[],
        value: 25,
        trait_display: "SMTP (port 25/tcp)",
        parse_display: "port 25/tcp (SMTP)",
    },
    Constant {
        name: "dns",
        aliases: &[],
        value: 53,
        trait_display: "DNS (port 53/udp)",
        parse_display: "port 53/udp (DNS)",
    },
    Constant {
        name: "http",
        aliases: &[],
        value: 80,
        trait_display: "HTTP (port 80/tcp)",
        parse_display: "port 80/tcp (HTTP)",
    },
    Constant {
        name: "https",
        aliases: &[],
        value: 443,
        trait_display: "HTTPS (port 443/tcp)",
        parse_display: "port 443/tcp (HTTPS)",
    },
    Constant {
        name: "mysql",
        aliases: &[],
        value: 3306,
        trait_display: "MySQL (port 3306/tcp)",
        parse_display: "port 3306/tcp (MySQL)",
    },
    Constant {
        name: "postgres",
        aliases: &["postgresql"],
        value: 5432,
        trait_display: "PostgreSQL (port 5432/tcp)",
        parse_display: "port 5432/tcp (PostgreSQL)",
    },
    Constant {
        name: "redis",
        aliases: &[],
        value: 6379,
        trait_display: "Redis (port 6379/tcp)",
        parse_display: "port 6379/tcp (Redis)",
    },
    Constant {
        name: "mongodb",
        aliases: &["mongo"],
        value: 27017,
        trait_display: "MongoDB (port 27017/tcp)",
        parse_display: "port 27017/tcp (MongoDB)",
    },
];

/// Common development ports.
const DEV_PORTS: &[Constant] = &[
    Constant {
        name: "dev",
        aliases: &["vite", "react"],
        value: 3000,
        trait_display: "dev server (port 3000)",
        parse_display: "port 3000 (dev server)",
    },
    Constant {
        name: "alt-http",
        aliases: &[],
        value: 8080,
        trait_display: "HTTP alt (port 8080)",
        parse_display: "port 8080 (HTTP alt)",
    },
    Constant {
        name: "alt-https",
        aliases: &[],
        value: 8443,
        trait_display: "HTTPS alt (port 8443)",
        parse_display: "port 8443 (HTTPS alt)",
    },
];

/// Unix signals - skip 1 (SIGHUP) and 2 (SIGINT) as they're often just numbers.
const SIGNALS: &[Constant] = &[
    Constant {
        name: "SIGKILL",
        aliases: &["kill"],
        value: 9,
        trait_display: "SIGKILL (9)",
        parse_display: "signal 9 (SIGKILL)",
    },
    Constant {
        name: "SIGSEGV",
        aliases: &["segfault"],
        value: 11,
        trait_display: "SIGSEGV (11)",
        parse_display: "signal 11 (SIGSEGV)",
    },
    Constant {
        name: "SIGTERM",
        aliases: &["term"],
        value: 15,
        trait_display: "SIGTERM (15)",
        parse_display: "signal 15 (SIGTERM)",
    },
    Constant {
        name: "SIGSTOP",
        aliases: &["stop"],
        value: 19,
        trait_display: "SIGSTOP (19)",
        parse_display: "signal 19 (SIGSTOP)",
    },
];

/// ASCII control characters - commonly looked up.
const ASCII_CTRL: &[Constant] = &[
    Constant {
        name: "NUL",
        aliases: &["null"],
        value: 0,
        trait_display: "NUL (ASCII 0)",
        parse_display: "ASCII 0 (0x00)",
    },
    Constant {
        name: "BEL",
        aliases: &["bell"],
        value: 7,
        trait_display: "BEL (ASCII 7)",
        parse_display: "ASCII 7 (0x07)",
    },
    Constant {
        name: "BS",
        aliases: &["backspace"],
        value: 8,
        trait_display: "BS (ASCII 8)",
        parse_display: "ASCII 8 (0x08)",
    },
    Constant {
        name: "TAB",
        aliases: &["ht"],
        value: 9,
        trait_display: "TAB (ASCII 9)",
        parse_display: "ASCII 9 (0x09)",
    },
    Constant {
        name: "LF",
        aliases: &["newline", "linefeed"],
        value: 10,
        trait_display: "LF (ASCII 10)",
        parse_display: "ASCII 10 (0x0A)",
    },
    Constant {
        name: "CR",
        aliases: &["return"],
        value: 13,
        trait_display: "CR (ASCII 13)",
        parse_display: "ASCII 13 (0x0D)",
    },
    Constant {
        name: "ESC",
        aliases: &["escape"],
        value: 27,
        trait_display: "ESC (ASCII 27)",
        parse_display: "ASCII 27 (0x1B)",
    },
    Constant {
        name: "DEL",
        aliases: &["delete"],
        value: 127,
        trait_display: "DEL (ASCII 127)",
        parse_display: "ASCII 127 (0x7F)",
    },
];

/// Exit codes - common Unix exit status values.
const EXIT_CODES: &[Constant] = &[
    Constant {
        name: "EXIT_SUCCESS",
        aliases: &[],
        value: 0,
        trait_display: "EXIT_SUCCESS (0)",
        parse_display: "exit code 0 (success)",
    },
    Constant {
        name: "EXIT_FAILURE",
        aliases: &[],
        value: 1,
        trait_display: "EXIT_FAILURE (1)",
        parse_display: "exit code 1 (failure)",
    },
    Constant {
        name: "SIGKILL_EXIT",
        aliases: &[],
        value: 137,
        trait_display: "killed by SIGKILL (137)",
        parse_display: "exit code 137 (128+9, SIGKILL)",
    },
    Constant {
        name: "SIGSEGV_EXIT",
        aliases: &[],
        value: 139,
        trait_display: "killed by SIGSEGV (139)",
        parse_display: "exit code 139 (128+11, SIGSEGV)",
    },
    Constant {
        name: "SIGTERM_EXIT",
        aliases: &[],
        value: 143,
        trait_display: "killed by SIGTERM (143)",
        parse_display: "exit code 143 (128+15, SIGTERM)",
    },
];

// =============================================================================
// Helper Functions
// =============================================================================

/// Find matching constants for an integer value.
fn find_constants_for_value(value: i128) -> Vec<&'static Constant> {
    let mut matches = Vec::new();

    for list in [
        HTTP_STATUS,
        PORTS,
        DEV_PORTS,
        SIGNALS,
        ASCII_CTRL,
        EXIT_CODES,
    ] {
        for constant in list {
            if constant.value == value {
                matches.push(constant);
            }
        }
    }

    matches
}

/// Find a constant by name (case-insensitive).
fn find_constant_by_name(name: &str) -> Option<&'static Constant> {
    let name_lower = name.to_lowercase();

    for list in [
        HTTP_STATUS,
        PORTS,
        DEV_PORTS,
        SIGNALS,
        ASCII_CTRL,
        EXIT_CODES,
    ] {
        for constant in list {
            // Match canonical name
            if constant.name.to_lowercase() == name_lower {
                return Some(constant);
            }
            // Match aliases
            for alias in constant.aliases {
                if alias.to_lowercase() == name_lower {
                    return Some(constant);
                }
            }
        }
    }

    None
}

// =============================================================================
// Format Implementation
// =============================================================================

pub struct ConstantsFormat;

impl Format for ConstantsFormat {
    fn id(&self) -> &'static str {
        "constants"
    }

    fn name(&self) -> &'static str {
        "Well-Known Constants"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Reference",
            description: "Bidirectional lookup for HTTP status codes, ports, signals, ASCII codes",
            examples: &["ssh", "ESC", "Not Found", "SIGKILL"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some(constant) = find_constant_by_name(input.trim()) else {
            return vec![];
        };

        vec![Interpretation {
            value: CoreValue::Int {
                value: constant.value,
                original_bytes: None,
            },
            source_format: "constants".to_string(),
            confidence: 0.9, // High confidence for exact name matches
            description: constant.parse_display.to_string(),
            rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue {
                pairs: vec![
                    ("name".to_string(), constant.name.to_string()),
                    ("value".to_string(), constant.value.to_string()),
                ],
            })],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        // Constants format is for trait display only, not for formatting values
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        // Constants are shown as traits via conversions(), not as a format output
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: int_val, .. } = value else {
            return vec![];
        };

        let matches = find_constants_for_value(*int_val);
        if matches.is_empty() {
            return vec![];
        }

        // Combine all matches into one display line
        let displays: Vec<&str> = matches.iter().map(|c| c.trait_display).collect();
        let combined_display = displays.join(", ");

        vec![Conversion {
            value: CoreValue::String(combined_display.clone()),
            target_format: "constants".to_string(),
            display: combined_display.clone(),
            path: vec!["constants".to_string()],
            steps: vec![ConversionStep {
                format: "constants".to_string(),
                value: CoreValue::String(combined_display.clone()),
                display: combined_display,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Trait,
            display_only: true,
            rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue {
                pairs: matches
                    .iter()
                    .map(|c| (c.name.to_string(), c.value.to_string()))
                    .collect(),
            })],
            ..Default::default()
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["const", "lookup"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh() {
        let format = ConstantsFormat;
        let results = format.parse("ssh");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 22);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_esc_case_insensitive() {
        let format = ConstantsFormat;
        let results = format.parse("esc");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 27);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_not_found() {
        let format = ConstantsFormat;
        let results = format.parse("Not Found");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 404);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_sigkill() {
        let format = ConstantsFormat;
        let results = format.parse("SIGKILL");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 9);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_trait_443() {
        let format = ConstantsFormat;
        let value = CoreValue::Int {
            value: 443,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);
        assert_eq!(conversions.len(), 1);
        assert!(conversions[0].display.contains("HTTPS"));
    }

    #[test]
    fn test_trait_404() {
        let format = ConstantsFormat;
        let value = CoreValue::Int {
            value: 404,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);
        assert_eq!(conversions.len(), 1);
        assert!(conversions[0].display.contains("Not Found"));
    }

    #[test]
    fn test_trait_multiple_meanings() {
        let format = ConstantsFormat;
        // 9 is both TAB (ASCII) and SIGKILL
        let value = CoreValue::Int {
            value: 9,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);
        assert_eq!(conversions.len(), 1);
        // Should contain both
        assert!(conversions[0].display.contains("TAB"));
        assert!(conversions[0].display.contains("SIGKILL"));
    }

    #[test]
    fn test_no_match() {
        let format = ConstantsFormat;
        let results = format.parse("not_a_constant");
        assert!(results.is_empty());

        let value = CoreValue::Int {
            value: 12345,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);
        assert!(conversions.is_empty());
    }
}
