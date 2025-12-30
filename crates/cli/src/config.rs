//! Configuration file loading and environment variable handling.
//!
//! Precedence: CLI args > Environment vars > Config file > Defaults

use serde::Deserialize;
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
"#;

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
