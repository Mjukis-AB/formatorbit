~# CLAUDE.md - Formatorbit

## Project Overview

Formatorbit is a cross-platform data format converter. Users input data (e.g., `691E01B8`) and the tool shows all possible interpretations and conversions automatically.

**Core idea:** Separate parsing (input → internal type) from conversion (internal type → internal type) from formatting (internal type → output string). This enables a graph-based approach where we can find all reachable conversions via BFS.

## Architecture

```
┌─────────────────┬─────────────────┬──────────────┐
│  macOS SwiftUI  │  Linux/Windows  │     CLI      │
└────────┬────────┴────────┬────────┴──────┬───────┘
         │                 │               │
         └────────────┬────┴───────────────┘
                      ▼
              ┌──────────────┐
              │  FFI (C ABI) │
              └──────┬───────┘
                     ▼
              ┌──────────────┐
              │  Rust Core   │
              │  + Plugins   │
              └──────────────┘
```

### Crate Structure

- `crates/core` - Main logic: types, parsing, conversion graph, formatting
- `crates/plugin-api` - Stable C ABI for external plugins
- `crates/ffi` - C bindings for Swift/other languages
- `crates/cli` - Command-line interface (`forb`)

## Build Commands

```bash
# Build everything
cargo build

# Build release
cargo build --release

# Run CLI
cargo run -p formatorbit-cli -- "691E01B8"

# Run tests
cargo test

# Run specific crate tests
cargo test -p formatorbit-core

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

## Code Conventions

### Rust Style

- Use `thiserror` for error types, not manual `impl Error`
- Prefer `&str` in function parameters, `String` in return types and structs
- Use `#[must_use]` on functions returning values that shouldn't be ignored
- Document public APIs with `///` comments including examples
- Keep functions small and focused; extract helpers liberally

### Naming

- Format IDs: lowercase with hyphens (`base64`, `epoch-seconds`, `int-be`)
- Type names: `CoreValue`, `Interpretation`, `Conversion`
- Trait: `Format` for things that can parse and format
- Modules: singular (`format`, `plugin`), except `formats/` directory

### Error Handling

```rust
// Define errors with thiserror
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid hex character: {0}")]
    InvalidHexChar(char),
    
    #[error("odd length hex string")]
    OddLengthHex,
}

// Use Result for fallible operations
fn parse_hex(input: &str) -> Result<Vec<u8>, ParseError>;

// Use Option for "might not apply" (not an error)
fn parse(&self, input: &str) -> Option<Interpretation>;
```

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_descriptive_name() {
        // Arrange
        let input = "691E01B8";
        
        // Act
        let result = parse_hex(input).unwrap();
        
        // Assert
        assert_eq!(result, vec![0x69, 0x1E, 0x01, 0xB8]);
    }
}
```

## Core Types

```rust
// The central value type - all formats convert to/from these
pub enum CoreValue {
    Bytes(Vec<u8>),
    String(String),
    Int { value: i128, original_bytes: Option<Vec<u8>> },
    Float(f64),
    Bool(bool),
    DateTime(chrono::DateTime<chrono::Utc>),
    Json(serde_json::Value),
}

// Result of parsing input
pub struct Interpretation {
    pub value: CoreValue,
    pub source_format: String,   // "hex", "base64", etc.
    pub confidence: f32,          // 0.0 - 1.0
    pub description: String,
}

// Result of converting a value
pub struct Conversion {
    pub value: CoreValue,
    pub target_format: String,
    pub display: String,          // Ready-to-show output
    pub path: Vec<String>,        // How we got here
    pub is_lossy: bool,
}
```

## Key Design Decisions

### 1. Confidence Scoring

Parsers return a confidence score (0.0-1.0) indicating how likely the interpretation is correct:

```rust
// High confidence: has 0x prefix, valid hex chars, even length
"0x691E01B8" -> hex, confidence: 0.95

// Medium confidence: valid hex chars, even length, but no prefix  
"691E01B8" -> hex, confidence: 0.7

// Lower confidence: could be hex but also valid decimal
"123456" -> hex, confidence: 0.5
"123456" -> decimal, confidence: 0.8
```

### 2. Endianness

Always show both big-endian and little-endian interpretations for bytes→int:

```rust
// bytes [0x69, 0x1E, 0x01, 0xB8]
// → int-be: 1763574200
// → int-le: 3087818345
```

Store `original_bytes` in `CoreValue::Int` to enable this.

### 3. Path Tracking

Track the conversion path so users understand how we got there:

```rust
Conversion {
    path: vec!["hex", "bytes", "int-be", "epoch-seconds", "datetime"],
    display: "2025-11-19T17:43:20Z",
    // ...
}
```

### 4. Plugin Architecture

Three tiers with different tradeoffs:

| Type | Language | Overhead | When to use |
|------|----------|----------|-------------|
| Native | Rust | Zero | Built-in formats, performance-critical |
| Dylib | Rust/Swift/C | Minimal | Third-party, platform-specific |
| Script | Python | Higher | Prototyping, simple formats |

All external plugins use C ABI. Rust plugins can use the `Format` trait directly.

### 5. Epoch Heuristics

Only interpret integers as epoch timestamps if they're in a reasonable range:

```rust
const MIN_EPOCH: i64 = 0;           // 1970-01-01
const MAX_EPOCH: i64 = 4_102_444_800; // 2100-01-01

