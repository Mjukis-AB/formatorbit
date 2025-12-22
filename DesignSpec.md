~# Formatorbit – Projektspecifikation

## Översikt

**Formatorbit** är ett cross-platform verktyg för att konvertera data mellan olika format. Användaren matar in data (t.ex. `691E01B8`) och verktyget visar alla möjliga tolkningar och konverteringar automatiskt.

### Exempel
Input: `691E01B8`
→ Tolkas som hex → bytes `[0x69, 0x1E, 0x01, 0xB8]`
→ Som int (big-endian): `1763574200`
→ Som int (little-endian): `3087818345`  
→ Som epoch → `2025-11-19T17:43:20Z`
→ Som Base64: `aR4BuA==`

---

## Arkitektur

```
┌─────────────────────────────────────────────────────────┐
│                      Plattformar                        │
├─────────────────┬─────────────────┬─────────────────────┤
│  macOS (SwiftUI)│  Linux/Windows  │        CLI          │
│  + Menu bar     │  (Tauri/egui)   │    (universal)      │
│  + Services     │                 │                     │
│  + Global hotkey│                 │                     │
└────────┬────────┴────────┬────────┴──────────┬──────────┘
         │                 │                   │
         ▼                 ▼                   ▼
┌─────────────────────────────────────────────────────────┐
│                    FFI Layer (C ABI)                    │
│                    crates/ffi                           │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                     Rust Core                           │
│                    crates/core                          │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │              Plugin Registry                     │   │
│  │   ┌─────────┐ ┌─────────┐ ┌─────────┐          │   │
│  │   │ Native  │ │  Dylib  │ │ Script  │          │   │
│  │   │ (Rust)  │ │ (C ABI) │ │(Python) │          │   │
│  │   └─────────┘ └─────────┘ └─────────┘          │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐           │
│  │  parse    │  │  convert  │  │  format   │           │
│  │           │  │  (graph)  │  │           │           │
│  └───────────┘  └───────────┘  └───────────┘           │
└─────────────────────────────────────────────────────────┘
```

---

## Projektstruktur

```
formatorbit/
├── Cargo.toml                 # Workspace
├── README.md
│
├── crates/
│   ├── core/                  # Huvudlogik
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs       # CoreValue, Interpretation, Conversion
│   │       ├── parse.rs       # String → Vec<Interpretation>
│   │       ├── convert.rs     # Konverteringsgraf, BFS
│   │       ├── format.rs      # CoreValue → display strings
│   │       ├── registry.rs    # Plugin registry
│   │       ├── plugin/
│   │       │   ├── mod.rs
│   │       │   ├── native.rs  # Rust-native plugins
│   │       │   ├── dylib.rs   # Dynamic loading (C ABI)
│   │       │   └── python.rs  # Python plugins (optional feature)
│   │       └── formats/       # Built-in formats
│   │           ├── mod.rs
│   │           ├── hex.rs
│   │           ├── base64.rs
│   │           ├── integers.rs
│   │           ├── datetime.rs
│   │           ├── json.rs
│   │           └── utf8.rs
│   │
│   ├── plugin-api/            # Stable C ABI for plugins
│   │   ├── Cargo.toml
│   │   ├── include/
│   │   │   └── formatorbit_plugin.h
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── ffi/                   # C-bindings för app-integration
│   │   ├── Cargo.toml
│   │   ├── cbindgen.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── cli/                   # Kommandoradsverktyg
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
│
├── plugins/                   # Exempel-plugins
│   ├── uuid/                  # Rust plugin exempel
│   ├── color/                 # Rust plugin exempel  
│   └── examples/
│       ├── swift/             # Swift plugin exempel
│       └── python/            # Python plugin exempel
│
└── apps/
    ├── macos/                 # SwiftUI-app
    └── desktop/               # Tauri för Linux/Windows
```

---

## Plugin-arkitektur

### Designprinciper

1. **C ABI som lingua franca** - alla externa plugins (oavsett språk) kommunicerar via stabil C ABI
2. **Rust-native fast path** - inbyggda format och Rust-plugins använder trait objects direkt (ingen FFI overhead)
3. **Lazy loading** - dylibs laddas on-demand, inte vid startup
4. **Sandboxing-ready** - plugin API:et designat för framtida sandboxing

