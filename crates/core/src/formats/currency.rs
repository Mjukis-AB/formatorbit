//! Currency format.
//!
//! Parses currency amounts and converts between currencies using cached exchange rates.
//!
//! Supports:
//! - Explicit codes: `100 USD`, `50EUR`, `5kSEK`
//! - SI prefixes: `5kUSD` (5000), `2.5MEUR` (2.5 million)
//! - Symbols: `$100`, `€50`, `£25`, `100$`
//!
//! For ambiguous symbols ($, kr), returns multiple interpretations with
//! confidence adjusted based on user locale.

use std::env;

use crate::format::{Format, FormatInfo};
use crate::formats::currency_rates::{self, RateCache};
use crate::formats::units::parse_number;
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
};

pub struct CurrencyFormat;

/// Currency symbol to ISO code mapping.
/// Symbols that map to multiple currencies return all possibilities.
const SYMBOLS: &[(&str, &[&str])] = &[
    // Unambiguous symbols
    ("€", &["EUR"]),
    ("£", &["GBP"]),
    ("¥", &["JPY", "CNY"]),
    ("₹", &["INR"]),
    ("₽", &["RUB"]),
    ("₿", &["BTC"]),
    ("₩", &["KRW"]),
    ("₪", &["ILS"]),
    ("฿", &["THB"]),
    ("₫", &["VND"]),
    ("₴", &["UAH"]),
    ("₺", &["TRY"]),
    ("₦", &["NGN"]),
    ("₱", &["PHP"]),
    ("₡", &["CRC"]),
    ("₲", &["PYG"]),
    ("₵", &["GHS"]),
    ("₸", &["KZT"]),
    ("₼", &["AZN"]),
    ("₾", &["GEL"]),
    // Ambiguous symbols
    ("$", &["USD", "CAD", "AUD", "NZD", "SGD", "HKD"]),
    ("kr", &["SEK", "NOK", "DKK", "ISK"]),
    ("Fr", &["CHF"]),
    ("R$", &["BRL"]),
    ("C$", &["CAD"]),
    ("A$", &["AUD"]),
    ("NZ$", &["NZD"]),
    ("S$", &["SGD"]),
    ("HK$", &["HKD"]),
    ("zł", &["PLN"]),
    ("Kč", &["CZK"]),
    ("Ft", &["HUF"]),
    ("lei", &["RON"]),
    ("лв", &["BGN"]),
    ("RM", &["MYR"]),
    ("R", &["ZAR", "BRL"]),
];

/// ISO 4217 currency codes we recognize.
const CURRENCY_CODES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "CNY", "HKD", "SGD", "SEK", "NOK",
    "DKK", "ISK", "PLN", "CZK", "HUF", "RON", "BGN", "TRY", "RUB", "INR", "KRW", "THB", "MYR",
    "IDR", "PHP", "VND", "BRL", "MXN", "ARS", "CLP", "COP", "PEN", "ZAR", "EGP", "NGN", "KES",
    "ILS", "AED", "SAR", "QAR", "KWD", "BHD", "OMR", "UAH", "KZT", "GEL", "AZN",
];

/// Check if a currency code is known (built-in or plugin).
fn is_known_currency(code: &str) -> bool {
    let code_upper = code.to_uppercase();
    CURRENCY_CODES.iter().any(|c| *c == code_upper)
        || currency_rates::plugin_currency_codes()
            .iter()
            .any(|c| c.eq_ignore_ascii_case(code))
}

/// Get all known currency codes (built-in + plugin).
fn all_currency_codes() -> Vec<String> {
    let mut codes: Vec<String> = CURRENCY_CODES.iter().map(|s| (*s).to_string()).collect();
    codes.extend(currency_rates::plugin_currency_codes());
    codes
}

/// Display currencies - these are shown in conversions.
const DISPLAY_CURRENCIES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "CHF", "SEK", "NOK", "DKK", "CAD", "AUD",
];

/// SI prefixes for currency amounts.
const SI_PREFIXES: &[(&str, f64)] = &[
    ("G", 1_000_000_000.0), // giga/billion
    ("M", 1_000_000.0),     // mega/million
    ("k", 1_000.0),         // kilo/thousand
];

