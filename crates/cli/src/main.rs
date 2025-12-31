mod analytics;
mod config;
mod pipe;
mod pretty;
mod tokenizer;

use config::Config;

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

use base64::Engine;
use clap::Parser;
use colored::{control::set_override, Colorize};
use formatorbit_core::{
    truncate_str, ConversionKind, CoreValue, Formatorbit, RichDisplay, RichDisplayOption,
};
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
    cat server.log | forb -o uuid,hex  Only look for specific formats

CONFIGURATION:
  Settings can be configured via CLI flags, environment variables, or config file.
  Precedence: CLI args > Environment vars > Config file > Defaults

  Setting      | CLI flag       | Env var            | Default
  -------------|----------------|--------------------|---------
  limit        | -l, --limit    | FORB_LIMIT         | 5
  threshold    | -t, --threshold| FORB_THRESHOLD     | 0.8
  no_color     | -C, --no-color | FORB_NO_COLOR      | false
  url_timeout  | --url-timeout  | FORB_URL_TIMEOUT   | 30
  url_max_size | --url-max-size | FORB_URL_MAX_SIZE  | 10M
  max_tokens   | --max-tokens   | FORB_MAX_TOKENS    | 50

  Config file location: forb --config-path
  Generate default config: forb --config-init

  Note: NO_COLOR env var is also respected (https://no-color.org/)

ANALYTICS:
  Local usage tracking is enabled by default (stored in human-readable TOML).
  Use --analytics status to view current analytics data.
  Use --analytics disable to learn how to opt out.
  Set FORB_ANALYTICS=0 to disable temporarily."##;

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
    #[arg(long, short = 't')]
    threshold: Option<f32>,

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
    #[arg(long, short = 'l')]
    limit: Option<usize>,

    /// Force pipe mode even when stdin is a TTY (for testing)
    #[arg(long, hide = true)]
    force_pipe: bool,

    /// Maximum tokens to analyze per line in pipe mode
    #[arg(long, hide = true)]
    max_tokens: Option<usize>,

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

    /// Show blockable path for each conversion (for configuring blocking)
    ///
    /// Displays the path in [source:target] format that can be copied
    /// directly into config.toml [blocking] paths array.
    #[arg(long)]
    show_paths: bool,

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

    /// Timeout for URL fetches in seconds
    #[arg(long, value_name = "SECS")]
    url_timeout: Option<u64>,

    /// Maximum response size for URL fetches (e.g., 10M, 50M, 1G)
    #[arg(long, value_name = "SIZE")]
    url_max_size: Option<String>,

    /// Show config file path
    #[arg(long)]
    config_path: bool,

    /// Generate default config file (see --config-path for location)
    #[arg(long)]
    config_init: bool,

    /// Analytics subcommand (status, show, clear, enable, disable)
    #[arg(long, value_name = "COMMAND")]
    analytics: Option<String>,
}

/// Parse size string like "10M", "50M", "1G" into bytes.
fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty size string".to_string());
    }

    // Find where the numeric part ends
    let num_end = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    let (num_str, suffix) = s.split_at(num_end);

    let base: u64 = num_str
        .parse()
        .map_err(|_| format!("Invalid size number: '{}'", num_str))?;

    let multiplier = match suffix.to_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        _ => {
            return Err(format!(
                "Unknown size suffix: '{}'. Use K, M, or G.",
                suffix
            ))
        }
    };

    Ok(base * multiplier)
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
        "Reference",
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

/// Fetch content from a URL with timeout and size limits.
fn fetch_url(url: &str, timeout_secs: u64, max_size: u64) -> Result<InputData, String> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .call()
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("timed out") || msg.contains("Timeout") {
                format!(
                    "Request timed out after {}s. Use --url-timeout to increase the limit.",
                    timeout_secs
                )
            } else {
                format!("Failed to fetch URL '{}': {}", url, e)
            }
        })?;

    let content_type = response
        .header("Content-Type")
        .unwrap_or("application/octet-stream")
        .to_string();

    // Read response body with size limit
    let mut buffer = Vec::new();
    response
        .into_reader()
        .take(max_size)
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Check if we hit the size limit
    if buffer.len() as u64 >= max_size {
        let size_mb = max_size / (1024 * 1024);
        return Err(format!(
            "Response exceeds {} MB limit. Use --url-max-size to increase (e.g., --url-max-size 50M).",
            size_mb
        ));
    }

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

