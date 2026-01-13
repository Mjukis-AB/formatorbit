# CLAUDE.md - Formatorbit

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
              └──────────────┘
```

### Crate Structure

- `crates/core` - Main logic: types, parsing, conversion graph, formatting
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
    Protobuf(Vec<ProtoField>),
    // Unit types for type-safe conversions
    Length(f64),      // meters
    Weight(f64),      // grams
    Volume(f64),      // milliliters
    Temperature(f64), // kelvin
    Currency { amount: f64, code: String },
    Coordinates { lat: f64, lon: f64 },
    // ... and more unit types
}

// Result of parsing input
pub struct Interpretation {
    pub value: CoreValue,
    pub source_format: String,   // "hex", "base64", etc.
    pub confidence: f32,          // 0.0 - 1.0
    pub description: String,      // Plain-text fallback
    pub rich_display: Vec<RichDisplayOption>, // Structured display for GUIs
}

// Result of converting a value
pub struct Conversion {
    pub value: CoreValue,
    pub target_format: String,
    pub display: String,          // Plain-text fallback
    pub path: Vec<String>,        // How we got here
    pub is_lossy: bool,
    pub priority: ConversionPriority, // Primary, Structured, Semantic, Encoding, Raw
    pub kind: ConversionKind,     // Conversion, Representation, or Trait
    pub display_only: bool,       // If true, don't explore further in BFS
    pub hidden: bool,             // If true, don't show in output (internal chaining)
    pub rich_display: Vec<RichDisplayOption>, // Structured display for GUIs
}

// Distinguishes types of conversions for UI grouping
pub enum ConversionKind {
    Conversion,     // Actual transformation (bytes → int, int → datetime)
    Representation, // Same value, different notation (1024 → 0x400, 5e-9 m → 5 nm)
    Trait,          // Observation/property (is prime, is power-of-2, is fibonacci)
}

// Priority for output ordering
pub enum ConversionPriority {
    Primary,    // Canonical result (expression results)
    Structured, // JSON, MessagePack, Protobuf
    Semantic,   // DateTime, UUID, IP, Color
    Encoding,   // Hex, Base64, URL encoding
    Raw,        // Bytes, raw integers
}
```

### Rich Display System

The `rich_display` field provides structured data for GUI rendering. When `rich_display` is populated, GUIs should render it instead of `description`/`display` to avoid redundancy.

```rust
pub enum RichDisplay {
    KeyValue { pairs: Vec<(String, String)> },  // IP parts, codepoints
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Tree { root: TreeNode },                     // JSON, protobuf
    Color { r: u8, g: u8, b: u8, a: u8 },       // Color swatch
    Map { lat: f64, lon: f64, label: Option<String> },
    Mermaid { source: String },                  // Diagram
    Dot { source: String },                      // Graphviz
    Code { language: String, content: String },
    Duration { millis: u64, human: String },
    DateTime { iso: String, relative: String },
    DataSize { bytes: u64, human: String },
    PacketLayout { segments: Vec<PacketSegment>, ... }, // Binary layout
    // ... and more
}

pub struct RichDisplayOption {
    pub preferred: RichDisplay,
    pub alternatives: Vec<RichDisplay>,
}
```

**Display Strategy for UI Apps:**
1. If `rich_display` is not empty → render `rich_display[0].preferred`, hide `description`
2. If `rich_display` is empty → fall back to `description` as plain text

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

### 4. Epoch Heuristics

Only interpret integers as epoch timestamps if they're in a reasonable range:

```rust
const MIN_EPOCH: i64 = 0;           // 1970-01-01
const MAX_EPOCH: i64 = 4_102_444_800; // 2100-01-01

// For milliseconds, multiply range by 1000
```

### 5. ConversionKind: Conversion vs Representation vs Trait

Use the correct `ConversionKind` to enable proper UI grouping:

**Conversion** - Actual data transformation. The output is semantically different:
- bytes → integer (interpreting bytes as a number)
- integer → datetime (epoch timestamp)
- hex → base64 (re-encoding)

**Representation** - Same underlying value, different notation. UI can group these:
- `5e-9 m` → `5 nm` (SI prefix)
- `5e-9 m` → `0.000000005 m` (decimal)
- `1024` → `0x400` (hex notation)
- `1024` → `1 KiB` (human-readable size)

**Trait** - Observation about the value. Doesn't produce a new value to convert:
- "is prime"
- "is power of 2"
- "is fibonacci number"

```rust
// Multiple representations of the same value
conversions.push(Conversion {
    target_format: "meters-si".to_string(),
    display: format_with_si_prefix(meters, "m"),  // "5 nm"
    kind: ConversionKind::Representation,
    display_only: true,  // Prevent BFS from exploring further
    ..Default::default()
});
```

Set `display_only: true` on representations to prevent the BFS from treating
the display string as input for further conversions (e.g., converting "5 nm"
as ASCII bytes → hex).

### 6. Noise Control and Blocking

The BFS conversion graph can produce excessive noise. We control this with:

**Root-based blocking** (`ROOT_BLOCKED_TARGETS` in `convert.rs`):
Block conversions based on the original interpretation, regardless of path.

```rust
// In convert.rs
const ROOT_BLOCKED_TARGETS: &[(&str, &str)] = &[
    // Text bytes shouldn't become IPs, colors, timestamps, UUIDs
    ("text", "ipv4"),
    ("text", "uuid"),
    ("text", "epoch-seconds"),
    // Hex bytes shouldn't become IPs or colors
    ("hex", "ipv4"),
    ("hex", "color-rgb"),
];
```

