# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.1] - 2026-01-07

### Fixed
- **ANSI escape codes in pipe mode** - colorized log output (e.g., from log colorization scripts) now works correctly. ANSI codes are stripped before tokenization so UUIDs, IPs, hex values etc. are properly detected.

## [0.9.0] - 2026-01-07

### Added
- **String reinterpretation in BFS** - when hex/base64 decodes to a string, automatically parse it as other formats:
  - Hex-encoded JSON: `7b22...` → `hex → utf8 → json`
  - Hex-encoded IP: `31 39 32 2e...` → `hex → utf8 → ipv4`
  - Hex-encoded UUID: `35 35 30 65...` → `hex → utf8 → uuid`
  - Configurable threshold via `--reinterpret-threshold` (default 0.7, set to 1.0 to disable)
- **xxd-style hex input** - space-separated multi-byte hex groups now supported:
  - `7b22 6865 6c6c 6f22` (2-byte groups from xxd default output)
  - `7b226865 6c6c6f22` (4-byte word groupings)
- **More date format parsing**:
  - ISO 8601 date-only: `2025-11-19` (95% confidence)
  - Asian/ISO with slashes: `2025/11/19` (90% confidence)
  - European dot format: `19.11.2025` (85% confidence)
  - Short dates without year: `11/19`, `25/2` (45-65% confidence, so expr wins for ambiguous cases)
- **crates.io publishing** - releases now automatically publish to crates.io

### Changed
- **Expression confidence now dynamic** - complex expressions like `5*9*3*9/23` now score 95% instead of fixed 60%
  - Multiple operators (`*`, `/`, `%`, `^`): 95%
  - Shifts/bitwise (`<<`, `>>`, `|`, `&`): 95%
  - Single multiply/divide: 85%
  - Addition/subtraction: 75%
  - Bare literals like `0xFF`: 50% (so hex wins)
- **Updated crate keywords** for better discoverability on crates.io

### Fixed
- **Large file performance** - 4MB image now processes in <1s (was 14s)
  - Skip expensive hashes (SHA-256+) for data >1MB
  - Avoid duplicate hash computation for binary files
  - Single interpretation for file/bytes input instead of redundant base64

## [0.8.0] - 2026-01-02

### Added
- **Natural language date/time parsing** - parse human-friendly date expressions:
  - Time of day: `15:00`, `3:30pm`, `9am` → datetime (today at that time)
  - Relative words: `now`, `today`, `tomorrow`, `yesterday`
  - Relative periods: `next week`, `last month`, `next year`
  - Weekdays: `monday`, `next friday`, `last tuesday`, `this wednesday`
  - Relative offsets: `in 2 days`, `3 weeks ago`, `a month from now`
  - Month + day: `15 dec`, `march 15th`, `jan 1`
  - Special dates: `christmas`, `halloween`, `new years`, `valentines`
  - Period boundaries: `end of month`, `eom`, `start of year`, `soy`
  - Quarters: `q1`, `q2`, `next quarter`, `last quarter`
  - Example: `forb "next friday"` → datetime with epoch conversions
- **RichDisplay::LiveClock** - special display type for "now" input enabling live-ticking clock in GUIs
- **Bytes and file APIs** - new FFI and core methods for binary data:
  - `convert_bytes(data)` - convert raw bytes directly (no base64 encoding needed)
  - `convert_bytes_filtered(data, formats)` - with format filter
  - `convert_file(path)` - read and convert file by path
  - `convert_file_from(path, format)` - with forced format
  - GUI apps can now pass `Data`/`[UInt8]` directly instead of pre-encoding
- **--graph flag** - show static format conversion graph without input data
  - `forb --graph --dot` - Graphviz DOT output of all format relationships
  - `forb --graph --mermaid` - Mermaid diagram output
- **RichDisplay enhancements** for GUI apps:
  - `RichDisplay::Color` on color interpretations (was only on conversions)
  - `RichDisplay::DateTime` with `epoch_millis` field for client-side relative time ticking
  - `RichDisplay::Duration` and `RichDisplay::DataSize` on interpretations
  - `RichDisplay::Epoch` with millisecond timestamps
- **Golden corpus tests** - comprehensive test suite for format interpretation confidence

### Changed
- **Hex confidence lowered** for short digit-only inputs like `15:00` (50%) so natural-date wins over hex
- **Geohash confidence refined** - measurements like `500cm` no longer match as coordinates
- **ISBN-10 confidence refined** - better detection with proper check digit validation
- **Duration confidence refined** - more accurate scoring based on input complexity

### Fixed
- **DOT/Mermaid reserved keywords** - `graph` format no longer collides with Graphviz/Mermaid syntax
- **DOT/Mermaid special characters** - proper escaping for quotes, backslashes, newlines
- **Graph format validation** - no longer accepts arbitrary strings as valid graph notation
- **Text→UUID blocking** - prevents duplicate checksum conversions

## [0.7.0] - 2025-12-31