// For milliseconds, multiply range by 1000
```

## File Organization

```
crates/core/src/
├── lib.rs           # Public API, re-exports
├── types.rs         # CoreValue, Interpretation, Conversion
├── registry.rs      # Plugin registry, format discovery
├── convert.rs       # BFS graph traversal
├── parse.rs         # Coordination of parsing
├── format.rs        # Coordination of formatting
├── error.rs         # Error types
│
├── formats/         # Built-in format implementations
│   ├── mod.rs       # Format trait, format list
│   ├── hex.rs
│   ├── base64.rs
│   ├── integers.rs
│   ├── datetime.rs
│   ├── json.rs
│   └── utf8.rs
│
└── plugin/          # Plugin loading
    ├── mod.rs
    ├── native.rs    # Rust trait-based plugins
    ├── dylib.rs     # Dynamic library loading
    └── python.rs    # Python support (feature-gated)
```

## Common Patterns

### Adding a New Built-in Format

1. Create `crates/core/src/formats/myformat.rs`
2. Implement the `Format` trait
3. Register in `formats/mod.rs`
4. Add tests

```rust
// formats/myformat.rs
pub struct MyFormat;

impl Format for MyFormat {
    fn id(&self) -> &'static str { "myformat" }
    fn name(&self) -> &'static str { "My Format" }
    
    fn parse(&self, input: &str) -> Vec<Interpretation> {
        // Try to parse, return empty vec if can't
        vec![]
    }
    
    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(_))
    }
    
    fn format(&self, value: &CoreValue) -> Option<String> {
        // Format to string
        None
    }
}
```

### Adding a Conversion Edge

Conversions are discovered by the graph traversal. To add a new edge:

1. Have a format's `conversions()` method return it, OR
2. Add it to the central conversion registry

```rust
impl Format for IntegerFormat {
    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Int { value: n, .. } = value else { 
            return vec![] 
        };
        
        let mut results = vec![];
        
        // int -> datetime (if reasonable epoch)
        if *n > 0 && *n < MAX_EPOCH {
            if let Some(dt) = DateTime::from_timestamp(*n as i64, 0) {
                results.push(Conversion {
                    value: CoreValue::DateTime(dt),
                    target_format: "datetime".into(),
                    display: dt.to_rfc3339(),
                    path: vec!["epoch-seconds".into()],
                    is_lossy: false,
                });
            }
        }
        
        results
    }
}
```

## What to Avoid

1. **Don't use `unwrap()` in library code** - use `?` or return `Option`/`Result`
2. **Don't allocate in hot paths** - reuse buffers where possible
3. **Don't add dependencies without consideration** - keep core lean
4. **Don't break the C ABI** - plugin API stability is critical
5. **Don't forget endianness** - always consider both BE and LE
6. **Don't hardcode format strings** - use constants or the format's `id()`

## Performance Considerations

- The conversion graph is traversed via BFS - keep it shallow
- Parsing is attempted for all formats - make `parse()` fast to fail
- Cache format instances (they're stateless)
- Use `Cow<str>` for strings that might not need allocation
- The CLI should feel instant - target <10ms for simple inputs

## FFI Safety

When working on `crates/ffi`:

```rust
// Always validate C strings
let input = unsafe { 
    CStr::from_ptr(input) 
}.to_str().unwrap_or("");

// Return allocated strings caller must free
let json = CString::new(result).unwrap();
json.into_raw() // Caller calls formatorbit_free_string

// Document ownership in header comments
/// Returns a newly allocated string. 
/// Caller must free with `formatorbit_free_string`.
#[no_mangle]
pub extern "C" fn formatorbit_convert_all(input: *const c_char) -> *mut c_char
```

## Useful Commands During Development

```bash
# Watch and run tests on change
cargo watch -x test

# Run with verbose output
RUST_LOG=debug cargo run -p formatorbit-cli -- "691E01B8"

# Generate docs
cargo doc --open

# Check for outdated dependencies
cargo outdated

# Security audit
cargo audit

# Build for release with LTO
cargo build --release --profile=release-lto
```

## Questions to Ask Yourself

When implementing a feature:

1. Does this belong in core, or should it be a plugin?
2. What's the confidence score for this interpretation?
3. Is this conversion lossy? Mark it if so.
4. Did I handle both endianness options?
5. Is the path tracking correct?
6. Are there tests for edge cases (empty input, huge numbers, invalid UTF-8)?~
