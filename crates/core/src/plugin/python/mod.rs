//! Python runtime and plugin loading via pyo3.
//!
//! This module provides the embedded Python runtime for loading and
//! executing Python plugins.
//!
//! Python is loaded dynamically at runtime using dlopen, which means:
//! - The binary works without Python installed (plugins just won't be available)
//! - Any Python 3.9+ installation will work
//! - No hardcoded paths to specific Python versions

mod currency;
mod decoder;
mod expr;
mod trait_plugin;
mod types;
mod visualizer;

use super::python_loader::{ensure_python_loaded, PythonLoadResult};
use super::{Plugin, PluginError, PluginMeta};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use std::ffi::CString;
use std::path::Path;
use std::sync::OnceLock;

// Thread-local storage for plugin registrations during module loading.
// This is used by decorators to register plugins as they're defined.
std::thread_local! {
    static PENDING_DECODERS: std::cell::RefCell<Vec<decoder::DecoderRegistration>> =
        std::cell::RefCell::new(Vec::new());
    static PENDING_EXPR_VARS: std::cell::RefCell<Vec<expr::ExprVarRegistration>> =
        std::cell::RefCell::new(Vec::new());
    static PENDING_EXPR_FUNCS: std::cell::RefCell<Vec<expr::ExprFuncRegistration>> =
        std::cell::RefCell::new(Vec::new());
    static PENDING_TRAITS: std::cell::RefCell<Vec<trait_plugin::TraitRegistration>> =
        std::cell::RefCell::new(Vec::new());
    static PENDING_VISUALIZERS: std::cell::RefCell<Vec<visualizer::VisualizerRegistration>> =
        std::cell::RefCell::new(Vec::new());
    static PENDING_CURRENCIES: std::cell::RefCell<Vec<currency::CurrencyRegistration>> =
        std::cell::RefCell::new(Vec::new());
}

/// Global Python runtime (initialized once).
static PYTHON_RUNTIME: OnceLock<PythonRuntime> = OnceLock::new();

/// The embedded Python runtime for plugins.
pub struct PythonRuntime {
    // No state needed - Python GIL is obtained on-demand
}

impl PythonRuntime {
    /// Initialize the Python runtime (call once at startup).
    ///
    /// This is safe to call multiple times - subsequent calls return
    /// the existing runtime.
    ///
    /// Returns an error if Python is not available on the system.
    pub fn init() -> Result<&'static Self, PluginError> {
        // Check if already initialized
        if let Some(runtime) = PYTHON_RUNTIME.get() {
            return Ok(runtime);
        }

        // First, ensure Python library is loaded via dlopen
        let load_result = ensure_python_loaded();
        match load_result {
            PythonLoadResult::Loaded { version, source } => {
                tracing::info!(
                    version = ?version,
                    source = ?source,
                    "Python library loaded successfully"
                );
            }
            PythonLoadResult::NotFound => {
                return Err(PluginError::RuntimeInit(
                    "Python not found. Install Python 3.9+ to enable plugins.".to_string(),
                ));
            }
            PythonLoadResult::LoadError(e) => {
                return Err(PluginError::RuntimeInit(format!(
                    "Failed to load Python: {}. Install Python 3.9+ to enable plugins.",
                    e
                )));
            }
        }

        // Initialize pyo3 for multi-threaded use
        // This now works because we've dlopen'd libpython with RTLD_GLOBAL
        pyo3::prepare_freethreaded_python();

        // Inject the `forb` module into Python
        Python::with_gil(|py| {
            inject_forb_module(py).map_err(|e| {
                PluginError::RuntimeInit(format!("Failed to inject forb module: {}", e))
            })
        })?;