/// Country code to currency code mapping for locale detection.
const COUNTRY_TO_CURRENCY: &[(&str, &str)] = &[
    ("US", "USD"),
    ("CA", "CAD"),
    ("AU", "AUD"),
    ("NZ", "NZD"),
    ("GB", "GBP"),
    ("SE", "SEK"),
    ("NO", "NOK"),
    ("DK", "DKK"),
    ("IS", "ISK"),
    ("CH", "CHF"),
    ("JP", "JPY"),
    ("CN", "CNY"),
    ("HK", "HKD"),
    ("SG", "SGD"),
    ("IN", "INR"),
    ("KR", "KRW"),
    ("TH", "THB"),
    ("MY", "MYR"),
    ("ID", "IDR"),
    ("PH", "PHP"),
    ("VN", "VND"),
    ("BR", "BRL"),
    ("MX", "MXN"),
    ("AR", "ARS"),
    ("CL", "CLP"),
    ("CO", "COP"),
    ("PE", "PEN"),
    ("ZA", "ZAR"),
    ("NG", "NGN"),
    ("KE", "KES"),
    ("EG", "EGP"),
    ("IL", "ILS"),
    ("AE", "AED"),
    ("SA", "SAR"),
    ("TR", "TRY"),
    ("RU", "RUB"),
    ("UA", "UAH"),
    ("PL", "PLN"),
    ("CZ", "CZK"),
    ("HU", "HUF"),
    ("RO", "RON"),
    ("BG", "BGN"),
    // Eurozone
    ("DE", "EUR"),
    ("FR", "EUR"),
    ("IT", "EUR"),
    ("ES", "EUR"),
    ("NL", "EUR"),
    ("BE", "EUR"),
    ("AT", "EUR"),
    ("PT", "EUR"),
    ("FI", "EUR"),
    ("IE", "EUR"),
    ("GR", "EUR"),
    ("LU", "EUR"),
    ("EE", "EUR"),
    ("LV", "EUR"),
    ("LT", "EUR"),
    ("SK", "EUR"),
    ("SI", "EUR"),
    ("MT", "EUR"),
    ("CY", "EUR"),
];

impl CurrencyFormat {
    /// Get the user's locale currency for confidence boosting.
    fn get_locale_currency() -> Option<String> {
        // Try LC_ALL, LANG, LC_MONETARY
        let locale = env::var("LC_ALL")
            .or_else(|_| env::var("LANG"))
            .or_else(|_| env::var("LC_MONETARY"))
            .ok()?;

        // Parse country code from locale (e.g., "en_US.UTF-8" -> "US")
        let country = if locale.contains('_') {
            locale.split('_').nth(1)?.split('.').next()?
        } else {
            return None;
        };

        // Look up currency for country
        COUNTRY_TO_CURRENCY
            .iter()
            .find(|(c, _)| *c == country)
            .map(|(_, currency)| (*currency).to_string())
    }

    /// Parse currency amount with optional SI prefix.
    /// Returns (amount, currency_code).
    fn parse_currency(input: &str) -> Vec<(f64, String, f32)> {
        let input = input.trim();
        let mut results = Vec::new();

        // Try symbol prefix first (e.g., $100, €50)
        for (symbol, codes) in SYMBOLS {
            if let Some(rest) = input.strip_prefix(symbol) {
                if let Some((amount, multiplier)) = Self::parse_amount_with_si(rest) {
                    for code in *codes {
                        let confidence = if codes.len() == 1 { 0.90 } else { 0.75 };
                        results.push((amount * multiplier, (*code).to_string(), confidence));
                    }
                    return results;
                }
            }
        }

        // Try symbol suffix (e.g., 100$, 50€)
        for (symbol, codes) in SYMBOLS {
            if let Some(rest) = input.strip_suffix(symbol) {
                if let Some((amount, multiplier)) = Self::parse_amount_with_si(rest) {
                    for code in *codes {
                        let confidence = if codes.len() == 1 { 0.90 } else { 0.75 };
                        results.push((amount * multiplier, (*code).to_string(), confidence));
                    }
                    return results;
                }
            }
        }

        // Try amount + code with SI prefix (e.g., 5kUSD, 2.5MEUR)
        for code in all_currency_codes() {
            let input_upper = input.to_uppercase();
            if input_upper.ends_with(&code) {
                let prefix_part = &input[..input.len() - code.len()];
                if let Some((amount, multiplier)) = Self::parse_amount_with_si(prefix_part) {
                    results.push((amount * multiplier, code.clone(), 0.90));
                    return results;
                }
            }
        }

        // Try amount + code with space (e.g., "100 USD", "50 EUR", "1 BTC")
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() == 2 {
            let code_upper = parts[1].to_uppercase();
            if is_known_currency(&code_upper) {
                if let Some((amount, multiplier)) = Self::parse_amount_with_si(parts[0]) {
                    results.push((amount * multiplier, code_upper, 0.90));
                    return results;
                }
            }
        }

        results
    }

