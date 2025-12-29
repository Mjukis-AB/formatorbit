mod pipe;
mod pretty;
mod tokenizer;

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

use base64::Engine;
use clap::Parser;
use colored::{control::set_override, Colorize};
use formatorbit_core::{ConversionKind, CoreValue, Formatorbit, RichDisplay, RichDisplayOption};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

use crate::pretty::{PacketMode, PrettyConfig};

const LONG_ABOUT: &str = r##"
Formatorbit automatically detects and converts data between formats.

Paste in data and see all possible interpretations - hex, base64, timestamps,
UUIDs, colors, math expressions, currencies, units, and more.

SUPPORTED FORMATS:
  Encoding:     hex, base64, binary, octal, url-encoding, escape sequences
  Numbers:      decimal, data sizes (1MB, 1MiB), temperature (30°C, 86°F)
  Math:         expressions (2 + 2, 0xFF + 1, 1 << 8)
  Units:        length, weight, volume, speed, pressure, energy, angle, area
  Currency:     100 USD, $50, 5kEUR, 2.5MSEK (with live exchange rates)
  Time:         Unix epoch (sec/ms), durations (1h30m), ISO 8601
  Hashing:      MD5, SHA-1, SHA-256, SHA-512, Blake2b, Blake3, CRC32
  Identifiers:  UUID (v1-v8), ULID, NanoID, CUID2, JWT
  Network:      IPv4, IPv6
  Coordinates:  DD, DMS, DDM, Geohash, Plus Code, UTM, MGRS
  Colors:       #RGB, rgb(), hsl(), 0xAARRGGBB (Android)
  Text:         plain text, ASCII codes, UTF-8 detection
  Data:         JSON, MessagePack, Protobuf, plist
  Images:       JPEG, PNG, GIF, WebP, BMP, TIFF (with EXIF metadata)

EXAMPLES:
  forb 691E01B8                 Interpret hex bytes
  forb aR4BuA==                 Decode base64
  forb "Hello"                  Text → SHA-256, MD5, base64, hex
  forb 1703456789               Unix timestamp (shows relative time)
  forb '0xFF + 1'               Evaluate expression
  forb 1h30m                    Parse duration
  forb 100USD                   Convert currency
  forb 5m                       Convert length units
  forb 30C                      Convert temperature
  forb 'rgb(35, 50, 35)'        Parse CSS color
  forb '59.3293, 18.0686'       Convert coordinates

FILE/URL INPUT:
  Use @path to read file contents or fetch URLs (like curl):
    forb @photo.jpg             Read and analyze image file
    forb @data.bin              Read binary file
    forb @-                     Read from stdin (binary)
    forb @https://example.com/image.png   Fetch and analyze URL

OUTPUT:
  Shows all possible interpretations ranked by confidence.
  Conversions sorted by usefulness (structured data first).
  Use -l to change how many conversions are shown (default: 5, use -l 0 for all).
  Use --formats to see all supported formats and aliases.

  Conversion symbols:
    →  Conversion     Actual transformation (e.g., metric → imperial)
    ≈  Representation Same value, different notation (e.g., 256 → 0x100)
    ✓  Trait          Property of the value (e.g., power-of-2, prime)

PIPE MODE:
  Pipe logs through forb to annotate interesting values:
    cat server.log | forb              Annotate log lines
    cat server.log | forb -t 0.5       Lower confidence threshold
    cat server.log | forb -H           Highlight matches inline
    cat server.log | forb -o uuid,hex  Only look for specific formats"##;

#[derive(Parser)]
#[command(name = "forb")]
#[command(version)]
#[command(about = "Automatically detect and convert data between formats")]
#[command(long_about = LONG_ABOUT)]
#[command(after_help = "For more information, visit: https://github.com/formatorbit/formatorbit")]
struct Cli {
    /// The input data to interpret and convert
    ///
    /// Can be hex, base64, timestamps, UUIDs, IP addresses, colors, etc.
    /// Hex input supports multiple formats: continuous, space-separated,
    /// colon-separated, C array style, and more.
    ///
    /// Use @path to read from a file or URL (like curl):
    ///   forb @image.jpg      Read image file
    ///   forb @data.bin       Read binary file
    ///   forb @-              Read from stdin
    ///   forb @https://...    Fetch from URL
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    /// Output results as JSON (for scripting/piping)
    #[arg(long, short = 'j')]
    json: bool,

