//! Unix file permissions format: bidirectional octal ↔ symbolic conversion.
//!
//! - Octal → Symbolic: `755` → `rwxr-xr-x`
//! - Symbolic → Octal: `rwxr-xr-x` → `0755`
//!
//! Supports special bits (setuid, setgid, sticky).

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
    RichDisplay, RichDisplayOption,
};

// =============================================================================
// Permission Parsing and Formatting
// =============================================================================

/// Parse octal permission string to decimal value.
/// Accepts: "755", "0755", "0o755"
fn parse_octal_permission(input: &str) -> Option<(u16, f32)> {
    let trimmed = input.trim();

    // Determine format and strip prefix
    let (digits, confidence) = if let Some(rest) = trimmed.strip_prefix("0o") {
        (rest, 0.95) // Rust-style 0o prefix
    } else if let Some(rest) = trimmed.strip_prefix("0O") {
        (rest, 0.95)
    } else if trimmed.starts_with('0')
        && trimmed.len() == 4
        && trimmed.chars().all(|c| c.is_ascii_digit())
    {
        // Classic Unix style: 0755 (4 digits starting with 0)
        (trimmed, 0.92)
    } else if trimmed.len() == 3 && trimmed.chars().all(|c| matches!(c, '0'..='7')) {
        // Simple 3-digit: 755
        (trimmed, 0.85)
    } else if trimmed.len() == 4 && trimmed.chars().all(|c| matches!(c, '0'..='7')) {
        // 4-digit with special bits: 4755
        (trimmed, 0.88)
    } else {
        return None;
    };

    // Validate all digits are octal
    if !digits.chars().all(|c| matches!(c, '0'..='7')) {
        return None;
    }

    // Parse as octal
    let value = u16::from_str_radix(digits, 8).ok()?;

    // Valid permission range: 0-7777 (0-4095 decimal)
    if value > 0o7777 {
        return None;
    }

    Some((value, confidence))
}

/// Parse symbolic permission string to decimal value.
/// Accepts: "rwxr-xr-x", "rw-r--r--", "drwxr-xr-x" (with optional type prefix)
fn parse_symbolic_permission(input: &str) -> Option<(u16, f32)> {
    let trimmed = input.trim();

    // Check for optional file type prefix (d, l, -, b, c, p, s)
    let perms = if trimmed.len() == 10
        && matches!(
            trimmed.chars().next(),
            Some('d' | 'l' | '-' | 'b' | 'c' | 'p' | 's')
        ) {
        &trimmed[1..]
    } else if trimmed.len() == 9 {
        trimmed
    } else {
        return None;
    };

    // Parse 9 permission characters
    let chars: Vec<char> = perms.chars().collect();
    if chars.len() != 9 {
        return None;
    }

    let mut value: u16 = 0;

    // Owner permissions (chars 0-2)
    if chars[0] == 'r' {
        value |= 0o400;
    } else if chars[0] != '-' {
        return None;
    }
    if chars[1] == 'w' {
        value |= 0o200;
    } else if chars[1] != '-' {
        return None;
    }
    match chars[2] {
        'x' => value |= 0o100,
        's' => value |= 0o4100, // setuid + execute
        'S' => value |= 0o4000, // setuid without execute
        '-' => {}
        _ => return None,
    }

    // Group permissions (chars 3-5)
    if chars[3] == 'r' {
        value |= 0o040;
    } else if chars[3] != '-' {
        return None;
    }
    if chars[4] == 'w' {
        value |= 0o020;
    } else if chars[4] != '-' {
        return None;
    }
    match chars[5] {
        'x' => value |= 0o010,
        's' => value |= 0o2010, // setgid + execute
        'S' => value |= 0o2000, // setgid without execute
        '-' => {}
        _ => return None,
    }

    // Other permissions (chars 6-8)
    if chars[6] == 'r' {
        value |= 0o004;
    } else if chars[6] != '-' {
        return None;
    }
    if chars[7] == 'w' {
        value |= 0o002;
    } else if chars[7] != '-' {
        return None;
    }
    match chars[8] {
        'x' => value |= 0o001,
        't' => value |= 0o1001, // sticky + execute
        'T' => value |= 0o1000, // sticky without execute
        '-' => {}
        _ => return None,
    }

    Some((value, 0.95))
}