### Added
- **Magic numbers and hexspeak** - recognize well-known hex values as traits on integers:
  - File signatures: `CAFEBABE` (Java class file), `FEEDFACE` (Mach-O)
  - Debug markers: `DEADBEEF`, `BAADF00D`, `DEADC0DE`, `FEE1DEAD`
  - Hexspeak words: `CAFE`, `BABE`, `BEEF`, `DEAD`, `FACE`, `FADE`, `FEED`, `C0DE`, `C0FFEE`
  - Example: `forb DEADBEEF` shows "✓ DEADBEEF (debug memory marker)"
- **Unix permissions format** - bidirectional octal ↔ symbolic conversion:
  - Parse octal: `755`, `0755`, `0o755`, `4755` (with setuid/setgid/sticky)
  - Parse symbolic: `rwxr-xr-x`, `rw-r--r--`, `drwxr-xr-x` (with optional type prefix)
  - Shows detailed breakdown: owner/group/other permissions with descriptions
- **CIDR notation format** - parse network ranges with detailed info:
  - Parse IPv4 CIDR: `192.168.1.0/24`, `10.0.0.0/8`, `172.16.0.0/12`
  - Parse IPv6 CIDR: `2001:db8::/32`
  - Shows: network, netmask, wildcard, broadcast, host range, usable host count
  - Detects private networks (RFC 1918) and network class
  - Handles edge cases: /31 (point-to-point), /32 (single host)
- **Well-known constants lookup** - bidirectional lookup for common developer constants:
  - **Number → Name**: integers show matching constants as traits (e.g., `443` → "HTTPS (port 443/tcp)")
  - **Name → Number**: parse constant names to values (e.g., `ssh` → 22, `ESC` → 27, `Not Found` → 404)
  - Includes: HTTP status codes (200, 404, 500...), ports (22, 80, 443, 3306...), Unix signals (SIGKILL, SIGTERM...), ASCII control chars (ESC, TAB, LF...), exit codes (137=SIGKILL, 139=SIGSEGV...)
- **Priority and blocking configuration** - customize conversion output ordering and filtering:
  - Category reordering: `[priority] category_order = ["Semantic", "Structured", ...]`
  - Per-format priority: `[priority.format_priority] datetime = 10` (bump up) or `ipv4 = "Primary"` (change category)
  - Format blocking: `[blocking] formats = ["octal", "binary"]`
  - Path blocking: `[blocking] paths = ["hex:msgpack", "uuid:epoch-seconds"]`
  - New `--show-paths` flag to display blockable paths for each conversion
- **Local usage analytics** - privacy-first tracking to help improve forb:
  - Enabled by default, stored in human-readable TOML at `~/.config/forb/analytics.toml`
  - Tracks format usage, conversion targets, and feature usage (pipe mode, file input, etc.)
  - No input data, filenames, or URLs are ever recorded
  - Disable via `FORB_ANALYTICS=0` or config file `[analytics] enabled = false`
  - Commands: `--analytics status`, `--analytics show`, `--analytics clear`
- **Anonymous contribution** - opt-in sharing of aggregate usage data:
  - `--analytics preview` - see exactly what would be sent
  - `--analytics contribute` - send anonymous data to TelemetryDeck
  - Fresh random UUID per contribution (no cross-session tracking)
  - Only aggregate counts: top formats, feature usage, CLI version, platform
- **Binary file metadata extraction** - detect and extract metadata from common binary formats:
  - **PDF** - pages, version, title, author, creation/modification dates, encryption status
  - **Audio** (MP3, FLAC, WAV, OGG, AAC) - duration, bitrate, sample rate, channels, ID3 tags (artist, album, title, year, genre)
  - **Video** (MP4, MKV, WebM) - duration, resolution, video/audio codecs, frame rate
  - **Office** (DOCX, XLSX, PPTX) - title, author, page/sheet/slide count, creation/modification dates
  - **Archive** (ZIP, TAR, GZIP) - file count, total/compressed size, compression ratio, file listing
  - **Font** (TTF, OTF, WOFF, WOFF2) - font family, style, version, glyph count