    /// List all supported formats
    #[arg(long)]
    formats: bool,

    // === Pipe mode options ===
    /// Minimum confidence threshold for showing annotations (0.0-1.0)
    ///
    /// In pipe mode, only values with confidence >= threshold are annotated.
    #[arg(long, short = 't', default_value = "0.8")]
    threshold: f32,

    /// Highlight interesting values inline with color
    ///
    /// In pipe mode, highlights matched tokens with background color.
    #[arg(long, short = 'H')]
    highlight: bool,

    /// Only use specific formats (comma-separated, supports aliases)
    ///
    /// Examples: --only uuid,hex,ts  or  -o b64,ip
    /// Use --formats to see available format IDs and aliases.
    #[arg(long, short = 'o', value_delimiter = ',')]
    only: Option<Vec<String>>,

    /// Maximum conversions to show per interpretation (0 = unlimited)
    ///
    /// With priority sorting, the most valuable conversions come first.
    #[arg(long, short = 'l', default_value = "5")]
    limit: usize,

    /// Force pipe mode even when stdin is a TTY (for testing)
    #[arg(long, hide = true)]
    force_pipe: bool,

    /// Maximum tokens to analyze per line in pipe mode
    #[arg(long, default_value = "50", hide = true)]
    max_tokens: usize,

    /// Disable colored output
    #[arg(long, short = 'C')]
    no_color: bool,

    /// Compact output for structured data (single line)
    #[arg(long, short = 'c')]
    compact: bool,

    /// Output only raw converted values (for scripting)
    ///
    /// Outputs just the conversion values without labels or formatting.
    /// Combine with --only to get specific format output.
    #[arg(long, short = 'r')]
    raw: bool,

    /// Show only the highest-confidence interpretation
    #[arg(long, short = '1')]
    first: bool,

    /// Force input to be interpreted as a specific format
    ///
    /// Skip auto-detection and treat input as the specified format.
    /// Use --formats to see available format IDs.
    #[arg(long, short = 'f', value_name = "FORMAT")]
    from: Option<String>,

    /// Output conversion graph in Graphviz DOT format
    ///
    /// Pipe to dot to render: forb --dot INPUT | dot -Tpng > graph.png
    #[arg(long)]
    dot: bool,

    /// Output conversion graph in Mermaid format (renders in GitHub/GitLab)
    #[arg(long)]
    mermaid: bool,

    /// Show packet layout for binary formats (protobuf, msgpack)
    ///
    /// Displays byte-level structure with offsets, lengths, and decoded values.
    /// Use --packet=compact for inline horizontal format or --packet=detailed for table format.
    #[arg(long, short = 'p', value_name = "MODE", num_args = 0..=1, default_missing_value = "compact")]
    packet: Option<String>,

    /// Enable verbose logging (use multiple times for more detail)
    ///
    /// -v shows debug messages, -vv shows trace messages.
    /// Useful for understanding why something was or wasn't matched.
    #[arg(long, short = 'v', action = clap::ArgAction::Count)]
    verbose: u8,
}

