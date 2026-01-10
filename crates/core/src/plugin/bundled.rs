//! Bundled plugins that ship with Formatorbit.
//!
//! These plugins are embedded in the binary and installed to the
//! bundled plugins directory on first run.
//!
//! - **Active plugins** are installed as `.py` and loaded by default
//! - **Sample plugins** are installed as `.py.sample` and must be renamed to enable

use std::fs;
use std::path::PathBuf;

use super::discovery;

/// Bundled plugin definition.
struct BundledPlugin {
    /// Base filename for the plugin (without .sample suffix).
    filename: &'static str,
    /// Plugin source code.
    source: &'static str,
    /// Whether this plugin is active by default.
    /// - true: installed as `name.py` (loaded)
    /// - false: installed as `name.py.sample` (not loaded)
    active: bool,
}

/// List of bundled plugins.
const BUNDLED_PLUGINS: &[BundledPlugin] = &[
    // Active plugins (loaded by default)
    BundledPlugin {
        filename: "crypto.py",
        source: include_str!("../../bundled-plugins/crypto.py"),
        active: true,
    },
    BundledPlugin {
        filename: "math_ext.py",
        source: include_str!("../../bundled-plugins/math_ext.py"),
        active: true,
    },
    // Sample plugins (not loaded, rename to enable)
    BundledPlugin {
        filename: "custom_decoder.py",
        source: include_str!("../../bundled-plugins/custom_decoder.py"),
        active: false,
    },
    BundledPlugin {
        filename: "dev_traits.py",
        source: include_str!("../../bundled-plugins/dev_traits.py"),
        active: false,
    },
    BundledPlugin {
        filename: "checksums.py",
        source: include_str!("../../bundled-plugins/checksums.py"),
        active: false,
    },
];

/// Install bundled plugins to the data directory if they don't exist.
///
/// This is called on startup to ensure default plugins are available.
/// Plugins are only installed if they don't already exist (never overwrites).
///
/// - Active plugins are installed as `name.py`
/// - Sample plugins are installed as `name.py.sample`
pub fn install_bundled_plugins() -> Result<Vec<PathBuf>, std::io::Error> {
    let dir = discovery::ensure_bundled_plugin_dir()?;
    let mut installed = Vec::new();

    for plugin in BUNDLED_PLUGINS {
        // Determine the installed filename
        let installed_filename = if plugin.active {
            plugin.filename.to_string()
        } else {
            format!("{}.sample", plugin.filename)
        };

        let path = dir.join(&installed_filename);

        // Also check if the opposite state exists (user toggled it)
        let alt_filename = if plugin.active {
            format!("{}.sample", plugin.filename)
        } else {
            plugin.filename.to_string()
        };
        let alt_path = dir.join(&alt_filename);

        // Only install if neither version exists (don't overwrite user modifications)
        if !path.exists() && !alt_path.exists() {
            fs::write(&path, plugin.source)?;
            tracing::info!(path = %path.display(), "Installed bundled plugin");
            installed.push(path);
        }
    }

    Ok(installed)
}

/// Get the paths of all bundled plugins (whether installed or not).
pub fn bundled_plugin_paths() -> Vec<PathBuf> {
    if let Some(dir) = discovery::bundled_plugin_dir() {
        BUNDLED_PLUGINS
            .iter()
            .map(|p| dir.join(p.filename))
            .collect()
    } else {
        Vec::new()
    }
}

/// Get list of sample plugin filenames (without .sample suffix).
pub fn sample_plugin_names() -> Vec<&'static str> {
    BUNDLED_PLUGINS
        .iter()
        .filter(|p| !p.active)
        .map(|p| p.filename.strip_suffix(".py").unwrap_or(p.filename))
        .collect()
}

/// Get list of active plugin filenames (without .py suffix).
pub fn active_plugin_names() -> Vec<&'static str> {
    BUNDLED_PLUGINS
        .iter()
        .filter(|p| p.active)
        .map(|p| p.filename.strip_suffix(".py").unwrap_or(p.filename))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_plugins_exist() {
        // Verify the embedded source is not empty
        for plugin in BUNDLED_PLUGINS {
            assert!(
                !plugin.source.is_empty(),
                "Plugin {} is empty",
                plugin.filename
            );
            assert!(
                plugin.source.contains("__forb_plugin__"),
                "Plugin {} missing metadata",
                plugin.filename
            );
        }
    }
}
