//! Expression variable and function plugins.

use super::{register_expr_func, register_expr_var};
use crate::plugin::{ExprFuncPlugin, ExprVarPlugin, PluginMeta};
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyModule, PyTuple};

/// Registration info for an expression variable.
pub struct ExprVarRegistration {
    pub name: String,
    pub description: String,
    pub func: PyObject,
}

/// Registration info for an expression function.
pub struct ExprFuncRegistration {
    pub name: String,
    pub description: String,
    pub func: PyObject,
}

/// Create an ExprVarPlugin from registration info.
pub fn create_expr_var_plugin(reg: ExprVarRegistration, meta: PluginMeta) -> ExprVarPlugin {
    ExprVarPlugin {
        name: reg.name,
        description: reg.description,
        meta,
        func: reg.func,
    }
}

/// Create an ExprFuncPlugin from registration info.
pub fn create_expr_func_plugin(reg: ExprFuncRegistration, meta: PluginMeta) -> ExprFuncPlugin {
    ExprFuncPlugin {
        name: reg.name,
        description: reg.description,
        meta,
        func: reg.func,
    }
}

impl ExprVarPlugin {
    /// Get the current value of this variable.
    pub fn get_value(&self) -> Result<evalexpr::Value, String> {
        Python::with_gil(|py| {
            match self.func.call0(py) {
                Ok(result) => {
                    let bound_result = result.bind(py);
                    // Try to extract as number
                    if let Ok(i) = bound_result.extract::<i64>() {
                        return Ok(evalexpr::Value::Int(i));
                    }
                    if let Ok(f) = bound_result.extract::<f64>() {
                        return Ok(evalexpr::Value::Float(f));
                    }
                    Err(format!(
                        "Variable '{}' returned non-numeric value",
                        self.name
                    ))
                }
                Err(e) => Err(format!("Variable '{}' raised exception: {}", self.name, e)),
            }
        })
    }
}

impl Clone for ExprVarPlugin {
    fn clone(&self) -> Self {
        Python::with_gil(|py| Self {
            name: self.name.clone(),
            description: self.description.clone(),
            meta: self.meta.clone(),
            func: self.func.clone_ref(py),
        })
    }
}

impl ExprFuncPlugin {
    /// Call this function with evalexpr arguments.
    pub fn call(&self, args: &evalexpr::Value) -> Result<evalexpr::Value, evalexpr::EvalexprError> {
        Python::with_gil(|py| {
            // Convert evalexpr Value to Python args
            let py_args = match args {
                evalexpr::Value::Tuple(tuple) => {
                    let items: Vec<PyObject> = tuple
                        .iter()
                        .map(|v| evalexpr_to_py(py, v))
                        .collect::<Result<_, _>>()
                        .map_err(|e| evalexpr::EvalexprError::CustomMessage(e))?;
                    PyTuple::new(py, items)
                        .map_err(|e| evalexpr::EvalexprError::CustomMessage(e.to_string()))?
                }
                single => {
                    let item = evalexpr_to_py(py, single)
                        .map_err(|e| evalexpr::EvalexprError::CustomMessage(e))?;
                    PyTuple::new(py, [item])
                        .map_err(|e| evalexpr::EvalexprError::CustomMessage(e.to_string()))?
                }
            };

            // Call the Python function
            match self.func.call1(py, py_args) {
                Ok(result) => {
                    let bound_result = result.bind(py);
                    py_to_evalexpr(bound_result).map_err(|e| {
                        evalexpr::EvalexprError::CustomMessage(format!(
                            "Function '{}' returned invalid value: {}",
                            self.name, e
                        ))
                    })
                }
                Err(e) => {
                    let msg = format!("Function '{}' raised exception: {}", self.name, e);
                    Err(evalexpr::EvalexprError::CustomMessage(msg))
                }
            }
        })
    }

    /// Convert this plugin to an evalexpr Function with default numeric types.
    pub fn as_evalexpr_fn(&self) -> evalexpr::Function<evalexpr::DefaultNumericTypes> {
        let plugin = self.clone();
        evalexpr::Function::new(move |args| plugin.call(args))
    }
}

