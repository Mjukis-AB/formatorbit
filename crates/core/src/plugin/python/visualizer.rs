//! Python visualizer plugin implementation.

use super::register_visualizer;
use super::types::core_value_to_py;
use crate::plugin::{PluginMeta, VisualizerPlugin};
use crate::types::{CoreValue, RichDisplay, TreeNode};
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyList, PyModule, PyTuple};

/// Registration info captured by the @forb.visualizer decorator.
pub struct VisualizerRegistration {
    pub id: String,
    pub name: String,
    pub value_types: Vec<String>,
    pub func: PyObject,
}

/// A Python visualizer plugin.
pub struct PyVisualizerPlugin {
    id: String,
    name: String,
    value_types: Vec<String>,
    meta: PluginMeta,
    func: PyObject,
}

impl PyVisualizerPlugin {
    /// Create a new visualizer plugin from registration info.
    pub fn new(reg: VisualizerRegistration, meta: PluginMeta) -> Self {
        Self {
            id: reg.id,
            name: reg.name,
            value_types: reg.value_types,
            meta,
            func: reg.func,
        }
    }
}

impl std::fmt::Debug for PyVisualizerPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyVisualizerPlugin")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl VisualizerPlugin for PyVisualizerPlugin {
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

    fn visualize(&self, value: &CoreValue) -> Option<RichDisplay> {
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
                    // None means visualizer doesn't apply
                    if bound_result.is_none() {
                        return None;
                    }
                    // Convert RichDisplay from Python
                    match py_to_rich_display(py, bound_result) {
                        Ok(rd) => Some(rd),
                        Err(e) => {
                            tracing::warn!(
                                plugin = %self.id,
                                error = %e,
                                "Plugin returned invalid RichDisplay"
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

/// Convert a Python RichDisplay object to Rust.
fn py_to_rich_display(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<RichDisplay> {
    // Check if it's our RichDisplay class
    let type_name: String = obj.getattr("_type")?.extract()?;
    let data = obj.getattr("_data")?;

    match type_name.as_str() {
        "key_value" => {
            let pairs_list = data.get_item("pairs")?;
            let pairs_pylist = pairs_list.downcast::<PyList>()?;
            let mut pairs = Vec::new();
            for item in pairs_pylist.iter() {
                let tuple = item.downcast::<PyTuple>()?;
                let key: String = tuple.get_item(0)?.extract()?;
                let val: String = tuple.get_item(1)?.extract()?;
                pairs.push((key, val));
            }
            Ok(RichDisplay::KeyValue { pairs })
        }
        "table" => {
            let headers: Vec<String> = data.get_item("headers")?.extract()?;
            let rows_list = data.get_item("rows")?;
            let rows_pylist = rows_list.downcast::<PyList>()?;
            let mut rows = Vec::new();
            for row in rows_pylist.iter() {
                let row_vec: Vec<String> = row.extract()?;
                rows.push(row_vec);
            }
            Ok(RichDisplay::Table { headers, rows })
        }
        "tree" => {
            let root_obj = data.get_item("root")?;
            let root = py_to_tree_node(py, &root_obj)?;
            Ok(RichDisplay::Tree { root })
        }
        "color" => {
            let r: u8 = data.get_item("r")?.extract()?;
            let g: u8 = data.get_item("g")?.extract()?;
            let b: u8 = data.get_item("b")?.extract()?;
            let a: u8 = data.get_item("a")?.extract()?;
            Ok(RichDisplay::Color { r, g, b, a })
        }
        "code" => {
            let language: String = data.get_item("language")?.extract()?;
            let content: String = data.get_item("content")?.extract()?;
            Ok(RichDisplay::Code { language, content })
        }
        "map" => {
            let lat: f64 = data.get_item("lat")?.extract()?;
            let lon: f64 = data.get_item("lon")?.extract()?;
            let label: Option<String> = data.get_item("label")?.extract().ok();
            Ok(RichDisplay::Map { lat, lon, label })
        }
        "duration" => {
            let millis: u64 = data.get_item("millis")?.extract()?;
            let human: String = data.get_item("human")?.extract()?;
            Ok(RichDisplay::Duration { millis, human })
        }
        "datetime" => {
            let epoch_millis: i64 = data.get_item("epoch_millis")?.extract()?;
            let iso: String = data.get_item("iso")?.extract()?;
            let relative: String = data.get_item("relative")?.extract()?;
            Ok(RichDisplay::DateTime {
                epoch_millis,
                iso,
                relative,
            })
        }
        "data_size" => {
            let bytes: u64 = data.get_item("bytes")?.extract()?;
            let human: String = data.get_item("human")?.extract()?;
            Ok(RichDisplay::DataSize { bytes, human })
        }
        "markdown" => {
            let content: String = data.get_item("content")?.extract()?;
            Ok(RichDisplay::Markdown { content })
        }
        "progress" => {
            let value: f32 = data.get_item("value")?.extract()?;
            let label: Option<String> = data.get_item("label")?.extract().ok();
            Ok(RichDisplay::Progress {
                value: value.into(),
                label,
            })
        }
        _ => Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Unknown RichDisplay type: {}",
            type_name
        ))),
    }
}

/// Convert a Python TreeNode to Rust.
fn py_to_tree_node(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<TreeNode> {
    let label: String = obj.getattr("label")?.extract()?;
    let value_obj = obj.getattr("value")?;
    let value: Option<String> = if value_obj.is_none() {
        None
    } else {
        Some(value_obj.extract()?)
    };
    let children_obj = obj.getattr("children")?;
    let children_list = children_obj.downcast::<PyList>()?;
    let mut children = Vec::new();
    for child in children_list.iter() {
        children.push(py_to_tree_node(py, &child)?);
    }
    Ok(TreeNode {
        label,
        value,
        children,
    })
}

/// Add the @forb.visualizer decorator to the module.
pub fn add_visualizer_decorator(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    let decorator_code = c"
def visualizer(id, name, value_types=None):
    \"\"\"
    Decorator to register a visualizer plugin.

    A visualizer provides custom rich display for values.

    Usage:
        @forb.visualizer(id=\"json-tree\", name=\"JSON Tree View\", value_types=[\"json\"])
        def visualize_json(value) -> RichDisplay | None:
            if not isinstance(value, dict):
                return None
            return RichDisplay.Tree(
                TreeNode(\"root\", None, [
                    TreeNode(k, str(v)) for k, v in value.items()
                ])
            )

    Args:
        id: Unique identifier for this visualizer
        name: Human-readable name
        value_types: List of CoreValue types to handle (empty = all types)
    \"\"\"
    def decorator(func):
        _register_visualizer(
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
        Some(c"_register_visualizer"),
        None,
        |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let id: String = args.get_item(0)?.extract()?;
            let name: String = args.get_item(1)?.extract()?;
            let value_types: Vec<String> = args.get_item(2)?.extract()?;
            let func: PyObject = args.get_item(3)?.unbind();

            register_visualizer(VisualizerRegistration {
                id,
                name,
                value_types,
                func,
            });

            Ok(())
        },
    )?;

    module.setattr("_register_visualizer", register_fn)?;

    Ok(())
}
