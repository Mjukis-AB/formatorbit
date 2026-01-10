//! Plugin system for extending Formatorbit with custom functionality.
//!
//! Plugins are Python files that extend Formatorbit with:
//! - **Decoders**: Parse custom input formats
//! - **Visualizers**: Custom rich display rendering
//! - **Currencies**: Custom currency exchange rates
//! - **Traits**: Value observations (like "is prime")
//! - **Expression Extensions**: Variables and functions for expressions
//!
//! # Plugin Location
//!
//! Plugins are loaded from `~/.config/forb/plugins/` by default.
//! Files ending in `.sample` are not loaded (rename to enable).
//!
//! # Plugin Format
//!
//! Each plugin is a single `.py` file with required metadata:
//!
//! ```python
//! __forb_plugin__ = {
//!     "name": "My Plugin",
//!     "version": "1.0.0",
//!     "author": "Your Name",      # optional
//!     "description": "What it does"  # optional
//! }
//! ```

pub mod bundled;
pub mod discovery;

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "python")]
pub mod python_discovery;

#[cfg(feature = "python")]
pub mod python_loader;

use crate::types::{CoreValue, Interpretation, RichDisplay};
#[cfg(feature = "python")]
use std::path::Path;
use std::path::PathBuf;

// Re-export Python runtime when feature is enabled
#[cfg(feature = "python")]
pub use python::PythonRuntime;

/// Errors that can occur during plugin operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Plugin file not found.
    #[error("plugin file not found: {0}")]
    FileNotFound(PathBuf),

    /// Plugin is missing required __forb_plugin__ metadata.
    #[error("plugin missing __forb_plugin__ metadata: {0}")]
    MissingMetadata(PathBuf),

    /// Plugin metadata is invalid.
    #[error("invalid plugin metadata in {path}: {message}")]
    InvalidMetadata { path: PathBuf, message: String },

    /// Python error during plugin execution.
    #[error("Python error in plugin '{plugin}': {message}")]
    PythonError {
        plugin: String,
        message: String,
        traceback: Option<String>,
    },

    /// Plugin returned an invalid value.
    #[error("plugin '{plugin}' returned invalid value: {message}")]
    InvalidReturnValue { plugin: String, message: String },

    /// Duplicate plugin ID.
    #[error("duplicate plugin ID: {0}")]
    DuplicateId(String),

    /// Python runtime initialization failed.
    #[error("Python runtime initialization failed: {0}")]
    RuntimeInit(String),

    /// I/O error reading plugin file.
    #[error("I/O error reading plugin: {0}")]
    Io(#[from] std::io::Error),

    /// Plugin feature not enabled.
    #[error("plugin feature not enabled (compile with --features python)")]
    FeatureNotEnabled,
}

/// Metadata about a loaded plugin.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    /// Plugin display name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Plugin author (optional).
    pub author: Option<String>,
    /// Plugin description (optional).
    pub description: Option<String>,
    /// Source file path.
    pub source_file: PathBuf,
}

/// Trait for decoder plugins that parse custom input formats.
pub trait DecoderPlugin: Send + Sync {
    /// Unique identifier for this decoder (e.g., "myformat").
    fn id(&self) -> &str;

    /// Human-readable name (e.g., "My Custom Format").
    fn name(&self) -> &str;

    /// Plugin metadata.
    fn meta(&self) -> &PluginMeta;

    /// Aliases for this decoder (alternative names).
    fn aliases(&self) -> &[String] {
        &[]
    }

    /// Parse input and return interpretations.
    ///
    /// Returns an empty Vec if input doesn't match this format.
    fn parse(&self, input: &str) -> Vec<Interpretation>;
}

/// Trait for visualizer plugins that provide custom rich display.
pub trait VisualizerPlugin: Send + Sync {
    /// Unique identifier for this visualizer.
    fn id(&self) -> &str;

    /// Human-readable name.
    fn name(&self) -> &str;

    /// Plugin metadata.
    fn meta(&self) -> &PluginMeta;

    /// Value types this visualizer handles (e.g., ["json", "bytes"]).
    /// Empty means it handles all types.
    fn value_types(&self) -> &[String];

    /// Generate a rich display for the value.
    ///
    /// Returns None if this visualizer doesn't apply to the value.
    fn visualize(&self, value: &CoreValue) -> Option<RichDisplay>;
}