### Plugin-typer

| Typ | Språk | Overhead | Use case |
|-----|-------|----------|----------|
| Native | Rust | Ingen | Built-in formats, performance-kritiska |
| Dylib | Rust/Swift/C/C++ | Minimal (FFI call) | Third-party, plattformsspecifika |
| Script | Python | Högre (interpreter) | Prototyping, enkla format |

### C ABI Definition (plugin-api)

```c
// crates/plugin-api/include/formatorbit_plugin.h

#ifndef FORMATORBIT_PLUGIN_H
#define FORMATORBIT_PLUGIN_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// Version & Metadata
// ============================================================================

#define FORMATORBIT_PLUGIN_API_VERSION 1

typedef struct {
    uint32_t api_version;
    const char* plugin_id;        // "com.example.uuid"
    const char* plugin_name;      // "UUID Format"
    const char* plugin_version;   // "1.0.0"
    const char* author;           // Optional
} FormatOrbitPluginInfo;

// ============================================================================
// Core Types (C representation)
// ============================================================================

typedef enum {
    FO_TYPE_BYTES = 0,
    FO_TYPE_STRING = 1,
    FO_TYPE_INT = 2,
    FO_TYPE_FLOAT = 3,
    FO_TYPE_BOOL = 4,
    FO_TYPE_DATETIME = 5,
    FO_TYPE_JSON = 6,
} FormatOrbitType;

typedef struct {
    FormatOrbitType type;
    union {
        struct { const uint8_t* data; size_t len; } bytes;
        const char* string;
        int64_t int_val;          // Note: C ABI uses i64, not i128
        double float_val;
        bool bool_val;
        int64_t datetime_unix;    // Unix timestamp
        const char* json_string;  // JSON as string
    } value;
} FormatOrbitValue;

typedef struct {
    FormatOrbitValue value;
    const char* source_format;
    float confidence;
    const char* description;
} FormatOrbitInterpretation;

typedef struct {
    FormatOrbitValue value;
    const char* target_format;
    const char* display;
    bool is_lossy;
} FormatOrbitConversion;

// ============================================================================
// Result types (caller-allocated buffers)
// ============================================================================

typedef struct {
    FormatOrbitInterpretation* items;
    size_t count;
    size_t capacity;
} FormatOrbitInterpretationList;

typedef struct {
    FormatOrbitConversion* items;
    size_t count;
    size_t capacity;
} FormatOrbitConversionList;

// ============================================================================
// Plugin Interface (vtable style)
// ============================================================================

typedef struct {
    // Required: Get plugin metadata
    FormatOrbitPluginInfo (*get_info)(void);
    
    // Required: List format IDs this plugin provides
    // Returns semicolon-separated list: "uuid;uuid-v4;guid"
    const char* (*get_format_ids)(void);
    
    // Required: Try to parse input string
    // Returns number of interpretations written to `out`
    // Returns 0 if cannot parse
    size_t (*parse)(
        const char* input,
        FormatOrbitInterpretationList* out
    );
    
    // Required: Check if plugin can format this value type
    bool (*can_format)(FormatOrbitType type);
    
    // Required: Format a value to string
    // Returns allocated string (caller must free with plugin_free_string)
    // Returns NULL if cannot format
    char* (*format)(const FormatOrbitValue* value, const char* format_id);
    
    // Optional: Get possible conversions from a value
    // Returns number of conversions written to `out`
    size_t (*get_conversions)(
        const FormatOrbitValue* value,
        FormatOrbitConversionList* out
    );
    
    // Required: Free a string allocated by this plugin
    void (*free_string)(char* s);
    
    // Optional: Cleanup when plugin is unloaded
    void (*shutdown)(void);
    
} FormatOrbitPlugin;

// ============================================================================
// Plugin Entry Point
// ============================================================================

// Every plugin dylib must export this symbol
// Returns NULL on initialization failure
typedef FormatOrbitPlugin* (*FormatOrbitPluginInit)(void);

#define FORMATORBIT_PLUGIN_ENTRY formatorbit_plugin_init

#ifdef __cplusplus
}
#endif

#endif // FORMATORBIT_PLUGIN_H
```

