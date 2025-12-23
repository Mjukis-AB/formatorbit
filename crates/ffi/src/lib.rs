//! C FFI bindings for Formatorbit.
//!
//! This crate exposes Formatorbit functionality via a C ABI for integration
//! with Swift, Python, and other languages.
//!
//! All functions returning strings allocate memory that must be freed with
//! `formatorbit_free_string`. Null pointers are handled gracefully.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use formatorbit_core::Formatorbit;
use std::sync::OnceLock;

static INSTANCE: OnceLock<Formatorbit> = OnceLock::new();

fn get_instance() -> &'static Formatorbit {
    INSTANCE.get_or_init(Formatorbit::new)
}

/// Helper to convert C string to Rust, returning empty string on null/invalid.
unsafe fn cstr_to_str(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
}

/// Helper to convert Rust string to allocated C string.
fn str_to_cstring(s: &str) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

// =============================================================================
// Version & Info
// =============================================================================

/// Get the library version string.
///
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub extern "C" fn formatorbit_version() -> *mut c_char {
    str_to_cstring(env!("CARGO_PKG_VERSION"))
}

/// Get information about all supported formats as JSON.
///
/// Returns a JSON array of format info objects:
/// ```json
/// [
///   {
///     "id": "hex",
///     "name": "Hexadecimal",
///     "category": "Encoding",
///     "description": "Hexadecimal byte encoding",
///     "examples": ["691E01B8", "0x691E01B8"],
///     "aliases": ["h", "x"]
///   },
///   ...
/// ]
/// ```
///
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub extern "C" fn formatorbit_list_formats() -> *mut c_char {
    let infos = get_instance().format_infos();
    let json = serde_json::to_string(&infos).unwrap_or_else(|_| "[]".to_string());
    str_to_cstring(&json)
}

// =============================================================================
// Conversion Functions
// =============================================================================

/// Convert input and return JSON with all results.
///
/// Returns all possible interpretations and their conversions, sorted by
/// confidence (highest first). Each interpretation includes all possible
/// conversions sorted by priority (structured data first).
///
/// # Safety
///
/// `input` must be a valid null-terminated C string, or null.
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
///
/// # Example Output
///
/// ```json
/// [
///   {
///     "input": "691E01B8",
///     "interpretation": {
///       "value": {"type": "Bytes", "value": [105, 30, 1, 184]},
///       "source_format": "hex",
///       "confidence": 0.92,
///       "description": "4 bytes"
///     },
///     "conversions": [
///       {
///         "value": {"type": "String", "value": "105.30.1.184"},
///         "target_format": "ipv4",
///         "display": "105.30.1.184",
///         "path": ["ipv4"],
///         "is_lossy": false,
///         "priority": "Semantic"
///       },
///       ...
///     ]
///   }
/// ]
/// ```
#[no_mangle]
pub unsafe extern "C" fn formatorbit_convert_all(input: *const c_char) -> *mut c_char {
    let input = unsafe { cstr_to_str(input) };
    let results = get_instance().convert_all(input);
    let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
    str_to_cstring(&json)
}

/// Convert input using only specific formats.
///
/// `formats` is a comma-separated list of format IDs or aliases (e.g., "hex,uuid,ts").
/// Use `formatorbit_list_formats()` to get available format IDs and aliases.
///
/// # Safety
///
/// `input` and `formats` must be valid null-terminated C strings, or null.
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub unsafe extern "C" fn formatorbit_convert_filtered(
    input: *const c_char,
    formats: *const c_char,
) -> *mut c_char {
    let input = unsafe { cstr_to_str(input) };
    let formats_str = unsafe { cstr_to_str(formats) };

    let format_filter: Vec<String> = if formats_str.is_empty() {
        vec![]
    } else {
        formats_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };

    let results = get_instance().convert_all_filtered(input, &format_filter);
    let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
    str_to_cstring(&json)
}

/// Convert input and return only the highest-confidence result.
///
/// This is more efficient when you only need the best interpretation.
/// Returns a single ConversionResult as JSON, or "null" if no interpretation found.
///
/// # Safety
///
/// `input` must be a valid null-terminated C string, or null.
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub unsafe extern "C" fn formatorbit_convert_first(input: *const c_char) -> *mut c_char {
    let input = unsafe { cstr_to_str(input) };
    let results = get_instance().convert_all(input);

    // Filter to meaningful results (confidence > 0.2) and take first
    let result = results
        .into_iter()
        .find(|r| r.interpretation.confidence > 0.2);

    let json = match result {
        Some(r) => serde_json::to_string(&r).unwrap_or_else(|_| "null".to_string()),
        None => "null".to_string(),
    };
    str_to_cstring(&json)
}