fn print_formats() {
    let forb = Formatorbit::new();
    let infos = forb.format_infos();

    println!("{}", "Supported Formats".bold().underline());
    println!();

    // Group by category
    let categories = [
        "Encoding",
        "Numbers",
        "Math",
        "Units",
        "Timestamps",
        "Time",
        "Hashing",
        "Identifiers",
        "Network",
        "Colors",
        "Data",
    ];

    for category in categories {
        let formats_in_cat: Vec<_> = infos.iter().filter(|f| f.category == category).collect();

        if formats_in_cat.is_empty() {
            continue;
        }

        println!("{} {}", "▶".blue(), category.green().bold());
        for info in formats_in_cat {
            print!("  {} {}", "→".cyan(), info.id.yellow());
            if !info.description.is_empty() {
                print!(" - {}", info.description);
            }
            println!();
            if !info.examples.is_empty() {
                let examples: Vec<_> = info
                    .examples
                    .iter()
                    .take(3)
                    .map(|e| e.green().to_string())
                    .collect();
                println!("      {}", format!("e.g. {}", examples.join(", ")).dimmed());
            }
        }
        println!();
    }

    println!("{}", "Hex Input Styles".bold().underline());
    println!(
        "  The {} format accepts multiple common paste styles:",
        "hex".yellow()
    );
    println!();
    println!(
        "    {}           {}",
        "691E01B8".green(),
        "Continuous".dimmed()
    );
    println!(
        "    {}         {}",
        "0x691E01B8".green(),
        "With 0x prefix".dimmed()
    );
    println!(
        "    {}        {}",
        "69 1E 01 B8".green(),
        "Space-separated (hex dumps)".dimmed()
    );
    println!(
        "    {}        {}",
        "69:1E:01:B8".green(),
        "Colon-separated (MAC address)".dimmed()
    );
    println!(
        "    {}        {}",
        "69-1E-01-B8".green(),
        "Dash-separated".dimmed()
    );
    println!(
        "    {}   {}",
        "0x69, 0x1E, 0x01".green(),
        "Comma-separated".dimmed()
    );
    println!(
        "    {}  {}",
        "{0x69, 0x1E, 0x01}".green(),
        "C/C++ array style".dimmed()
    );
}

/// Result of processing input (either direct string or file contents)
enum InputData {
    /// Text input to be parsed as string
    Text(String),
    /// Binary data from file, encoded as base64 for processing
    Binary { base64: String, path: String },
}

/// Fetch content from a URL
fn fetch_url(url: &str) -> Result<InputData, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("Failed to fetch URL '{}': {}", url, e))?;

    let content_type = response
        .header("Content-Type")
        .unwrap_or("application/octet-stream")
        .to_string();

    // Read response body
    let mut buffer = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Determine if it's text or binary based on content-type
    let is_text = content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("xml")
        || content_type.contains("javascript");

    if is_text {
        // Try to interpret as UTF-8 text
        match String::from_utf8(buffer) {
            Ok(text) => Ok(InputData::Text(text)),
            Err(e) => {
                // Fall back to binary if not valid UTF-8
                let b64 = base64::engine::general_purpose::STANDARD.encode(e.into_bytes());
                Ok(InputData::Binary {
                    base64: b64,
                    path: url.to_string(),
                })
            }
        }
    } else {
        // Binary content - encode as base64
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
        Ok(InputData::Binary {
            base64: b64,
            path: url.to_string(),
        })
    }
}

/// Read input, handling @path syntax for file reading and URL fetching
fn read_input(input: &str) -> Result<InputData, String> {
    if !input.starts_with('@') {
        return Ok(InputData::Text(input.to_string()));
    }

    let path = &input[1..];

    // Handle URLs (http:// or https://)
    if path.starts_with("http://") || path.starts_with("https://") {
        return fetch_url(path);
    }

    // Handle @- for stdin
    if path == "-" {
        let mut buffer = Vec::new();
        io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;

        // Try to interpret as UTF-8 text first
        if let Ok(text) = String::from_utf8(buffer.clone()) {
            // If it's valid UTF-8 and doesn't look like binary, treat as text
            if !text.contains('\0') {
                return Ok(InputData::Text(text));
            }
        }

        // Binary data - encode as base64
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
        return Ok(InputData::Binary {
            base64: b64,
            path: "stdin".to_string(),
        });
    }

    // Check if file exists
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Read file contents
    let buffer =
        fs::read(file_path).map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

    // Try to interpret as UTF-8 text first
    if let Ok(text) = String::from_utf8(buffer.clone()) {
        // If it's valid UTF-8 and doesn't look like binary, treat as text
        if !text.contains('\0') {
            return Ok(InputData::Text(text));
        }
    }

    // Binary data - encode as base64 for processing
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
    Ok(InputData::Binary {
        base64: b64,
        path: path.to_string(),
    })
}