- **Configuration file support** - persistent settings in TOML config file
  - Location: `forb --config-path` (platform-specific: `~/.config/forb/` on Linux, `Library/Application Support/forb/` on macOS)
  - Generate default config: `forb --config-init`
  - Precedence: CLI args > Environment vars (`FORB_*`) > Config file > Defaults
  - Configurable: `limit`, `threshold`, `no_color`, `url_timeout`, `url_max_size`, `max_tokens`
  - Verbose mode (`-v`) logs which source each setting came from
  - Supports `NO_COLOR` standard (https://no-color.org/)
- **URL fetch limits** - configurable timeout and size limits for `@https://...` fetches
  - `--url-timeout <SECS>` - timeout in seconds (default: 30)
  - `--url-max-size <SIZE>` - max response size, e.g., `10M`, `50M`, `1G` (default: 10M)
  - Helpful error messages guide users to the relevant flag when limits are exceeded
- **Format validation errors** - helpful error messages when `--only` or `--from` fails to parse
  - `forb --only json '{bad'` → `error: Cannot parse as json: line 1, column 2: key must be a string`
  - `forb --only uuid 'not-uuid'` → `error: Cannot parse as uuid: invalid character...`
  - `forb --only hex '123'` → `error: Cannot parse as hex: odd number of hex digits (3)`
  - `forb --only ip '999.1.1.1'` → `error: Cannot parse as ip: invalid IPv4 address`
  - `forb --only color '#GGG'` → `error: Cannot parse as color: contains non-hex characters`
  - `forb --only msgpack 'data'` → `error: msgpack is a binary format - provide hex or base64`
  - `forb --only badformat 'x'` → `error: Unknown format 'badformat'. Use --formats to see available formats.`
  - Implemented for: json, uuid, hex, base64, plist, ip, color, msgpack, protobuf
- **Nautical miles** - length format now supports `nmi`, `NM`, `nautical mile(s)` (1852 meters)

### Changed
- **Reduced output noise** - significantly cleaner output for common inputs:
  - `DEADBEEF`: 211 → 58 lines (-72%), 4 → 2 interpretations
  - `test`: 147 → 17 conversions (-88%)
  - Traits now grouped on one line by default (use `-v` for separate lines with paths)
  - Hashes moved to bottom of output
- **Smarter format detection** - fewer false positives:
  - Hash: removed CRC-32 (8 chars) - too ambiguous for magic numbers
  - Base64: reject pure hex strings like `DEADBEEF`
  - Color: require `#` prefix for 6/8 char hex colors
  - Root-based blocking prevents nonsensical conversion chains (text→ipv4, hex→color)

### Fixed
- **Geohash false positives** - measurements like `500cm` no longer match as geohash coordinates
- **NaN panic in confidence sorting** - use `total_cmp()` instead of `partial_cmp().unwrap()` for safe float comparison
- **UTF-8 truncation panic** - string truncation in graph output now uses character count, not byte slicing
- **Conversion de-duplication** - now shows different values for the same format (e.g., both int-be and int-le epoch interpretations)
- **Currency rate cache retry** - library consumers in long-running processes can now recover from transient network failures (5-minute retry backoff)
- **Base64 detection** - strings that decode to valid UTF-8 (like `dGVzdGFz`) now correctly recognized as base64

## [0.6.0] - 2025-12-29

### Added
- **Hash digest calculation** - calculate CRC32, MD5, SHA-1, SHA-256, SHA-512, Blake2b-256, Blake3 from any byte data
  - Works on hex, base64, binary files, and text input
  - Digests appear as conversions from bytes
- **Plain text parsing** - any input now has a fallback "text" interpretation (10% confidence)
  - Enables hash calculation for arbitrary text (`forb "Hello"` → SHA-256, MD5, etc.)
  - Shows ASCII codes for short strings (decimal and hex)
  - ASCII/UTF-8 detection trait
- **Epoch timestamp parsing** - numeric timestamps now appear as top-level interpretation
  - Dynamic confidence based on proximity to current time (0.95 within a week, 0.87 within 30 years)
  - Epoch timestamps appear before decimal for recent dates
  - Support for microseconds and nanoseconds precision (in addition to seconds/milliseconds)
- **More datetime string formats** - parse additional date formats:
  - `12/28/2025 @ 10:41am` (US with time)
  - `Dec 28, 2025` / `December 28, 2025`
  - `28 Dec 2025` / `28 December 2025`
  - Ambiguous `01/02/2025` returns both US and European interpretations
- **DateTime → epoch conversions** - parsed datetime strings now show epoch-seconds, epoch-millis, and relative time
- **ASCII/Unicode character display** - integers show their character representation:
  - Printable ASCII (32-126): `65` → `'A'`
  - Control characters (0-31): `10` → `'\u{a}' LF (line feed)`
  - Unicode: `128512` → `'😀' (U+1F600)`
- **Unicode character parsing** - single characters/emojis parsed with codepoint breakdown:
  - Simple: `🤑` → `U+1F911 '🤑'` with decimal, hex, UTF-8 bytes
  - Composite emojis show full breakdown:
    - `👨‍👩‍👧‍👦` → 👨 + ZWJ + 👩 + ZWJ + 👧 + ZWJ + 👦 (7 codepoints, 25 bytes)
    - `🏳️‍🌈` → 🏳 + VS16 + ZWJ + 🌈 (4 codepoints)
    - `👍🏽` → 👍 + medium skin tone (2 codepoints)

### Fixed
- Overflow panic when processing very large integers (triangular number check, duration conversion)
- Truncate hex/base64 output for large binary data to prevent massive output

## [0.5.2] - 2025-12-26

### Added
- **CLI conversion kind symbols** - visual distinction between conversion types:
  - `→` Conversion (cyan) - actual transformation (metric → imperial)
  - `≈` Representation (blue) - same value, different notation (256 → 0x100)
  - `✓` Trait (magenta) - property of the value (power-of-2, prime)
- **Unit format improvements** - all unit formats now:
  - Show decimal base unit as Primary result (first conversion)
  - Correctly distinguish Conversion vs Representation kinds
- **Expression hex/binary/octal** - math expressions now show integer representations

### Changed
- `display_only` field hidden from JSON API (internal implementation detail)
- `--help` and README updated to document conversion kind symbols

### Fixed
- **Base64/NanoID false positives** - camelCase identifiers like `aspectButton` no longer match

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