/// Convert input, forcing interpretation as a specific format.
///
/// Skips auto-detection and treats the input as the specified format.
/// Useful when you know the input format and want to see conversions.
///
/// # Safety
///
/// `input` and `from_format` must be valid null-terminated C strings, or null.
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub unsafe extern "C" fn formatorbit_convert_from(
    input: *const c_char,
    from_format: *const c_char,
) -> *mut c_char {
    let input_str = unsafe { cstr_to_str(input) };
    let from_format = unsafe { cstr_to_str(from_format) };

    let results = if from_format.is_empty() {
        get_instance().convert_all(input_str)
    } else {
        get_instance().convert_all_filtered(input_str, &[from_format.to_string()])
    };
    let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
    str_to_cstring(&json)
}

// =============================================================================
// Memory Management
// =============================================================================

/// Free a string allocated by formatorbit functions.
///
/// # Safety
///
/// `s` must be a pointer returned by a formatorbit function, or null.
/// After calling this function, the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn formatorbit_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_version() {
        let version = formatorbit_version();
        assert!(!version.is_null());
        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert!(version_str.starts_with("0."));
        unsafe { formatorbit_free_string(version) };
    }

    #[test]
    fn test_list_formats() {
        let formats = formatorbit_list_formats();
        assert!(!formats.is_null());
        let formats_str = unsafe { CStr::from_ptr(formats) }.to_str().unwrap();
        assert!(formats_str.contains("hex"));
        assert!(formats_str.contains("uuid"));
        unsafe { formatorbit_free_string(formats) };
    }

    #[test]
    fn test_convert_all() {
        let input = CString::new("691E01B8").unwrap();
        let result = unsafe { formatorbit_convert_all(input.as_ptr()) };
        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(result_str.contains("hex"));
        unsafe { formatorbit_free_string(result) };
    }

    #[test]
    fn test_convert_filtered() {
        let input = CString::new("691E01B8").unwrap();
        let formats = CString::new("hex").unwrap();
        let result = unsafe { formatorbit_convert_filtered(input.as_ptr(), formats.as_ptr()) };
        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(result_str.contains("hex"));
        // Should NOT contain decimal since we filtered to hex only
        let parsed: serde_json::Value = serde_json::from_str(result_str).unwrap();
        let sources: Vec<_> = parsed
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["interpretation"]["source_format"].as_str().unwrap())
            .collect();
        assert!(sources.contains(&"hex"));
        assert!(!sources.contains(&"decimal"));
        unsafe { formatorbit_free_string(result) };
    }

    #[test]
    fn test_convert_first() {
        let input = CString::new("691E01B8").unwrap();
        let result = unsafe { formatorbit_convert_first(input.as_ptr()) };
        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        // Should be a single object, not an array
        assert!(result_str.starts_with('{'));
        assert!(result_str.contains("interpretation"));
        unsafe { formatorbit_free_string(result) };
    }

    #[test]
    fn test_convert_from() {
        let input = CString::new("1234").unwrap();
        let from = CString::new("hex").unwrap();
        let result = unsafe { formatorbit_convert_from(input.as_ptr(), from.as_ptr()) };
        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        // Should interpret as hex, not decimal
        let parsed: serde_json::Value = serde_json::from_str(result_str).unwrap();
        let first = &parsed[0];
        assert_eq!(
            first["interpretation"]["source_format"].as_str(),
            Some("hex")
        );
        unsafe { formatorbit_free_string(result) };
    }

    #[test]
    fn test_null_handling() {
        // Should not crash on null input
        let result = unsafe { formatorbit_convert_all(std::ptr::null()) };
        assert!(!result.is_null());
        unsafe { formatorbit_free_string(result) };

        let result = unsafe { formatorbit_convert_filtered(std::ptr::null(), std::ptr::null()) };
        assert!(!result.is_null());
        unsafe { formatorbit_free_string(result) };
    }
}
