//! Global expression evaluation context.
//!
//! This module provides a global context for evalexpr that can be populated
//! with plugin-provided variables and functions.

use std::sync::RwLock;

/// Global expression context for evalexpr.
///
/// This is populated when plugins are loaded and used by ExprFormat::parse().
static EXPR_CONTEXT: RwLock<Option<ExprContextData>> = RwLock::new(None);

/// Data stored in the global expression context.
struct ExprContextData {
    /// Variable names and their values.
    variables: Vec<(String, evalexpr::Value)>,
    /// Function names and their implementations.
    #[cfg(feature = "python")]
    functions: Vec<(String, crate::plugin::ExprFuncPlugin)>,
}

/// Set the global expression context from a plugin registry.
#[cfg(feature = "python")]
pub fn set_from_registry(registry: &crate::plugin::PluginRegistry) {
    let mut variables = Vec::new();
    let mut functions = Vec::new();

    // Collect variables
    for var in registry.expr_vars() {
        if let Ok(value) = var.get_value() {
            variables.push((var.name.clone(), value));
        }
    }

    // Collect functions (clone the plugins)
    for func in registry.expr_funcs() {
        functions.push((func.name.clone(), func.clone()));
    }

    let data = ExprContextData {
        variables,
        functions,
    };

    if let Ok(mut guard) = EXPR_CONTEXT.write() {
        *guard = Some(data);
    }
}

/// Clear the global expression context.
pub fn clear() {
    if let Ok(mut guard) = EXPR_CONTEXT.write() {
        *guard = None;
    }
}

/// Evaluate an expression using the global context (if available).
///
/// Falls back to evalexpr::eval() if no context is set.
pub fn eval(expr: &str) -> Result<evalexpr::Value, evalexpr::EvalexprError> {
    let guard = match EXPR_CONTEXT.read() {
        Ok(g) => g,
        Err(_) => return evalexpr::eval(expr),
    };

    match &*guard {
        Some(data) => {
            #[cfg(feature = "python")]
            use evalexpr::ContextWithMutableFunctions;
            use evalexpr::ContextWithMutableVariables;

            let mut context = evalexpr::HashMapContext::new();

            // Add variables
            for (name, value) in &data.variables {
                let _ = context.set_value(name.clone(), value.clone());
            }

            // Add functions
            #[cfg(feature = "python")]
            for (name, func) in &data.functions {
                let _ = context.set_function(name.clone(), func.as_evalexpr_fn());
            }

            evalexpr::eval_with_context(expr, &context)
        }
        None => evalexpr::eval(expr),
    }
}

/// Check if the global context has any variables or functions.
pub fn has_context() -> bool {
    match EXPR_CONTEXT.read() {
        Ok(guard) => guard.is_some(),
        Err(_) => false,
    }
}
