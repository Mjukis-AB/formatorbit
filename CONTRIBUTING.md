# Contributing to Formatorbit

Thank you for considering contributing to Formatorbit! This document covers development setup, testing, benchmarking, and guidelines for contributors.

For architecture details and coding conventions, see [CLAUDE.md](CLAUDE.md).

## Development Setup

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Python 3.9+ (optional, for plugin support)

### Building

```bash
# Clone the repository
git clone https://github.com/mjukis-ab/formatorbit
cd formatorbit

# Build all crates
cargo build

# Build release version
cargo build --release

# Run the CLI
cargo run -p formatorbit-cli -- "691E01B8"
```

### Project Structure

```
formatorbit/
├── crates/
│   ├── core/      # Main library: parsing, conversion, formats
│   ├── cli/       # Command-line interface (forb)
│   └── ffi/       # C/Swift bindings for GUI apps
├── CLAUDE.md      # Architecture & coding conventions
└── CONTRIBUTING.md # This file
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p formatorbit-core

# Run tests matching a pattern
cargo test -p formatorbit-core -- mac

# Run tests with output
cargo test -- --nocapture
```

### Snapshot Tests

The core crate uses [insta](https://insta.rs/) for snapshot testing. When you change output format:

```bash
# Run tests and review changes
cargo test -p formatorbit-core

# Review snapshot changes interactively
cargo insta review

# Or accept after careful review
cargo insta accept
```

**Important:** Never blindly accept snapshot changes. Review from an end-user perspective.

## Benchmarks

We use [Criterion](https://bheisler.github.io/criterion.rs/book/) for benchmarking. Benchmarks help catch performance regressions.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p formatorbit-core

# Run specific benchmark group
cargo bench -p formatorbit-core --bench benchmarks -- convert_all

# Quick benchmark (fewer iterations)
cargo bench -p formatorbit-core --bench benchmarks -- --quick

# Generate HTML report only (no terminal output)
cargo bench -p formatorbit-core --bench benchmarks -- --plotting-backend plotters
```

### Benchmark Results

Results are saved to `target/criterion/` with HTML reports. Open `target/criterion/report/index.html` to view.

### Benchmark Groups

| Group | Description |
|-------|-------------|
| `convert_all` | Full pipeline for various input types |
| `interpret` | Parsing phase only |
| `Formatorbit::new` | Instance creation overhead |
| `oui_lookup` | MAC address with vendor lookup |
| `convert_filtered` | `--only` flag use case |
| `throughput` | Varying input sizes |

### Performance Targets

- **Formatorbit::new**: < 1µs
- **Simple inputs**: < 500µs
- **Complex inputs** (JSON, protobuf): < 1ms

## Code Quality

### Formatting & Linting

```bash
# Check formatting
cargo fmt --check

# Apply formatting
cargo fmt

# Run clippy
cargo clippy --all-targets -- -D warnings
```

### Pre-commit Checklist

Before submitting a PR:

1. `cargo fmt` - Format code
2. `cargo clippy -- -D warnings` - No warnings
3. `cargo test` - All tests pass
4. `cargo bench -p formatorbit-core --bench benchmarks -- --quick` - No major regressions

## CI Pipeline

Our CI runs on every push and PR:

| Job | Description |
|-----|-------------|
| **Test** | Build and test on Linux, macOS, Windows |
| **Lint** | Format check and Clippy |
| **Binary Size** | Tracks release binary size (fails if > 20MB) |
| **Benchmarks** | Runs on main branch, uploads results as artifacts |

### Binary Size

The release binary is currently ~12MB. The CI will fail if it exceeds 20MB. Major size increases should be justified and discussed.

## Adding a New Format

1. Create `crates/core/src/formats/myformat.rs`
2. Implement the `Format` trait (see [CLAUDE.md](CLAUDE.md) for details)
3. Register in `crates/core/src/formats/mod.rs`
4. Add to format list in `crates/core/src/lib.rs`
5. Add tests
6. Update `CHANGELOG.md`

### Format Trait Methods

```rust
pub trait Format: Send + Sync {
    fn id(&self) -> &'static str;           // Unique ID: "myformat"
    fn name(&self) -> &'static str;         // Display name: "My Format"
    fn parse(&self, input: &str) -> Vec<Interpretation>;
    fn can_format(&self, value: &CoreValue) -> bool;
    fn format(&self, value: &CoreValue) -> Option<String>;
    fn conversions(&self, value: &CoreValue) -> Vec<Conversion>;
    fn aliases(&self) -> &'static [&'static str];  // Short names: ["mf"]
}
```

## Changelog

All changes should be documented in `CHANGELOG.md` under the `[Unreleased]` section. We follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.

Categories:
- **Added** - New features
- **Changed** - Changes to existing functionality
- **Fixed** - Bug fixes
- **Removed** - Removed features

## Questions?

- Open an issue for bugs or feature requests
- See [CLAUDE.md](CLAUDE.md) for architecture questions
- Check existing issues/PRs for similar topics