/// Convert permission value to symbolic string.
fn to_symbolic(value: u16) -> String {
    let mut result = String::with_capacity(9);

    // Owner
    result.push(if value & 0o400 != 0 { 'r' } else { '-' });
    result.push(if value & 0o200 != 0 { 'w' } else { '-' });
    result.push(match (value & 0o4000 != 0, value & 0o100 != 0) {
        (true, true) => 's',
        (true, false) => 'S',
        (false, true) => 'x',
        (false, false) => '-',
    });

    // Group
    result.push(if value & 0o040 != 0 { 'r' } else { '-' });
    result.push(if value & 0o020 != 0 { 'w' } else { '-' });
    result.push(match (value & 0o2000 != 0, value & 0o010 != 0) {
        (true, true) => 's',
        (true, false) => 'S',
        (false, true) => 'x',
        (false, false) => '-',
    });

    // Other
    result.push(if value & 0o004 != 0 { 'r' } else { '-' });
    result.push(if value & 0o002 != 0 { 'w' } else { '-' });
    result.push(match (value & 0o1000 != 0, value & 0o001 != 0) {
        (true, true) => 't',
        (true, false) => 'T',
        (false, true) => 'x',
        (false, false) => '-',
    });

    result
}

/// Convert permission value to octal string with leading zero.
fn to_octal(value: u16) -> String {
    if value > 0o777 {
        format!("{:04o}", value) // 4 digits for special bits
    } else {
        format!("0{:03o}", value) // 0755 style
    }
}

