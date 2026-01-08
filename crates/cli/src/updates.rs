//! Version update checking.
//!
//! Checks GitHub releases API to notify users of available updates.
//! Results are cached locally to avoid excessive API calls.

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Current CLI version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub API endpoint for latest release.
const RELEASES_API: &str = "https://api.github.com/repos/mjukis-ab/formatorbit/releases/latest";

/// Cache duration in hours.
const CACHE_HOURS: i64 = 24;

/// How forb was installed, detected from executable path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    /// Homebrew (macOS/Linux): /opt/homebrew/ or /usr/local/Cellar/
    Homebrew,
    /// Cargo: ~/.cargo/bin/
    Cargo,
    /// Scoop (Windows): scoop/apps/forb
    Scoop,
    /// Debian package: /usr/bin/forb (Linux, non-Homebrew)
    Deb,
    /// Unknown install method
    Unknown,
}

impl InstallMethod {
    /// Detect install method from current executable path.
    pub fn detect() -> Self {
        let exe = std::env::current_exe().ok();
        let path = exe.as_ref().and_then(|p| p.to_str()).unwrap_or("");

        if path.contains("/homebrew/") || path.contains("/usr/local/Cellar/") {
            Self::Homebrew
        } else if path.contains("/.cargo/bin/") {
            Self::Cargo
        } else if path.to_lowercase().contains("scoop") {
            Self::Scoop
        } else if cfg!(target_os = "linux")
            && (path.starts_with("/usr/bin/") || path.starts_with("/usr/local/bin/"))
        {
            Self::Deb
        } else {
            Self::Unknown
        }
    }

    /// Get upgrade command/hint for this install method.
    pub fn upgrade_hint(&self) -> &'static str {
        match self {
            Self::Homebrew => "brew upgrade forb",
            Self::Cargo => "cargo install formatorbit-cli",
            Self::Scoop => "scoop update forb",
            Self::Deb => "download .deb from releases",
            Self::Unknown => "https://github.com/mjukis-ab/formatorbit/releases",
        }
    }
}

/// GitHub release response (only fields we need).
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

/// Check if enough time has passed since last check.
pub fn should_check(last_check: Option<DateTime<Utc>>) -> bool {
    let Some(last) = last_check else {
        return true; // Never checked
    };

    let now = Utc::now();
    let hours_since = now.signed_duration_since(last).num_hours();
    hours_since >= CACHE_HOURS
}

/// Fetch latest version from GitHub API.
///
/// Returns `Ok(Some(version))` if a newer version is available,
/// `Ok(None)` if current version is up-to-date,
/// `Err` on network/parse errors.
pub fn fetch_latest_version() -> Result<String, String> {
    let response = ureq::get(RELEASES_API)
        .set("User-Agent", &format!("forb/{}", VERSION))
        .set("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .map_err(|e| format!("Failed to fetch releases: {}", e))?;

    let release: GitHubRelease = response
        .into_json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Strip 'v' prefix if present
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    Ok(version.to_string())
}

/// Compare versions and return newer version if available.
///
/// Returns `Some(new_version)` if `latest` is newer than `current`,
/// `None` if current is up-to-date or newer.
pub fn compare_versions(current: &str, latest: &str) -> Option<String> {
    // Simple semver comparison: split on dots, compare numerically
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };

    let current_parts = parse(current);
    let latest_parts = parse(latest);

    // Compare each component
    for (c, l) in current_parts.iter().zip(latest_parts.iter()) {
        if l > c {
            return Some(latest.to_string());
        }
        if c > l {
            return None; // Current is newer (dev version?)
        }
    }

    // If latest has more components (e.g., 1.0 vs 1.0.1), it's newer
    if latest_parts.len() > current_parts.len() {
        return Some(latest.to_string());
    }

    None // Same version
}

/// Check for updates and return new version if available.
///
/// This is a convenience function that fetches and compares in one call.
pub fn check_for_update() -> Result<Option<String>, String> {
    let latest = fetch_latest_version()?;
    Ok(compare_versions(VERSION, &latest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_newer() {
        assert_eq!(
            compare_versions("0.9.2", "0.9.3"),
            Some("0.9.3".to_string())
        );
        assert_eq!(
            compare_versions("0.9.2", "1.0.0"),
            Some("1.0.0".to_string())
        );
        assert_eq!(
            compare_versions("0.9.2", "0.10.0"),
            Some("0.10.0".to_string())
        );
    }

    #[test]
    fn test_compare_versions_same() {
        assert_eq!(compare_versions("0.9.2", "0.9.2"), None);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), None);
    }

    #[test]
    fn test_compare_versions_older() {
        assert_eq!(compare_versions("0.9.3", "0.9.2"), None);
        assert_eq!(compare_versions("1.0.0", "0.9.9"), None);
    }

    #[test]
    fn test_compare_versions_extra_components() {
        assert_eq!(compare_versions("1.0", "1.0.1"), Some("1.0.1".to_string()));
        assert_eq!(compare_versions("1.0.0", "1.0"), None);
    }

    #[test]
    fn test_should_check_never_checked() {
        assert!(should_check(None));
    }

    #[test]
    fn test_should_check_recent() {
        let recent = Utc::now() - chrono::Duration::hours(1);
        assert!(!should_check(Some(recent)));
    }

    #[test]
    fn test_should_check_old() {
        let old = Utc::now() - chrono::Duration::hours(25);
        assert!(should_check(Some(old)));
    }

    #[test]
    fn test_install_method_hints() {
        assert_eq!(InstallMethod::Homebrew.upgrade_hint(), "brew upgrade forb");
        assert_eq!(
            InstallMethod::Cargo.upgrade_hint(),
            "cargo install formatorbit-cli"
        );
        assert_eq!(InstallMethod::Scoop.upgrade_hint(), "scoop update forb");
    }
}