    /// Parse amount with optional SI prefix (k, M, G).
    /// Returns (base_amount, multiplier).
    fn parse_amount_with_si(s: &str) -> Option<(f64, f64)> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        // Check for SI prefix at end of number part
        for (prefix, multiplier) in SI_PREFIXES {
            if let Some(num_part) = s.strip_suffix(prefix) {
                if let Some(amount) = parse_number(num_part) {
                    return Some((amount, *multiplier));
                }
            }
        }

        // No SI prefix, just parse the number
        parse_number(s).map(|amount| (amount, 1.0))
    }

    /// Format currency amount with symbol if available.
    fn format_amount(amount: f64, code: &str) -> String {
        // Find symbol for this code
        let symbol = SYMBOLS
            .iter()
            .find(|(_, codes)| codes.len() == 1 && codes[0] == code)
            .map(|(s, _)| *s);

        let formatted = Self::format_number(amount);

        match (symbol, code) {
            (Some(s), _) => format!("{}{}", s, formatted),
            (_, code) => format!("{} {}", formatted, code),
        }
    }

    /// Format a number with thousand separators and appropriate decimals.
    fn format_number(value: f64) -> String {
        let abs_value = value.abs();

        // Determine decimal places
        let decimals = if abs_value >= 1.0 { 2 } else { 4 };

        let formatted = format!("{:.prec$}", value, prec = decimals);

        // Add thousand separators
        let parts: Vec<&str> = formatted.split('.').collect();
        let int_part = parts[0];
        let dec_part = parts.get(1);

        let int_with_sep: String = int_part
            .chars()
            .rev()
            .collect::<Vec<_>>()
            .chunks(3)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(",")
            .chars()
            .rev()
            .collect();

        match dec_part {
            Some(d) => format!("{}.{}", int_with_sep, d),
            None => int_with_sep,
        }
    }
}