/// Trait for currency plugins that provide exchange rates.
pub trait CurrencyPlugin: Send + Sync {
    /// Currency code (e.g., "BTC", "ETH").
    fn code(&self) -> &str;

    /// Currency symbol (e.g., "₿", "Ξ").
    fn symbol(&self) -> &str;

    /// Currency name (e.g., "Bitcoin", "Ethereum").
    fn name(&self) -> &str;

    /// Number of decimal places.
    fn decimals(&self) -> u8;

    /// Plugin metadata.
    fn meta(&self) -> &PluginMeta;

    /// Get current exchange rate.
    ///
    /// Returns `(rate, base_currency)` where `rate` is how much 1 unit of this
    /// currency is worth in the base currency.
    ///
    /// For example, for BTC returning `(42000.0, "USD")` means 1 BTC = 42000 USD.
    ///
    /// Returns None if rate is unavailable.
    fn rate(&self) -> Option<(f64, String)>;
}

/// Trait for trait plugins that observe properties of values.
pub trait TraitPlugin: Send + Sync {
    /// Unique identifier for this trait (e.g., "is-lucky").
    fn id(&self) -> &str;

    /// Human-readable name (e.g., "Lucky Number").
    fn name(&self) -> &str;

    /// Plugin metadata.
    fn meta(&self) -> &PluginMeta;

    /// Value types this trait checks (e.g., ["int"]).
    /// Empty means it checks all types.
    fn value_types(&self) -> &[String];

    /// Check if the trait applies to the value.
    ///
    /// Returns the trait description (e.g., "Lucky number (42)") or None.
    fn check(&self, value: &CoreValue) -> Option<String>;
}

/// An expression variable provided by a plugin.
pub struct ExprVarPlugin {
    /// Variable name (e.g., "PI").
    pub name: String,
    /// Description for help.
    pub description: String,
    /// Plugin metadata.
    pub meta: PluginMeta,
    /// Function to get the value.
    #[cfg(feature = "python")]
    pub(crate) func: pyo3::PyObject,
    #[cfg(not(feature = "python"))]
    pub(crate) _phantom: std::marker::PhantomData<()>,
}

impl std::fmt::Debug for ExprVarPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprVarPlugin")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

#[cfg(not(feature = "python"))]
impl Clone for ExprVarPlugin {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            meta: self.meta.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

/// An expression function provided by a plugin.
pub struct ExprFuncPlugin {
    /// Function name (e.g., "factorial").
    pub name: String,
    /// Description for help.
    pub description: String,
    /// Plugin metadata.
    pub meta: PluginMeta,
    /// Python function object.
    #[cfg(feature = "python")]
    pub(crate) func: pyo3::PyObject,
    #[cfg(not(feature = "python"))]
    pub(crate) _phantom: std::marker::PhantomData<()>,
}

impl std::fmt::Debug for ExprFuncPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprFuncPlugin")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

#[cfg(not(feature = "python"))]
impl Clone for ExprFuncPlugin {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            meta: self.meta.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

/// A loaded plugin (any type).
pub enum Plugin {
    Decoder(Box<dyn DecoderPlugin>),
    Visualizer(Box<dyn VisualizerPlugin>),
    Currency(Box<dyn CurrencyPlugin>),
    Trait(Box<dyn TraitPlugin>),
    ExprVar(ExprVarPlugin),
    ExprFunc(ExprFuncPlugin),
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decoder(d) => write!(f, "Plugin::Decoder({})", d.id()),
            Self::Visualizer(v) => write!(f, "Plugin::Visualizer({})", v.id()),
            Self::Currency(c) => write!(f, "Plugin::Currency({})", c.code()),
            Self::Trait(t) => write!(f, "Plugin::Trait({})", t.id()),
            Self::ExprVar(v) => write!(f, "Plugin::ExprVar({})", v.name),
            Self::ExprFunc(func) => write!(f, "Plugin::ExprFunc({})", func.name),
        }
    }
}

/// Information about a loaded plugin item.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin identifier or name.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description (if provided).
    pub description: Option<String>,
    /// Source file path.
    pub source_file: PathBuf,
    /// Plugin file metadata (name, version, author).
    pub plugin_meta: PluginMeta,
}

