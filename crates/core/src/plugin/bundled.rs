//! Bundled plugins that ship with Formatorbit.
//!
//! These plugins are embedded in the binary and installed to the
//! bundled plugins directory on first run.

use std::fs;
use std::path::PathBuf;

use super::discovery;

/// Bundled plugin definition.
struct BundledPlugin {
    /// Filename for the plugin.
    filename: &'static str,
    /// Plugin source code.
    source: &'static str,
}

/// List of bundled plugins.
const BUNDLED_PLUGINS: &[BundledPlugin] = &[BundledPlugin {
    filename: "crypto.py",
    source: include_str!("../../../../bundled-plugins/crypto.py"),
}];

/// Install bundled plugins to the data directory if they don't exist.
///
/// This is called on startup to ensure default plugins are available.
/// Plugins are only installed if they don't already exist (never overwrites).
pub fn install_bundled_plugins() -> Result<Vec<PathBuf>, std::io::Error> {
    let dir = discovery::ensure_bundled_plugin_dir()?;
    let mut installed = Vec::new();

    for plugin in BUNDLED_PLUGINS {
        let path = dir.join(plugin.filename);

        // Only install if not present (don't overwrite user modifications)
        if !path.exists() {
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
