# Format Ideas for Formatorbit

A collection of format ideas organized by developer domain.

## Web/Backend Developers

- [x] **JWT tokens** - Decode header.payload (without verification), show claims, expiry (implemented)
- [ ] **Unix permissions** - `755` → `rwxr-xr-x`
- [ ] **HTTP status codes** - `404` → "Not Found"
- [ ] **Cron expressions** - `*/5 * * * *` → "every 5 minutes"
- [ ] **Regex visualization** - Show what pattern matches, capture groups
- [ ] **ULID** - Universally Unique Lexicographically Sortable Identifier
- [ ] **KSUID** - K-Sortable Unique Identifier

## iOS/macOS Developers

- [x] **Apple/Cocoa timestamp** - Seconds since 2001-01-01 (implemented)
- [x] **Binary plist** - Decode Apple property list format (implemented)
- [x] **XML plist** - Decode Apple XML property list (implemented)
- [ ] **CFAbsoluteTime** - Core Foundation time
- [ ] **UTI** - Uniform Type Identifiers (`public.jpeg` → MIME type)
- [ ] **Bundle IDs** - Validate/parse reverse-DNS format

## Android/Mobile

- [ ] **Android resource IDs** - `0x7F0A0001` → type/name breakdown
- [ ] **Intent URIs** - Parse Android intent scheme
- [x] **ARGB colors** - `0xAARRGGBB` format (implemented)

## Hardware/Firmware/Embedded

- [ ] **IEEE 754 float** - Show bit layout (sign|exponent|mantissa)
- [x] **Binary literals** - `0b10101010` parsing and display (implemented)
- [ ] **Endian visualization** - Show BE/LE side by side
- [ ] **CRC detection** - Identify CRC-16, CRC-32 by length
- [ ] **MAC address OUI** - Vendor lookup from first 3 bytes
- [ ] **Bitmask visualization** - Show which bits are set

## Network/Security

- [ ] **ASN.1/DER** - Basic structure parsing
- [ ] **X.509 certificate** - Extract subject, issuer, dates
- [ ] **SSH key fingerprint** - Parse and show fingerprint
- [ ] **Port numbers** - `443` → "HTTPS", `22` → "SSH"
- [ ] **CIDR notation** - `192.168.1.0/24` → range, netmask
- [ ] **DNS record types** - Identify A, AAAA, CNAME, etc.

## Data Formats

- [x] **MessagePack** - Binary JSON-like format (implemented)
- [ ] **Protobuf** - Wire format decode (without schema, show field numbers)
- [ ] **BSON** - MongoDB binary JSON
- [ ] **CBOR** - Concise Binary Object Representation (IoT common)
- [ ] **Bencode** - BitTorrent encoding

## Timestamps

- [x] **Unix epoch** - Seconds since 1970-01-01 (implemented)
- [x] **Unix epoch millis** - Milliseconds since 1970 (implemented)
- [x] **Apple epoch** - Seconds since 2001-01-01 (implemented)
- [ ] **Windows FILETIME** - 100-nanosecond intervals since 1601-01-01
- [ ] **GPS time** - Seconds since 1980-01-06 (no leap seconds)
- [ ] **NTP timestamp** - Seconds since 1900-01-01
- [ ] **UUID v1 timestamp** - Extract timestamp from UUID v1
- [ ] **HFS+ timestamp** - Mac Classic (seconds since 1904-01-01)
- [ ] **WebKit/Chrome timestamp** - Microseconds since 1601-01-01

## Hashing/Encoding

- [x] **Hash detection** - Identify by length (32 hex=MD5, 40=SHA1, 64=SHA256) (implemented)
- [x] **Base64** - Standard base64 (implemented)
- [ ] **Base32** - RFC 4648
- [ ] **Base58** - Bitcoin/IPFS alphabet (no 0OIl)
- [ ] **Base85/Ascii85** - Used in PDF, Git
- [ ] **Punycode** - International domain names
- [ ] **HTML entities** - `&amp;` → `&`, `&#x2F;` → `/`
- [ ] **Quoted-printable** - Email encoding

## Already Implemented

- [x] Hexadecimal (multiple input formats)
- [x] Base64
- [x] Decimal integers
- [x] DateTime (ISO 8601, RFC 2822, RFC 3339)
- [x] JSON
- [x] UTF-8
- [x] UUID (with version detection)
- [x] IP addresses (IPv4, IPv6)
- [x] Colors (RGB, RGBA, ARGB, HSL)
- [x] URL encoding
- [x] MessagePack
- [x] Apple/Cocoa timestamps
- [x] Binary literals (0b prefix, % prefix, space-separated)
- [x] Apple plist (XML and binary formats)
- [x] JWT tokens (header/payload decode, claims display)
- [x] Hash detection (MD5, SHA-1, SHA-256, SHA-512, etc. by length)

## Priority Suggestions

High value, relatively easy to implement:
1. **JWT** - Extremely common, high utility
2. **IEEE 754 float bits** - Useful for firmware debugging
3. **Hash detection** - Simple length-based heuristic
4. **Windows FILETIME** - Common in forensics/Windows dev
5. **Base58** - Blockchain/crypto ecosystem
6. **Port numbers** - Quick reference lookup