**Path-based blocking** (`BLOCKED_PATHS`):
Block specific immediate conversions.

```rust
const BLOCKED_PATHS: &[(&str, &str)] = &[
    ("bytes", "int-be"),  // Require explicit hex interpretation first
];
```

**When to add blocking:**
- If `forb "some input"` produces duplicate or nonsense conversions
- If any 16 bytes becoming a UUID, any 4 bytes becoming an IP, etc.
- Test with `-l 0` to see full output and identify noise

### 7. Heuristics for False Positives

Many formats overlap syntactically. Use heuristics to reject false positives:

```rust
// In base64.rs - reject pure hex strings
fn looks_like_hex(s: &str) -> bool {
    s.len() >= 2 && s.len() % 2 == 0 && s.chars().all(|c| c.is_ascii_hexdigit())
}

// In hash.rs - don't detect 8-char hex as CRC-32
// "DEADBEEF" is hex, not a hash
const HASH_TYPES: &[(&str, usize, &str)] = &[
    ("MD5", 32, "128-bit"),
    ("SHA-1", 40, "160-bit"),
    // CRC-32 (8 chars) removed - too ambiguous
];

// In color.rs - require # prefix for 6/8 char colors
// "DEADBE" without # is hex, not a color
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
└── formats/         # Built-in format implementations
    ├── mod.rs       # Format trait, format list
    ├── hex.rs
    ├── base64.rs
    ├── integers.rs
    ├── datetime.rs
    ├── json.rs
    └── utf8.rs
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

### Adding Constants and Magic Numbers

Constants (well-known values) are defined in `formats/constants.rs`:

```rust
// Named constants (parsed from input like "SIGKILL" or "HTTP 404")
const NAMED_CONSTANTS: &[Constant] = &[
    Constant {
        name: "SIGKILL",
        aliases: &["kill"],
        value: 9,
        trait_display: "SIGKILL (Unix signal: kill)",
        parse_display: "SIGKILL = 9",
    },
];

// Magic numbers (shown as traits on matching integers)
const MAGIC_NUMBERS: &[Constant] = &[
    Constant {
        name: "DEADBEEF",
        aliases: &[],
        value: 0xDEADBEEF,
        trait_display: "DEADBEEF (debug memory marker)",
        parse_display: "0xDEADBEEF (debug memory marker)",
    },
    Constant {
        name: "CAFEBABE",
        aliases: &[],
        value: 0xCAFEBABE,
        trait_display: "CAFEBABE (Java class file)",
        parse_display: "0xCAFEBABE (Java class file)",
    },
];
```

Constants appear:
- As interpretations when input matches the name (e.g., "SIGKILL" → 9)
- As traits on integers when value matches (e.g., 3735928559 → "DEADBEEF")

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
4. **Don't forget endianness** - always consider both BE and LE
5. **Don't hardcode format strings** - use constants or the format's `id()`

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

## Snapshot Testing Philosophy

Snapshots in `crates/core/tests/snapshots.rs` capture expected user-facing output.

### Golden Rule

**Never blindly accept snapshot changes.** Before running `cargo insta accept`:

1. **Review the diff** - What changed and why?
2. **Evaluate from end-user perspective** - Is this an improvement or regression?
3. **Check conversion counts** - Did noise increase? (see `*_conversion_count` snapshots)

### Types of Snapshot Tests

```rust
// Interpretation snapshots - what does the parser understand?
stable_snapshot!("ipv4_private_interpretation", &result.interpretation);

// Conversion snapshots - what outputs are generated?
stable_snapshot!("color_hex_conversions", &result.conversions);

// Count snapshots - noise regression detection
let count = result.conversions.len();
stable_snapshot!("hex_conversion_count", count);  // Should NOT increase unexpectedly
```

### When Snapshot Changes Are OK

- **Adding new useful conversions** (e.g., new hash algorithm)
- **Improving descriptions** (clearer wording)
- **Fixing bugs** (wrong values corrected)
- **Adding rich_display** (new structured data)

### When Snapshot Changes Are NOT OK

- **Conversion count increased significantly** without clear benefit
- **Duplicate information** appearing (same data via different paths)
- **Noise formats** being detected (DEADBEEF as base64, arbitrary bytes as UUID)
- **Regressions** in confidence scores without justification

### Reviewing Snapshots

```bash
# Run tests, see what changed
cargo test -p formatorbit-core

# Review changes interactively
cargo insta review

# Or accept after careful review
cargo insta accept
```

## Questions to Ask Yourself

When implementing a feature:

1. What's the confidence score for this interpretation?
2. Is this conversion lossy? Mark it if so.
3. Did I handle both endianness options?
4. Is the path tracking correct?
5. Are there tests for edge cases (empty input, huge numbers, invalid UTF-8)?
6. **Does this add noise?** Test with `forb "input" -l 0` to see full output
7. **Should this be blocked?** Would this conversion make sense to an end user?
8. **Are snapshots affected?** Review changes from end-user perspective

## Changelog

This project uses [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. When adding features or fixing bugs, update `CHANGELOG.md` under the `[Unreleased]` section with the appropriate category (Added, Changed, Fixed, etc.).