        // Store and return the runtime
        let _ = PYTHON_RUNTIME.set(PythonRuntime {});
        Ok(PYTHON_RUNTIME.get().unwrap())
    }

    /// Load a plugin file and return all plugins it defines.
    pub fn load_plugin(&self, path: &Path) -> Result<Vec<Plugin>, PluginError> {
        if !path.exists() {
            return Err(PluginError::FileNotFound(path.to_path_buf()));
        }

        let code = std::fs::read_to_string(path)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("plugin.py");

        Python::with_gil(|py| {
            // Clear pending registrations from previous loads
            clear_pending_registrations();

            // Create a new module for this plugin
            let module = PyModule::new(py, filename).map_err(|e| PluginError::PythonError {
                plugin: filename.to_string(),
                message: format!("Failed to create module: {}", e),
                traceback: None,
            })?;

            // Add built-ins to the module dict
            let globals = module.dict();
            let builtins = py
                .import("builtins")
                .map_err(|e| PluginError::PythonError {
                    plugin: filename.to_string(),
                    message: format!("Failed to import builtins: {}", e),
                    traceback: None,
                })?;
            globals.set_item("__builtins__", builtins).ok();
            globals.set_item("__name__", filename).ok();
            globals
                .set_item("__file__", path.to_string_lossy().as_ref())
                .ok();

            // Execute the plugin code
            let code_cstr = CString::new(code.as_str()).map_err(|e| PluginError::PythonError {
                plugin: filename.to_string(),
                message: format!("Invalid plugin code (null byte): {}", e),
                traceback: None,
            })?;
            py.run(code_cstr.as_c_str(), Some(&globals), Some(&globals))
                .map_err(|e| {
                    let traceback = e.traceback(py).map(|tb| {
                        tb.format()
                            .unwrap_or_else(|_| "Failed to format traceback".to_string())
                    });
                    PluginError::PythonError {
                        plugin: filename.to_string(),
                        message: e.to_string(),
                        traceback,
                    }
                })?;

            // Extract metadata
            let meta = extract_metadata(py, &globals, path)?;

            // Collect registered plugins from decorators
            let plugins = collect_registered_plugins(py, meta)?;

            Ok(plugins)
        })
    }
}

