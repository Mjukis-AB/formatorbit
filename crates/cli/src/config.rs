//! Configuration file loading and environment variable handling.
//!
//! Precedence: CLI args > Environment vars > Config file > Defaults

use formatorbit_core::{BlockingConfig, ConversionConfig, PriorityAdjustment, PriorityConfig};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Default config file content for `--config-init`.
pub const DEFAULT_CONFIG: &str = r#"# Formatorbit configuration
# See: forb --help for all options

# Conversions to show per interpretation (0 = unlimited)
limit = 5

# Minimum confidence for pipe mode annotations (0.0-1.0)
threshold = 0.8

# Disable colored output
no_color = false

# Maximum tokens to analyze per line in pipe mode
max_tokens = 50

# URL fetch timeout in seconds
url_timeout = 30

# Maximum response size for URL fetches (K, M, G suffixes)
url_max_size = "10M"

# ============================================================================
# Priority Configuration (optional)
# ============================================================================
# Customize how conversion results are ordered.

# [priority]
# # Reorder the 5 main categories (highest priority first)
# # Default: ["Primary", "Structured", "Semantic", "Encoding", "Raw"]
# category_order = ["Semantic", "Structured", "Primary", "Encoding", "Raw"]

# # Adjust individual format priorities
# # Integer: +/- offset within category (higher = shown earlier)
# # String: move to different category
# [priority.format_priority]
# datetime = 10        # Bump up within Semantic
# uuid = 5             # Also bump up, but less
# hex = -10            # Push down within Encoding
# ipv4 = "Primary"     # Move to Primary category

# ============================================================================
# Blocking Configuration (optional)
# ============================================================================
# Block formats or conversion paths you don't want to see.
# Use `forb --show-paths` to see blockable paths for any input.

# [blocking]
# # Block entire formats (never parse or convert)
# formats = ["octal", "binary"]
#
# # Block specific conversion paths (source:target or full path)
# paths = ["hex:msgpack", "uuid:epoch-seconds"]

# ============================================================================
# Analytics Configuration (optional)
# ============================================================================
# Privacy-first usage tracking. Data is stored locally in human-readable TOML.

# [analytics]
# # Enable local tracking (default: true)
# # Data stored at: forb analytics status (to see path)
# enabled = true
#
# # Anonymous contribution (default: false)
# # When enabled, periodically sends anonymized aggregate stats
# contribute = false
#
# # Days between automatic contributions (if contribute = true)
# contribute_interval = 7

# ============================================================================
# Updates Configuration (optional)
# ============================================================================
# Check for new versions automatically.

# [updates]
# # Enable update checking (default: true)
# # Checks GitHub releases once per day
# check = true
#
# # Can also be disabled via: FORB_CHECK_UPDATES=0
"#;

/// Priority configuration as stored in TOML.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CliPriorityConfig {
    /// Category order (highest to lowest priority).
    pub category_order: Vec<String>,
    /// Per-format priority adjustments (integer offset or category name).
    pub format_priority: HashMap<String, toml::Value>,
}

/// Blocking configuration as stored in TOML.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CliBlockingConfig {
    /// Blocked format IDs.
    pub formats: Vec<String>,
    /// Blocked paths (source:target or full path).
    pub paths: Vec<String>,
    /// Root-based blocking (root:target - blocks target regardless of path).
    pub root_paths: Vec<String>,
}

/// Analytics configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct CliAnalyticsConfig {
    /// Enable local tracking (default: true).
    pub enabled: bool,
    /// Enable anonymous contribution (default: false).
    pub contribute: bool,
    /// Days between automatic contributions.
    pub contribute_interval: u32,
}

impl Default for CliAnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            contribute: false,
            contribute_interval: 7,
        }
    }
}

/// Updates configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct CliUpdatesConfig {
    /// Enable update checking (default: true).
    pub check: bool,
}

impl Default for CliUpdatesConfig {
    fn default() -> Self {
        Self { check: true }
    }
}

/// Configuration loaded from file and environment.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    pub no_color: Option<bool>,
    pub url_timeout: Option<u64>,
    pub url_max_size: Option<String>,
    pub max_tokens: Option<usize>,
    /// Priority configuration.
    pub priority: Option<CliPriorityConfig>,
    /// Blocking configuration.
    pub blocking: Option<CliBlockingConfig>,
    /// Analytics configuration.
    #[serde(default)]
    pub analytics: CliAnalyticsConfig,
    /// Updates configuration.
    #[serde(default)]
    pub updates: CliUpdatesConfig,
}

