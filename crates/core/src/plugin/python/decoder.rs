//! Python decoder plugin implementation.

use super::register_decoder;
use super::types::py_to_interpretations;
use crate::plugin::{DecoderPlugin, PluginMeta};
use crate::types::Interpretation;
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyModule, PyTuple};

/// Registration info captured by the @forb.decoder decorator.
pub struct DecoderRegistration {
    pub id: String,
    pub name: String,
    pub category: String,
    pub examples: Vec<String>,
    pub aliases: Vec<String>,
    pub func: PyObject,
}

/// A Python decoder plugin.
pub struct PyDecoderPlugin {
    id: String,
    name: String,
    category: String,
    examples: Vec<String>,
    aliases: Vec<String>,
    meta: PluginMeta,
    func: PyObject,
}

impl PyDecoderPlugin {
    /// Create a new decoder plugin from registration info.
    pub fn new(reg: DecoderRegistration, meta: PluginMeta) -> Self {
        Self {
            id: reg.id,
            name: reg.name,
            category: reg.category,
            examples: reg.examples,
            aliases: reg.aliases,
            meta,
            func: reg.func,
        }
    }
}

impl std::fmt::Debug for PyDecoderPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyDecoderPlugin")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl DecoderPlugin for PyDecoderPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn meta(&self) -> &PluginMeta {
        &self.meta
    }

    fn aliases(&self) -> &[String] {
        &self.aliases
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        Python::with_gil(|py| {
            match self.func.call1(py, (input,)) {
                Ok(result) => {
                    let bound_result = result.bind(py);
                    match py_to_interpretations(py, bound_result) {
                        Ok(mut interps) => {
                            // Set source_format for all interpretations
                            for interp in &mut interps {
                                if interp.source_format.is_empty() {
                                    interp.source_format = self.id.clone();
                                }
                            }
                            interps
                        }
                        Err(e) => {
                            tracing::warn!(
                                plugin = %self.id,
                                error = %e,
                                "Plugin returned invalid value"
                            );
                            vec![]
                        }
                    }
                }
                Err(e) => {
                    // Extract traceback for better debugging
                    let traceback = e.traceback(py).map(|tb| {
                        tb.format()
                            .unwrap_or_else(|_| "Failed to format traceback".to_string())
                    });

                    tracing::warn!(
                        plugin = %self.id,
                        error = %e,
                        traceback = ?traceback,
                        input = %input,
                        "Plugin raised exception"
                    );

                    // Return empty - plugin failure shouldn't crash forb
                    vec![]
                }
            }
        })
    }
}

/// Add the @forb.decoder decorator to the module.
pub fn add_decoder_decorator(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Create the decorator factory function
    let decorator_code = c"
def decoder(id, name, category=\"Custom\", examples=None, aliases=None):
    \"\"\"
    Decorator to register a decoder plugin.

    Usage:
        @forb.decoder(id=\"myformat\", name=\"My Format\")
        def decode_myformat(input: str) -> list[Interpretation]:
            ...

    Args:
        id: Unique identifier for this decoder
        name: Human-readable name
        category: Category for grouping (default: \"Custom\")
        examples: Example inputs that this decoder handles
        aliases: Alternative names for this decoder
    \"\"\"
    def decorator(func):
        # Register with the Rust runtime
        _register_decoder(
            id,
            name,
            category,
            examples or [],
            aliases or [],
            func
        )
        return func
    return decorator
";

    py.run(decorator_code, Some(&module.dict()), None)?;

    // Add the registration function that calls back into Rust
    let register_fn = PyCFunction::new_closure(
        py,
        Some(c"_register_decoder"),
        None,
        |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let id: String = args.get_item(0)?.extract()?;
            let name: String = args.get_item(1)?.extract()?;
            let category: String = args.get_item(2)?.extract()?;
            let examples: Vec<String> = args.get_item(3)?.extract()?;
            let aliases: Vec<String> = args.get_item(4)?.extract()?;
            let func: PyObject = args.get_item(5)?.unbind();

            register_decoder(DecoderRegistration {
                id,
                name,
                category,
                examples,
                aliases,
                func,
            });

            Ok(())
        },
    )?;

    module.setattr("_register_decoder", register_fn)?;

    Ok(())
}
