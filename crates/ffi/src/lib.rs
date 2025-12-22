//! C FFI bindings for Formatorbit.
//!
//! This crate exposes Formatorbit functionality via a C ABI for integration
//! with Swift, Python, and other languages.
//!
//! For now, this is a placeholder. The full FFI will be implemented in Phase 3.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use formatorbit_core::Formatorbit;
use std::sync::OnceLock;

static INSTANCE: OnceLock<Formatorbit> = OnceLock::new();

fn get_instance() -> &'static Formatorbit {
    INSTANCE.get_or_init(Formatorbit::new)
}

/// Convert input and return JSON with all results.
///
/// # Safety
///
/// `input` must be a valid null-terminated C string.
/// Returns a newly allocated string. Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub unsafe extern "C" fn formatorbit_convert_all(input: *const c_char) -> *mut c_char {
    let input = unsafe { CStr::from_ptr(input) }.to_str().unwrap_or("");
    let results = get_instance().convert_all(input);
    let json = serde_json::to_string(&results).unwrap_or_default();
    CString::new(json).unwrap().into_raw()
}

/// Free a string allocated by formatorbit functions.
///
/// # Safety
///
/// `s` must be a pointer returned by a formatorbit function, or null.
#[no_mangle]
pub unsafe extern "C" fn formatorbit_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}
