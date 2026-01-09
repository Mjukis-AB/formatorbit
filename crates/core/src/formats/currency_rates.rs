//! Exchange rate fetching and caching.
//!
//! Fetches rates from Frankfurter API (European Central Bank data)
//! and caches them locally with 24-hour TTL.
//!
//! Also supports plugin-provided currencies (like BTC, ETH) that provide
//! rates to a known base currency (usually USD), which are then chained
//! through the ECB rates for full convertibility.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, OnceLock, RwLock};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Cached exchange rates with retry support.
/// Uses OnceLock for the Mutex itself, then Mutex for interior mutability.
static RATE_CACHE: OnceLock<Mutex<CacheState>> = OnceLock::new();

/// Plugin-provided currency rates.
/// Maps currency code -> (rate, base_currency).
/// For example, BTC -> (42000.0, "USD") means 1 BTC = 42000 USD.
static PLUGIN_RATES: LazyLock<RwLock<HashMap<String, PluginCurrencyInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Information about a plugin-provided currency.
#[derive(Debug, Clone)]
pub struct PluginCurrencyInfo {
    /// Exchange rate: 1 unit of this currency = rate units of base_currency.
    pub rate: f64,
    /// The base currency code (e.g., "USD").
    pub base_currency: String,
    /// Currency symbol (e.g., "₿").
    pub symbol: String,
    /// Number of decimal places.
    pub decimals: u8,
}

/// Internal cache state that supports retry on failure.
struct CacheState {
    /// The cached rates, if available.
    cache: Option<RateCache>,
    /// When we last attempted to fetch (for backoff on failures).
    last_attempt: Option<DateTime<Utc>>,
}

/// Exchange rates relative to EUR (ECB base currency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCache {
    /// When the rates were fetched.
    pub fetched_at: DateTime<Utc>,
    /// Base currency (always EUR from ECB).
    pub base: String,
    /// Rates relative to base currency.
    pub rates: HashMap<String, f64>,
}

/// Response from Frankfurter API.
#[derive(Debug, Deserialize)]
struct FrankfurterResponse {
    base: String,
    rates: HashMap<String, f64>,
}

impl RateCache {
    /// Cache TTL: 24 hours.
    const TTL_HOURS: i64 = 24;

    /// Retry interval after failed fetch: 5 minutes.
    const RETRY_MINUTES: i64 = 5;

    /// Get or initialize the global rate cache.
    ///
    /// This will retry fetching if the cache is empty and enough time has passed
    /// since the last failed attempt (5 minute backoff). This ensures library
    /// consumers in long-running processes can recover from transient failures.
    pub fn get() -> Option<RateCache> {
        let state_mutex = RATE_CACHE.get_or_init(|| {
            Mutex::new(CacheState {
                cache: None,
                last_attempt: None,
            })
        });

        let mut state = state_mutex.lock().ok()?;

        // If we have a valid (non-expired) cache, return it
        if let Some(ref cache) = state.cache {
            if !cache.is_expired() {
                return Some(cache.clone());
            }
        }

        // Check if we should attempt a fetch (respecting backoff)
        let should_fetch = match state.last_attempt {
            None => true,
            Some(last) => Utc::now() - last > Duration::minutes(Self::RETRY_MINUTES),
        };

        if should_fetch {
            state.last_attempt = Some(Utc::now());

            if let Some(fresh) = Self::load_or_fetch_inner(state.cache.as_ref()) {
                state.cache = Some(fresh.clone());
                return Some(fresh);
            }
        }

        // Return stale cache if available, otherwise None
        state.cache.clone()
    }

    /// Load from disk or fetch from API.
    fn load_or_fetch_inner(existing: Option<&RateCache>) -> Option<Self> {
        // Try loading from disk first
        if let Some(cached) = Self::load_from_disk() {
            if !cached.is_expired() {
                return Some(cached);
            }
            // Cache expired, try to refresh
            if let Some(fresh) = Self::fetch_from_api() {
                fresh.save_to_disk();
                return Some(fresh);
            }
            // Fetch failed, use stale disk cache
            return Some(cached);
        }

        // No disk cache, try to fetch
        if let Some(fresh) = Self::fetch_from_api() {
            fresh.save_to_disk();
            return Some(fresh);
        }

        // Return existing in-memory cache if we have one (even if stale)
        existing.cloned()
    }

    /// Check if cache has expired.
    fn is_expired(&self) -> bool {
        let ttl = Duration::hours(Self::TTL_HOURS);
        Utc::now() - self.fetched_at > ttl
    }

    /// Get cache file path.
    fn cache_path() -> Option<PathBuf> {
        dirs::cache_dir().map(|p| p.join("formatorbit").join("exchange_rates.json"))
    }

