//! Benchmarks for formatorbit-core.
//!
//! Run with: `cargo bench -p formatorbit-core`
//!
//! Results are saved to `target/criterion/` with HTML reports.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use formatorbit_core::Formatorbit;

/// Benchmark inputs representing common use cases.
struct BenchmarkInputs {
    /// Simple hex string
    hex_simple: &'static str,
    /// Hex with prefix
    hex_prefixed: &'static str,
    /// UUID
    uuid: &'static str,
    /// IPv4 address
    ipv4: &'static str,
    /// MAC address
    mac_address: &'static str,
    /// Unix timestamp
    epoch: &'static str,
    /// JSON object
    json_small: &'static str,
    /// Base64 encoded data
    base64: &'static str,
    /// Mathematical expression
    expression: &'static str,
    /// ISO 8601 datetime
    datetime: &'static str,
    /// Plain text (low confidence matches)
    text: &'static str,
}

const INPUTS: BenchmarkInputs = BenchmarkInputs {
    hex_simple: "DEADBEEF",
    hex_prefixed: "0x691E01B8",
    uuid: "550e8400-e29b-41d4-a716-446655440000",
    ipv4: "192.168.1.1",
    mac_address: "00:1A:2B:3C:4D:5E",
    epoch: "1703456789",
    json_small: r#"{"name":"test","value":42}"#,
    base64: "SGVsbG8gV29ybGQh",
    expression: "2 + 2 * 3",
    datetime: "2024-01-15T10:30:00Z",
    text: "hello world",
};

/// Benchmark the full convert_all pipeline for various input types.
fn bench_convert_all(c: &mut Criterion) {
    let forb = Formatorbit::new();

    let mut group = c.benchmark_group("convert_all");

    // Test each input type
    let inputs = [
        ("hex_simple", INPUTS.hex_simple),
        ("hex_prefixed", INPUTS.hex_prefixed),
        ("uuid", INPUTS.uuid),
        ("ipv4", INPUTS.ipv4),
        ("mac_address", INPUTS.mac_address),
        ("epoch", INPUTS.epoch),
        ("json_small", INPUTS.json_small),
        ("base64", INPUTS.base64),
        ("expression", INPUTS.expression),
        ("datetime", INPUTS.datetime),
        ("text", INPUTS.text),
    ];

    for (name, input) in inputs {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("input", name), &input, |b, input| {
            b.iter(|| forb.convert_all(black_box(input)));
        });
    }

    group.finish();
}

/// Benchmark just the interpretation phase (parsing without conversion).
fn bench_interpret(c: &mut Criterion) {
    let forb = Formatorbit::new();

    let mut group = c.benchmark_group("interpret");

    let inputs = [
        ("hex_simple", INPUTS.hex_simple),
        ("uuid", INPUTS.uuid),
        ("ipv4", INPUTS.ipv4),
        ("mac_address", INPUTS.mac_address),
        ("json_small", INPUTS.json_small),
    ];

    for (name, input) in inputs {
        group.bench_with_input(BenchmarkId::new("input", name), &input, |b, input| {
            b.iter(|| forb.interpret(black_box(input)));
        });
    }

    group.finish();
}

/// Benchmark Formatorbit instance creation.
fn bench_initialization(c: &mut Criterion) {
    c.bench_function("Formatorbit::new", |b| {
        b.iter(|| Formatorbit::new());
    });
}

/// Benchmark OUI lookup specifically (important due to database size).
fn bench_oui_lookup(c: &mut Criterion) {
    use formatorbit_core::Formatorbit;

    let forb = Formatorbit::new();

    let mut group = c.benchmark_group("oui_lookup");

    // Various MAC addresses to test lookup distribution
    let macs = [
        ("cisco", "00:00:0C:12:34:56"),
        ("apple", "A4:83:E7:12:34:56"),
        ("unknown", "AA:BB:CC:DD:EE:FF"),
        ("broadcast", "FF:FF:FF:FF:FF:FF"),
    ];

    for (name, mac) in macs {
        group.bench_with_input(BenchmarkId::new("mac", name), &mac, |b, mac| {
            b.iter(|| forb.convert_all(black_box(mac)));
        });
    }

    group.finish();
}

/// Benchmark filtered conversion (common CLI use case with --only flag).
fn bench_convert_filtered(c: &mut Criterion) {
    let forb = Formatorbit::new();

    let mut group = c.benchmark_group("convert_filtered");

    // Common filter scenarios
    let cases = [
        ("hex_only", "DEADBEEF", vec!["hex".to_string()]),
        ("uuid_only", INPUTS.uuid, vec!["uuid".to_string()]),
        ("epoch_only", INPUTS.epoch, vec!["epoch".to_string()]),
    ];

    for (name, input, filter) in cases {
        group.bench_with_input(BenchmarkId::new("filter", name), &(input, filter), |b, (input, filter)| {
            b.iter(|| forb.convert_all_filtered(black_box(input), black_box(filter)));
        });
    }

    group.finish();
}

/// Benchmark throughput with varying input sizes.
fn bench_throughput(c: &mut Criterion) {
    let forb = Formatorbit::new();

    let mut group = c.benchmark_group("throughput");

    // Generate hex strings of various sizes
    let sizes = [8, 32, 128, 512];
    for size in sizes {
        let input: String = (0..size).map(|i| format!("{:02X}", i % 256)).collect();
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("hex_bytes", size), &input, |b, input| {
            b.iter(|| forb.convert_all(black_box(input)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_convert_all,
    bench_interpret,
    bench_initialization,
    bench_oui_lookup,
    bench_convert_filtered,
    bench_throughput,
);

criterion_main!(benches);