### Rust Plugin Helper (plugin-api crate)

```rust
// crates/plugin-api/src/lib.rs

//! Helper utilities for writing Formatorbit plugins in Rust.
//! 
//! This crate provides:
//! - Safe wrappers around the C ABI types
//! - Macros for generating the plugin entry point
//! - Conversion utilities

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// Re-export C types
pub mod ffi {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// Macro to generate plugin boilerplate
/// 
/// # Example
/// ```rust
/// use formatorbit_plugin_api::prelude::*;
/// 
/// struct UuidPlugin;
/// 
/// impl Plugin for UuidPlugin {
///     fn info(&self) -> PluginInfo {
///         PluginInfo {
///             id: "com.example.uuid",
///             name: "UUID Format",
///             version: "1.0.0",
///             author: Some("Example"),
///         }
///     }
///     
///     fn format_ids(&self) -> &[&str] {
///         &["uuid", "uuid-v4", "guid"]
///     }
///     
///     fn parse(&self, input: &str) -> Vec<Interpretation> {
///         // ... parsing logic
///     }
///     
///     // ... other methods
/// }
/// 
/// formatorbit_plugin!(UuidPlugin);
/// ```
#[macro_export]
macro_rules! formatorbit_plugin {
    ($plugin_type:ty) => {
        static PLUGIN_INSTANCE: std::sync::OnceLock<$plugin_type> = std::sync::OnceLock::new();
        
        #[no_mangle]
        pub extern "C" fn formatorbit_plugin_init() -> *mut $crate::ffi::FormatOrbitPlugin {
            let plugin = PLUGIN_INSTANCE.get_or_init(|| <$plugin_type>::default());
            // ... generate vtable
            todo!("Generate C vtable from Plugin trait impl")
        }
    };
}

/// Safe plugin trait that Rust plugins implement
pub trait Plugin: Send + Sync {
    fn info(&self) -> PluginInfo;
    fn format_ids(&self) -> &[&str];
    fn parse(&self, input: &str) -> Vec<Interpretation>;
    fn can_format(&self, value_type: ValueType) -> bool;
    fn format(&self, value: &Value, format_id: &str) -> Option<String>;
    fn get_conversions(&self, value: &Value) -> Vec<Conversion> {
        vec![] // Default: no conversions
    }
}

pub struct PluginInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub author: Option<&'static str>,
}

// ... safe wrapper types matching core types
```

### Swift Plugin Exempel

```swift
// plugins/examples/swift/ColorPlugin/Sources/ColorPlugin.swift

import Foundation

// Import the C header
// In Package.swift: .systemLibrary(name: "FormatorbitPluginAPI")

@_cdecl("formatorbit_plugin_init")
public func pluginInit() -> UnsafeMutablePointer<FormatOrbitPlugin>? {
    let plugin = UnsafeMutablePointer<FormatOrbitPlugin>.allocate(capacity: 1)
    
    plugin.pointee = FormatOrbitPlugin(
        get_info: { 
            FormatOrbitPluginInfo(
                api_version: UInt32(FORMATORBIT_PLUGIN_API_VERSION),
                plugin_id: strdup("com.example.color"),
                plugin_name: strdup("Color Format"),
                plugin_version: strdup("1.0.0"),
                author: nil
            )
        },
        get_format_ids: {
            strdup("color-hex;rgb;hsl")
        },
        parse: { input, out in
            guard let input = input.map({ String(cString: $0) }) else { return 0 }
            // Parse #RRGGBB, rgb(r,g,b), etc.
            // Write results to `out`
            return 0
        },
        can_format: { type in
            type == FO_TYPE_BYTES || type == FO_TYPE_INT
        },
        format: { value, formatId in
            // Format color value
            nil
        },
        get_conversions: { value, out in
            0
        },
        free_string: { s in
            s?.deallocate()
        },
        shutdown: nil
    )
    
    return plugin
}
```

### Python Plugin System

```rust
// crates/core/src/plugin/python.rs

//! Python plugin support via PyO3
//! 
//! Enabled with `python` feature flag.

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
pub struct PythonPlugin {
    module: Py<PyModule>,
    // Cached function references
    parse_fn: Py<PyAny>,
    format_fn: Py<PyAny>,
}