/// Describe a single permission set (owner/group/other).
fn describe_perms(r: bool, w: bool, x: bool) -> String {
    let mut parts = Vec::new();
    if r {
        parts.push("read");
    }
    if w {
        parts.push("write");
    }
    if x {
        parts.push("execute");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// Describe special bits.
fn describe_special(value: u16) -> String {
    let mut parts = Vec::new();
    if value & 0o4000 != 0 {
        parts.push("setuid");
    }
    if value & 0o2000 != 0 {
        parts.push("setgid");
    }
    if value & 0o1000 != 0 {
        parts.push("sticky");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// Build rich display for permission breakdown.
fn build_rich_display(value: u16) -> RichDisplayOption {
    let owner_r = value & 0o400 != 0;
    let owner_w = value & 0o200 != 0;
    let owner_x = value & 0o100 != 0;
    let group_r = value & 0o040 != 0;
    let group_w = value & 0o020 != 0;
    let group_x = value & 0o010 != 0;
    let other_r = value & 0o004 != 0;
    let other_w = value & 0o002 != 0;
    let other_x = value & 0o001 != 0;

    let owner_bits = format!(
        "{}{}{}",
        if owner_r { 'r' } else { '-' },
        if owner_w { 'w' } else { '-' },
        if owner_x { 'x' } else { '-' }
    );
    let group_bits = format!(
        "{}{}{}",
        if group_r { 'r' } else { '-' },
        if group_w { 'w' } else { '-' },
        if group_x { 'x' } else { '-' }
    );
    let other_bits = format!(
        "{}{}{}",
        if other_r { 'r' } else { '-' },
        if other_w { 'w' } else { '-' },
        if other_x { 'x' } else { '-' }
    );

    let mut pairs = vec![
        (
            "owner".to_string(),
            format!(
                "{} ({})",
                owner_bits,
                describe_perms(owner_r, owner_w, owner_x)
            ),
        ),
        (
            "group".to_string(),
            format!(
                "{} ({})",
                group_bits,
                describe_perms(group_r, group_w, group_x)
            ),
        ),
        (
            "other".to_string(),
            format!(
                "{} ({})",
                other_bits,
                describe_perms(other_r, other_w, other_x)
            ),
        ),
    ];

    // Only show special bits if any are set
    let special = value & 0o7000;
    if special != 0 {
        pairs.push(("special".to_string(), describe_special(value)));
    }

    RichDisplayOption::new(RichDisplay::KeyValue { pairs })
}

// =============================================================================
// Format Implementation
// =============================================================================

pub struct PermissionsFormat;

impl Format for PermissionsFormat {
    fn id(&self) -> &'static str {
        "permissions"
    }

    fn name(&self) -> &'static str {
        "Unix Permissions"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Reference",
            description: "Unix file permissions (octal ↔ symbolic)",
            examples: &["755", "rwxr-xr-x", "0644", "rw-r--r--"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try octal first
        if let Some((value, confidence)) = parse_octal_permission(input) {
            let symbolic = to_symbolic(value);
            let octal = to_octal(value);
            return vec![Interpretation {
                value: CoreValue::Int {
                    value: i128::from(value),
                    original_bytes: None,
                },
                source_format: "permissions".to_string(),
                confidence,
                description: format!("{} ({})", symbolic, octal),
                rich_display: vec![build_rich_display(value)],
            }];
        }

        // Try symbolic
        if let Some((value, confidence)) = parse_symbolic_permission(input) {
            let symbolic = to_symbolic(value);
            let octal = to_octal(value);
            return vec![Interpretation {
                value: CoreValue::Int {
                    value: i128::from(value),
                    original_bytes: None,
                },
                source_format: "permissions".to_string(),
                confidence,
                description: format!("{} ({})", symbolic, octal),
                rich_display: vec![build_rich_display(value)],
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

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: int_val, .. } = value else {
            return vec![];
        };

        // Only valid for permission range (0-4095)
        if *int_val < 0 || *int_val > 4095 {
            return vec![];
        }

        let perm_value = *int_val as u16;

        // Only show conversions for "realistic" permission values.
        // Real file permissions almost always have:
        // - At least read for owner (0o400)
        // - Or are one of the common special cases (0o000, 0o111, etc.)
        // This avoids showing permission conversions for arbitrary numbers like 1024.
        let owner_perms = (perm_value >> 6) & 0o7;
        let is_common_permission = owner_perms >= 4  // Owner has at least read
            || perm_value == 0  // No permissions (rare but valid)
            || perm_value == 0o111  // Execute only (for directories)
            || perm_value == 0o222  // Write only (rare)
            || perm_value == 0o333; // Write+execute (rare)

        if !is_common_permission {
            return vec![];
        }
        let symbolic = to_symbolic(perm_value);
        let octal = to_octal(perm_value);

        vec![
            // Symbolic representation
            Conversion {
                value: CoreValue::String(symbolic.clone()),
                target_format: "permission-symbolic".to_string(),
                display: symbolic.clone(),
                path: vec!["permission-symbolic".to_string()],
                steps: vec![ConversionStep {
                    format: "permission-symbolic".to_string(),
                    value: CoreValue::String(symbolic),
                    display: octal.clone(),
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                rich_display: vec![build_rich_display(perm_value)],
                ..Default::default()
            },
            // Octal representation
            Conversion {
                value: CoreValue::String(octal.clone()),
                target_format: "permission-octal".to_string(),
                display: octal.clone(),
                path: vec!["permission-octal".to_string()],
                steps: vec![ConversionStep {
                    format: "permission-octal".to_string(),
                    value: CoreValue::String(octal.clone()),
                    display: octal,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                ..Default::default()
            },
        ]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["perm", "chmod", "mode"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_octal_755() {
        let format = PermissionsFormat;
        let results = format.parse("755");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755);
        } else {
            panic!("Expected Int");
        }
        assert!(results[0].description.contains("rwxr-xr-x"));
    }

    #[test]
    fn test_parse_octal_0755() {
        let format = PermissionsFormat;
        let results = format.parse("0755");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_symbolic() {
        let format = PermissionsFormat;
        let results = format.parse("rwxr-xr-x");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_symbolic_644() {
        let format = PermissionsFormat;
        let results = format.parse("rw-r--r--");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o644);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_symbolic_with_prefix() {
        let format = PermissionsFormat;
        let results = format.parse("drwxr-xr-x");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o755);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_setuid() {
        let format = PermissionsFormat;
        let results = format.parse("4755");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o4755);
        } else {
            panic!("Expected Int");
        }
        assert!(results[0].description.contains("rwsr-xr-x"));
    }

    #[test]
    fn test_sticky() {
        let format = PermissionsFormat;
        let results = format.parse("1777");
        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 0o1777);
        } else {
            panic!("Expected Int");
        }
        assert!(results[0].description.contains("rwxrwxrwt"));
    }

    #[test]
    fn test_symbolic_to_octal() {
        assert_eq!(to_symbolic(0o755), "rwxr-xr-x");
        assert_eq!(to_symbolic(0o644), "rw-r--r--");
        assert_eq!(to_symbolic(0o777), "rwxrwxrwx");
        assert_eq!(to_symbolic(0o000), "---------");
        assert_eq!(to_symbolic(0o4755), "rwsr-xr-x");
        assert_eq!(to_symbolic(0o2755), "rwxr-sr-x");
        assert_eq!(to_symbolic(0o1777), "rwxrwxrwt");
    }

    #[test]
    fn test_conversions() {
        let format = PermissionsFormat;
        let value = CoreValue::Int {
            value: 0o755,
            original_bytes: None,
        };
        let conversions = format.conversions(&value);
        assert_eq!(conversions.len(), 2);

        let symbolic = conversions
            .iter()
            .find(|c| c.target_format == "permission-symbolic")
            .unwrap();
        assert_eq!(symbolic.display, "rwxr-xr-x");

        let octal = conversions
            .iter()
            .find(|c| c.target_format == "permission-octal")
            .unwrap();
        assert_eq!(octal.display, "0755");
    }

    #[test]
    fn test_no_match() {
        let format = PermissionsFormat;
        assert!(format.parse("999").is_empty()); // Invalid octal
        assert!(format.parse("rwxrwx").is_empty()); // Too short
        assert!(format.parse("hello").is_empty()); // Not permissions
    }
}
