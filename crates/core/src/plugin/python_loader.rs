//! Runtime Python library loading.
//!
//! This module loads libpython at runtime using dlopen with RTLD_GLOBAL,
//! making Python symbols available for pyo3 to use.

use super::python_discovery::{discover_python, PythonDiscovery, PythonSource};
use std::sync::OnceLock;

/// Global state for loaded Python library.
static PYTHON_LOADED: OnceLock<PythonLoadResult> = OnceLock::new();

/// Result of attempting to load Python.
#[derive(Debug, Clone)]
pub enum PythonLoadResult {
    /// Python was successfully loaded.
    Loaded {
        version: Option<String>,
        source: PythonSource,
    },
    /// Python was not found.
    NotFound,
    /// Failed to load Python library.
    LoadError(String),
}

impl PythonLoadResult {
    /// Check if Python is available.
    pub fn is_available(&self) -> bool {
        matches!(self, PythonLoadResult::Loaded { .. })
    }
}

/// Attempt to load Python at runtime.
///
/// This function:
/// 1. Discovers Python installation on the system
/// 2. Loads libpython with RTLD_GLOBAL so symbols are available globally
/// 3. Returns the result (can be called multiple times, only loads once)
///
/// After calling this, pyo3's `prepare_freethreaded_python()` will work
/// because the Python symbols are already loaded.
pub fn ensure_python_loaded() -> &'static PythonLoadResult {
    PYTHON_LOADED.get_or_init(|| {
        let discovery = discover_python();
        load_python_library(discovery)
    })
}

/// Check if Python is available without loading it.
pub fn is_python_available() -> bool {
    PYTHON_LOADED
        .get()
        .map(|r| r.is_available())
        .unwrap_or(false)
}

/// Load the Python library from discovery result.
fn load_python_library(discovery: PythonDiscovery) -> PythonLoadResult {
    let lib_path = match discovery.library_path {
        Some(path) => path,
        None => {
            tracing::debug!("Python not found on system");
            return PythonLoadResult::NotFound;
        }
    };

    tracing::debug!(
        path = %lib_path.display(),
        version = ?discovery.version,
        source = ?discovery.source,
        "Attempting to load Python library"
    );

    // Load the library with RTLD_GLOBAL so symbols are available to pyo3
    match load_library_global(&lib_path) {
        Ok(()) => {
            tracing::info!(
                path = %lib_path.display(),
                version = ?discovery.version,
                "Successfully loaded Python library"
            );
            PythonLoadResult::Loaded {
                version: discovery.version,
                source: discovery.source,
            }
        }
        Err(e) => {
            tracing::warn!(
                path = %lib_path.display(),
                error = %e,
                "Failed to load Python library"
            );
            PythonLoadResult::LoadError(e)
        }
    }
}

/// Load a shared library with RTLD_GLOBAL flag.
#[cfg(unix)]
fn load_library_global(path: &std::path::Path) -> Result<(), String> {
    use std::ffi::CString;
    use std::os::raw::c_int;

    // RTLD_NOW | RTLD_GLOBAL
    const RTLD_NOW: c_int = 0x2;

    #[cfg(target_os = "macos")]
    const RTLD_GLOBAL: c_int = 0x8; // macOS uses different value

    #[cfg(not(target_os = "macos"))]
    const RTLD_GLOBAL: c_int = 0x100;

    extern "C" {
        fn dlopen(filename: *const std::os::raw::c_char, flags: c_int) -> *mut std::ffi::c_void;
        fn dlerror() -> *const std::os::raw::c_char;
    }

    let path_cstr = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|e| format!("Invalid path: {}", e))?;

    let handle = unsafe { dlopen(path_cstr.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };

    if handle.is_null() {
        let error = unsafe {
            let err_ptr = dlerror();
            if err_ptr.is_null() {
                "Unknown error".to_string()
            } else {
                std::ffi::CStr::from_ptr(err_ptr)
                    .to_string_lossy()
                    .into_owned()
            }
        };
        Err(error)
    } else {
        // Note: We intentionally don't call dlclose - Python must remain loaded
        // for the lifetime of the process. The handle is leaked.
        Ok(())
    }
}

/// Load a shared library on Windows.
#[cfg(windows)]
fn load_library_global(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    // Windows doesn't have RTLD_GLOBAL concept - LoadLibrary always makes symbols available
    extern "system" {
        fn LoadLibraryW(lpLibFileName: *const u16) -> *mut std::ffi::c_void;
        fn GetLastError() -> u32;
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };

    if handle.is_null() {
        let error_code = unsafe { GetLastError() };
        Err(format!("LoadLibrary failed with error code {}", error_code))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_python_loaded() {
        let result = ensure_python_loaded();
        println!("Python load result: {:?}", result);
        // Don't assert availability - depends on system
    }

    #[test]
    fn test_load_result_is_available() {
        assert!(PythonLoadResult::Loaded {
            version: Some("3.12".to_string()),
            source: PythonSource::System,
        }
        .is_available());

        assert!(!PythonLoadResult::NotFound.is_available());
        assert!(!PythonLoadResult::LoadError("test".to_string()).is_available());
    }
}