fn main() {
    let cli = Cli::parse();

    // Initialize tracing based on verbosity level
    let level = match cli.verbose {
        0 => LevelFilter::OFF,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };
    if level != LevelFilter::OFF {
        let filter = EnvFilter::builder()
            .with_default_directive(level.into())
            .from_env_lossy();
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(std::io::stderr)
            .init();
    }

    if cli.formats {
        print_formats();
        return;
    }

    let forb = Formatorbit::new();

    // Parse packet mode early (needed for both pipe and direct mode)
    let packet_mode = match cli.packet.as_deref() {
        Some("compact") | Some("c") | Some("") => PacketMode::Compact,
        Some("detailed") | Some("detail") | Some("d") | Some("table") => PacketMode::Detailed,
        Some(other) => {
            eprintln!(
                "{}: Unknown packet mode '{}'. Use 'compact' or 'detailed'.",
                "error".red().bold(),
                other
            );
            std::process::exit(1);
        }
        None => PacketMode::None,
    };

    // Check if we should run in pipe mode
    // Only use pipe mode if stdin is not a terminal AND no direct input was given
    let stdin_is_pipe = !std::io::stdin().is_terminal();
    if (stdin_is_pipe && cli.input.is_none()) || cli.force_pipe {
        let config = pipe::PipeModeConfig {
            threshold: cli.threshold,
            highlight: cli.highlight,
            max_tokens: cli.max_tokens,
            json: cli.json,
            format_filter: cli.only.clone().unwrap_or_default(),
            packet_mode,
        };

        if let Err(e) = pipe::run_pipe_mode(&forb, &config) {
            eprintln!("{}: Failed to read stdin: {}", "error".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    // Direct input mode
    let Some(raw_input) = cli.input else {
        eprintln!("{}: No input provided", "error".red().bold());
        eprintln!();
        eprintln!("Usage: {} <INPUT>", "forb".bold());
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  forb 691E01B8              Interpret hex bytes");
        eprintln!("  forb \"87 A3 69 6E 74 01\"   Space-separated hex");
        eprintln!("  forb 1703456789            Unix timestamp");
        eprintln!("  forb \"#FF5733\"             Color");
        eprintln!();
        eprintln!("File input:");
        eprintln!("  forb @image.jpg            Read and analyze file");
        eprintln!("  forb @-                    Read from stdin");
        eprintln!();
        eprintln!("Pipe mode:");
        eprintln!("  cat logs.txt | forb        Annotate log lines");
        eprintln!("  cat logs.txt | forb -H     With highlighting");
        eprintln!();
        eprintln!("Run {} for more information.", "forb --help".bold());
        std::process::exit(1);
    };

    // Process input (handle @path syntax for file reading)
    let (input, file_path) = match read_input(&raw_input) {
        Ok(InputData::Text(text)) => (text, None),
        Ok(InputData::Binary { base64, path }) => (base64, Some(path)),
        Err(e) => {
            eprintln!("{}: {}", "error".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Handle --no-color flag
    if cli.no_color {
        set_override(false);
    }

    // Build pretty config
    let pretty_config = PrettyConfig {
        color: !cli.no_color,
        indent: "  ",
        compact: cli.compact,
        packet_mode,
    };

    // Get results - either forced format or auto-detect
    let format_filter = cli.only.unwrap_or_default();
    let results = if let Some(ref from_format) = cli.from {
        // Force specific format interpretation
        forb.convert_all_filtered(&input, std::slice::from_ref(from_format))
    } else {
        forb.convert_all_filtered(&input, &format_filter)
    };

    if results.is_empty() {
        if cli.raw {
            // Silent failure for raw mode
            std::process::exit(1);
        }
        let display_input = file_path.as_ref().map_or(input.as_str(), |p| p.as_str());
        println!("No interpretations found for: {display_input}");
        return;
    }

    // Filter to show only high-confidence interpretations (skip utf8 fallback for hex-like input)
    let meaningful_results: Vec<_> = results
        .iter()
        .filter(|r| r.interpretation.confidence > 0.2)
        .collect();

    let results_to_show: Vec<_> = if meaningful_results.is_empty() {
        results.iter().collect()
    } else {
        meaningful_results
    };

    // Apply --first flag
    let results_to_show: Vec<_> = if cli.first {
        results_to_show.into_iter().take(1).collect()
    } else {
        results_to_show
    };

    // Handle --dot output
    if cli.dot {
        print_dot_graph(&input, &results_to_show);
        return;
    }

    // Handle --mermaid output
    if cli.mermaid {
        print_mermaid_graph(&input, &results_to_show);
        return;
    }

    // Handle --json output
    if cli.json {
        let output: Vec<_> = results_to_show.iter().map(|r| (*r).clone()).collect();
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return;
    }

    // Handle --raw output
    if cli.raw {
        for result in &results_to_show {
            // Print conversion values only
            let conversions_to_show: Vec<_> = if cli.limit == 0 {
                result.conversions.iter().collect()
            } else {
                result.conversions.iter().take(cli.limit).collect()
            };

            for conv in conversions_to_show {
                let display = format_conversion_display(
                    &conv.value,
                    &conv.display,
                    &conv.rich_display,
                    &pretty_config,
                );
                println!("{}", display);
            }
        }
        return;
    }

    // Standard human-readable output
    for result in results_to_show {
        let conf = (result.interpretation.confidence * 100.0) as u32;
        println!(
            "{} {} ({}% confidence)",
            "▶".blue(),
            result.interpretation.source_format.green().bold(),
            conf
        );
        println!("  {}", result.interpretation.description.dimmed());

        if result.conversions.is_empty() {
            println!("  {}", "(no conversions available)".dimmed());
        } else {
            // Apply limit (0 = unlimited)
            let conversions_to_show: Vec<_> = if cli.limit == 0 {
                result.conversions.iter().collect()
            } else {
                result.conversions.iter().take(cli.limit).collect()
            };

            for conv in &conversions_to_show {
                let path_str = if conv.path.len() > 1 {
                    format!(" (via {})", conv.path.join(" → "))
                } else {
                    String::new()
                };

                // Pretty-print JSON values (and packet layout if enabled)
                let display = format_conversion_display(
                    &conv.value,
                    &conv.display,
                    &conv.rich_display,
                    &pretty_config,
                );

                // Symbol based on conversion kind:
                // → Conversion (actual transformation)
                // ≈ Representation (same value, different notation)
                // ✓ Trait (property/observation)
                let kind_symbol = match conv.kind {
                    ConversionKind::Conversion => "→".cyan(),
                    ConversionKind::Representation => "≈".blue(),
                    ConversionKind::Trait => "✓".magenta(),
                };

                // Indent multi-line output
                let display_lines: Vec<&str> = display.lines().collect();
                if display_lines.len() > 1 {
                    println!(
                        "  {} {}:{}",
                        kind_symbol,
                        conv.target_format.yellow(),
                        path_str.dimmed()
                    );
                    for line in display_lines {
                        println!("    {}", line);
                    }
                } else {
                    println!(
                        "  {} {}: {}{}",
                        kind_symbol,
                        conv.target_format.yellow(),
                        display,
                        path_str.dimmed()
                    );
                }
            }

            // Show how many more are hidden
            let hidden = result
                .conversions
                .len()
                .saturating_sub(conversions_to_show.len());
            if hidden > 0 {
                println!(
                    "  {} {}",
                    "…".dimmed(),
                    format!("({} more, use -l 0 to show all)", hidden).dimmed()
                );
            }
        }
        println!();
    }
}

/// Format a conversion's display string, applying pretty-printing for structured data.
fn format_conversion_display(
    value: &CoreValue,
    original_display: &str,
    rich_display: &[RichDisplayOption],
    config: &PrettyConfig,
) -> String {
    // If packet mode is enabled and we have PacketLayout in rich_display, show that
    if config.packet_mode != PacketMode::None {
        for opt in rich_display {
            if let RichDisplay::PacketLayout { segments, .. } = &opt.preferred {
                return match config.packet_mode {
                    PacketMode::Compact => pretty::pretty_packet_compact(segments, config),
                    PacketMode::Detailed => pretty::pretty_packet_detailed(segments, config),
                    PacketMode::None => unreachable!(),
                };
            }
        }
    }

    match value {
        CoreValue::Json(json) => {
            // Pretty-print JSON with colors
            pretty::pretty_json(json, config)
        }
        CoreValue::Protobuf(fields) => {
            // Pretty-print protobuf with colors
            pretty::pretty_protobuf(fields, config)
        }
        _ => {
            // For other types, use the original display
            original_display.to_string()
        }
    }
}

/// Output conversion graph in Graphviz DOT format.
fn print_dot_graph(input: &str, results: &[&formatorbit_core::ConversionResult]) {
    println!("digraph conversions {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box, fontname=\"Helvetica\"];");
    println!("  edge [fontname=\"Helvetica\", fontsize=10];");
    println!();

    // Input node
    let input_label = escape_dot_label(input);
    println!(
        "  input [label=\"{}\", shape=ellipse, style=filled, fillcolor=\"#e8e8e8\"];",
        input_label
    );
    println!();

    let mut node_id = 0;
    for result in results {
        let interp = &result.interpretation;
        let conf = (interp.confidence * 100.0) as u32;
        let interp_node = format!("interp_{}", node_id);
        node_id += 1;

        // Interpretation node
        let interp_label = format!("{}\\n({}%)", interp.source_format, conf);
        println!(
            "  {} [label=\"{}\", style=filled, fillcolor=\"#c8e6c9\"];",
            interp_node, interp_label
        );
        println!("  input -> {} [label=\"{}%\"];", interp_node, conf);

        // Conversion nodes
        for conv in &result.conversions {
            let conv_node = format!("conv_{}", node_id);
            node_id += 1;

            // Truncate long display values
            let display = if conv.display.len() > 30 {
                format!("{}...", &conv.display[..27])
            } else {
                conv.display.clone()
            };
            let display = escape_dot_label(&display);

            let conv_label = format!("{}\\n{}", conv.target_format, display);
            println!("  {} [label=\"{}\"];", conv_node, conv_label);

            let edge_label = if conv.path.len() > 1 {
                conv.path[..conv.path.len() - 1].join(" → ")
            } else {
                String::new()
            };

            if edge_label.is_empty() {
                println!("  {} -> {};", interp_node, conv_node);
            } else {
                println!(
                    "  {} -> {} [label=\"{}\"];",
                    interp_node,
                    conv_node,
                    escape_dot_label(&edge_label)
                );
            }
        }
        println!();
    }

    println!("}}");
}

/// Output conversion graph in Mermaid format.
fn print_mermaid_graph(input: &str, results: &[&formatorbit_core::ConversionResult]) {
    println!("```mermaid");
    println!("graph LR");

    // Input node
    let input_label = escape_mermaid_label(input);
    println!("  input([\"{}\"]);", input_label);

    let mut node_id = 0;
    for result in results {
        let interp = &result.interpretation;
        let conf = (interp.confidence * 100.0) as u32;
        let interp_node = format!("interp_{}", node_id);
        node_id += 1;

        // Interpretation node
        let interp_label = format!("{} ({}%)", interp.source_format, conf);
        println!(
            "  {}[\"{}\"];",
            interp_node,
            escape_mermaid_label(&interp_label)
        );
        println!("  input -->|{}%| {};", conf, interp_node);

        // Conversion nodes
        for conv in &result.conversions {
            let conv_node = format!("conv_{}", node_id);
            node_id += 1;

            // Truncate long display values
            let display = if conv.display.len() > 25 {
                format!("{}...", &conv.display[..22])
            } else {
                conv.display.clone()
            };

            let conv_label = format!("{}: {}", conv.target_format, display);
            println!(
                "  {}[\"{}\"];",
                conv_node,
                escape_mermaid_label(&conv_label)
            );

            if conv.path.len() > 1 {
                let edge_label = conv.path[..conv.path.len() - 1].join(" → ");
                println!(
                    "  {} -->|{}| {};",
                    interp_node,
                    escape_mermaid_label(&edge_label),
                    conv_node
                );
            } else {
                println!("  {} --> {};", interp_node, conv_node);
            }
        }
    }

    println!("```");
}

/// Escape special characters for DOT labels.
fn escape_dot_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

/// Escape special characters for Mermaid labels.
fn escape_mermaid_label(s: &str) -> String {
    s.replace('"', "'")
        .replace('\n', " ")
        .replace('\r', "")
        .replace('[', "(")
        .replace(']', ")")
}
