# Formatorbit Development Progress

## Overview

Formatorbit is a cross-platform data format converter. Input data like `691E01B8` and see all possible interpretations and conversions automatically.

Architecture:
```
┌─────────────────┬─────────────────┬──────────────┐
│  macOS SwiftUI  │  Linux/Windows  │     CLI      │
│  + Menu bar     │  (Tauri/egui)   │  (universal) │
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

---

## Phase 1: Core + CLI [COMPLETE]

### Workspace Setup
- [x] Rust workspace with 4 crates
  - `formatorbit-core` - Main library
  - `formatorbit-plugin-api` - Plugin helpers (placeholder)
  - `formatorbit-ffi` - C FFI bindings (placeholder)
  - `formatorbit-cli` - CLI binary `forb`

### Core Types
- [x] `CoreValue` enum (Bytes, String, Int, Float, Bool, DateTime, Json)
- [x] `Interpretation` struct (value, source_format, confidence, description)
- [x] `Conversion` struct (value, target_format, display, path, is_lossy)
- [x] `ConversionResult` struct (interpretation + conversions)

### Format Trait
- [x] `Format` trait with `id()`, `name()`, `parse()`, `can_format()`, `format()`, `conversions()`
- [x] `FormatInfo` struct for self-documentation
- [x] `info()` method on Format trait

### Built-in Formats

#### Encoding
- [x] **hex** - Multiple input styles (continuous, 0x prefix, space/colon/dash-separated, C arrays)
- [x] **base64** - Standard base64 encoding
- [x] **url-encoded** - Percent encoding with + as space

#### Numbers
- [x] **decimal** - Integer parsing
- [x] **int-be** - Big-endian bytes to int
- [x] **int-le** - Little-endian bytes to int

#### Timestamps
- [x] **epoch-seconds** - Unix timestamp (seconds since 1970)
- [x] **epoch-millis** - Unix timestamp milliseconds
- [x] **apple-cocoa** - Apple epoch (seconds since 2001-01-01)
- [x] **datetime** - ISO 8601, RFC 2822, RFC 3339 parsing

#### Identifiers
- [x] **uuid** - UUID parsing with version detection (v1-v8)

#### Network
- [x] **ipv4** - IPv4 address parsing
- [x] **ipv6** - IPv6 address parsing

#### Colors
- [x] **color-hex** - #RGB, #RRGGBB, #RRGGBBAA
- [x] **color-argb** - 0xAARRGGBB (Android style)
- [x] **color-rgb** - rgb()/rgba() CSS format
- [x] **color-hsl** - hsl() CSS format

#### Data
- [x] **json** - JSON parsing and validation
- [x] **msgpack** - MessagePack binary format
- [x] **utf8** - UTF-8 text (fallback interpretation)

### CLI
- [x] Comprehensive `--help` with examples
- [x] `--formats` flag to list all supported formats
- [x] `--json` flag for machine-readable output
- [x] Colored output matching conversion display style
- [x] Self-documenting format system (FormatInfo from each format)
- [x] **Pipe mode** - auto-detects stdin pipe, annotates log lines
- [x] `--threshold` / `-t` - confidence threshold for annotations (default 0.8)
- [x] `--highlight` / `-H` - highlight matched tokens inline
- [x] `--only` / `-o` - filter to specific formats (supports aliases like `b64`, `ts`, `uuid`)
- [x] Format aliases for quick filtering (e.g., `hex`→`h`, `base64`→`b64`, `datetime`→`ts`)

---

## Phase 2: Plugin System [NOT STARTED]

### C ABI Plugin API
- [ ] Define C ABI in `plugin-api` crate
- [ ] C header file (`formatorbit_plugin.h`)
- [ ] FormatOrbitPluginInfo struct
- [ ] FormatOrbitValue, FormatOrbitInterpretation, FormatOrbitConversion types
- [ ] FormatOrbitPlugin vtable interface

### Plugin Loading
- [ ] Implement dylib loading in registry
- [ ] Platform-specific plugin paths:
  - `~/.config/formatorbit/plugins/` (Linux)
  - `~/Library/Application Support/Formatorbit/Plugins/` (macOS)
- [ ] Plugin version checking

### Rust Plugin Helpers
- [ ] `formatorbit_plugin!` macro for easy plugin creation
- [ ] Safe wrappers around C ABI types
- [ ] Plugin trait for Rust plugins

### Example Plugins
- [ ] Rust plugin example
- [ ] Swift plugin example
- [ ] Plugin development documentation

### Python Support (Optional)
- [ ] PyO3 integration (feature-gated)
- [ ] Python plugin loader
- [ ] Python plugin example

---

## Phase 3: FFI & macOS App [NOT STARTED]

### FFI Layer (`crates/ffi`)
- [ ] Complete C bindings for Swift/other languages
- [ ] `formatorbit_convert_all()` function
- [ ] `formatorbit_free_string()` memory management
- [ ] `formatorbit_load_plugins()` for plugin loading
- [ ] cbindgen for header generation

### macOS App (`apps/macos`)
- [ ] SwiftUI application
- [ ] Menu bar presence
- [ ] Floating panel UI
- [ ] Global hotkey support
- [ ] macOS Services integration (right-click → Convert)
- [ ] Clipboard integration

---

## Phase 4: Cross-Platform Desktop [NOT STARTED]

### Linux/Windows App (`apps/desktop`)
- [ ] Tauri or egui application
- [ ] System tray integration
- [ ] Clipboard monitoring
- [ ] Cross-platform keyboard shortcuts

---

## Future Enhancements

### High Priority Formats (from FORMAT_IDEAS.md)
- [ ] JWT tokens - Decode header.payload, show claims
- [ ] IEEE 754 float bits - Hardware debugging
- [ ] Hash detection - Length-based heuristic (MD5, SHA1, SHA256)
- [ ] Windows FILETIME - Forensics/Windows dev
- [ ] Base58 - Blockchain/crypto

### Additional Features
- [ ] `--from`/`--to` flags for specific conversion
- [ ] `--plugin-dir` flag for custom plugin location
- [ ] `--list-plugins` to show loaded plugins
- [ ] Stdin input support (`echo "data" | forb`)
- [ ] WASM plugins for sandboxing

---

## Current State

The CLI tool `forb` is fully functional with:
- 13 format parsers covering common data types
- Smart hex input normalization for various paste formats
- BFS-based conversion graph traversal
- Confidence scoring for interpretations
- Colored, user-friendly output

### Quick Start

```bash
# Build
cargo build

# Direct input examples
cargo run -p formatorbit-cli -- "691E01B8"
cargo run -p formatorbit-cli -- "87 A3 69 6E 74 01"
cargo run -p formatorbit-cli -- "#FF5733"
cargo run -p formatorbit-cli -- --formats
cargo run -p formatorbit-cli -- --help

# Pipe mode (annotate log files)
cat logs.txt | cargo run -p formatorbit-cli --
cat logs.txt | cargo run -p formatorbit-cli -- -t 0.5 -H
cat logs.txt | cargo run -p formatorbit-cli -- -o uuid,hex

# Run tests
cargo test
```

---

## Notes

- All parsing happens at input stage (smart hex normalization)
- Recursive reparsing was considered but kept simple to maintain confidence in core functionality
- Each format is self-documenting via `FormatInfo`
- Plugin system uses C ABI for cross-language compatibility
