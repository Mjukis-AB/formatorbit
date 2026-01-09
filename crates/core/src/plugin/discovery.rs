//! Plugin discovery - finding plugin files in the filesystem.

use std::ffi::OsStr;
use std::path::PathBuf;

/// Discover all directories that may contain plugins.
///
/// Returns directories in priority order:
/// 1. User's config directory (~/.config/forb/plugins/ on Linux/macOS)
/// 2. Bundled plugins directory (for default plugins shipped with forb)
/// 3. Additional paths from config (if any)
///
/// Directories are deduplicated (on macOS config_dir and data_dir may be the same).
pub fn discover_plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Primary user plugin directory
    if let Some(config_dir) = dirs::config_dir() {
        let plugin_dir = config_dir.join("forb").join("plugins");
        if plugin_dir.exists() {
            dirs.push(plugin_dir);
        }
    }

    // Bundled plugins directory (shipped with forb)
    // Check data directory for system-wide bundled plugins
    if let Some(data_dir) = dirs::data_dir() {
        let bundled_dir = data_dir.join("forb").join("plugins");
        // Only add if different from config dir (they're the same on macOS)
        if bundled_dir.exists() && !dirs.contains(&bundled_dir) {
            dirs.push(bundled_dir);
        }
    }

    // TODO: Load additional paths from config file
    // This would read from ~/.config/forb/config.toml [plugins] paths = [...]

    dirs
}

/// Get the bundled plugins directory path.
///
/// Returns the path where bundled/default plugins should be installed.
/// On Linux: `~/.local/share/forb/plugins/`
/// On macOS: `~/Library/Application Support/forb/plugins/`
/// On Windows: `%APPDATA%\forb\plugins\`
pub fn bundled_plugin_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("forb").join("plugins"))
}

/// Ensure the bundled plugins directory exists.
pub fn ensure_bundled_plugin_dir() -> Result<PathBuf, std::io::Error> {
    let dir = bundled_plugin_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine data directory",
        )
    })?;

    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Find all plugin files in a directory.
///
/// Returns paths to `.py` files (excluding `.sample` files).
pub fn find_plugin_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!(dir = %dir.display(), error = %e, "Could not read plugin directory");
            return files;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Check for .py extension
        let extension = path.extension().and_then(OsStr::to_str);
        if extension != Some("py") {
            continue;
        }

        // Skip .sample files (check if filename ends with .py.sample)
        let filename = path.file_name().and_then(OsStr::to_str).unwrap_or("");
        if filename.ends_with(".sample") {
            tracing::debug!(path = %path.display(), "Skipping sample plugin");
            continue;
        }

        files.push(path);
    }

    // Sort for deterministic loading order
    files.sort();

    files
}

/// Get the default plugin directory path.
///
/// Returns `~/.config/forb/plugins/` on Linux/macOS or equivalent on other platforms.
pub fn default_plugin_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("forb").join("plugins"))
}

/// Ensure the default plugin directory exists.
pub fn ensure_plugin_dir() -> Result<PathBuf, std::io::Error> {
    let dir = default_plugin_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine config directory",
        )
    })?;

    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_plugin_dir() {
        let dir = default_plugin_dir();
        // Should return Some on all platforms
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(path.ends_with("forb/plugins") || path.ends_with("forb\\plugins"));
    }

    #[test]
    fn test_find_plugin_files_skips_samples() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create test files
        std::fs::write(temp_dir.path().join("active.py"), "# active").unwrap();
        std::fs::write(temp_dir.path().join("sample.py.sample"), "# sample").unwrap();
        std::fs::write(temp_dir.path().join("readme.txt"), "# readme").unwrap();

        let files = find_plugin_files(temp_dir.path());

        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "active.py");
    }
}
