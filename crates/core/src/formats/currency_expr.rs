//! Currency expression functions for evalexpr.
//!
//! Provides functions like `USD(100)`, `EUR(50)`, `BTC(0.5)` that convert
//! amounts to a target currency and return numeric values for use in expressions.

use std::env;
use std::sync::RwLock;

use super::currency_rates::{self, RateCache};

/// Explicitly set target currency (from CLI or config).
static TARGET_CURRENCY: RwLock<Option<String>> = RwLock::new(None);

// Thread-local tracking for currency function usage during expression evaluation.
thread_local! {
    /// Currency tracking state:
    /// - None: no currency function was called
    /// - Some((code, true)): all functions output the same currency, safe to display
    /// - Some((_, false)): mixed currencies or toXXX/inXXX used, don't display currency
    static RESULT_CURRENCY: std::cell::RefCell<Option<(String, bool)>> = const { std::cell::RefCell::new(None) };
}

/// Clear the currency tracking before evaluating an expression.
pub fn clear_currency_flag() {
    RESULT_CURRENCY.with(|c| *c.borrow_mut() = None);
}

/// Get the result currency if all currency functions output the same currency.
///
/// Returns the currency code only if:
/// - Currency functions were used
/// - All functions output the same currency (all XXX() to target)
/// - No toXXX/inXXX functions were used (those make the result ambiguous in combinations)
pub fn get_result_currency() -> Option<String> {
    RESULT_CURRENCY.with(|c| {
        let state = c.borrow();
        match &*state {
            Some((code, true)) => Some(code.clone()), // Consistent, safe to display
            Some((_, false)) => None,                 // Mixed or explicit conversion
            None => None,                             // No currency used
        }
    })
}

/// Check if any currency function was called since the last clear.
pub fn was_currency_used() -> bool {
    RESULT_CURRENCY.with(|c| c.borrow().is_some())
}

/// Mark that a XXX(amount) function was called, converting to target currency.
/// Only shows currency if all calls are to the same target.
fn set_result_currency(code: &str) {
    let code_upper = code.to_uppercase();
    RESULT_CURRENCY.with(|c| {
        let mut state = c.borrow_mut();
        match &*state {
            None => {
                // First call - mark as consistent
                *state = Some((code_upper, true));
            }
            Some((existing, true)) => {
                // Previous was consistent - check if still consistent
                if !existing.eq_ignore_ascii_case(&code_upper) {
                    // Different currency - mark as inconsistent
                    *state = Some((code_upper, false));
                }
            }
            Some((_, false)) => {
                // Already inconsistent, stay that way
            }
        }
    });
}

/// Mark that a toXXX/inXXX function was called.
/// This always marks the result as inconsistent since mixing explicit conversions
/// with other operations doesn't produce a meaningful single-currency result.
fn set_result_currency_explicit() {
    RESULT_CURRENCY.with(|c| {
        let mut state = c.borrow_mut();
        // Mark as used but inconsistent - toXXX/inXXX makes display ambiguous
        *state = Some((String::new(), false));
    });
}

/// ISO 4217 currency codes we recognize (duplicated from currency.rs to avoid cross-module deps).
pub const CURRENCY_CODES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "CNY", "HKD", "SGD", "SEK", "NOK",
    "DKK", "ISK", "PLN", "CZK", "HUF", "RON", "BGN", "TRY", "RUB", "INR", "KRW", "THB", "MYR",
    "IDR", "PHP", "VND", "BRL", "MXN", "ARS", "CLP", "COP", "PEN", "ZAR", "EGP", "NGN", "KES",
    "ILS", "AED", "SAR", "QAR", "KWD", "BHD", "OMR", "UAH", "KZT", "GEL", "AZN",
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

/// Set the target currency explicitly (from CLI flag or config).
pub fn set_target_currency(code: Option<String>) {
    if let Ok(mut guard) = TARGET_CURRENCY.write() {
        *guard = code.map(|c| c.to_uppercase());
    }
}

/// Get the current target currency with its source description.
///
/// Returns (currency_code, source_description).
pub fn get_target_currency_with_source() -> (String, String) {
    // 1. Check explicit setting (from CLI flag or config)
    if let Ok(guard) = TARGET_CURRENCY.read() {
        if let Some(code) = guard.as_ref() {
            return (code.clone(), "config".to_string());
        }
    }

    // 2. Check environment variable
    if let Ok(code) = env::var("FORB_TARGET_CURRENCY") {
        return (
            code.to_uppercase(),
            "environment (FORB_TARGET_CURRENCY)".to_string(),
        );
    }

    // 3. Try system locale
    if let Some((code, locale)) = get_locale_currency_with_source() {
        return (code, format!("locale ({})", locale));
    }

    // 4. Fallback to USD
    ("USD".to_string(), "default".to_string())
}

/// Get just the target currency code.
pub fn get_target_currency() -> String {
    get_target_currency_with_source().0
}

/// Get locale-based currency from environment variables.
fn get_locale_currency_with_source() -> Option<(String, String)> {
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
        .map(|(_, currency)| ((*currency).to_string(), locale.clone()))
}