    /// Load rates from disk cache.
    fn load_from_disk() -> Option<Self> {
        let path = Self::cache_path()?;
        let contents = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// Save rates to disk cache.
    fn save_to_disk(&self) {
        if let Some(path) = Self::cache_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&path, json);
            }
        }
    }

    /// Fetch rates from Frankfurter API.
    fn fetch_from_api() -> Option<Self> {
        // Frankfurter uses EUR as base by default
        let url = "https://api.frankfurter.app/latest";

        let response: FrankfurterResponse = ureq::get(url)
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .ok()?
            .into_json()
            .ok()?;

        let mut rates = response.rates;
        // Add EUR itself with rate 1.0
        rates.insert("EUR".to_string(), 1.0);

        Some(Self {
            fetched_at: Utc::now(),
            base: response.base,
            rates,
        })
    }

    /// Convert amount from one currency to another.
    ///
    /// Supports both built-in ECB currencies and plugin-provided currencies.
    /// Plugin currencies are chained through their base currency.
    pub fn convert(&self, amount: f64, from: &str, to: &str) -> Option<f64> {
        let from_upper = from.to_uppercase();
        let to_upper = to.to_uppercase();

        if from_upper == to_upper {
            return Some(amount);
        }

        // Get plugin rates (if any)
        let plugin_rates = PLUGIN_RATES.read().ok()?;

        // Check if FROM is a plugin currency
        let from_in_eur = if let Some(plugin_info) = plugin_rates.get(&from_upper) {
            // Plugin currency: convert to base, then to EUR
            let amount_in_base = amount * plugin_info.rate;
            let base_upper = plugin_info.base_currency.to_uppercase();
            let base_rate = self.rates.get(&base_upper)?;
            amount_in_base / base_rate
        } else {
            // Regular ECB currency: convert to EUR
            let from_rate = self.rates.get(&from_upper)?;
            amount / from_rate
        };

        // Check if TO is a plugin currency
        if let Some(plugin_info) = plugin_rates.get(&to_upper) {
            // Plugin currency: convert from EUR to base, then to plugin currency
            let base_upper = plugin_info.base_currency.to_uppercase();
            let base_rate = self.rates.get(&base_upper)?;
            let amount_in_base = from_in_eur * base_rate;
            Some(amount_in_base / plugin_info.rate)
        } else {
            // Regular ECB currency: convert from EUR
            let to_rate = self.rates.get(&to_upper)?;
            Some(from_in_eur * to_rate)
        }
    }

    /// Check if a currency code is known (either ECB or plugin).
    pub fn has_currency(&self, code: &str) -> bool {
        let code_upper = code.to_uppercase();
        if self.rates.contains_key(&code_upper) {
            return true;
        }
        if let Ok(plugin_rates) = PLUGIN_RATES.read() {
            return plugin_rates.contains_key(&code_upper);
        }
        false
    }
}

/// Register a plugin-provided currency.
pub fn register_plugin_currency(code: &str, info: PluginCurrencyInfo) {
    if let Ok(mut rates) = PLUGIN_RATES.write() {
        rates.insert(code.to_uppercase(), info);
    }
}

/// Get information about a plugin currency.
pub fn get_plugin_currency(code: &str) -> Option<PluginCurrencyInfo> {
    PLUGIN_RATES.read().ok()?.get(&code.to_uppercase()).cloned()
}

/// Get all plugin currency codes.
pub fn plugin_currency_codes() -> Vec<String> {
    PLUGIN_RATES
        .read()
        .map(|rates| rates.keys().cloned().collect())
        .unwrap_or_default()
}

