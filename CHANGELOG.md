# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
