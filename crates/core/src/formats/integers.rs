//! Integer formats (decimal, with endianness handling).

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

// =============================================================================
// Number Trait Detection
// =============================================================================

/// Maximum value for prime checking (10^12) - fast enough to feel instant.
const PRIME_CHECK_LIMIT: i128 = 1_000_000_000_000;

/// Check if n is prime. Returns None if n is too large to check efficiently.
fn is_prime(n: i128) -> Option<bool> {
    if n > PRIME_CHECK_LIMIT {
        return None;
    }
    if n < 2 {
        return Some(false);
    }
    if n == 2 || n == 3 {
        return Some(true);
    }
    if n % 2 == 0 || n % 3 == 0 {
        return Some(false);
    }
    let sqrt = (n as f64).sqrt() as i128;
    // 6k +/- 1 optimization
    let mut i = 5;
    while i <= sqrt {
        if n % i == 0 || n % (i + 2) == 0 {
            return Some(false);
        }
        i += 6;
    }
    Some(true)
}

/// First 93 Fibonacci numbers (all that fit in i128).
const FIBONACCI: &[i128] = &[
    0,
    1,
    1,
    2,
    3,
    5,
    8,
    13,
    21,
    34,
    55,
    89,
    144,
    233,
    377,
    610,
    987,
    1597,
    2584,
    4181,
    6765,
    10946,
    17711,
    28657,
    46368,
    75025,
    121393,
    196418,
    317811,
    514229,
    832040,
    1346269,
    2178309,
    3524578,
    5702887,
    9227465,
    14930352,
    24157817,
    39088169,
    63245986,
    102334155,
    165580141,
    267914296,
    433494437,
    701408733,
    1134903170,
    1836311903,
    2971215073,
    4807526976,
    7778742049,
    12586269025,
    20365011074,
    32951280099,
    53316291173,
    86267571272,
    139583862445,
    225851433717,
    365435296162,
    591286729879,
    956722026041,
    1548008755920,
    2504730781961,
    4052739537881,
    6557470319842,
    10610209857723,
    17167680177565,
    27777890035288,
    44945570212853,
    72723460248141,
    117669030460994,
    190392490709135,
    308061521170129,
    498454011879264,
    806515533049393,
    1304969544928657,
    2111485077978050,
    3416454622906707,
    5527939700884757,
    8944394323791464,
    14472334024676221,
    23416728348467685,
    37889062373143906,
    61305790721611591,
    99194853094755497,
    160500643816367088,
    259695496911122585,
    420196140727489673,
    679891637638612258,
    1100087778366101931,
    1779979416004714189,
    2880067194370816120,
    4660046610375530309,
    7540113804746346429,
];

/// Returns the Fibonacci index if n is a Fibonacci number.
fn fibonacci_index(n: i128) -> Option<usize> {
    FIBONACCI.binary_search(&n).ok()
}

/// Perfect numbers that fit in i128 (only 8 known perfect numbers, 4 fit in i64).
const PERFECT_NUMBERS: &[i128] = &[
    6,
    28,
    496,
    8128,
    33550336,
    8589869056,
    137438691328,
    2305843008139952128,
];

/// Check if n is a perfect number.
fn is_perfect(n: i128) -> bool {
    PERFECT_NUMBERS.contains(&n)
}

/// Returns k if n is triangular (n = k*(k+1)/2).
fn triangular_root(n: i128) -> Option<i128> {
    if n < 0 {
        return None;
    }
    // Solve k^2 + k - 2n = 0: k = (-1 + sqrt(1 + 8n)) / 2
    // Use checked arithmetic to avoid overflow for large n
    let disc = 8_i128.checked_mul(n)?.checked_add(1)?;
    let sqrt = (disc as f64).sqrt() as i128;
    // Verify it's exact
    if sqrt * sqrt != disc {
        return None;
    }
    if (sqrt - 1) % 2 != 0 {
        return None;
    }
    let k = (sqrt - 1) / 2;
    // Double-check: k*(k+1)/2 == n
    if k * (k + 1) / 2 == n {
        Some(k)
    } else {
        None
    }
}

