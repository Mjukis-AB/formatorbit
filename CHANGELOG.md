# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1] - 2025-12-26

### Added
- **Primary conversion priority** - `conversions[0]` is now always the canonical result value
  - New `ConversionPriority::Primary` ensures the main result appears first
  - Expression results now show `result: 256` before hex/binary representations
  - Float expressions now have conversions (previously showed none)

### Fixed
- **Escape format false positives** - text with sparse `\x` sequences no longer matches
  - Requires at least 10% escape sequence density for longer inputs
  - Fixes pasting terminal output containing escape-hex conversion results
- **Cross-domain conversion noise** - filter nonsensical conversions:
  - datasize ↔ duration (bytes aren't seconds)
  - duration → duration-ms (don't reinterpret time scales)
  - color → duration/datasize (colors aren't timestamps or byte counts)

## [0.5.0] - 2025-12-26

### Added
- **Currency conversion** - parse `100 USD`, `$50`, `€25`, `5kEUR`, `2.5MSEK` with live exchange rates
  - Fetches rates from European Central Bank via Frankfurter API
  - Caches rates locally with 24-hour TTL
  - SI prefix support for large amounts (k=thousand, M=million, G=billion)
  - Ambiguous symbols (`$`, `kr`) show multiple interpretations with locale-aware confidence
- **Unit conversion formats** - 8 new unit categories with full SI prefix support:
  - Length: `5km`, `100m`, `3.5 miles`, `50nm` (nanometers)
  - Weight: `5kg`, `150lbs`, `100mg`, `50ng` (nanograms)
  - Volume: `500mL`, `2L`, `1 gallon`, `50µL`
  - Speed: `100km/h`, `60mph`, `10 m/s`, `30 knots`
  - Pressure: `101.3kPa`, `14.7psi`, `1 atm`, `760mmHg`
  - Energy: `100kJ`, `500 calories`, `1 kWh`, `1 BTU`
  - Angle: `90deg`, `3.14rad`, `45°`, `100grad`
  - Area: `100m²`, `500 sqft`, `2 acres`, `1 hectare`
- **Temperature conversion** - parse `30°C`, `86°F`, `300K` with conversions between Celsius, Fahrenheit, and Kelvin
- **Multiple representations** for unit values - shows SI prefix, scientific notation, and decimal forms
- **Unit-specific CoreValue variants** - `Length(f64)`, `Weight(f64)`, `Currency { amount, code }`, etc. for type-safe conversions

### Changed
- `--formats` now shows all categories including Math, Units, Time, and Hashing
- `--help` updated with currency and unit examples
- README expanded with currency, unit, and temperature examples
- README now explains Conversion Kinds (Conversion vs Representation vs Trait)

### Fixed
- Unit conversions no longer cross-contaminate (e.g., weight doesn't show length conversions)

## [0.4.0] - 2025-12-25

### Added
- **Packet layout visualization** - byte-level structure display for protobuf and msgpack with `--packet/-p` flag
  - Compact mode: `[08:tag₁][96 01:150][12:tag₂]...`
  - Detailed mode: table with offset, length, field, type, and value columns
- **Duration format** - parse `1h30m`, `2d`, `3y`, ISO 8601 durations (`PT1H30M`), and many more
- **Data size format** - parse `1MB`, `1MiB`, `1.5GB` with IEC/SI unit conversions
- **Expression evaluation** - evaluate math expressions like `0xFF + 1`, `1 << 8`, `0b1010 | 0b0101`
- **Octal format** - parse `0o777`, `0755` (C-style), and convert integers to octal
- **Escape sequences** - decode C-style escapes (`\x48\x65\x6c\x6c\x6f` → "Hello")
- **CSS color functions** - parse `rgb()`, `rgba()`, `hsl()`, `hsla()`
- **Hexdump format** - traditional hex dump output for binary data
- **Hex/binary/octal integer conversions** - decimal 255 now shows `0xFF`, `0b11111111`, `0o377`
- **Structured metadata** for conversions - enables richer UI rendering (DataSize, Duration, Color, PacketLayout)
- **Conversion path tracking** - path now includes source format for full chain visibility

### Changed
- Removed UTF-8 as input parser (kept only for bytes→string conversion)
- JSON is now a terminal format (doesn't chain to further conversions)
- Removed plugin system (simplifies codebase)
- Improved duration parser with many more formats (spelled out units, decimals, weeks, years)

### Fixed
- Binary conversions no longer create nonsense chains
- Duplicate escape format removed (kept escape-hex/escape-unicode)
- Conversion paths now show full chain from source format

## [0.3.0] - 2024-12-22

### Added
- **Protobuf wire format decoder** - decode protobuf binary without a schema, showing field numbers, wire types, and values
- **Pretty-printing with colors** - JSON, MessagePack, and Protobuf output now has jq-style syntax highlighting (keys blue, strings green, numbers cyan)
- **ULID support** - parse ULIDs with embedded timestamp extraction
- **NanoID support** - detect 21-character URL-safe identifiers
- **CUID2 support** - detect CUID2 identifiers
- **JWT token support** - decode JWT header and payload, show claims and expiry
- **Hash detection** - identify MD5, SHA-1, SHA-256, SHA-512 by length
- **Windows FILETIME** - decode Windows 64-bit timestamps
- **Apple plist** - decode both XML and binary property lists
- **Binary format** - parse `0b1010`, `%1010`, and space-separated binary
- `--compact` (`-c`) flag for single-line JSON/Protobuf output
- `--no-color` (`-C`) flag to disable colored output
- `--raw` (`-r`) flag for scriptable output (just values, no labels)
- `--first` (`-1`) flag to show only the highest-confidence interpretation
- `--from` (`-f`) flag to force input to be interpreted as a specific format
- `--dot` flag to output conversion graph in Graphviz DOT format
- `--mermaid` flag to output conversion graph in Mermaid format
- `CoreValue::Protobuf` type for structured protobuf data (enables custom UI rendering)

### Changed
- Hex output now shows hash hints (e.g., "20 bytes - possible SHA-1 hash")
- Skip self-conversions (hex input no longer shows hex output)
- Smarter noise reduction: filter nonsensical conversions like IP→timestamp, UUID→msgpack
- Intelligent msgpack scoring based on decoded content type
- Raised minimum epoch thresholds to reduce false timestamp matches

### Fixed
- Reduced conversion noise in pipe mode output

## [0.2.0] - 2024-12-15

### Added
- Conversion priority sorting (structured data first, then semantic types, then encodings)
- `--limit` (`-l`) flag to control number of conversions shown (default: 5)
- Homebrew tap installation

## [0.1.0] - 2024-12-10

### Added
- Initial release
- Hex parsing (multiple input styles: continuous, space-separated, colon-separated, C array)
- Base64 encoding/decoding
- Unix epoch timestamps (seconds and milliseconds)
- Apple/Cocoa timestamps
- UUID parsing with version detection (v1-v8)
- IPv4 and IPv6 address parsing
- Color parsing (#RGB, #RRGGBB, #RRGGBBAA, 0xAARRGGBB)
- JSON parsing
- MessagePack decoding
- URL encoding/decoding
- UTF-8 string handling
- Pipe mode for log annotation
- Confidence scoring for interpretations
- Graph-based conversion discovery