/// Report of plugin loading results.
#[derive(Debug, Default)]
pub struct PluginLoadReport {
    /// Successfully loaded decoders (id only, for backwards compat).
    pub decoders: Vec<String>,
    /// Successfully loaded visualizers.
    pub visualizers: Vec<String>,
    /// Successfully loaded currencies.
    pub currencies: Vec<String>,
    /// Successfully loaded traits.
    pub traits: Vec<String>,
    /// Successfully loaded expression variables.
    pub expr_vars: Vec<String>,
    /// Successfully loaded expression functions.
    pub expr_funcs: Vec<String>,
    /// Detailed info about loaded plugins.
    pub plugins: Vec<PluginInfo>,
    /// Errors encountered during loading.
    pub errors: Vec<(PathBuf, PluginError)>,
}

impl PluginLoadReport {
    /// Total number of successfully loaded plugins.
    pub fn total_loaded(&self) -> usize {
        self.decoders.len()
            + self.visualizers.len()
            + self.currencies.len()
            + self.traits.len()
            + self.expr_vars.len()
            + self.expr_funcs.len()
    }

    /// Check if any plugins were loaded.
    pub fn has_plugins(&self) -> bool {
        self.total_loaded() > 0
    }

    /// Check if there were any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Registry of loaded plugins.
#[derive(Default)]
pub struct PluginRegistry {
    decoders: Vec<Box<dyn DecoderPlugin>>,
    visualizers: Vec<Box<dyn VisualizerPlugin>>,
    currencies: Vec<Box<dyn CurrencyPlugin>>,
    traits: Vec<Box<dyn TraitPlugin>>,
    expr_vars: Vec<ExprVarPlugin>,
    expr_funcs: Vec<ExprFuncPlugin>,
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("decoders", &self.decoders.len())
            .field("visualizers", &self.visualizers.len())
            .field("currencies", &self.currencies.len())
            .field("traits", &self.traits.len())
            .field("expr_vars", &self.expr_vars.len())
            .field("expr_funcs", &self.expr_funcs.len())
            .finish()
    }
}