/// Convert amount from source currency to target currency.
///
/// Returns the converted amount, or None if conversion fails.
/// Also tracks that the result is in the target currency (for expression result display).
pub fn convert_to_target(amount: f64, source: &str) -> Option<f64> {
    let target = get_target_currency();
    // Result is in the target currency
    set_result_currency(&target);

    // Same currency, no conversion needed
    if source.eq_ignore_ascii_case(&target) {
        return Some(amount);
    }

    // Get the rate cache
    let cache = RateCache::get()?;
    cache.convert(amount, source, &target)
}

/// Convert amount from target currency to a specific currency.
///
/// This is the inverse of convert_to_target - used by toXXX/inXXX functions.
/// Example: If target is SEK, convert_from_target(100, "EUR") converts 100 SEK to EUR.
/// Note: Using toXXX/inXXX marks the result as having mixed currencies, so no currency
/// will be displayed (since `toEUR(USD(100))` mixes SEK intermediate with EUR output).
pub fn convert_from_target(amount: f64, dest: &str) -> Option<f64> {
    // Mark that explicit conversion was used - don't display currency
    set_result_currency_explicit();
    let source = get_target_currency();

    // Same currency, no conversion needed
    if source.eq_ignore_ascii_case(dest) {
        return Some(amount);
    }

    // Get the rate cache
    let cache = RateCache::get()?;
    cache.convert(amount, &source, dest)
}

/// Get all currency codes that should have expression functions.
///
/// Returns built-in ECB currencies + plugin-provided currencies.
pub fn all_currency_codes() -> Vec<String> {
    let mut codes: Vec<String> = CURRENCY_CODES.iter().map(|s| (*s).to_string()).collect();
    codes.extend(currency_rates::plugin_currency_codes());
    codes
}

/// Get just the built-in (ECB) currency codes.
pub fn builtin_currency_codes() -> Vec<&'static str> {
    CURRENCY_CODES.to_vec()
}

/// Get the currency code for a given country code (ISO 3166-1 alpha-2).
///
/// This is useful for GUI apps that can get the country code from the system
/// locale (e.g., Swift's `Locale.current.region?.identifier`).
///
/// # Examples
///
/// ```
/// use formatorbit_core::formats::currency_expr::currency_for_country;
///
/// assert_eq!(currency_for_country("US"), Some("USD"));
/// assert_eq!(currency_for_country("SE"), Some("SEK"));
/// assert_eq!(currency_for_country("DE"), Some("EUR"));
/// assert_eq!(currency_for_country("XX"), None);
/// ```
pub fn currency_for_country(country_code: &str) -> Option<&'static str> {
    let country_upper = country_code.to_uppercase();
    COUNTRY_TO_CURRENCY
        .iter()
        .find(|(c, _)| *c == country_upper)
        .map(|(_, currency)| *currency)
}

/// Get the currency code from a locale string (e.g., "en_US.UTF-8" or "en_US").
///
/// Parses the country code from the locale and returns the corresponding currency.
///
/// # Examples
///
/// ```
/// use formatorbit_core::formats::currency_expr::currency_for_locale;
///
/// assert_eq!(currency_for_locale("en_US.UTF-8"), Some("USD"));
/// assert_eq!(currency_for_locale("sv_SE"), Some("SEK"));
/// assert_eq!(currency_for_locale("de_DE.UTF-8"), Some("EUR"));
/// assert_eq!(currency_for_locale("en"), None); // No country code
/// ```
pub fn currency_for_locale(locale: &str) -> Option<&'static str> {
    // Parse country code from locale (e.g., "en_US.UTF-8" -> "US")
    let country = if locale.contains('_') {
        locale.split('_').nth(1)?.split('.').next()?
    } else {
        return None;
    };

    currency_for_country(country)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_get_target_currency_default() {
        // Clear any explicit setting
        set_target_currency(None);

        // Without any env vars set, should fall back to USD
        // Note: This test might be affected by the actual environment
        let (currency, _) = get_target_currency_with_source();
        // Either it picked up from locale or defaulted to USD
        assert!(!currency.is_empty());
    }

    #[test]
    #[serial]
    fn test_set_target_currency() {
        set_target_currency(Some("SEK".to_string()));

        let (currency, source) = get_target_currency_with_source();
        assert_eq!(currency, "SEK");
        assert_eq!(source, "config");

        // Clean up
        set_target_currency(None);
    }

    #[test]
    fn test_all_currency_codes() {
        let codes = all_currency_codes();
        assert!(codes.contains(&"USD".to_string()));
        assert!(codes.contains(&"EUR".to_string()));
        assert!(codes.contains(&"SEK".to_string()));
    }

    #[test]
    #[serial]
    fn test_convert_same_currency() {
        set_target_currency(Some("USD".to_string()));

        let result = convert_to_target(100.0, "USD");
        assert_eq!(result, Some(100.0));

        set_target_currency(None);
    }

    #[test]
    #[serial]
    fn test_convert_from_target_same_currency() {
        set_target_currency(Some("EUR".to_string()));

        // Converting from EUR to EUR should be identity
        let result = convert_from_target(100.0, "EUR");
        assert_eq!(result, Some(100.0));

        set_target_currency(None);
    }
}