impl Config {
    /// Get the config file path.
    ///
    /// - Linux/macOS: `~/.config/forb/config.toml`
    /// - Windows: `%APPDATA%\forb\config.toml`
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("forb").join("config.toml"))
    }

    /// Load config from file. Returns default if file doesn't exist.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };

        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };

        toml::from_str(&contents).unwrap_or_else(|e| {
            eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            Self::default()
        })
    }

    /// Read value from environment variable.
    fn env_var<T: std::str::FromStr>(name: &str) -> Option<T> {
        std::env::var(name).ok()?.parse().ok()
    }

    /// Get limit with precedence: env > config > default.
    pub fn limit(&self) -> usize {
        Self::env_var("FORB_LIMIT").or(self.limit).unwrap_or(5)
    }

    /// Get threshold with precedence: env > config > default.
    pub fn threshold(&self) -> f32 {
        Self::env_var("FORB_THRESHOLD")
            .or(self.threshold)
            .unwrap_or(0.8)
    }

    /// Get no_color with precedence: env > config > default.
    ///
    /// Respects the `NO_COLOR` standard (https://no-color.org/).
    pub fn no_color(&self) -> bool {
        // NO_COLOR is a standard - presence means disable color
        if std::env::var("NO_COLOR").is_ok() {
            return true;
        }
        if std::env::var("FORB_NO_COLOR").is_ok() {
            return true;
        }
        self.no_color.unwrap_or(false)
    }

    /// Get url_timeout with precedence: env > config > default.
    pub fn url_timeout(&self) -> u64 {
        Self::env_var("FORB_URL_TIMEOUT")
            .or(self.url_timeout)
            .unwrap_or(30)
    }

    /// Get url_max_size with precedence: env > config > default.
    pub fn url_max_size(&self) -> String {
        std::env::var("FORB_URL_MAX_SIZE")
            .ok()
            .or_else(|| self.url_max_size.clone())
            .unwrap_or_else(|| "10M".to_string())
    }

    /// Get max_tokens with precedence: env > config > default.
    pub fn max_tokens(&self) -> usize {
        Self::env_var("FORB_MAX_TOKENS")
            .or(self.max_tokens)
            .unwrap_or(50)
    }

    /// Get analytics_enabled with precedence: env > config > default (true).
    pub fn analytics_enabled(&self) -> bool {
        // FORB_ANALYTICS=0 or FORB_ANALYTICS=false disables analytics
        if let Ok(val) = std::env::var("FORB_ANALYTICS") {
            return !matches!(val.to_lowercase().as_str(), "0" | "false" | "no" | "off");
        }
        self.analytics.enabled
    }

    /// Get analytics contribution setting.
    ///
    /// Reserved for Phase 3 (contribution) functionality.
    #[allow(dead_code)]
    pub fn analytics_contribute(&self) -> bool {
        self.analytics.contribute
    }

    /// Get update checking enabled with precedence: env > config > default (true).
    pub fn updates_enabled(&self) -> bool {
        // FORB_CHECK_UPDATES=0 or FORB_CHECK_UPDATES=false disables checking
        if let Ok(val) = std::env::var("FORB_CHECK_UPDATES") {
            return !matches!(val.to_lowercase().as_str(), "0" | "false" | "no" | "off");
        }
        self.updates.check
    }

    /// Convert CLI config to core ConversionConfig.
    ///
    /// Returns `Some(config)` if there's any priority or blocking customization,
    /// otherwise `None` (to use defaults).
    #[must_use]
    pub fn conversion_config(&self) -> Option<ConversionConfig> {
        let priority = self.priority.as_ref().map(|p| {
            let format_priority = p
                .format_priority
                .iter()
                .filter_map(|(k, v)| {
                    let adj = match v {
                        toml::Value::Integer(i) => Some(PriorityAdjustment::Offset(*i as i32)),
                        toml::Value::String(s) => Some(PriorityAdjustment::Category(s.clone())),
                        _ => None,
                    };
                    adj.map(|a| (k.clone(), a))
                })
                .collect();

            PriorityConfig {
                category_order: p.category_order.clone(),
                format_priority,
            }
        });

        let blocking = self.blocking.as_ref().map(|b| BlockingConfig {
            formats: b.formats.clone(),
            paths: b.paths.clone(),
            root_paths: b.root_paths.clone(),
        });

        // Only return Some if there's actual customization
        match (&priority, &blocking) {
            (None, None) => None,
            _ => Some(ConversionConfig {
                priority: priority.unwrap_or_default(),
                blocking: blocking.unwrap_or_default(),
                reinterpret_threshold: 0.0, // Use default (0.7) - will be overridden by CLI if set
            }),
        }
    }
}

/// Create a default config file at the standard location.
pub fn init_config() -> Result<PathBuf, String> {
    let path = Config::path().ok_or("Cannot determine config directory")?;

    if path.exists() {
        return Err(format!("Config file already exists: {}", path.display()));
    }

    // Create parent directory
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(&path, DEFAULT_CONFIG).map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid_toml() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).expect("DEFAULT_CONFIG should parse");
        assert_eq!(config.limit, Some(5));
        assert_eq!(config.threshold, Some(0.8));
        assert_eq!(config.no_color, Some(false));
        assert_eq!(config.max_tokens, Some(50));
        assert_eq!(config.url_timeout, Some(30));
        assert_eq!(config.url_max_size, Some("10M".to_string()));
    }

    #[test]
    fn test_partial_config() {
        let toml = r#"
limit = 10
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.limit, Some(10));
        assert_eq!(config.threshold, None);

        // Getters should use defaults for missing values
        assert_eq!(config.limit(), 10);
        assert_eq!(config.threshold(), 0.8);
    }

    #[test]
    fn test_empty_config() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.limit(), 5);
        assert_eq!(config.threshold(), 0.8);
        assert!(!config.no_color());
    }
}
