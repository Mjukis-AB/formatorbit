//! Global expression evaluation context.
//!
//! This module provides a global context for evalexpr that can be populated
//! with plugin-provided variables and functions, as well as built-in currency
//! conversion functions.

use std::sync::RwLock;

use crate::formats::currency_expr;

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
    #[cfg(feature = "python")]
    use evalexpr::ContextWithMutableFunctions;
    use evalexpr::ContextWithMutableVariables;

    let mut context = evalexpr::HashMapContext::new();

    // 1. Add built-in currency functions FIRST (so plugins can override)
    add_currency_functions(&mut context);

    // 2. Add plugin context if available
    if let Ok(guard) = EXPR_CONTEXT.read() {
        if let Some(data) = &*guard {
            // Add plugin variables
            for (name, value) in &data.variables {
                let _ = context.set_value(name.clone(), value.clone());
            }

            // Add plugin functions (these override currency functions if same name)
            #[cfg(feature = "python")]
            for (name, func) in &data.functions {
                let _ = context.set_function(name.clone(), func.as_evalexpr_fn());
            }
        }
    }

    evalexpr::eval_with_context(expr, &context)
}

/// Add currency conversion functions to the context.
///
/// Registers functions like USD(amount), EUR(amount), BTC(amount) that convert
/// to the target currency, plus toXXX/inXXX functions for explicit target conversion.
fn add_currency_functions(context: &mut evalexpr::HashMapContext) {
    use evalexpr::ContextWithMutableFunctions;

    for code in currency_expr::all_currency_codes() {
        // Register uppercase version (USD, EUR, BTC) - converts to user's target currency
        let code_upper = code.to_uppercase();
        let code_for_upper = code_upper.clone();
        let _ = context.set_function(
            code_upper,
            evalexpr::Function::new(move |args| {
                let amount = args.as_number()?;
                match currency_expr::convert_to_target(amount, &code_for_upper) {
                    Some(result) => Ok(evalexpr::Value::Float(result)),
                    None => Err(evalexpr::EvalexprError::CustomMessage(format!(
                        "Cannot convert {} to target currency (no exchange rates available)",
                        code_for_upper
                    ))),
                }
            }),
        );

        // Register lowercase version (usd, eur, btc)
        let code_lower = code.to_lowercase();
        let code_for_lower = code.to_uppercase(); // Still use uppercase for conversion
        let _ = context.set_function(
            code_lower,
            evalexpr::Function::new(move |args| {
                let amount = args.as_number()?;
                match currency_expr::convert_to_target(amount, &code_for_lower) {
                    Some(result) => Ok(evalexpr::Value::Float(result)),
                    None => Err(evalexpr::EvalexprError::CustomMessage(format!(
                        "Cannot convert {} to target currency (no exchange rates available)",
                        code_for_lower
                    ))),
                }
            }),
        );

        // Register toXXX and inXXX functions for explicit target conversion
        // These convert FROM the user's target currency TO the specified currency
        add_explicit_conversion_function(context, &code, "to");
        add_explicit_conversion_function(context, &code, "in");
    }
}

/// Add an explicit conversion function (toXXX or inXXX) for a currency.
///
/// These functions convert from the user's target currency to the specified currency.
/// Example: toEUR(USD(100)) - USD(100) returns amount in user's currency, then toEUR converts to EUR.
fn add_explicit_conversion_function(
    context: &mut evalexpr::HashMapContext,
    currency_code: &str,
    prefix: &str,
) {
    use evalexpr::ContextWithMutableFunctions;

    let target = currency_code.to_uppercase();

    // Uppercase: toEUR, inEUR
    let func_upper = format!("{}{}", prefix, target);
    let target_for_upper = target.clone();
    let _ = context.set_function(
        func_upper,
        evalexpr::Function::new(move |args| {
            let amount = args.as_number()?;
            match currency_expr::convert_from_target(amount, &target_for_upper) {
                Some(result) => Ok(evalexpr::Value::Float(result)),
                None => Err(evalexpr::EvalexprError::CustomMessage(format!(
                    "Cannot convert to {} (no exchange rates available)",
                    target_for_upper
                ))),
            }
        }),
    );

    // Lowercase: toeur, ineur
    let func_lower = format!("{}{}", prefix, currency_code.to_lowercase());
    let target_for_lower = target.clone();
    let _ = context.set_function(
        func_lower,
        evalexpr::Function::new(move |args| {
            let amount = args.as_number()?;
            match currency_expr::convert_from_target(amount, &target_for_lower) {
                Some(result) => Ok(evalexpr::Value::Float(result)),
                None => Err(evalexpr::EvalexprError::CustomMessage(format!(
                    "Cannot convert to {} (no exchange rates available)",
                    target_for_lower
                ))),
            }
        }),
    );
}

/// Check if the global context has any variables or functions.
pub fn has_context() -> bool {
    match EXPR_CONTEXT.read() {
        Ok(guard) => guard.is_some(),
        Err(_) => false,
    }
}
