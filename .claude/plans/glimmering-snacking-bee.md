# Plan: Constants Format (Bidirectional Lookup)

## Overview

Add a `ConstantsFormat` that provides bidirectional lookup for well-known constants:
- **Number → Name**: Show as trait on integers (e.g., `443` → "HTTPS port")
- **Name → Number**: Parse strings (e.g., `ESC` → 27, `ssh` → 22)

## Design Decisions

1. **Single format** - One `ConstantsFormat`, consistent with other formats (like `ipaddr` handles both v4/v6)
2. **Curated lists** - Only "greatest hits" that developers commonly look up
3. **Skip ambiguous values** - Don't show traits for 0, 1, 2 etc. that are overloaded
4. **Single output line** - Group all matches into one `✓ constants:` trait
5. **Standard blocking** - `formats = ["constants"]` to disable, no sub-domain granularity

## Constant Domains

### HTTP Status Codes
**Parse:** `"OK"`, `"Not Found"`, `"Internal Server Error"`
**Trait:** 200, 201, 204, 301, 302, 304, 400, 401, 403, 404, 405, 500, 502, 503, 504

### Well-Known Ports
**Parse:** `"ssh"`, `"https"`, `"mysql"`, `"redis"`
**Trait:** 22, 25, 53, 80, 443, 3000, 3306, 5432, 6379, 8080, 8443, 27017

### Unix Signals (skip 1, 2 - too common)
**Parse:** `"SIGKILL"`, `"SIGTERM"`, `"SIGSEGV"`
**Trait:** 9, 11, 15, 19

### ASCII Control Characters
**Parse:** `"ESC"`, `"TAB"`, `"LF"`, `"CR"`, `"CRLF"`, `"NUL"`, `"BEL"`, `"DEL"`
**Trait:** 7, 8, 9, 10, 13, 27, 127

## Output Examples

```
443
├── decimal: 443
└── ✓ constants: HTTPS (port 443/tcp)

404
├── decimal: 404
└── ✓ constants: HTTP 404 Not Found

ssh
├── constants: port 22/tcp (SSH)

ESC
├── constants: ASCII 27 (0x1B)
```

## Files to Modify

| File | Action |
|------|--------|
| `crates/core/src/formats/constants.rs` | Create |
| `crates/core/src/formats/mod.rs` | Register |
| `crates/core/src/lib.rs` | Add to format list |
| `CHANGELOG.md` | Document |

## Implementation Notes

- Use `ConversionKind::Trait` for integer→name
- Use `ConversionPriority::Semantic`
- Set `display_only: true` to prevent BFS expansion
- Case-insensitive string matching for parse
- RichDisplay::KeyValue for UI clients