/// Inject the `forb` module into Python's sys.modules.
fn inject_forb_module(py: Python<'_>) -> PyResult<()> {
    let forb_module = PyModule::new(py, "forb")?;

    // Add type classes
    types::add_types_to_module(py, &forb_module)?;

    // Add decorator functions
    decoder::add_decoder_decorator(py, &forb_module)?;
    expr::add_expr_decorators(py, &forb_module)?;
    trait_plugin::add_trait_decorator(py, &forb_module)?;
    visualizer::add_visualizer_decorator(py, &forb_module)?;
    currency::add_currency_decorator(py, &forb_module)?;

    // Register module in sys.modules
    let sys = py.import("sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item("forb", &forb_module)?;

    Ok(())
}

/// Extract plugin metadata from the module's __forb_plugin__ dict.
fn extract_metadata(
    py: Python<'_>,
    globals: &Bound<'_, PyDict>,
    path: &Path,
) -> Result<PluginMeta, PluginError> {
    let meta_obj = globals
        .get_item("__forb_plugin__")
        .map_err(|_| PluginError::MissingMetadata(path.to_path_buf()))?;

    let meta_dict = match meta_obj {
        Some(obj) => obj
            .downcast::<PyDict>()
            .map_err(|_| PluginError::InvalidMetadata {
                path: path.to_path_buf(),
                message: "__forb_plugin__ must be a dict".to_string(),
            })?
            .clone(),
        None => {
            return Err(PluginError::MissingMetadata(path.to_path_buf()));
        }
    };

    // Extract required fields
    let name = extract_string_field(py, &meta_dict, "name").ok_or_else(|| {
        PluginError::InvalidMetadata {
            path: path.to_path_buf(),
            message: "missing required field 'name'".to_string(),
        }
    })?;

    let version = extract_string_field(py, &meta_dict, "version").ok_or_else(|| {
        PluginError::InvalidMetadata {
            path: path.to_path_buf(),
            message: "missing required field 'version'".to_string(),
        }
    })?;

    // Extract optional fields
    let author = extract_string_field(py, &meta_dict, "author");
    let description = extract_string_field(py, &meta_dict, "description");

    Ok(PluginMeta {
        name,
        version,
        author,
        description,
        source_file: path.to_path_buf(),
    })
}

/// Extract a string field from a PyDict.
fn extract_string_field(_py: Python<'_>, dict: &Bound<'_, PyDict>, key: &str) -> Option<String> {
    dict.get_item(key)
        .ok()
        .flatten()
        .and_then(|v| v.extract::<String>().ok())
}

/// Clear all pending registrations (called before loading a new plugin).
fn clear_pending_registrations() {
    PENDING_DECODERS.with(|d| d.borrow_mut().clear());
    PENDING_EXPR_VARS.with(|v| v.borrow_mut().clear());
    PENDING_EXPR_FUNCS.with(|f| f.borrow_mut().clear());
    PENDING_TRAITS.with(|t| t.borrow_mut().clear());
    PENDING_VISUALIZERS.with(|v| v.borrow_mut().clear());
    PENDING_CURRENCIES.with(|c| c.borrow_mut().clear());
}

/// Collect all plugins registered via decorators during module execution.
fn collect_registered_plugins(
    _py: Python<'_>,
    meta: PluginMeta,
) -> Result<Vec<Plugin>, PluginError> {
    let mut plugins = Vec::new();

    // Collect decoders
    PENDING_DECODERS.with(|decoders| {
        for reg in decoders.borrow_mut().drain(..) {
            plugins.push(Plugin::Decoder(Box::new(decoder::PyDecoderPlugin::new(
                reg,
                meta.clone(),
            ))));
        }
    });

    // Collect expression variables
    PENDING_EXPR_VARS.with(|vars| {
        for reg in vars.borrow_mut().drain(..) {
            plugins.push(Plugin::ExprVar(expr::create_expr_var_plugin(
                reg,
                meta.clone(),
            )));
        }
    });

    // Collect expression functions
    PENDING_EXPR_FUNCS.with(|funcs| {
        for reg in funcs.borrow_mut().drain(..) {
            plugins.push(Plugin::ExprFunc(expr::create_expr_func_plugin(
                reg,
                meta.clone(),
            )));
        }
    });

    // Collect traits
    PENDING_TRAITS.with(|traits| {
        for reg in traits.borrow_mut().drain(..) {
            plugins.push(Plugin::Trait(Box::new(trait_plugin::PyTraitPlugin::new(
                reg,
                meta.clone(),
            ))));
        }
    });

    // Collect visualizers
    PENDING_VISUALIZERS.with(|visualizers| {
        for reg in visualizers.borrow_mut().drain(..) {
            plugins.push(Plugin::Visualizer(Box::new(
                visualizer::PyVisualizerPlugin::new(reg, meta.clone()),
            )));
        }
    });

    // Collect currencies
    PENDING_CURRENCIES.with(|currencies| {
        for reg in currencies.borrow_mut().drain(..) {
            plugins.push(Plugin::Currency(Box::new(currency::PyCurrencyPlugin::new(
                reg,
                meta.clone(),
            ))));
        }
    });

    Ok(plugins)
}

/// Register a decoder from the @forb.decoder decorator.
pub(crate) fn register_decoder(reg: decoder::DecoderRegistration) {
    PENDING_DECODERS.with(|d| d.borrow_mut().push(reg));
}

/// Register an expression variable from the @forb.expr_var decorator.
pub(crate) fn register_expr_var(reg: expr::ExprVarRegistration) {
    PENDING_EXPR_VARS.with(|v| v.borrow_mut().push(reg));
}

/// Register an expression function from the @forb.expr_func decorator.
pub(crate) fn register_expr_func(reg: expr::ExprFuncRegistration) {
    PENDING_EXPR_FUNCS.with(|f| f.borrow_mut().push(reg));
}

/// Register a trait from the @forb.trait decorator.
pub(crate) fn register_trait(reg: trait_plugin::TraitRegistration) {
    PENDING_TRAITS.with(|t| t.borrow_mut().push(reg));
}

/// Register a visualizer from the @forb.visualizer decorator.
pub(crate) fn register_visualizer(reg: visualizer::VisualizerRegistration) {
    PENDING_VISUALIZERS.with(|v| v.borrow_mut().push(reg));
}

/// Register a currency from the @forb.currency decorator.
pub(crate) fn register_currency(reg: currency::CurrencyRegistration) {
    PENDING_CURRENCIES.with(|c| c.borrow_mut().push(reg));
}