impl Format for CurrencyFormat {
    fn id(&self) -> &'static str {
        "currency"
    }

    fn name(&self) -> &'static str {
        "Currency"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Currency amounts with exchange rate conversion",
            examples: &["100 USD", "$50", "5kEUR", "2.5MSEK", "£100"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let parsed = Self::parse_currency(input);
        if parsed.is_empty() {
            return vec![];
        }

        let locale_currency = Self::get_locale_currency();

        parsed
            .into_iter()
            .filter_map(|(amount, code, mut confidence)| {
                // Boost confidence if code matches locale
                if let Some(ref locale_code) = locale_currency {
                    if locale_code == &code {
                        confidence = (confidence + 0.15).min(0.95);
                    }
                }

                // Reject negative amounts
                if amount < 0.0 {
                    return None;
                }

                let description = Self::format_amount(amount, &code);

                Some(Interpretation {
                    value: CoreValue::Currency {
                        amount,
                        code: code.clone(),
                    },
                    source_format: "currency".to_string(),
                    confidence,
                    description,
                    rich_display: vec![],
                })
            })
            .collect()
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Currency { amount, code } = value else {
            return vec![];
        };

        let Some(cache) = RateCache::get() else {
            // No rates available, can't convert
            return vec![];
        };

        let mut conversions = Vec::new();

        // Convert to standard display currencies
        for target in DISPLAY_CURRENCIES {
            // Skip same currency
            if *target == code {
                continue;
            }

            let Some(converted) = cache.convert(*amount, code, target) else {
                continue;
            };

            let display = Self::format_amount(converted, target);

            conversions.push(Conversion {
                value: CoreValue::Currency {
                    amount: converted,
                    code: (*target).to_string(),
                },
                target_format: target.to_lowercase(),
                display: display.clone(),
                path: vec![target.to_lowercase()],
                steps: vec![ConversionStep {
                    format: target.to_lowercase(),
                    value: CoreValue::Currency {
                        amount: converted,
                        code: (*target).to_string(),
                    },
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                ..Default::default()
            });
        }

        // Convert to/from plugin currencies
        for plugin_code in currency_rates::plugin_currency_codes() {
            // Skip same currency
            if plugin_code.eq_ignore_ascii_case(code) {
                continue;
            }

            let Some(converted) = cache.convert(*amount, code, &plugin_code) else {
                continue;
            };

            // Get plugin info for symbol
            let display = if let Some(info) = currency_rates::get_plugin_currency(&plugin_code) {
                format!("{}{}", info.symbol, Self::format_number(converted))
            } else {
                format!("{} {}", Self::format_number(converted), plugin_code)
            };

            conversions.push(Conversion {
                value: CoreValue::Currency {
                    amount: converted,
                    code: plugin_code.clone(),
                },
                target_format: plugin_code.to_lowercase(),
                display: display.clone(),
                path: vec![plugin_code.to_lowercase()],
                steps: vec![ConversionStep {
                    format: plugin_code.to_lowercase(),
                    value: CoreValue::Currency {
                        amount: converted,
                        code: plugin_code.clone(),
                    },
                    display,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                ..Default::default()
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["money", "fx"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_code_suffix() {
        let format = CurrencyFormat;
        let results = format.parse("100 USD");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("100"));
    }

    #[test]
    fn test_parse_code_no_space() {
        let format = CurrencyFormat;
        let results = format.parse("50EUR");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("50"));
    }

    #[test]
    fn test_parse_symbol_prefix() {
        let format = CurrencyFormat;
        let results = format.parse("€100");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("€"));
    }

    #[test]
    fn test_parse_symbol_suffix() {
        let format = CurrencyFormat;
        let results = format.parse("100€");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("€"));
    }

    #[test]
    fn test_parse_si_prefix_k() {
        let format = CurrencyFormat;
        let results = format.parse("5kUSD");
        assert_eq!(results.len(), 1);

        if let CoreValue::Currency { amount, code } = &results[0].value {
            assert_eq!(*amount, 5000.0);
            assert_eq!(code, "USD");
        } else {
            panic!("Expected Currency value");
        }
    }

    #[test]
    fn test_parse_si_prefix_m() {
        let format = CurrencyFormat;
        let results = format.parse("2.5MEUR");
        assert_eq!(results.len(), 1);

        if let CoreValue::Currency { amount, code } = &results[0].value {
            assert_eq!(*amount, 2_500_000.0);
            assert_eq!(code, "EUR");
        } else {
            panic!("Expected Currency value");
        }
    }

    #[test]
    fn test_parse_ambiguous_dollar() {
        let format = CurrencyFormat;
        let results = format.parse("$100");
        // Should return multiple interpretations for $
        assert!(results.len() > 1);
        // First should be USD (most common)
        if let CoreValue::Currency { code, .. } = &results[0].value {
            assert_eq!(code, "USD");
        }
    }

    #[test]
    fn test_format_number() {
        assert_eq!(CurrencyFormat::format_number(1234.56), "1,234.56");
        assert_eq!(CurrencyFormat::format_number(1000000.00), "1,000,000.00");
        // Values < 1.0 use 4 decimals
        assert_eq!(CurrencyFormat::format_number(0.50), "0.5000");
    }

    #[test]
    fn test_parse_amount_with_si() {
        assert_eq!(
            CurrencyFormat::parse_amount_with_si("5k"),
            Some((5.0, 1000.0))
        );
        assert_eq!(
            CurrencyFormat::parse_amount_with_si("2.5M"),
            Some((2.5, 1_000_000.0))
        );
        assert_eq!(
            CurrencyFormat::parse_amount_with_si("1G"),
            Some((1.0, 1_000_000_000.0))
        );
        assert_eq!(
            CurrencyFormat::parse_amount_with_si("100"),
            Some((100.0, 1.0))
        );
    }
}
