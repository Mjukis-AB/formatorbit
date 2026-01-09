//! Python currency plugin implementation.

use super::register_currency;
use crate::plugin::{CurrencyPlugin, PluginMeta};
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyModule, PyTuple};

/// Registration info captured by the @forb.currency decorator.
pub struct CurrencyRegistration {
    pub code: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub func: PyObject,
}

/// A Python currency plugin.
pub struct PyCurrencyPlugin {
    code: String,
    symbol: String,
    name: String,
    decimals: u8,
    meta: PluginMeta,
    func: PyObject,
}

impl PyCurrencyPlugin {
    /// Create a new currency plugin from registration info.
    pub fn new(reg: CurrencyRegistration, meta: PluginMeta) -> Self {
        Self {
            code: reg.code,
            symbol: reg.symbol,
            name: reg.name,
            decimals: reg.decimals,
            meta,
            func: reg.func,
        }
    }
}

impl std::fmt::Debug for PyCurrencyPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyCurrencyPlugin")
            .field("code", &self.code)
            .field("name", &self.name)
            .finish()
    }
}

impl CurrencyPlugin for PyCurrencyPlugin {
    fn code(&self) -> &str {
        &self.code
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn decimals(&self) -> u8 {
        self.decimals
    }

    fn meta(&self) -> &PluginMeta {
        &self.meta
    }

    fn rate_to_usd(&self) -> Option<f64> {
        Python::with_gil(|py| {
            match self.func.call0(py) {
                Ok(result) => {
                    let bound_result = result.bind(py);
                    // None means rate unavailable
                    if bound_result.is_none() {
                        return None;
                    }
                    // Extract float rate
                    match bound_result.extract::<f64>() {
                        Ok(rate) => Some(rate),
                        Err(e) => {
                            tracing::warn!(
                                plugin = %self.code,
                                error = %e,
                                "Plugin returned non-float value"
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
                        plugin = %self.code,
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

/// Add the @forb.currency decorator to the module.
pub fn add_currency_decorator(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    let decorator_code = c"
def currency(code, symbol, name, decimals=2):
    \"\"\"
    Decorator to register a currency plugin.

    A currency plugin provides exchange rates to USD.

    Usage:
        @forb.currency(code=\"BTC\", symbol=\"\\u20bf\", name=\"Bitcoin\", decimals=8)
        def btc_rate() -> float | None:
            # Fetch current rate from API
            # Return None if unavailable
            return 42000.0

    Args:
        code: Currency code (e.g., \"BTC\", \"ETH\")
        symbol: Currency symbol (e.g., \"\\u20bf\" for Bitcoin)
        name: Full currency name (e.g., \"Bitcoin\")
        decimals: Number of decimal places (default: 2)
    \"\"\"
    def decorator(func):
        _register_currency(
            code,
            symbol,
            name,
            decimals,
            func
        )
        return func
    return decorator
";

    py.run(decorator_code, Some(&module.dict()), None)?;

    // Add the registration function
    let register_fn = PyCFunction::new_closure(
        py,
        Some(c"_register_currency"),
        None,
        |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let code: String = args.get_item(0)?.extract()?;
            let symbol: String = args.get_item(1)?.extract()?;
            let name: String = args.get_item(2)?.extract()?;
            let decimals: u8 = args.get_item(3)?.extract()?;
            let func: PyObject = args.get_item(4)?.unbind();

            register_currency(CurrencyRegistration {
                code,
                symbol,
                name,
                decimals,
                func,
            });

            Ok(())
        },
    )?;

    module.setattr("_register_currency", register_fn)?;

    Ok(())
}