#[cfg(feature = "python")]
impl PythonPlugin {
    pub fn load(path: &Path) -> Result<Self, PluginError> {
        Python::with_gil(|py| {
            // Load Python file as module
            let code = std::fs::read_to_string(path)?;
            let module = PyModule::from_code(py, &code, path.to_str().unwrap(), "")?;
            
            // Get required functions
            let parse_fn = module.getattr("parse")?.into();
            let format_fn = module.getattr("format")?.into();
            
            Ok(Self {
                module: module.into(),
                parse_fn,
                format_fn,
            })
        })
    }
}
```

```python
# plugins/examples/python/ipaddress_plugin.py

"""
Formatorbit plugin for IP address parsing and formatting.

Required functions:
- info() -> dict
- format_ids() -> list[str]  
- parse(input: str) -> list[dict]
- can_format(type: str) -> bool
- format(value: dict, format_id: str) -> str | None
"""

import ipaddress

def info():
    return {
        "id": "com.example.ipaddress",
        "name": "IP Address Format",
        "version": "1.0.0",
    }

def format_ids():
    return ["ipv4", "ipv6", "ip"]

def parse(input: str) -> list:
    results = []
    
    # Try IPv4
    try:
        addr = ipaddress.IPv4Address(input)
        results.append({
            "value": {"type": "bytes", "bytes": addr.packed},
            "source_format": "ipv4",
            "confidence": 0.9,
            "description": f"IPv4 address: {addr}",
        })
    except:
        pass
    
    # Try IPv6
    try:
        addr = ipaddress.IPv6Address(input)
        results.append({
            "value": {"type": "bytes", "bytes": addr.packed},
            "source_format": "ipv6",
            "confidence": 0.9,
            "description": f"IPv6 address: {addr}",
        })
    except:
        pass
    
    return results

def can_format(value_type: str) -> bool:
    return value_type == "bytes"

def format(value: dict, format_id: str) -> str | None:
    if value["type"] != "bytes":
        return None
    
    data = bytes(value["bytes"])
    
    if format_id == "ipv4" and len(data) == 4:
        return str(ipaddress.IPv4Address(data))
    elif format_id == "ipv6" and len(data) == 16:
        return str(ipaddress.IPv6Address(data))
    
    return None
```

### Plugin Loading & Registry

```rust
// crates/core/src/registry.rs

use std::path::Path;
use std::sync::Arc;
use libloading::Library;

pub struct PluginRegistry {
    /// Native Rust plugins (no FFI overhead)
    native: Vec<Arc<dyn Format>>,
    
    /// Loaded dylib plugins
    dylibs: Vec<DylibPlugin>,
    
    /// Python plugins (if feature enabled)
    #[cfg(feature = "python")]
    python: Vec<PythonPlugin>,
}

struct DylibPlugin {
    _library: Library,  // Keep loaded
    vtable: *const FormatOrbitPlugin,
    info: PluginInfo,
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            native: vec![],
            dylibs: vec![],
            #[cfg(feature = "python")]
            python: vec![],
        };
        
        // Register built-in formats
        registry.register_native(Arc::new(formats::HexFormat));
        registry.register_native(Arc::new(formats::Base64Format));
        registry.register_native(Arc::new(formats::IntegerFormat));
        registry.register_native(Arc::new(formats::DateTimeFormat));
        registry.register_native(Arc::new(formats::JsonFormat));
        registry.register_native(Arc::new(formats::Utf8Format));
        
        registry
    }
    
    /// Load plugins from a directory
    pub fn load_plugins_from(&mut self, dir: &Path) -> Result<(), PluginError> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            
            if let Some(ext) = path.extension() {
                match ext.to_str() {
                    Some("dylib") | Some("so") | Some("dll") => {
                        self.load_dylib(&path)?;
                    }
                    #[cfg(feature = "python")]
                    Some("py") => {
                        self.load_python(&path)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    
    fn load_dylib(&mut self, path: &Path) -> Result<(), PluginError> {
        unsafe {
            let library = Library::new(path)?;
            
            let init: libloading::Symbol<FormatOrbitPluginInit> = 
                library.get(b"formatorbit_plugin_init")?;
            
            let vtable = init();
            if vtable.is_null() {
                return Err(PluginError::InitFailed);
            }
            
            // Verify API version
            let info = ((*vtable).get_info)();
            if info.api_version != FORMATORBIT_PLUGIN_API_VERSION {
                return Err(PluginError::VersionMismatch);
            }
            
            self.dylibs.push(DylibPlugin {
                _library: library,
                vtable,
                info: info.into(),
            });
        }
        Ok(())
    }
}
```

---

## Domänmodell (Rust Core)

### Kärntyper

```rust
// crates/core/src/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Interna värdetyper som allt konverteras mellan
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CoreValue {
    Bytes(Vec<u8>),
    String(String),
    Int { 
        value: i128, 
        /// Original bytes för att kunna visa endianness-varianter
        #[serde(skip_serializing_if = "Option::is_none")]
        original_bytes: Option<Vec<u8>>,
    },
    Float(f64),
    Bool(bool),
    DateTime(DateTime<Utc>),
    Json(JsonValue),
}

impl CoreValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bytes(_) => "bytes",
            Self::String(_) => "string",
            Self::Int { .. } => "int",
            Self::Float(_) => "float",
            Self::Bool(_) => "bool",
            Self::DateTime(_) => "datetime",
            Self::Json(_) => "json",
        }
    }
}