impl Clone for ExprFuncPlugin {
    fn clone(&self) -> Self {
        Python::with_gil(|py| Self {
            name: self.name.clone(),
            description: self.description.clone(),
            meta: self.meta.clone(),
            func: self.func.clone_ref(py),
        })
    }
}

/// Convert evalexpr Value to Python object.
fn evalexpr_to_py(py: Python<'_>, value: &evalexpr::Value) -> Result<PyObject, String> {
    match value {
        evalexpr::Value::Int(i) => Ok(i
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .into_any()
            .unbind()),
        evalexpr::Value::Float(f) => Ok(f
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .into_any()
            .unbind()),
        evalexpr::Value::Boolean(b) => Ok((*b)
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .to_owned()
            .into_any()
            .unbind()),
        evalexpr::Value::String(s) => Ok(s
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .into_any()
            .unbind()),
        evalexpr::Value::Tuple(items) => {
            let py_items: Vec<PyObject> = items
                .iter()
                .map(|v| evalexpr_to_py(py, v))
                .collect::<Result<_, _>>()?;
            Ok(PyTuple::new(py, py_items)
                .map_err(|e| e.to_string())?
                .into_any()
                .unbind())
        }
        evalexpr::Value::Empty => Ok(py.None()),
    }
}

/// Convert Python object to evalexpr Value.
fn py_to_evalexpr(obj: &Bound<'_, PyAny>) -> Result<evalexpr::Value, String> {
    if obj.is_none() {
        return Ok(evalexpr::Value::Empty);
    }
    // Check bool before int (bool is subclass of int in Python)
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(evalexpr::Value::Boolean(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(evalexpr::Value::Int(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(evalexpr::Value::Float(f));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(evalexpr::Value::String(s));
    }

    let type_name = obj
        .get_type()
        .name()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    Err(format!(
        "Cannot convert Python {} to evalexpr Value",
        type_name
    ))
}

/// Add expression decorators to the forb module.
pub fn add_expr_decorators(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Add @forb.expr_var decorator
    let var_code = c"
def expr_var(name, description=\"\"):
    \"\"\"
    Decorator to register an expression variable.

    Usage:
        @forb.expr_var(\"PI\", description=\"Mathematical constant pi\")
        def pi():
            return 3.14159265359

    The function should take no arguments and return a number (int or float).
    \"\"\"
    def decorator(func):
        _register_expr_var(name, description, func)
        return func
    return decorator
";

    py.run(var_code, Some(&module.dict()), None)?;

    // Add @forb.expr_func decorator
    let func_code = c"
def expr_func(name, description=\"\"):
    \"\"\"
    Decorator to register an expression function.

    Usage:
        @forb.expr_func(\"factorial\", description=\"Calculate n!\")
        def factorial(n):
            import math
            return math.factorial(int(n))

    The function can take any number of numeric arguments and should return a number.
    \"\"\"
    def decorator(func):
        _register_expr_func(name, description, func)
        return func
    return decorator
";

    py.run(func_code, Some(&module.dict()), None)?;

    // Add registration functions
    let var_register = PyCFunction::new_closure(
        py,
        Some(c"_register_expr_var"),
        None,
        |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let name: String = args.get_item(0)?.extract()?;
            let description: String = args.get_item(1)?.extract()?;
            let func: PyObject = args.get_item(2)?.unbind();

            register_expr_var(ExprVarRegistration {
                name,
                description,
                func,
            });

            Ok(())
        },
    )?;
    module.setattr("_register_expr_var", var_register)?;

    let func_register = PyCFunction::new_closure(
        py,
        Some(c"_register_expr_func"),
        None,
        |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let name: String = args.get_item(0)?.extract()?;
            let description: String = args.get_item(1)?.extract()?;
            let func: PyObject = args.get_item(2)?.unbind();

            register_expr_func(ExprFuncRegistration {
                name,
                description,
                func,
            });

            Ok(())
        },
    )?;
    module.setattr("_register_expr_func", func_register)?;

    Ok(())
}
