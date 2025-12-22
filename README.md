# forb

**Paste data, see what it is.** A CLI tool that automatically detects and converts data between formats.

```
$ forb 691E01B8

▶ hex (92% confidence)
  4 bytes
  → int-be: 1763574200
  → int-le: 3087081065
  → epoch-seconds: 2025-11-19T17:43:20+00:00
  → base64: aR4BuA==
  → ip: 105.30.1.184
```

## Why forb?

Ever paste a blob of hex into a dozen different tools trying to figure out what it is? `forb` does it all at once. It tries every interpretation and shows you what makes sense.

- **Hex dump from a debugger?** See it as integers, timestamps, base64
- **Random UUID in logs?** Instantly see which version and extract the timestamp
- **Timestamp that could be seconds or milliseconds?** See both interpretations
- **Space-separated hex bytes?** Just paste them directly

## Installation

### Homebrew (macOS/Linux)

```bash
brew tap mjukis-ab/tap
brew install forb
```

### Cargo (Rust)

```bash
cargo install formatorbit-cli
```

### From Source

```bash
git clone https://github.com/mjukis-ab/formatorbit
cd formatorbit
cargo build --release
# Binary is at target/release/forb
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/mjukis-ab/formatorbit/releases).

## Usage

### Direct Input

```bash
# Hex (multiple formats supported)
forb 691E01B8
forb 0x691E01B8
forb "69 1E 01 B8"
forb "69:1E:01:B8"
forb "{0x69, 0x1E, 0x01, 0xB8}"

# Base64
forb aR4BuA==

# Timestamps
forb 1703456789
forb 2024-01-15T10:30:00Z

# UUIDs
forb 550e8400-e29b-41d4-a716-446655440000

# IP addresses
forb 192.168.1.1
forb "::1"

# Colors
forb '#FF5733'
forb 0x80FF5733
```

### Pipe Mode

Pipe logs through `forb` to automatically annotate interesting values:

```bash
cat server.log | forb
```

```
[2024-01-15 10:30:45] User 550e8400-e29b-41d4-a716-446655440000 logged in
                           ↳ uuid: UUID v4 (random) → hex: 550E8400E29B41D4A716446655440000

[2024-01-15 10:30:46] Received payload: 69 1E 01 B8
                                        ↳ hex: 4 bytes → int-be: 1763574200, epoch: 2025-11-19T17:43:20Z
```

#### Pipe Mode Options

```bash
# Lower threshold to catch more matches (default: 0.8)
cat logs.txt | forb -t 0.5

# Highlight matched values inline
cat logs.txt | forb -H

# Only look for specific formats
cat logs.txt | forb -o uuid,hex,ts

# JSON output for scripting
cat logs.txt | forb -j
```

### Output Formats

```bash
# Human-readable (default)
forb 691E01B8

# JSON for scripting
forb 691E01B8 --json

# List all supported formats
forb --formats
```

## Supported Formats

| Category | Formats |
|----------|---------|
| **Encoding** | hex, base64, url-encoding |
| **Numbers** | decimal, big-endian int, little-endian int |
| **Timestamps** | Unix epoch (sec/ms), Apple/Cocoa epoch, ISO 8601, RFC 2822 |
| **Identifiers** | UUID (v1-v8 detection) |
| **Network** | IPv4, IPv6 |
| **Colors** | #RGB, #RRGGBB, #RRGGBBAA, 0xAARRGGBB (Android) |
| **Data** | JSON, MessagePack, UTF-8 |

### Hex Input Styles

`forb` accepts hex in many common formats:

```
691E01B8                    Continuous
0x691E01B8                  With 0x prefix
69 1E 01 B8                 Space-separated (hex dumps)
69:1E:01:B8                 Colon-separated (MAC address)
69-1E-01-B8                 Dash-separated
0x69, 0x1E, 0x01, 0xB8      Comma-separated
{0x69, 0x1E, 0x01, 0xB8}    C/C++ array style
```

### Format Aliases

For quick filtering with `--only`, formats have short aliases:

| Format | Aliases |
|--------|---------|
| hex | h, x |
| base64 | b64 |
| datetime | ts, time, date |
| decimal | dec, int, num |
| uuid | guid |
| ip | ipv4, ipv6 |
| color | col, rgb, argb |
| json | j |
| url-encoded | url, percent |
| msgpack | mp, mpack |

## Examples

### Debugging Binary Data

```bash
$ forb "69 1E 01 B8"

▶ hex (92% confidence)
  4 bytes (space-separated)
  → int-be: 1763574200
  → int-le: 3087081065
  → epoch-seconds: 2025-11-19T17:43:20+00:00
  → base64: aR4BuA==
  → ip: 105.30.1.184
  → color: #691E01B8
```

### Identifying UUIDs

```bash
$ forb 550e8400-e29b-41d4-a716-446655440000

▶ uuid (95% confidence)
  UUID v4 (random)
  → uuid: 550e8400-e29b-41d4-a716-446655440000
  → hex: 550E8400E29B41D4A716446655440000
  → base64: VQ6EAOKbQdSnFkRmVUQAAA==
```

### Decoding Timestamps

```bash
$ forb 1703456789

▶ decimal (85% confidence)
  Integer: 1703456789
  → epoch-seconds: 2023-12-24T23:06:29+00:00
  → epoch-millis: 1970-01-20T17:17:36.789+00:00
  → hex: 6588C555
```

### Analyzing Colors

```bash
$ forb '#FF5733'

▶ color-hex (95% confidence)
  RGB: RGB(255, 87, 51) / HSL(11°, 100%, 60%)
  → color-rgb: rgb(255, 87, 51)
  → color-hsl: hsl(11°, 100%, 60%)
  → color-0x: 0xFF5733
```

### Processing Logs

```bash
$ echo '[INFO] Request from 192.168.1.100 with ID 550e8400-e29b-41d4-a716-446655440000' | forb -t 0.5

[INFO] Request from 192.168.1.100 with ID 550e8400-e29b-41d4-a716-446655440000
                    ↳ ipv4: ip: 192.168.1.100, hex: C0A80164
                                               ↳ uuid: UUID v4 (random) → hex: 550E8400E29B41D4A716446655440000
```

## How It Works

1. **Parse**: Try all format parsers on the input
2. **Rank**: Sort interpretations by confidence score
3. **Convert**: For each interpretation, find all possible conversions via graph traversal
4. **Display**: Show results with the most likely interpretation first

The confidence score (0-100%) indicates how likely each interpretation is:
- **90%+**: Strong indicators (0x prefix, UUID dashes, base64 padding)
- **70-90%**: Plausible match (valid hex chars, reasonable timestamp range)
- **<70%**: Possible but less certain

## License

MIT

## Contributing

Contributions welcome! See [CLAUDE.md](CLAUDE.md) for architecture details and coding conventions.
