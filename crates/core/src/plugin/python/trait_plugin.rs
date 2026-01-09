//! Python trait plugin implementation.

use super::register_trait;
use super::types::core_value_to_py;
use crate::plugin::{PluginMeta, TraitPlugin};
use crate::types::CoreValue;
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyModule, PyTuple};

/// Registration info captured by the @forb.trait decorator.
pub struct TraitRegistration {
    pub id: String,
    pub name: String,
    pub value_types: Vec<String>,
    pub func: PyObject,
}

/// A Python trait plugin.
pub struct PyTraitPlugin {
    id: String,
    name: String,
    value_types: Vec<String>,
    meta: PluginMeta,
    func: PyObject,
}

impl PyTraitPlugin {
    /// Create a new trait plugin from registration info.
    pub fn new(reg: TraitRegistration, meta: PluginMeta) -> Self {
        Self {
            id: reg.id,
            name: reg.name,
            value_types: reg.value_types,
            meta,
            func: reg.func,
        }
    }
}

impl std::fmt::Debug for PyTraitPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyTraitPlugin")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl TraitPlugin for PyTraitPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn meta(&self) -> &PluginMeta {
        &self.meta
    }

    fn value_types(&self) -> &[String] {
        &self.value_types
    }

    fn check(&self, value: &CoreValue) -> Option<String> {
        Python::with_gil(|py| {
            // Convert CoreValue to Python object
            let py_value = match core_value_to_py(py, value) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        plugin = %self.id,
                        error = %e,
                        "Failed to convert value to Python"
                    );
                    return None;
                }
            };

            // Call the Python function
            match self.func.call1(py, (py_value,)) {
                Ok(result) => {
                    let bound_result = result.bind(py);
                    // None means trait doesn't apply
                    if bound_result.is_none() {
                        return None;
                    }
                    // Extract string description
                    match bound_result.extract::<String>() {
                        Ok(s) => Some(s),
                        Err(e) => {
                            tracing::warn!(
                                plugin = %self.id,
                                error = %e,
                                "Plugin returned non-string value"
                            );
                            None
                        }
                    }
                }
                Err(e) => {
                    let traceback = e.traceback(py).map(|tb| {
                        tb.format()
                            .unwrap_or_else(|_| "Failed to format traceback".to_string())
                    });

                    tracing::warn!(
                        plugin = %self.id,
                        error = %e,
                        traceback = ?traceback,
                        "Plugin raised exception"
                    );
                    None
                }
            }
        })
    }
}

/// Add the @forb.trait decorator to the module.
pub fn add_trait_decorator(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    let decorator_code = c"
def trait(id, name, value_types=None):
    \"\"\"
    Decorator to register a trait plugin.

    A trait observes properties of values without transforming them.

    Usage:
        @forb.trait(id=\"is-lucky\", name=\"Lucky Number\", value_types=[\"int\"])
        def check_lucky(value) -> str | None:
            if value in {7, 42, 777}:
                return f\"Lucky number ({value})\"
            return None

    Args:
        id: Unique identifier for this trait
        name: Human-readable name
        value_types: List of CoreValue types to check (empty = all types)
                     Valid types: \"int\", \"float\", \"string\", \"bytes\", \"bool\", \"datetime\", \"json\"
    \"\"\"
    def decorator(func):
        _register_trait(
            id,
            name,
            value_types or [],
            func
        )
        return func
    return decorator
";

    py.run(decorator_code, Some(&module.dict()), None)?;

    // Add the registration function
    let register_fn = PyCFunction::new_closure(
        py,
        Some(c"_register_trait"),
        None,
        |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let id: String = args.get_item(0)?.extract()?;
            let name: String = args.get_item(1)?.extract()?;
            let value_types: Vec<String> = args.get_item(2)?.extract()?;
            let func: PyObject = args.get_item(3)?.unbind();

            register_trait(TraitRegistration {
                id,
                name,
                value_types,
                func,
            });

            Ok(())
        },
    )?;

    module.setattr("_register_trait", register_fn)?;

    Ok(())
}