/// Returns the square root if n is a perfect square.
fn perfect_square_root(n: i128) -> Option<i128> {
    if n < 0 {
        return None;
    }
    let sqrt = (n as f64).sqrt() as i128;
    // Check both sqrt and sqrt+1 due to floating point imprecision
    if sqrt * sqrt == n {
        Some(sqrt)
    } else if (sqrt + 1) * (sqrt + 1) == n {
        Some(sqrt + 1)
    } else {
        None
    }
}

/// Factorials up to 33! (all that fit in i128).
const FACTORIALS: &[(i128, u32)] = &[
    (1, 0),
    (1, 1),
    (2, 2),
    (6, 3),
    (24, 4),
    (120, 5),
    (720, 6),
    (5040, 7),
    (40320, 8),
    (362880, 9),
    (3628800, 10),
    (39916800, 11),
    (479001600, 12),
    (6227020800, 13),
    (87178291200, 14),
    (1307674368000, 15),
    (20922789888000, 16),
    (355687428096000, 17),
    (6402373705728000, 18),
    (121645100408832000, 19),
    (2432902008176640000, 20),
    (51090942171709440000, 21),
    (1124000727777607680000, 22),
    (25852016738884976640000, 23),
    (620448401733239439360000, 24),
    (15511210043330985984000000, 25),
    (403291461126605635584000000, 26),
    (10888869450418352160768000000, 27),
    (304888344611713860501504000000, 28),
    (8841761993739701954543616000000, 29),
    (265252859812191058636308480000000, 30),
    (8222838654177922817725562880000000, 31),
    (263130836933693530167218012160000000, 32),
    (8683317618811886495518194401280000000, 33),
];

/// Returns k if n = k! (factorial).
fn factorial_of(n: i128) -> Option<u32> {
    FACTORIALS
        .binary_search_by_key(&n, |&(val, _)| val)
        .ok()
        .map(|idx| FACTORIALS[idx].1)
}

/// Check if a number passes the Luhn checksum (used in OCR, credit cards, IMEI, etc.).
/// Returns true if the number has a valid Luhn check digit.
/// Only valid for positive integers with at least 2 digits.
fn is_valid_luhn(n: i128) -> bool {
    if n < 10 {
        return false; // Need at least 2 digits
    }

    let mut sum = 0i128;
    let mut double = false;
    let mut num = n;

    while num > 0 {
        let mut digit = num % 10;
        num /= 10;

        if double {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }

        sum += digit;
        double = !double;
    }

    sum % 10 == 0
}

/// Check if a 10-digit number is a valid ISBN-10 (when check digit is 0-9, not X).
/// ISBN-10 uses mod 11 weighted checksum: sum of digit[i] * (10-i) must be divisible by 11.
fn is_valid_isbn10_numeric(n: i128) -> bool {
    // Must be exactly 10 digits
    if !(1_000_000_000..10_000_000_000).contains(&n) {
        return false;
    }

    let mut sum = 0i128;
    let mut num = n;
    let mut weight = 1;

    while num > 0 {
        let digit = num % 10;
        num /= 10;
        sum += digit * weight;
        weight += 1;
    }

    sum % 11 == 0
}

/// Check if a 13-digit number is a valid ISBN-13 or EAN-13.
/// Uses alternating weights of 1 and 3, sum must be divisible by 10.
fn is_valid_ean13(n: i128) -> bool {
    // Must be exactly 13 digits
    if !(1_000_000_000_000..10_000_000_000_000).contains(&n) {
        return false;
    }

    let mut sum = 0i128;
    let mut num = n;
    let mut weight = 1; // Rightmost digit has weight 1

    while num > 0 {
        let digit = num % 10;
        num /= 10;
        sum += digit * weight;
        weight = if weight == 1 { 3 } else { 1 };
    }

    sum % 10 == 0
}

