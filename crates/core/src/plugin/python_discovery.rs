//! Runtime Python discovery and loading.
//!
//! This module finds and loads Python at runtime rather than linking at compile time.
//! This allows the binary to work without Python installed - plugins simply won't be available.

use std::path::PathBuf;

/// Result of attempting to discover Python.
#[derive(Debug)]
pub struct PythonDiscovery {
    /// Path to the Python shared library (if found).
    pub library_path: Option<PathBuf>,
    /// Python version string (if detected).
    pub version: Option<String>,
    /// How Python was found.
    pub source: PythonSource,
}

/// How Python was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonSource {
    /// Found via PYTHONHOME or similar environment variable.
    Environment,
    /// Found in a standard system location.
    System,
    /// Found via `python3-config --ldflags`.
    PythonConfig,
    /// Bundled with the application.
    Bundled,
    /// Python was not found.
    NotFound,
}

/// Discover Python installation on the system.
///
/// Checks multiple locations in order of preference:
/// 1. PYTHONHOME environment variable
/// 2. python3-config --ldflags (if available)
/// 3. Standard system paths for the current OS
pub fn discover_python() -> PythonDiscovery {
    // Try environment variable first
    if let Some(discovery) = try_environment() {
        return discovery;
    }

    // Try python3-config
    if let Some(discovery) = try_python_config() {
        return discovery;
    }

    // Try standard system paths
    if let Some(discovery) = try_system_paths() {
        return discovery;
    }

    PythonDiscovery {
        library_path: None,
        version: None,
        source: PythonSource::NotFound,
    }
}

/// Try to find Python via environment variables.
fn try_environment() -> Option<PythonDiscovery> {
    // Check PYTHONHOME
    if let Ok(python_home) = std::env::var("PYTHONHOME") {
        let path = PathBuf::from(&python_home);
        if let Some(lib_path) = find_libpython_in_prefix(&path) {
            return Some(PythonDiscovery {
                library_path: Some(lib_path),
                version: detect_version_from_path(&path),
                source: PythonSource::Environment,
            });
        }
    }

    // Check VIRTUAL_ENV (for virtualenvs)
    if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
        let path = PathBuf::from(&venv);
        if let Some(lib_path) = find_libpython_in_prefix(&path) {
            return Some(PythonDiscovery {
                library_path: Some(lib_path),
                version: detect_version_from_path(&path),
                source: PythonSource::Environment,
            });
        }
    }

    None
}

/// Try to find Python via python3-config.
fn try_python_config() -> Option<PythonDiscovery> {
    let output = std::process::Command::new("python3-config")
        .arg("--ldflags")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let ldflags = String::from_utf8_lossy(&output.stdout);

    // Parse -L flags to find library directory
    for flag in ldflags.split_whitespace() {
        if let Some(dir) = flag.strip_prefix("-L") {
            let path = PathBuf::from(dir);
            if let Some(lib_path) = find_libpython_in_dir(&path) {
                // Get version from python3 --version
                let version = std::process::Command::new("python3")
                    .arg("--version")
                    .output()
                    .ok()
                    .and_then(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .strip_prefix("Python ")
                            .map(|s| s.trim().to_string())
                    });

                return Some(PythonDiscovery {
                    library_path: Some(lib_path),
                    version,
                    source: PythonSource::PythonConfig,
                });
            }
        }
    }

    None
}

/// Try standard system paths for Python.
fn try_system_paths() -> Option<PythonDiscovery> {
    let search_paths = get_system_python_paths();

    for path in search_paths {
        if let Some(lib_path) = find_libpython_in_dir(&path) {
            return Some(PythonDiscovery {
                library_path: Some(lib_path.clone()),
                version: detect_version_from_lib_path(&lib_path),
                source: PythonSource::System,
            });
        }
    }

    None
}