/// En möjlig tolkning av input-strängen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    pub value: CoreValue,
    pub source_format: String,
    pub confidence: f32,
    pub description: String,
    /// Which plugin provided this interpretation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

/// En möjlig konvertering från ett värde
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    pub value: CoreValue,
    pub target_format: String,
    pub display: String,
    pub path: Vec<String>,
    pub is_lossy: bool,
}

/// Komplett resultat för en input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub input: String,
    pub interpretation: Interpretation,
    pub conversions: Vec<Conversion>,
}
```

### Format Trait (för native plugins)

```rust
// crates/core/src/formats/mod.rs

use crate::types::*;

/// Trait för inbyggda format och Rust-plugins
/// 
/// Detta är den "snabba vägen" utan FFI overhead.
/// Externa plugins (dylibs) wrappas för att implementera detta trait.
pub trait Format: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    
    /// Försök tolka en input-sträng
    fn parse(&self, input: &str) -> Vec<Interpretation>;
    
    /// Vilka CoreValue-typer kan detta format formatera?
    fn can_format(&self, value: &CoreValue) -> bool;
    
    /// Formatera ett värde till en sträng
    fn format(&self, value: &CoreValue) -> Option<String>;
    
    /// Hämta möjliga konverteringar (för graf-traversering)
    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        vec![]
    }
}

// Built-in formats
pub mod hex;
pub mod base64;
pub mod integers;
pub mod datetime;
pub mod json;
pub mod utf8;
```

---

## Publikt API

```rust
// crates/core/src/lib.rs

pub mod types;
pub mod formats;
pub mod registry;
pub mod convert;
pub mod plugin;

pub use types::*;
pub use registry::PluginRegistry;

/// Main entry point - create a configured converter
pub struct Formatorbit {
    registry: PluginRegistry,
}

impl Formatorbit {
    /// Create with only built-in formats
    pub fn new() -> Self {
        Self {
            registry: PluginRegistry::new(),
        }
    }
    
    /// Create and load plugins from default locations
    pub fn with_plugins() -> Result<Self, PluginError> {
        let mut instance = Self::new();
        instance.load_default_plugins()?;
        Ok(instance)
    }
    
    /// Load plugins from default locations:
    /// - ~/.config/formatorbit/plugins/
    /// - /usr/share/formatorbit/plugins/ (Linux)
    /// - ~/Library/Application Support/Formatorbit/Plugins/ (macOS)
    pub fn load_default_plugins(&mut self) -> Result<(), PluginError> {
        // ... platform-specific paths
        Ok(())
    }
    
    /// Tolka en input-sträng på alla möjliga sätt
    pub fn interpret(&self, input: &str) -> Vec<Interpretation> {
        self.registry.parse_all(input)
    }
    
    /// Hitta alla möjliga konverteringar från ett värde
    pub fn convert(&self, value: &CoreValue) -> Vec<Conversion> {
        convert::find_all_conversions(&self.registry, value)
    }
    
