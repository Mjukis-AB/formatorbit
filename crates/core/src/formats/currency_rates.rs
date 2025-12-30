//! Exchange rate fetching and caching.
//!
//! Fetches rates from Frankfurter API (European Central Bank data)
//! and caches them locally with 24-hour TTL.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Cached exchange rates with retry support.
/// Uses OnceLock for the Mutex itself, then Mutex for interior mutability.
static RATE_CACHE: OnceLock<Mutex<CacheState>> = OnceLock::new();

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
    pub fn convert(&self, amount: f64, from: &str, to: &str) -> Option<f64> {
        let from_upper = from.to_uppercase();
        let to_upper = to.to_uppercase();

        // Get rates relative to EUR
        let from_rate = self.rates.get(&from_upper)?;
        let to_rate = self.rates.get(&to_upper)?;

        // Convert: amount in FROM -> EUR -> TO
        // EUR = amount / from_rate
        // TO = EUR * to_rate
        Some(amount / from_rate * to_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