/// Get standard Python library search paths for the current OS.
fn get_system_python_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Homebrew locations (both Intel and Apple Silicon)
        paths.push(PathBuf::from(
            "/opt/homebrew/opt/python@3.13/Frameworks/Python.framework/Versions/3.13/lib",
        ));
        paths.push(PathBuf::from(
            "/opt/homebrew/opt/python@3.12/Frameworks/Python.framework/Versions/3.12/lib",
        ));
        paths.push(PathBuf::from(
            "/opt/homebrew/opt/python@3.11/Frameworks/Python.framework/Versions/3.11/lib",
        ));
        paths.push(PathBuf::from(
            "/opt/homebrew/opt/python@3.10/Frameworks/Python.framework/Versions/3.10/lib",
        ));
        paths.push(PathBuf::from(
            "/opt/homebrew/opt/python@3.9/Frameworks/Python.framework/Versions/3.9/lib",
        ));
        paths.push(PathBuf::from(
            "/usr/local/opt/python@3.13/Frameworks/Python.framework/Versions/3.13/lib",
        ));
        paths.push(PathBuf::from(
            "/usr/local/opt/python@3.12/Frameworks/Python.framework/Versions/3.12/lib",
        ));
        paths.push(PathBuf::from(
            "/usr/local/opt/python@3.11/Frameworks/Python.framework/Versions/3.11/lib",
        ));
        paths.push(PathBuf::from(
            "/usr/local/opt/python@3.10/Frameworks/Python.framework/Versions/3.10/lib",
        ));
        paths.push(PathBuf::from(
            "/usr/local/opt/python@3.9/Frameworks/Python.framework/Versions/3.9/lib",
        ));

        // Homebrew lib directories
        paths.push(PathBuf::from("/opt/homebrew/lib"));
        paths.push(PathBuf::from("/usr/local/lib"));

        // System Python framework
        paths.push(PathBuf::from(
            "/Library/Frameworks/Python.framework/Versions/3.13/lib",
        ));
        paths.push(PathBuf::from(
            "/Library/Frameworks/Python.framework/Versions/3.12/lib",
        ));
        paths.push(PathBuf::from(
            "/Library/Frameworks/Python.framework/Versions/3.11/lib",
        ));
        paths.push(PathBuf::from(
            "/Library/Frameworks/Python.framework/Versions/3.10/lib",
        ));
        paths.push(PathBuf::from(
            "/Library/Frameworks/Python.framework/Versions/3.9/lib",
        ));

        // Xcode Command Line Tools Python (limited, but exists)
        paths.push(PathBuf::from("/Library/Developer/CommandLineTools/Library/Frameworks/Python3.framework/Versions/Current/lib"));
    }

    #[cfg(target_os = "linux")]
    {
        // Common Linux paths
        paths.push(PathBuf::from("/usr/lib/x86_64-linux-gnu"));
        paths.push(PathBuf::from("/usr/lib/aarch64-linux-gnu"));
        paths.push(PathBuf::from("/usr/lib64"));
        paths.push(PathBuf::from("/usr/lib"));
        paths.push(PathBuf::from("/usr/local/lib"));

        // pyenv default location
        if let Ok(home) = std::env::var("HOME") {
            for version in ["3.13", "3.12", "3.11", "3.10", "3.9"] {
                paths.push(PathBuf::from(format!(
                    "{}/.pyenv/versions/{}/lib",
                    home, version
                )));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Python installations
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            for version in ["313", "312", "311", "310", "39"] {
                paths.push(PathBuf::from(format!(
                    "{}\\Programs\\Python\\Python{}",
                    localappdata, version
                )));
            }
        }

        // System-wide installations
        for version in ["313", "312", "311", "310", "39"] {
            paths.push(PathBuf::from(format!("C:\\Python{}", version)));
            paths.push(PathBuf::from(format!(
                "C:\\Program Files\\Python{}",
                version
            )));
        }
    }

    paths
}

/// Find libpython in a Python prefix directory.
fn find_libpython_in_prefix(prefix: &PathBuf) -> Option<PathBuf> {
    // Try lib subdirectory
    let lib_dir = prefix.join("lib");
    if let Some(path) = find_libpython_in_dir(&lib_dir) {
        return Some(path);
    }

    // Try directly in prefix (Windows)
    find_libpython_in_dir(prefix)
}

/// Find libpython shared library in a directory.
fn find_libpython_in_dir(dir: &PathBuf) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }

    // Library names to search for, in order of preference (newer versions first)
    let lib_names = get_libpython_names();

    for name in lib_names {
        let path = dir.join(&name);
        if path.exists() {
            return Some(path);
        }
    }

    // On macOS, also check for framework structure
    #[cfg(target_os = "macos")]
    {
        for version in ["3.13", "3.12", "3.11", "3.10", "3.9"] {
            let framework_lib = dir.join(format!("libpython{}.dylib", version));
            if framework_lib.exists() {
                return Some(framework_lib);
            }
        }
    }

    None
}

/// Get platform-specific libpython names to search for.
fn get_libpython_names() -> Vec<String> {
    let mut names = Vec::new();

    #[cfg(target_os = "macos")]
    {
        for version in ["3.13", "3.12", "3.11", "3.10", "3.9"] {
            names.push(format!("libpython{}.dylib", version));
        }
    }

    #[cfg(target_os = "linux")]
    {
        for version in ["3.13", "3.12", "3.11", "3.10", "3.9"] {
            names.push(format!("libpython{}.so.1.0", version));
            names.push(format!("libpython{}.so", version));
        }
    }

    #[cfg(target_os = "windows")]
    {
        for version in ["313", "312", "311", "310", "39"] {
            names.push(format!("python{}.dll", version));
        }
    }

    names
}

/// Try to detect Python version from a path.
fn detect_version_from_path(path: &PathBuf) -> Option<String> {
    let path_str = path.to_string_lossy();

    // Look for version patterns like "3.12", "3.11", etc.
    for version in ["3.13", "3.12", "3.11", "3.10", "3.9"] {
        if path_str.contains(version) {
            return Some(version.to_string());
        }
    }

    None
}

/// Try to detect Python version from library path.
fn detect_version_from_lib_path(lib_path: &PathBuf) -> Option<String> {
    let filename = lib_path.file_name()?.to_string_lossy();

    // Extract version from filename like "libpython3.12.dylib"
    for version in ["3.13", "3.12", "3.11", "3.10", "3.9"] {
        if filename.contains(version) {
            return Some(version.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_python() {
        let discovery = discover_python();
        // This test just ensures discovery doesn't panic
        // Actual Python presence varies by system
        println!("Python discovery: {:?}", discovery);
    }

    #[test]
    fn test_get_system_python_paths() {
        let paths = get_system_python_paths();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_get_libpython_names() {
        let names = get_libpython_names();
        assert!(!names.is_empty());
    }
}