    /// Kombinerad: tolka input och hitta alla konverteringar
    pub fn convert_all(&self, input: &str) -> Vec<ConversionResult> {
        self.interpret(input)
            .into_iter()
            .map(|interp| {
                let conversions = self.convert(&interp.value);
                ConversionResult {
                    input: input.to_string(),
                    interpretation: interp,
                    conversions,
                }
            })
            .collect()
    }
    
    /// Specifik konvertering
    pub fn convert_to(&self, input: &str, from: &str, to: &str) -> Option<String> {
        // ... direct conversion
        None
    }
}

// Convenience functions using default instance
pub fn interpret(input: &str) -> Vec<Interpretation> {
    Formatorbit::new().interpret(input)
}

pub fn convert_all(input: &str) -> Vec<ConversionResult> {
    Formatorbit::new().convert_all(input)
}
```

---

## MVP Format & Konverteringar

### Built-in Format (fas 1)

| Format | Input-exempel | Confidence-heuristik |
|--------|--------------|---------------------|
| `hex` | `691E01B8`, `0x691E01B8` | Bara [0-9A-Fa-f], jämn längd |
| `decimal` | `1763574200`, `-42` | Bara siffror, ev. minus |
| `base64` | `aR4BuA==` | Giltig base64-charset, korrekt padding |
| `json` | `{"key": "value"}` | `serde_json::from_str` lyckas |
| `utf8` | `hello world` | Fallback, allt är valid UTF-8 |

### Konverteringsgraf

```
bytes ←→ hex_string
bytes ←→ base64_string  
bytes → int (big-endian)
bytes → int (little-endian)
bytes → utf8_string (om valid)

int → bytes (big-endian)
int → bytes (little-endian)
int → decimal_string
int → hex_string
int (positiv, rimligt range) → datetime (epoch seconds)
int (positiv, rimligt range) → datetime (epoch millis)

datetime → int (epoch seconds)
datetime → int (epoch millis)
datetime → iso8601_string
datetime → rfc2822_string

string → bytes (utf8)
string → json (om valid)

json → string (pretty)
json → string (compact)
```

---

## CLI Design

```bash
# Grundanvändning
formatorbit "691E01B8"
# Alias
forb "691E01B8"

# Specifik konvertering
forb "691E01B8" --from hex --to base64

# Från stdin
echo "691E01B8" | forb

# JSON-output för scripting
forb "691E01B8" --json

# Lista laddade plugins
forb --list-plugins

# Ladda extra plugin-katalog
forb "691E01B8" --plugin-dir ./my-plugins/
```

---

## FFI för App-integration

```rust
// crates/ffi/src/lib.rs

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use formatorbit_core::Formatorbit;

static INSTANCE: std::sync::OnceLock<Formatorbit> = std::sync::OnceLock::new();

fn get_instance() -> &'static Formatorbit {
    INSTANCE.get_or_init(|| {
        Formatorbit::with_plugins().unwrap_or_else(|_| Formatorbit::new())
    })
}