impl PluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load plugins from the default directories.
    ///
    /// This first installs bundled plugins (if not already present), then
    /// loads all plugins from:
    /// 1. User's config directory (~/.config/forb/plugins/)
    /// 2. Bundled plugins directory (~/.local/share/forb/plugins/)
    #[cfg(feature = "python")]
    pub fn load_default(&mut self) -> Result<PluginLoadReport, PluginError> {
        // Install bundled plugins if not present
        if let Err(e) = bundled::install_bundled_plugins() {
            tracing::debug!(error = %e, "Could not install bundled plugins");
        }

        let plugin_dirs = discovery::discover_plugin_dirs();
        let mut report = PluginLoadReport::default();

        for dir in plugin_dirs {
            match self.load_from_dir(&dir, &mut report) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!(dir = %dir.display(), error = %e, "Failed to load plugins from directory");
                }
            }
        }

        Ok(report)
    }

    /// Load plugins from the default directory (~/.config/forb/plugins/).
    #[cfg(not(feature = "python"))]
    pub fn load_default(&mut self) -> Result<PluginLoadReport, PluginError> {
        Err(PluginError::FeatureNotEnabled)
    }

    /// Load plugins from a specific directory.
    #[cfg(feature = "python")]
    pub fn load_from_dir(
        &mut self,
        dir: &std::path::Path,
        report: &mut PluginLoadReport,
    ) -> Result<(), PluginError> {
        let plugin_files = discovery::find_plugin_files(dir);

        let runtime = PythonRuntime::init()?;

        for path in plugin_files {
            match runtime.load_plugin(&path) {
                Ok(plugins) => {
                    for plugin in plugins {
                        self.register_plugin(plugin, &path, report);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to load plugin"
                    );
                    report.errors.push((path, e));
                }
            }
        }

        Ok(())
    }

    /// Register a plugin in the registry.
    #[cfg(feature = "python")]
    fn register_plugin(
        &mut self,
        plugin: Plugin,
        source_file: &Path,
        report: &mut PluginLoadReport,
    ) {
        match plugin {
            Plugin::Decoder(d) => {
                report.plugins.push(PluginInfo {
                    id: d.id().to_string(),
                    name: d.name().to_string(),
                    description: d.meta().description.clone().filter(|s| !s.is_empty()),
                    source_file: source_file.to_path_buf(),
                    plugin_meta: d.meta().clone(),
                });
                report.decoders.push(d.id().to_string());
                self.decoders.push(d);
            }
            Plugin::Visualizer(v) => {
                report.plugins.push(PluginInfo {
                    id: v.id().to_string(),
                    name: v.name().to_string(),
                    description: v.meta().description.clone().filter(|s| !s.is_empty()),
                    source_file: source_file.to_path_buf(),
                    plugin_meta: v.meta().clone(),
                });
                report.visualizers.push(v.id().to_string());
                self.visualizers.push(v);
            }
            Plugin::Currency(c) => {
                report.plugins.push(PluginInfo {
                    id: c.code().to_string(),
                    name: c.name().to_string(),
                    description: c.meta().description.clone().filter(|s| !s.is_empty()),
                    source_file: source_file.to_path_buf(),
                    plugin_meta: c.meta().clone(),
                });
                report.currencies.push(c.code().to_string());
                self.currencies.push(c);
            }
            Plugin::Trait(t) => {
                report.plugins.push(PluginInfo {
                    id: t.id().to_string(),
                    name: t.name().to_string(),
                    description: t.meta().description.clone().filter(|s| !s.is_empty()),
                    source_file: source_file.to_path_buf(),
                    plugin_meta: t.meta().clone(),
                });
                report.traits.push(t.id().to_string());
                self.traits.push(t);
            }
            Plugin::ExprVar(v) => {
                report.plugins.push(PluginInfo {
                    id: v.name.clone(),
                    name: v.name.clone(),
                    description: if v.description.is_empty() {
                        None
                    } else {
                        Some(v.description.clone())
                    },
                    source_file: source_file.to_path_buf(),
                    plugin_meta: v.meta.clone(),
                });
                report.expr_vars.push(v.name.clone());
                self.expr_vars.push(v);
            }
            Plugin::ExprFunc(f) => {
                report.plugins.push(PluginInfo {
                    id: f.name.clone(),
                    name: format!("{}()", f.name),
                    description: if f.description.is_empty() {
                        None
                    } else {
                        Some(f.description.clone())
                    },
                    source_file: source_file.to_path_buf(),
                    plugin_meta: f.meta.clone(),
                });
                report.expr_funcs.push(f.name.clone());
                self.expr_funcs.push(f);
            }
        }
    }

    /// Get all decoder plugins.
    pub fn decoders(&self) -> &[Box<dyn DecoderPlugin>] {
        &self.decoders
    }

    /// Get all visualizer plugins.
    pub fn visualizers(&self) -> &[Box<dyn VisualizerPlugin>] {
        &self.visualizers
    }

    /// Get all currency plugins.
    pub fn currencies(&self) -> &[Box<dyn CurrencyPlugin>] {
        &self.currencies
    }

    /// Get all trait plugins.
    pub fn traits(&self) -> &[Box<dyn TraitPlugin>] {
        &self.traits
    }

    /// Get all expression variable plugins.
    pub fn expr_vars(&self) -> &[ExprVarPlugin] {
        &self.expr_vars
    }

    /// Get all expression function plugins.
    pub fn expr_funcs(&self) -> &[ExprFuncPlugin] {
        &self.expr_funcs
    }

    /// Check if the registry has any plugins.
    pub fn is_empty(&self) -> bool {
        self.decoders.is_empty()
            && self.visualizers.is_empty()
            && self.currencies.is_empty()
            && self.traits.is_empty()
            && self.expr_vars.is_empty()
            && self.expr_funcs.is_empty()
    }

    /// Get the total number of loaded plugins.
    pub fn len(&self) -> usize {
        self.decoders.len()
            + self.visualizers.len()
            + self.currencies.len()
            + self.traits.len()
            + self.expr_vars.len()
            + self.expr_funcs.len()
    }

    /// Build an evalexpr context with plugin variables and functions.
    #[cfg(feature = "python")]
    pub fn build_expr_context(&self) -> evalexpr::HashMapContext<evalexpr::DefaultNumericTypes> {
        use evalexpr::{ContextWithMutableFunctions, ContextWithMutableVariables};

        let mut context = evalexpr::HashMapContext::new();

        // Add plugin variables
        for var in &self.expr_vars {
            if let Ok(value) = var.get_value() {
                let _ = context.set_value(var.name.clone(), value);
            }
        }

        // Add plugin functions
        for func in &self.expr_funcs {
            let _ = context.set_function(func.name.clone(), func.as_evalexpr_fn());
        }

        context
    }
}