/// Check if a 12-digit number is a valid UPC-A.
/// Same algorithm as EAN-13 (UPC-A is EAN-13 with implicit leading 0).
fn is_valid_upc_a(n: i128) -> bool {
    // Must be exactly 12 digits
    if !(100_000_000_000..1_000_000_000_000).contains(&n) {
        return false;
    }

    let mut sum = 0i128;
    let mut num = n;
    let mut weight = 1; // Rightmost digit has weight 1

    while num > 0 {
        let digit = num % 10;
        num /= 10;
        sum += digit * weight;
        weight = if weight == 1 { 3 } else { 1 };
    }

    sum % 10 == 0
}

/// Check if an 8-digit number is a valid EAN-8.
/// Same algorithm as EAN-13, just shorter.
fn is_valid_ean8(n: i128) -> bool {
    // Must be exactly 8 digits
    if !(10_000_000..100_000_000).contains(&n) {
        return false;
    }

    let mut sum = 0i128;
    let mut num = n;
    let mut weight = 1;

    while num > 0 {
        let digit = num % 10;
        num /= 10;
        sum += digit * weight;
        weight = if weight == 1 { 3 } else { 1 };
    }

    sum % 10 == 0
}

pub struct DecimalFormat;

impl Format for DecimalFormat {
    fn id(&self) -> &'static str {
        "decimal"
    }

    fn name(&self) -> &'static str {
        "Decimal Integer"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Numbers",
            description: "Decimal integer parsing",
            examples: &["1763574200", "-42", "255"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Ok(value) = input.parse::<i128>() else {
            return vec![];
        };

        // Higher confidence for pure numeric input
        let confidence = if input.starts_with('-') || input.starts_with('+') {
            0.9
        } else if input.chars().all(|c| c.is_ascii_digit()) {
            0.85
        } else {
            0.5
        };

        vec![Interpretation {
            value: CoreValue::Int {
                value,
                original_bytes: None,
            },
            source_format: "decimal".to_string(),
            confidence,
            description: format!("Integer: {value}"),
            rich_display: vec![],
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Int { .. })
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Int { value, .. } => Some(value.to_string()),
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: int_val, .. } = value else {
            return vec![];
        };

        let mut conversions = Vec::new();

        // Only show base conversions for non-negative values that fit in u64
        // (negative numbers and huge numbers have less useful hex/binary representations)
        if *int_val >= 0 && *int_val <= u64::MAX as i128 {
            let val = *int_val as u64;

            // Hex representation - Representation kind
            let hex_display = format!("0x{:X}", val);
            conversions.push(Conversion {
                value: CoreValue::String(hex_display.clone()),
                target_format: "hex-int".to_string(),
                display: hex_display.clone(),
                path: vec!["hex-int".to_string()],
                steps: vec![ConversionStep {
                    format: "hex-int".to_string(),
                    value: CoreValue::String(hex_display.clone()),
                    display: hex_display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                ..Default::default()
            });

            // Binary representation (only for reasonably small numbers)
            if val <= 0xFFFF_FFFF {
                let bin_display = format!("0b{:b}", val);
                conversions.push(Conversion {
                    value: CoreValue::String(bin_display.clone()),
                    target_format: "binary-int".to_string(),
                    display: bin_display.clone(),
                    path: vec!["binary-int".to_string()],
                    steps: vec![ConversionStep {
                        format: "binary-int".to_string(),
                        value: CoreValue::String(bin_display.clone()),
                        display: bin_display,
                    }],
                    priority: ConversionPriority::Semantic,
                    kind: ConversionKind::Representation,
                    display_only: true,
                    ..Default::default()
                });
            }

            // Octal representation
            let oct_display = format!("0o{:o}", val);
            conversions.push(Conversion {
                value: CoreValue::String(oct_display.clone()),
                target_format: "octal-int".to_string(),
                display: oct_display.clone(),
                path: vec!["octal-int".to_string()],
                steps: vec![ConversionStep {
                    format: "octal-int".to_string(),
                    value: CoreValue::String(oct_display.clone()),
                    display: oct_display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                ..Default::default()
            });

            // ASCII/Unicode character representation
            // Show character for printable ASCII (32-126) and valid Unicode codepoints
            if let Some(ch) = char::from_u32(val as u32) {
                // Only show for reasonable Unicode range and if it's a "useful" character
                if val <= 0x10FFFF {
                    let display = if val < 32 {
                        // Control characters - show name
                        let name = match val {
                            0 => "NUL (null)",
                            1 => "SOH (start of heading)",
                            2 => "STX (start of text)",
                            3 => "ETX (end of text)",
                            4 => "EOT (end of transmission)",
                            5 => "ENQ (enquiry)",
                            6 => "ACK (acknowledge)",
                            7 => "BEL (bell)",
                            8 => "BS (backspace)",
                            9 => "HT (horizontal tab)",
                            10 => "LF (line feed)",
                            11 => "VT (vertical tab)",
                            12 => "FF (form feed)",
                            13 => "CR (carriage return)",
                            14 => "SO (shift out)",
                            15 => "SI (shift in)",
                            16 => "DLE (data link escape)",
                            17 => "DC1 (device control 1)",
                            18 => "DC2 (device control 2)",
                            19 => "DC3 (device control 3)",
                            20 => "DC4 (device control 4)",
                            21 => "NAK (negative ack)",
                            22 => "SYN (synchronous idle)",
                            23 => "ETB (end of trans. block)",
                            24 => "CAN (cancel)",
                            25 => "EM (end of medium)",
                            26 => "SUB (substitute)",
                            27 => "ESC (escape)",
                            28 => "FS (file separator)",
                            29 => "GS (group separator)",
                            30 => "RS (record separator)",
                            31 => "US (unit separator)",
                            _ => "control",
                        };
                        format!("'{}' {}", ch.escape_unicode(), name)
                    } else if val == 127 {
                        "'\\u{7f}' DEL (delete)".to_string()
                    } else if val <= 126 {
                        // Printable ASCII
                        format!("'{}'", ch)
                    } else if ch.is_alphanumeric() || ch.is_ascii_punctuation() || val > 127 {
                        // Show Unicode characters that are "interesting"
                        format!("'{}' (U+{:04X})", ch, val)
                    } else {
                        // Skip other non-printable characters
                        String::new()
                    };

                    if !display.is_empty() {
                        conversions.push(Conversion {
                            value: CoreValue::String(ch.to_string()),
                            target_format: "char".to_string(),
                            display: display.clone(),
                            path: vec!["char".to_string()],
                            steps: vec![ConversionStep {
                                format: "char".to_string(),
                                value: CoreValue::String(ch.to_string()),
                                display,
                            }],
                            priority: ConversionPriority::Semantic,
                            kind: ConversionKind::Representation,
                            display_only: true,
                            ..Default::default()
                        });
                    }
                }
            }

            // =========================================================
            // Number Traits (observations about the value)
            // =========================================================

            // Power of 2 detection (for values >= 2)
            if val >= 2 && val.is_power_of_two() {
                let exp = val.trailing_zeros();
                let display = format!("2^{}", exp);
                conversions.push(Conversion {
                    value: CoreValue::String(display.clone()),
                    target_format: "power-of-2".to_string(),
                    display: display.clone(),
                    path: vec!["power-of-2".to_string()],
                    steps: vec![ConversionStep {
                        format: "power-of-2".to_string(),
                        value: CoreValue::String(display.clone()),
                        display,
                    }],
                    priority: ConversionPriority::Semantic,
                    kind: ConversionKind::Trait,
                    display_only: true,
                    ..Default::default()
                });
            }

            // Perfect square detection (for values >= 4, skip 0 and 1)
            if val >= 4 {
                if let Some(root) = perfect_square_root(*int_val) {
                    let display = format!("{}²", root);
                    conversions.push(Conversion {
                        value: CoreValue::String(display.clone()),
                        target_format: "perfect-square".to_string(),
                        display: display.clone(),
                        path: vec!["perfect-square".to_string()],
                        steps: vec![ConversionStep {
                            format: "perfect-square".to_string(),
                            value: CoreValue::String(display.clone()),
                            display,
                        }],
                        priority: ConversionPriority::Semantic,
                        kind: ConversionKind::Trait,
                        display_only: true,
                        ..Default::default()
                    });
                }
            }
        }

        // Traits that work on i128 (including negative numbers)

        // Prime detection (only for positive numbers up to 10^12)
        if *int_val >= 2 {
            if let Some(true) = is_prime(*int_val) {
                let display = "prime".to_string();
                conversions.push(Conversion {
                    value: CoreValue::String(display.clone()),
                    target_format: "prime".to_string(),
                    display: display.clone(),
                    path: vec!["prime".to_string()],
                    steps: vec![ConversionStep {
                        format: "prime".to_string(),
                        value: CoreValue::String(display.clone()),
                        display,
                    }],
                    priority: ConversionPriority::Semantic,
                    kind: ConversionKind::Trait,
                    display_only: true,
                    ..Default::default()
                });
            }
        }

        // Fibonacci detection (for non-negative values)
        if *int_val >= 0 {
            if let Some(idx) = fibonacci_index(*int_val) {
                // Skip fib(0)=0 and fib(1)=1, fib(2)=1 as they're trivial
                if idx >= 3 {
                    let display = format!("fib({})", idx);
                    conversions.push(Conversion {
                        value: CoreValue::String(display.clone()),
                        target_format: "fibonacci".to_string(),
                        display: display.clone(),
                        path: vec!["fibonacci".to_string()],
                        steps: vec![ConversionStep {
                            format: "fibonacci".to_string(),
                            value: CoreValue::String(display.clone()),
                            display,
                        }],
                        priority: ConversionPriority::Semantic,
                        kind: ConversionKind::Trait,
                        display_only: true,
                        ..Default::default()
                    });
                }
            }
        }

        // Perfect number detection
        if is_perfect(*int_val) {
            let display = "perfect".to_string();
            conversions.push(Conversion {
                value: CoreValue::String(display.clone()),
                target_format: "perfect-number".to_string(),
                display: display.clone(),
                path: vec!["perfect-number".to_string()],
                steps: vec![ConversionStep {
                    format: "perfect-number".to_string(),
                    value: CoreValue::String(display.clone()),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Trait,
                display_only: true,
                ..Default::default()
            });
        }

        // Triangular number detection (for positive values, skip trivial 0, 1)
        if *int_val >= 3 {
            if let Some(k) = triangular_root(*int_val) {
                let display = format!("triangular({})", k);
                conversions.push(Conversion {
                    value: CoreValue::String(display.clone()),
                    target_format: "triangular".to_string(),
                    display: display.clone(),
                    path: vec!["triangular".to_string()],
                    steps: vec![ConversionStep {
                        format: "triangular".to_string(),
                        value: CoreValue::String(display.clone()),
                        display,
                    }],
                    priority: ConversionPriority::Semantic,
                    kind: ConversionKind::Trait,
                    display_only: true,
                    ..Default::default()
                });
            }
        }

        // Factorial detection (skip 0!=1, 1!=1, 2!=2 as trivial)
        if *int_val >= 6 {
            if let Some(k) = factorial_of(*int_val) {
                if k >= 3 {
                    let display = format!("{}!", k);
                    conversions.push(Conversion {
                        value: CoreValue::String(display.clone()),
                        target_format: "factorial".to_string(),
                        display: display.clone(),
                        path: vec!["factorial".to_string()],
                        steps: vec![ConversionStep {
                            format: "factorial".to_string(),
                            value: CoreValue::String(display.clone()),
                            display,
                        }],
                        priority: ConversionPriority::Semantic,
                        kind: ConversionKind::Trait,
                        display_only: true,
                        ..Default::default()
                    });
                }
            }
        }

        // Luhn checksum detection (OCR references, credit cards, IMEI, etc.)
        if *int_val >= 10 && is_valid_luhn(*int_val) {
            let display = "valid Luhn checksum".to_string();
            conversions.push(Conversion {
                value: CoreValue::String(display.clone()),
                target_format: "luhn".to_string(),
                display: display.clone(),
                path: vec!["luhn".to_string()],
                steps: vec![ConversionStep {
                    format: "luhn".to_string(),
                    value: CoreValue::String(display.clone()),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Trait,
                display_only: true,
                ..Default::default()
            });
        }

        // ISBN-10 detection (10 digits, check digit 0-9)
        if is_valid_isbn10_numeric(*int_val) {
            let display = "valid ISBN-10".to_string();
            conversions.push(Conversion {
                value: CoreValue::String(display.clone()),
                target_format: "isbn-10".to_string(),
                display: display.clone(),
                path: vec!["isbn-10".to_string()],
                steps: vec![ConversionStep {
                    format: "isbn-10".to_string(),
                    value: CoreValue::String(display.clone()),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Trait,
                display_only: true,
                ..Default::default()
            });
        }

        // ISBN-13 / EAN-13 detection (13 digits)
        if is_valid_ean13(*int_val) {
            // Check if it's an ISBN-13 (starts with 978 or 979)
            let is_isbn = *int_val >= 9_780_000_000_000 && *int_val < 9_800_000_000_000;
            let display = if is_isbn {
                "valid ISBN-13".to_string()
            } else {
                "valid EAN-13".to_string()
            };
            let format_name = if is_isbn { "isbn-13" } else { "ean-13" };
            conversions.push(Conversion {
                value: CoreValue::String(display.clone()),
                target_format: format_name.to_string(),
                display: display.clone(),
                path: vec![format_name.to_string()],
                steps: vec![ConversionStep {
                    format: format_name.to_string(),
                    value: CoreValue::String(display.clone()),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Trait,
                display_only: true,
                ..Default::default()
            });
        }

        // UPC-A detection (12 digits)
        if is_valid_upc_a(*int_val) {
            let display = "valid UPC-A".to_string();
            conversions.push(Conversion {
                value: CoreValue::String(display.clone()),
                target_format: "upc-a".to_string(),
                display: display.clone(),
                path: vec!["upc-a".to_string()],
                steps: vec![ConversionStep {
                    format: "upc-a".to_string(),
                    value: CoreValue::String(display.clone()),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Trait,
                display_only: true,
                ..Default::default()
            });
        }

        // EAN-8 detection (8 digits)
        if is_valid_ean8(*int_val) {
            let display = "valid EAN-8".to_string();
            conversions.push(Conversion {
                value: CoreValue::String(display.clone()),
                target_format: "ean-8".to_string(),
                display: display.clone(),
                path: vec!["ean-8".to_string()],
                steps: vec![ConversionStep {
                    format: "ean-8".to_string(),
                    value: CoreValue::String(display.clone()),
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Trait,
                display_only: true,
                ..Default::default()
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["dec", "int", "num"]
    }
}

/// Converts bytes to integers (both endianness).
pub struct BytesToIntFormat;

impl BytesToIntFormat {
    /// Convert bytes to i128 (big-endian).
    fn bytes_to_int_be(bytes: &[u8]) -> i128 {
        let mut result: i128 = 0;
        for &b in bytes {
            result = (result << 8) | (b as i128);
        }
        result
    }

    /// Convert bytes to i128 (little-endian).
    fn bytes_to_int_le(bytes: &[u8]) -> i128 {
        let mut result: i128 = 0;
        for (i, &b) in bytes.iter().enumerate() {
            result |= (b as i128) << (i * 8);
        }
        result
    }
}

impl Format for BytesToIntFormat {
    fn id(&self) -> &'static str {
        "bytes-to-int"
    }

    fn name(&self) -> &'static str {
        "Bytes to Integer"
    }

    fn parse(&self, _input: &str) -> Vec<Interpretation> {
        // This format doesn't parse strings directly
        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        // Only convert reasonable byte lengths (1-16 bytes)
        if bytes.is_empty() || bytes.len() > 16 {
            return vec![];
        }

        // Skip if bytes look like text (valid UTF-8 with printable content)
        // This avoids interpreting "hello" or "héllo" as big-endian integers
        if bytes.len() > 4 {
            if let Ok(text) = std::str::from_utf8(bytes) {
                // Check if it's mostly printable text (allow some control chars like newline)
                let printable_ratio = text
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
                    .count() as f32
                    / text.chars().count() as f32;
                if printable_ratio > 0.8 {
                    return vec![];
                }
            }
        }

        let be_value = Self::bytes_to_int_be(bytes);
        let le_value = Self::bytes_to_int_le(bytes);

        let be_int = CoreValue::Int {
            value: be_value,
            original_bytes: Some(bytes.clone()),
        };
        let be_display = be_value.to_string();

        let mut conversions = vec![Conversion {
            value: be_int,
            target_format: "int-be".to_string(),
            display: be_display.clone(),
            path: vec!["int-be".to_string()],
            steps: vec![ConversionStep {
                format: "int-be".to_string(),
                value: CoreValue::Int {
                    value: be_value,
                    original_bytes: Some(bytes.clone()),
                },
                display: be_display,
            }],
            is_lossy: false,
            priority: ConversionPriority::Raw,
            display_only: false,
            kind: ConversionKind::default(),
            hidden: false,
            rich_display: vec![],
        }];

        // Only add little-endian if it's different
        if le_value != be_value {
            let le_int = CoreValue::Int {
                value: le_value,
                original_bytes: Some(bytes.clone()),
            };
            let le_display = le_value.to_string();

            conversions.push(Conversion {
                value: le_int.clone(),
                target_format: "int-le".to_string(),
                display: le_display.clone(),
                path: vec!["int-le".to_string()],
                steps: vec![ConversionStep {
                    format: "int-le".to_string(),
                    value: le_int,
                    display: le_display,
                }],
                is_lossy: false,
                priority: ConversionPriority::Raw,
                display_only: false,
                kind: ConversionKind::default(),
                hidden: false,
                rich_display: vec![],
            });
        }

        conversions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal() {
        let format = DecimalFormat;
        let results = format.parse("1763574200");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, 1763574200);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_parse_negative() {
        let format = DecimalFormat;
        let results = format.parse("-42");

        assert_eq!(results.len(), 1);
        if let CoreValue::Int { value, .. } = &results[0].value {
            assert_eq!(*value, -42);
        } else {
            panic!("Expected Int");
        }
    }

    #[test]
    fn test_bytes_to_int_be() {
        let bytes = vec![0x69, 0x1E, 0x01, 0xB8];
        let value = BytesToIntFormat::bytes_to_int_be(&bytes);
        assert_eq!(value, 1763574200);
    }

    #[test]
    fn test_bytes_to_int_le() {
        let bytes = vec![0x69, 0x1E, 0x01, 0xB8];
        let value = BytesToIntFormat::bytes_to_int_le(&bytes);
        // LE: bytes reversed = 0xB8011E69 = 3087081065
        assert_eq!(value, 3087081065);
    }

    #[test]
    fn test_bytes_to_int_conversions() {
        let format = BytesToIntFormat;
        let value = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 2);

        let be = conversions
            .iter()
            .find(|c| c.target_format == "int-be")
            .unwrap();
        assert_eq!(be.display, "1763574200");

        let le = conversions
            .iter()
            .find(|c| c.target_format == "int-le")
            .unwrap();
        assert_eq!(le.display, "3087081065");
    }

    #[test]
    fn test_luhn_valid() {
        // Known valid Luhn numbers
        assert!(is_valid_luhn(79927398713)); // Wikipedia example
        assert!(is_valid_luhn(4532015112830366)); // Valid credit card pattern
        assert!(is_valid_luhn(49927398716)); // Another Wikipedia example
    }

    #[test]
    fn test_luhn_invalid() {
        // Invalid Luhn numbers
        assert!(!is_valid_luhn(79927398710)); // Wrong check digit
        assert!(!is_valid_luhn(1234567890)); // Random number
        assert!(!is_valid_luhn(9)); // Single digit
        assert!(!is_valid_luhn(0)); // Zero
    }

    #[test]
    fn test_luhn_trait_conversion() {
        let format = DecimalFormat;
        let int_value = CoreValue::Int {
            value: 79927398713, // Valid Luhn
            original_bytes: None,
        };
        let conversions = format.conversions(&int_value);

        let luhn = conversions.iter().find(|c| c.target_format == "luhn");
        assert!(luhn.is_some(), "Should have Luhn trait");
        assert_eq!(luhn.unwrap().display, "valid Luhn checksum");
    }

    #[test]
    fn test_isbn10_valid() {
        // Known valid ISBN-10 numbers (without X check digit)
        // Note: ISBN-10 starting with 0 won't be detected as integers since leading 0 is dropped
        // 9654487659: 9*10 + 6*9 + 5*8 + 4*7 + 4*6 + 8*5 + 7*4 + 6*3 + 5*2 + 9*1 = 352, 352 % 11 = 0
        assert!(is_valid_isbn10_numeric(9654487659));
        // 1593279280: Programming Rust (O'Reilly)
        assert!(is_valid_isbn10_numeric(1593279280));
    }

    #[test]
    fn test_isbn10_invalid() {
        assert!(!is_valid_isbn10_numeric(9654487650)); // Wrong check digit
        assert!(!is_valid_isbn10_numeric(123456789)); // 9 digits
        assert!(!is_valid_isbn10_numeric(12345678901)); // 11 digits
    }

    #[test]
    fn test_ean13_valid() {
        // ISBN-13
        assert!(is_valid_ean13(9780306406157)); // 978-0-306-40615-7
        assert!(is_valid_ean13(9780201633610)); // 978-0-201-63361-0
                                                // Regular EAN-13 (not ISBN)
        assert!(is_valid_ean13(5901234123457)); // Example barcode
    }

    #[test]
    fn test_ean13_invalid() {
        assert!(!is_valid_ean13(9780306406158)); // Wrong check digit
        assert!(!is_valid_ean13(978030640615)); // 12 digits
    }

    #[test]
    fn test_upc_a_valid() {
        assert!(is_valid_upc_a(123456789012)); // Example
                                               // Note: Can't use leading 0 in integer literals (octal)
                                               // 042100005264 as decimal is tested via CLI
    }

    #[test]
    fn test_ean8_valid() {
        assert!(is_valid_ean8(96385074)); // Example EAN-8
        assert!(is_valid_ean8(65833254)); // Another example
    }

    #[test]
    fn test_isbn13_trait_conversion() {
        let format = DecimalFormat;
        let int_value = CoreValue::Int {
            value: 9780306406157, // Valid ISBN-13
            original_bytes: None,
        };
        let conversions = format.conversions(&int_value);

        let isbn = conversions.iter().find(|c| c.target_format == "isbn-13");
        assert!(isbn.is_some(), "Should have ISBN-13 trait");
        assert_eq!(isbn.unwrap().display, "valid ISBN-13");
    }
}
