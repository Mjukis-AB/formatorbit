# Feature Ideas for Formatorbit

Ideas for future development, organized by effort and value.

**Current state (2026-01):** 45 formats, rich display system, plugin architecture, OUI database

---

## Quick Wins (< 1 hour)

| Feature | Description | Example | Value |
|---------|-------------|---------|-------|
| **Base32** | RFC 4648 encoding | `JBSWY3DPEHPK3PXP` → "Hello..." | TOTP secrets, onion addresses |
| **Base32hex** | RFC 4648 extended hex | `91IMOR3F41BMUSJC` → bytes | DNS, DNSSEC |
| **z-base-32** | Human-friendly variant | Used in Mnet, some P2P | Avoids confusing chars |
| **HTML entities** | Entity decoding | `&amp;` → `&`, `&#x2F;` → `/` | Web development |
| **Punycode/IDN** | Domain encoding | `münchen.de` ↔ `xn--mnchen-3ya.de` | Internationalized domains |
| **Quoted-Printable** | Email encoding | `=20` → space, soft line breaks | MIME messages |
| **Roman numerals** | Bidirectional | `MCMXCIV` ↔ `1994` | Fun, occasionally useful |
| **Base85/Ascii85** | Higher density encoding | `<~87cURD]j~>` → bytes | PDF, Git binary patches |
| **Z85** | ZeroMQ variant | `HelloWorld` → bytes | Binary in JSON/XML |

*Note: Base16 is equivalent to hex (already supported). Base64 is already supported.*

---

## Medium Effort (2-4 hours)

| Feature | Description | Example | Value |
|---------|-------------|---------|-------|
| **Semver parsing** | Version analysis | `1.2.3-beta.1+build.456` → components, comparisons | Dev tooling |
| **IEEE 754 float bits** | Bit-level breakdown | `3.14` → sign, exponent, mantissa visualization | Educational |
| **Regex explanation** | Pattern breakdown | `\d{3}-\d{4}` → human description | Learning tool |
| **KSUID** | K-Sortable IDs | `1srOrx2ZWZBpBUvZwXKQmoEYga2` → timestamp + payload | Segment-style IDs |
| **Snowflake IDs** | Twitter/Discord IDs | Extract timestamp, worker, sequence | Social platform debugging |
| **Phone numbers** | E.164 parsing | `+1-555-123-4567` → country, region, type | International formatting |
| **IBAN validation** | Bank account numbers | `DE89 3704 0044 0532 0130 00` → country, checksum, bank | Financial |
| **Credit card BIN** | Issuer detection | `4111...` → Visa, `5500...` → Mastercard | Payment debugging |

---

## Higher Effort (4+ hours)

| Feature | Description | Notes |
|---------|-------------|-------|
| **X.509 certificates** | Parse PEM/DER certs | Subject, issuer, validity, SANs, fingerprint |
| **SSH public keys** | Parse authorized_keys | Algorithm, fingerprint, comment |
| **PGP/GPG keys** | Key info extraction | Key ID, user IDs, creation date |
| **ASN.1/DER viewer** | Generic structure | Foundation for crypto formats |
| **QR code decoding** | From image bytes | Decode embedded data |
| **DNS lookup** | Live resolution | `example.com` → A, AAAA, MX, TXT records |
| **WHOIS lookup** | Domain info | Registrar, expiration, nameservers |
| **GeoIP lookup** | IP → location | Country, city, ASN (needs database) |

---

## Additional Timestamp Epochs

| Format | Epoch | Notes |
|--------|-------|-------|
| **NTP timestamp** | 1900-01-01 | 64-bit, network protocols |
| **GPS time** | 1980-01-06 | No leap seconds |
| **WebKit/Chrome** | 1601-01-01 | Microseconds, Chrome/SQLite |
| **LDAP/AD** | 1601-01-01 | 100-nanosecond intervals |
| **.NET DateTime** | 0001-01-01 | Ticks (100ns), very large numbers |

---

## Binary Format Metadata

| Format | What to Extract |
|--------|-----------------|
| **SQLite** | Tables, row counts, page size |
| **Parquet** | Schema, row groups, compression |
| **WASM** | Imports, exports, memory, sections |
| **ELF** | Architecture, sections, entry point |
| **Mach-O** | Architecture, load commands, dylibs |
| **PE/COFF** | Architecture, sections, imports |
| **Java class** | Version, class name, methods |
| **APK/IPA** | Package name, version, permissions |

---

## CLI Enhancements

| Feature | Description | Example |
|---------|-------------|---------|
| **`--to <format>`** | Force output format | `forb --to base64 "hello"` |
| **`--watch`** | Monitor file changes | `forb --watch config.json` |
| **`--clipboard`** | Read from clipboard | `forb -C` (macOS/Linux/Windows) |
| **Interactive REPL** | Live exploration | `forb -i` with history, tab completion |
| **Batch mode** | Multiple inputs | `forb -b file1.bin file2.bin` |
| **Diff mode** | Compare interpretations | `forb --diff old.bin new.bin` |

---

## Plugin Ideas

| Plugin | Description |
|--------|-------------|
| **AWS ARN parser** | Extract service, region, account, resource |
| **Kubernetes resource** | Parse k8s object references |
| **Terraform state** | Extract resource info |
| **Docker image refs** | Registry, repo, tag, digest |
| **Git object IDs** | Commit, tree, blob detection |
| **npm/PyPI packages** | Parse package specifiers |

---

## Architecture Improvements

| Improvement | Description |
|-------------|-------------|
| **Streaming input** | Handle large files without loading entirely |
| **Async conversions** | Non-blocking network lookups (DNS, GeoIP) |
| **Format dependencies** | Lazy-load heavy formats (image, audio) |
| **WASM plugins** | Cross-platform plugin support |
| **Conversion hints** | "To convert back: `forb -f hex`" |

---

## Rich Display Extensions

The RichDisplay system could support:

| Display Type | Use Case |
|--------------|----------|
| **Diff view** | Side-by-side comparison |
| **Hex editor view** | Byte-level with highlighting |
| **Timeline** | Multiple timestamps on axis |
| **Network diagram** | IP relationships, CIDR overlaps |
| **QR code render** | Generate from data |

---

## Priority Recommendation

### Next Up (Encodings)
1. **Base32** - TOTP secrets, Tor onion addresses
2. **Base85/Ascii85** - PDF streams, Git patches
3. **Punycode** - International domain names

### Soon (Developer Tools)
1. Semver parsing
2. Snowflake IDs (Discord/Twitter)
3. `--to` output format

### Later
1. X.509 certificates
2. DNS/WHOIS lookups
3. Binary format metadata (ELF, Mach-O)
4. Interactive REPL mode
