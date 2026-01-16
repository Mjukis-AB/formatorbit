# Feature Ideas for Formatorbit

This document tracks potential features and improvements identified during codebase review.

## Current State (as of 2025-12-30)

- **48 formats** implemented
- **15 RichDisplay variants** for UI rendering
- Well-architected with clear separation of concerns

---

## Quick Wins (< 1 hour each)

| Feature | Description | Example | Value |
|---------|-------------|---------|-------|
| **Port numbers** | Well-known port lookup | `443` → "HTTPS", `22` → "SSH", `3306` → "MySQL" | Very common lookup |
| **HTTP status codes** | Status code descriptions | `404` → "Not Found", `502` → "Bad Gateway" | Developers use constantly |
| **Unix permissions** | Octal ↔ symbolic | `755` ↔ `rwxr-xr-x`, `644` ↔ `rw-r--r--` | Common sysadmin task |
| **CIDR notation** | Network range parsing | `192.168.1.0/24` → range, netmask, broadcast, host count | Network debugging |
| **Base32** | RFC 4648 encoding | `JBSWY3DPEHPK3PXP` ↔ bytes | Common encoding gap |
| **HTML entities** | Entity decoding | `&amp;` → `&`, `&#x2F;` → `/`, `&nbsp;` → ` ` | Web development |

---

## Medium Effort (2-4 hours each)

| Feature | Description | Example | Value |
|---------|-------------|---------|-------|
| **IEEE 754 visualization** | Float bit breakdown | `3.14` → sign(0) exp(10000000) mantissa(1001...) | Educational/debugging |
| **MAC address OUI** | Vendor lookup | `00:1A:2B:...` → "Apple, Inc." | Network troubleshooting |
| **Cron expressions** | Human-readable cron | `*/5 * * * *` → "every 5 minutes" | Devops utility |
| **Semver parsing** | Version analysis | `1.2.3-beta.1` → major, minor, patch, prerelease | Dev tooling |
| **Regex explanation** | Pattern breakdown | `\d{3}-\d{4}` → "3 digits, hyphen, 4 digits" | Learning/debugging |
| **KSUID** | K-Sortable IDs | Similar to ULID, used by Segment | Identifier detection |

---

## Higher Effort (4+ hours)

| Feature | Description | Notes |
|---------|-------------|-------|
| **X.509 certificate parsing** | Extract subject, issuer, validity, SANs | Useful for TLS debugging |
| **Regex visualization** | Test patterns, highlight matches, show capture groups | Complex but very useful |
| **ASN.1/DER parsing** | Basic structure parsing | Foundation for cert/key parsing |
| **SSH key fingerprints** | Parse and display key info | Common security task |
| **DNS record parsing** | A, AAAA, MX, CNAME, TXT | Network debugging |
| **TLD/Domain parsing** | Extract TLD, domain parts, public suffix | `foo.co.uk` → TLD: `co.uk`, domain: `foo` |
| **DNS lookup** | Resolve domain to IP, MX, TXT records | `example.com` → A: `93.184.216.34` |
| **WHOIS lookup** | Domain registration info | Registrar, expiration, nameservers |

---

## Additional Timestamp Formats

| Format | Epoch | Notes |
|--------|-------|-------|
| **NTP timestamp** | 1900-01-01 | 64-bit, used in network protocols |
| **GPS time** | 1980-01-06 | No leap seconds, used in GPS |
| **HFS+ timestamp** | 1904-01-01 | Classic Mac, still in some files |
| **WebKit/Chrome** | 1601-01-01 | Microseconds, used in Chrome/SQLite |
| **LDAP/AD timestamp** | 1601-01-01 | 100-nanosecond intervals |

---

## Additional Encodings

| Encoding | Description | Use Case |
|----------|-------------|----------|
| **Base32** | RFC 4648 | TOTP secrets, some APIs |
| **Base85/Ascii85** | Higher density | PDF, Git binary patches |
| **Quoted-Printable** | Email encoding | MIME messages |
| **Punycode** | IDN domains | `münchen.de` → `xn--mnchen-3ya.de` |
| **Z85** | ZeroMQ variant | Binary in JSON |

---

## CLI Enhancements

| Feature | Description | Example |
|---------|-------------|---------|
| **`--to <format>`** | Force output format | `forb --to base64 "hello"` |
| **Batch processing** | Multiple files | `forb *.bin` or `forb -b file1 file2` |
| **Watch mode** | Monitor changes | `forb --watch data.json` |
| **Fuzzy matching** | Partial format names | `forb -f time` matches `datetime` |
| **Clipboard integration** | Read/write clipboard | `forb --clipboard` or `forb -C` |
| **Interactive mode** | REPL for exploration | `forb -i` with history |

---

## Binary Format Extensions

| Format | Metadata to Extract |
|--------|---------------------|
| **SQLite** | Tables, row counts, schema version |
| **Parquet** | Schema, row groups, compression |
| **Avro** | Schema, record count |
| **WASM** | Imports, exports, memory size |
| **ELF/Mach-O/PE** | Architecture, sections, symbols |
| **DWARF debug** | Source file references |

---

## Conversion Improvements

| Improvement | Description |
|-------------|-------------|
| **Confidence explanation** | Show why confidence is X% |
| **Conversion cost** | Show "lossy" warnings more prominently |
| **Alternative paths** | Show multiple ways to reach same result |
| **Inverse conversion** | "To convert back: `forb -f X`" hints |

---

## Priority Recommendation

### Phase 1: Quick Wins
1. Port numbers
2. HTTP status codes
3. Unix permissions
4. CIDR notation
5. Base32

### Phase 2: Developer Tools
1. Cron expressions
2. Semver parsing
3. Regex explanation
4. IEEE 754 visualization

### Phase 3: Network/Security
1. MAC address OUI lookup
2. X.509 certificates
3. SSH key fingerprints
4. Additional timestamp formats

### Phase 4: CLI Polish
1. `--to` output format
2. Batch processing
3. Clipboard integration
4. Interactive mode