/// Clear all plugin currencies (useful for testing).
pub fn clear_plugin_currencies() {
    if let Ok(mut rates) = PLUGIN_RATES.write() {
        rates.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_convert_same_currency() {
        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([
                ("EUR".to_string(), 1.0),
                ("USD".to_string(), 1.05),
                ("SEK".to_string(), 11.5),
            ]),
        };

        let result = cache.convert(100.0, "EUR", "EUR").unwrap();
        assert!((result - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_convert_eur_to_usd() {
        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([("EUR".to_string(), 1.0), ("USD".to_string(), 1.05)]),
        };

        // 100 EUR should be 105 USD
        let result = cache.convert(100.0, "EUR", "USD").unwrap();
        assert!((result - 105.0).abs() < 0.01);
    }

    #[test]
    fn test_convert_usd_to_eur() {
        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([("EUR".to_string(), 1.0), ("USD".to_string(), 1.05)]),
        };

        // 105 USD should be ~100 EUR
        let result = cache.convert(105.0, "USD", "EUR").unwrap();
        assert!((result - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_convert_cross_rate() {
        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([
                ("EUR".to_string(), 1.0),
                ("USD".to_string(), 1.05),
                ("SEK".to_string(), 11.5),
            ]),
        };

        // 1000 SEK in USD: 1000 / 11.5 * 1.05 = 91.30
        let result = cache.convert(1000.0, "SEK", "USD").unwrap();
        assert!((result - 91.30).abs() < 0.1);
    }

    #[test]
    fn test_is_expired() {
        let fresh = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::new(),
        };
        assert!(!fresh.is_expired());

        let old = RateCache {
            fetched_at: Utc::now() - Duration::hours(25),
            base: "EUR".to_string(),
            rates: HashMap::new(),
        };
        assert!(old.is_expired());
    }

    #[test]
    #[serial]
    fn test_plugin_currency_to_builtin() {
        // Clear any existing plugin currencies
        clear_plugin_currencies();

        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([
                ("EUR".to_string(), 1.0),
                ("USD".to_string(), 1.05),
                ("SEK".to_string(), 11.5),
            ]),
        };

        // Register a plugin currency: 1 XYZ = 100 USD
        register_plugin_currency(
            "XYZ",
            PluginCurrencyInfo {
                rate: 100.0,
                base_currency: "USD".to_string(),
                symbol: "X".to_string(),
                decimals: 2,
            },
        );

        // 1 XYZ = 100 USD = 100/1.05 EUR = ~95.24 EUR
        let result = cache.convert(1.0, "XYZ", "EUR").unwrap();
        assert!((result - 95.24).abs() < 0.1);

        // 1 XYZ = 100 USD
        let result = cache.convert(1.0, "XYZ", "USD").unwrap();
        assert!((result - 100.0).abs() < 0.01);

        // 1 XYZ = 100 USD = 100/1.05*11.5 EUR = ~1095.24 SEK
        let result = cache.convert(1.0, "XYZ", "SEK").unwrap();
        assert!((result - 1095.24).abs() < 1.0);

        clear_plugin_currencies();
    }

    #[test]
    #[serial]
    fn test_cross_plugin_currencies_same_base() {
        // Test conversion between two plugin currencies with the same base currency
        clear_plugin_currencies();

        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([("EUR".to_string(), 1.0), ("USD".to_string(), 1.05)]),
        };

        // 1 AAA = 50 USD
        register_plugin_currency(
            "AAA",
            PluginCurrencyInfo {
                rate: 50.0,
                base_currency: "USD".to_string(),
                symbol: "A".to_string(),
                decimals: 2,
            },
        );

        // 1 BBB = 200 USD
        register_plugin_currency(
            "BBB",
            PluginCurrencyInfo {
                rate: 200.0,
                base_currency: "USD".to_string(),
                symbol: "B".to_string(),
                decimals: 2,
            },
        );

        // 1 AAA = 50 USD, 1 BBB = 200 USD
        // So 1 AAA = 50/200 BBB = 0.25 BBB
        let result = cache.convert(1.0, "AAA", "BBB").unwrap();
        assert!((result - 0.25).abs() < 0.001);

        // And 1 BBB = 200/50 AAA = 4 AAA
        let result = cache.convert(1.0, "BBB", "AAA").unwrap();
        assert!((result - 4.0).abs() < 0.001);

        clear_plugin_currencies();
    }

    #[test]
    #[serial]
    fn test_cross_plugin_currencies_different_bases() {
        // Test conversion between two plugin currencies with DIFFERENT base currencies
        // This is the key test: plugin X uses USD as base, plugin Y uses SEK as base
        clear_plugin_currencies();

        let cache = RateCache {
            fetched_at: Utc::now(),
            base: "EUR".to_string(),
            rates: HashMap::from([
                ("EUR".to_string(), 1.0),
                ("USD".to_string(), 1.05), // 1 EUR = 1.05 USD
                ("SEK".to_string(), 11.5), // 1 EUR = 11.5 SEK
            ]),
        };

        // Plugin X: 1 XXX = 100 USD
        register_plugin_currency(
            "XXX",
            PluginCurrencyInfo {
                rate: 100.0,
                base_currency: "USD".to_string(),
                symbol: "X".to_string(),
                decimals: 2,
            },
        );

        // Plugin Y: 1 YYY = 500 SEK
        register_plugin_currency(
            "YYY",
            PluginCurrencyInfo {
                rate: 500.0,
                base_currency: "SEK".to_string(),
                symbol: "Y".to_string(),
                decimals: 2,
            },
        );

        // Convert XXX to YYY:
        // 1 XXX = 100 USD
        // 100 USD = 100/1.05 EUR = 95.238 EUR
        // 95.238 EUR = 95.238 * 11.5 SEK = 1095.24 SEK
        // 1095.24 SEK = 1095.24/500 YYY = 2.19 YYY
        let result = cache.convert(1.0, "XXX", "YYY").unwrap();
        let expected = 100.0 / 1.05 * 11.5 / 500.0; // ~2.19
        assert!(
            (result - expected).abs() < 0.01,
            "Expected ~{:.2}, got {:.2}",
            expected,
            result
        );

        // And the reverse: 1 YYY to XXX
        // 1 YYY = 500 SEK
        // 500 SEK = 500/11.5 EUR = 43.478 EUR
        // 43.478 EUR = 43.478 * 1.05 USD = 45.65 USD
        // 45.65 USD = 45.65/100 XXX = 0.4565 XXX
        let result = cache.convert(1.0, "YYY", "XXX").unwrap();
        let expected = 500.0 / 11.5 * 1.05 / 100.0; // ~0.4565
        assert!(
            (result - expected).abs() < 0.001,
            "Expected ~{:.4}, got {:.4}",
            expected,
            result
        );

        clear_plugin_currencies();
    }
}