/// Read input, handling @path syntax for file reading and URL fetching.
///
/// For URL fetching, uses the provided timeout and size limits.
fn read_input(input: &str, url_timeout: u64, url_max_size: u64) -> Result<InputData, String> {
    if !input.starts_with('@') {
        return Ok(InputData::Text(input.to_string()));
    }

    let path = &input[1..];

    // Handle URLs (http:// or https://)
    if path.starts_with("http://") || path.starts_with("https://") {
        return fetch_url(path, url_timeout, url_max_size);
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

    // Handle --config-path
    if cli.config_path {
        match Config::path() {
            Some(path) => println!("{}", path.display()),
            None => {
                eprintln!(
                    "{}: Cannot determine config directory",
                    "error".red().bold()
                );
                std::process::exit(1);
            }
        }
        return;
    }

    // Handle --config-init
    if cli.config_init {
        match config::init_config() {
            Ok(path) => println!("Created config file: {}", path.display()),
            Err(e) => {
                eprintln!("{}: {}", "error".red().bold(), e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Handle --analytics subcommand (early, before config loading for disable)
    if let Some(ref cmd) = cli.analytics {
        handle_analytics_command(cmd);
        return;
    }

    // Initialize tracing based on verbosity level (before config loading for logging)
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

    // Load config file and merge with CLI args
    // Precedence: CLI args > Environment vars > Config file > Defaults
    let file_config = Config::load();

    if let Some(path) = Config::path() {
        if path.exists() {
            tracing::debug!("Loaded config from: {}", path.display());
        } else {
            tracing::trace!("No config file at: {}", path.display());
        }
    }

    // Merge settings with source logging
    let limit = if let Some(l) = cli.limit {
        tracing::debug!("limit = {} (from CLI)", l);
        l
    } else {
        let l = file_config.limit();
        let source = if std::env::var("FORB_LIMIT").is_ok() {
            "env FORB_LIMIT"
        } else if file_config.limit.is_some() {
            "config file"
        } else {
            "default"
        };
        tracing::debug!("limit = {} (from {})", l, source);
        l
    };

    let threshold = if let Some(t) = cli.threshold {
        tracing::debug!("threshold = {} (from CLI)", t);
        t
    } else {
        let t = file_config.threshold();
        let source = if std::env::var("FORB_THRESHOLD").is_ok() {
            "env FORB_THRESHOLD"
        } else if file_config.threshold.is_some() {
            "config file"
        } else {
            "default"
        };
        tracing::debug!("threshold = {} (from {})", t, source);
        t
    };

    let no_color = if cli.no_color {
        tracing::debug!("no_color = true (from CLI)");
        true
    } else {
        let nc = file_config.no_color();
        if nc {
            let source = if std::env::var("NO_COLOR").is_ok() {
                "env NO_COLOR"
            } else if std::env::var("FORB_NO_COLOR").is_ok() {
                "env FORB_NO_COLOR"
            } else {
                "config file"
            };
            tracing::debug!("no_color = true (from {})", source);
        }
        nc
    };

    let url_timeout = if let Some(t) = cli.url_timeout {
        tracing::debug!("url_timeout = {} (from CLI)", t);
        t
    } else {
        let t = file_config.url_timeout();
        let source = if std::env::var("FORB_URL_TIMEOUT").is_ok() {
            "env FORB_URL_TIMEOUT"
        } else if file_config.url_timeout.is_some() {
            "config file"
        } else {
            "default"
        };
        tracing::debug!("url_timeout = {} (from {})", t, source);
        t
    };

    let url_max_size_str = if let Some(ref s) = cli.url_max_size {
        tracing::debug!("url_max_size = {} (from CLI)", s);
        s.clone()
    } else {
        let s = file_config.url_max_size();
        let source = if std::env::var("FORB_URL_MAX_SIZE").is_ok() {
            "env FORB_URL_MAX_SIZE"
        } else if file_config.url_max_size.is_some() {
            "config file"
        } else {
            "default"
        };
        tracing::debug!("url_max_size = {} (from {})", s, source);
        s
    };

    let max_tokens = if let Some(m) = cli.max_tokens {
        tracing::debug!("max_tokens = {} (from CLI)", m);
        m
    } else {
        let m = file_config.max_tokens();
        let source = if std::env::var("FORB_MAX_TOKENS").is_ok() {
            "env FORB_MAX_TOKENS"
        } else if file_config.max_tokens.is_some() {
            "config file"
        } else {
            "default"
        };
        tracing::debug!("max_tokens = {} (from {})", m, source);
        m
    };

    if cli.formats {
        print_formats();
        return;
    }

    // Initialize analytics tracker
    let mut tracker = analytics::AnalyticsTracker::new(file_config.analytics_enabled());
    tracker.record_invocation();

    // Record config customizations
    if cli.limit.is_some() || file_config.limit.is_some() {
        tracker.record_limit_customized();
    }
    if cli.threshold.is_some() || file_config.threshold.is_some() {
        tracker.record_threshold_customized();
    }
    if let Some(ref only) = cli.only {
        tracker.record_only_filter(only);
    }
    if file_config.priority.is_some() {
        tracker.record_priority_customized();
    }
    if let Some(ref blocking) = file_config.blocking {
        tracker.record_blocking_customized(&blocking.formats);
    }

    // Record output mode
    if cli.json {
        tracker.record_json_output();
    }
    if cli.raw {
        tracker.record_raw_output();
    }
    if cli.dot {
        tracker.record_dot_output();
    }
    if cli.mermaid {
        tracker.record_mermaid_output();
    }

    // Create Formatorbit with optional conversion config from file
    let forb = if let Some(conv_config) = file_config.conversion_config() {
        if cli.verbose > 0 {
            tracing::debug!("Using custom priority/blocking config from config file");
        }
        Formatorbit::with_config(conv_config)
    } else {
        Formatorbit::new()
    };

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
        tracker.record_pipe_mode();

        let pipe_config = pipe::PipeModeConfig {
            threshold,
            highlight: cli.highlight,
            max_tokens,
            json: cli.json,
            format_filter: cli.only.clone().unwrap_or_default(),
            packet_mode,
        };

        if let Err(e) = pipe::run_pipe_mode(&forb, &pipe_config) {
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

    // Parse URL size limit
    let url_max_size = match parse_size(&url_max_size_str) {
        Ok(size) => size,
        Err(e) => {
            eprintln!("{}: invalid --url-max-size: {}", "error".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Process input (handle @path syntax for file reading)
    // Track file/URL input for analytics
    if raw_input.starts_with("@http://") || raw_input.starts_with("@https://") {
        tracker.record_url_fetch();
    } else if raw_input.starts_with('@') {
        tracker.record_file_input();
    }

    let (input, file_path) = match read_input(&raw_input, url_timeout, url_max_size) {
        Ok(InputData::Text(text)) => (text, None),
        Ok(InputData::Binary { base64, path }) => (base64, Some(path)),
        Err(e) => {
            eprintln!("{}: {}", "error".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Handle --no-color flag
    if no_color {
        set_override(false);
    }

    // Build pretty config
    let pretty_config = PrettyConfig {
        color: !no_color,
        indent: "  ",
        compact: cli.compact,
        packet_mode,
        show_paths: cli.show_paths,
    };

    // Get results - either forced format or auto-detect
    let format_filter = cli.only.unwrap_or_default();

    // Validate format names early
    if let Some(ref from_format) = cli.from {
        if !forb.is_valid_format(from_format) {
            eprintln!(
                "{}: Unknown format '{}'. Use {} to see available formats.",
                "error".red().bold(),
                from_format.yellow(),
                "--formats".bold()
            );
            std::process::exit(1);
        }
    }
    for name in &format_filter {
        if !forb.is_valid_format(name) {
            eprintln!(
                "{}: Unknown format '{}'. Use {} to see available formats.",
                "error".red().bold(),
                name.yellow(),
                "--formats".bold()
            );
            std::process::exit(1);
        }
    }

    let results = if let Some(ref from_format) = cli.from {
        // Force specific format interpretation
        forb.convert_all_filtered(&input, std::slice::from_ref(from_format))
    } else {
        forb.convert_all_filtered(&input, &format_filter)
    };

    // Track format usage and conversion targets for analytics
    for result in &results {
        tracker.record_format_usage(&result.interpretation.source_format);
        for conv in &result.conversions {
            tracker.record_conversion_target(&conv.target_format);
        }
    }

    if results.is_empty() {
        if cli.raw {
            // Silent failure for raw mode
            std::process::exit(1);
        }
        let display_input = file_path.as_ref().map_or(input.as_str(), |p| p.as_str());

        // If a specific format was requested, try to show a validation error
        if let Some(ref from_format) = cli.from {
            if let Some(error) = forb.validate(&input, from_format) {
                eprintln!(
                    "{}: Cannot parse as {}: {}",
                    "error".red().bold(),
                    from_format.yellow(),
                    error
                );
                std::process::exit(1);
            }
        } else if format_filter.len() == 1 {
            // Single format in --only filter
            if let Some(error) = forb.validate(&input, &format_filter[0]) {
                eprintln!(
                    "{}: Cannot parse as {}: {}",
                    "error".red().bold(),
                    format_filter[0].yellow(),
                    error
                );
                std::process::exit(1);
            }
        }

        if display_input.is_empty() {
            println!("No interpretations found for (empty input)");
        } else {
            println!("No interpretations found for: {display_input}");
        }
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
            let conversions_to_show: Vec<_> = if limit == 0 {
                result.conversions.iter().collect()
            } else {
                result.conversions.iter().take(limit).collect()
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
            let conversions_to_show: Vec<_> = if limit == 0 {
                result.conversions.iter().collect()
            } else {
                result.conversions.iter().take(limit).collect()
            };

            for conv in &conversions_to_show {
                let path_str = if conv.path.len() > 1 {
                    format!(" (via {})", conv.path.join(" → "))
                } else {
                    String::new()
                };

                // Build blockable path string for --show-paths
                let block_path_str = if pretty_config.show_paths && !conv.path.is_empty() {
                    format!(" {}", format!("[{}]", conv.path.join(":")).dimmed())
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
                        "  {} {}:{}{}",
                        kind_symbol,
                        conv.target_format.yellow(),
                        path_str.dimmed(),
                        block_path_str
                    );
                    for line in display_lines {
                        println!("    {}", line);
                    }
                } else {
                    println!(
                        "  {} {}: {}{}{}",
                        kind_symbol,
                        conv.target_format.yellow(),
                        display,
                        path_str.dimmed(),
                        block_path_str
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

            // Truncate long display values (UTF-8 safe)
            let display = truncate_str(&conv.display, 30);
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

            // Truncate long display values (UTF-8 safe)
            let display = truncate_str(&conv.display, 25);

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

/// Handle analytics subcommand.
fn handle_analytics_command(cmd: &str) {
    use colored::Colorize;

    match cmd {
        "status" => {
            let data = analytics::AnalyticsData::load();
            // For status, check if enabled via config
            let config = Config::load();
            let enabled = config.analytics_enabled();
            println!("{}", analytics::format_status(&data, enabled));
        }
        "show" => {
            let data = analytics::AnalyticsData::load();
            println!("{}", analytics::format_full(&data));
        }
        "clear" => {
            let mut tracker = analytics::AnalyticsTracker::new(true);
            tracker.data_mut().clear();
            tracker.save();
            println!("Analytics data cleared.");
        }
        "enable" => {
            println!(
                "To enable analytics, add to your config file ({}):",
                Config::path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "~/.config/forb/config.toml".to_string())
            );
            println!();
            println!("  [analytics]");
            println!("  enabled = true");
            println!();
            println!("Or unset FORB_ANALYTICS environment variable.");
        }
        "disable" => {
            println!(
                "To disable analytics, add to your config file ({}):",
                Config::path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "~/.config/forb/config.toml".to_string())
            );
            println!();
            println!("  [analytics]");
            println!("  enabled = false");
            println!();
            println!("Or set FORB_ANALYTICS=0 environment variable.");
        }
        "path" => {
            if let Some(path) = analytics::AnalyticsData::path() {
                println!("{}", path.display());
            } else {
                eprintln!("{}: Cannot determine analytics path", "error".red().bold());
                std::process::exit(1);
            }
        }
        "preview" => {
            let data = analytics::AnalyticsData::load();
            let payload = analytics::ContributionPayload::from_data(&data);
            println!("{}", payload.format_preview());
        }
        "contribute" => {
            let data = analytics::AnalyticsData::load();

            if data.session_stats.total_invocations == 0 {
                eprintln!(
                    "{}: No analytics data to contribute yet.",
                    "note".yellow().bold()
                );
                eprintln!("Use forb a few times first, then try again.");
                return;
            }

            // Show preview first
            let payload = analytics::ContributionPayload::from_data(&data);
            println!("{}", payload.format_preview());
            println!("Sending to TelemetryDeck...");

            match analytics::send_contribution(&data) {
                Ok(()) => {
                    println!(
                        "{} Thank you for contributing anonymous usage data!",
                        "✓".green().bold()
                    );
                    println!("This helps improve forb for everyone.");
                }
                Err(e) => {
                    eprintln!("{}: {}", "error".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
        other => {
            eprintln!(
                "{}: Unknown analytics command '{}'\n",
                "error".red().bold(),
                other
            );
            eprintln!("Available commands:");
            eprintln!("  --analytics status     Show analytics status and summary");
            eprintln!("  --analytics show       Show full analytics data (TOML)");
            eprintln!("  --analytics preview    Preview what would be sent");
            eprintln!("  --analytics contribute Send anonymous usage data");
            eprintln!("  --analytics clear      Clear all analytics data");
            eprintln!("  --analytics enable     Show how to enable analytics");
            eprintln!("  --analytics disable    Show how to disable analytics");
            eprintln!("  --analytics path       Show analytics file path");
            std::process::exit(1);
        }
    }
}