/// Konvertera input och returnera JSON med alla resultat
#[no_mangle]
pub extern "C" fn formatorbit_convert_all(input: *const c_char) -> *mut c_char {
    let input = unsafe { CStr::from_ptr(input) }.to_str().unwrap_or("");
    let results = get_instance().convert_all(input);
    let json = serde_json::to_string(&results).unwrap_or_default();
    CString::new(json).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn formatorbit_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// Ladda plugins från en katalog
#[no_mangle]
pub extern "C" fn formatorbit_load_plugins(dir: *const c_char) -> bool {
    // Note: would need mutable access, consider redesign
    false
}
```

---

## Dependencies

```toml
# crates/core/Cargo.toml
[package]
name = "formatorbit-core"
version = "0.1.0"
edition = "2021"

[features]
default = []
python = ["pyo3"]

[dependencies]
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
thiserror = "1"
libloading = "0.8"

[dependencies.pyo3]
version = "0.20"
optional = true

[dev-dependencies]
pretty_assertions = "1"
```

```toml
# crates/cli/Cargo.toml
[package]
name = "formatorbit-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "forb"
path = "src/main.rs"

[dependencies]
formatorbit-core = { path = "../core" }
clap = { version = "4", features = ["derive"] }
colored = "2"
serde_json = "1"
```

---

## Implementationsordning

### Fas 1: Core + CLI (börja här)

1. Sätt upp workspace
2. Implementera `types.rs`
3. Implementera `formats/` (hex, integers, base64, datetime, utf8, json)
4. Implementera `convert.rs` (BFS graf-traversering)
5. Implementera `registry.rs` (utan dylib-loading först)
6. Implementera CLI
7. Tester

### Fas 2: Plugin System

1. Definiera C ABI i `plugin-api` crate
2. Implementera dylib loading i `registry.rs`
3. Skapa `formatorbit_plugin!` macro
4. Exempel-plugin i Rust (uuid eller color)
5. Dokumentation för plugin-utvecklare

### Fas 3: FFI & macOS App

1. Implementera `crates/ffi`
2. Swift-wrapper
3. macOS app med floating panel

### Fas 4: Utökningar

1. Python plugin support (feature flag)
2. Fler built-in formats
3. Linux/Windows desktop app (Tauri)

---

## Testfall

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_epoch() {
        let forb = Formatorbit::new();
        let results = forb.convert_all("691E01B8");
        
        let hex_result = results.iter()
            .find(|r| r.interpretation.source_format == "hex")
            .expect("Should find hex interpretation");
        
        let datetime_conv = hex_result.conversions.iter()
            .find(|c| c.target_format.contains("datetime"))
            .expect("Should have datetime conversion");
        
        assert!(datetime_conv.display.contains("2025"));
    }

    #[test]
    fn test_both_endianness() {
        let forb = Formatorbit::new();
        let results = forb.convert_all("691E01B8");
        
        let hex_result = results.iter()
            .find(|r| r.interpretation.source_format == "hex")
            .unwrap();
        
        // Should have both BE and LE interpretations
        let formats: Vec<_> = hex_result.conversions.iter()
            .map(|c| c.target_format.as_str())
            .collect();
        
        assert!(formats.iter().any(|f| f.contains("be") || f.contains("big")));
        assert!(formats.iter().any(|f| f.contains("le") || f.contains("little")));
    }
    
    #[test]
    fn test_base64_roundtrip() {
        let forb = Formatorbit::new();
        
        // hex -> bytes -> base64
        let results = forb.convert_all("48656c6c6f");  // "Hello" in hex
        
        let hex_result = results.iter()
            .find(|r| r.interpretation.source_format == "hex")
            .unwrap();
        
        let base64_conv = hex_result.conversions.iter()
            .find(|c| c.target_format == "base64")
            .unwrap();
        
        assert_eq!(base64_conv.display, "SGVsbG8=");
    }
}
```

---

## Plugin Development Guide (för dokumentation)

### Rust Plugin (rekommenderat)

```rust
// my_plugin/src/lib.rs
use formatorbit_plugin_api::prelude::*;

#[derive(Default)]
struct MyPlugin;

impl Plugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "com.mycompany.myformat",
            name: "My Format",
            version: env!("CARGO_PKG_VERSION"),
            author: Some("Me"),
        }
    }
    
    fn format_ids(&self) -> &[&str] {
        &["myformat"]
    }
    
    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Your parsing logic
        vec![]
    }
    
    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(_))
    }
    
    fn format(&self, value: &CoreValue, _format_id: &str) -> Option<String> {
        // Your formatting logic
        None
    }
}

formatorbit_plugin!(MyPlugin);
```

Build: `cargo build --release`
Install: Copy `.dylib`/`.so`/`.dll` to plugin directory.

### Swift Plugin

Se exempel i `plugins/examples/swift/`.

### Python Plugin

Se exempel i `plugins/examples/python/`.

---

## Framtida utökningar

- UUID parsing/formatting
- IPv4/IPv6 adresser  
- Color codes (hex ↔ RGB/HSL)
- URL encoding/decoding
- JWT decode (header + payload)
- Protobuf/MessagePack
- QR code generation
- Hash detection (MD5, SHA, etc längder)
- Regex-baserade custom patterns
- WASM plugins för sandboxing~
