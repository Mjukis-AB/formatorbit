mod analytics;
mod config;
mod graph;
mod pipe;
mod pretty;
mod tokenizer;
mod updates;

use config::Config;

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

use clap::Parser;
use colored::{control::set_override, Colorize};
use formatorbit_core::{
    truncate_str, Conversion, ConversionKind, CoreValue, Formatorbit, RichDisplay,
    RichDisplayOption,
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

PIPED INPUT:
  Piped data is automatically detected and processed:
    echo "hello" | forb                Same as: forb "hello"
    cat data.bin | forb                Binary data (like forb @-)

TEE MODE:
  Pass through input while annotating interesting values (like tee):
    tail -f server.log | forb --tee    Annotate log lines live
    cat server.log | forb -T -t 0.5    Lower confidence threshold
    cat server.log | forb -T -H        Highlight matches inline
    cat server.log | forb -T -o uuid   Only look for specific formats

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

    // === Tee mode options ===
    /// Tee mode: pass through stdin while annotating interesting values
    ///
    /// Like Unix `tee`, but annotates timestamps, UUIDs, hex, etc. inline.
    /// Useful for live log monitoring: tail -f app.log | forb --tee
    #[arg(long, short = 'T')]
    tee: bool,

    /// Minimum confidence threshold for showing annotations (0.0-1.0)
    ///
    /// In tee mode, only values with confidence >= threshold are annotated.
    #[arg(long, short = 't')]
    threshold: Option<f32>,

    /// Highlight interesting values inline with color
    ///
    /// In tee mode, highlights matched tokens with background color.
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

    /// Force tee mode even when stdin is a TTY (for testing)
    #[arg(long, hide = true)]
    force_tee: bool,

    /// Maximum tokens to analyze per line in tee mode
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

    /// Minimum confidence for reinterpreting decoded strings (0.0-1.0)
    ///
    /// When hex/base64 decodes to a string, try parsing that string as
    /// other formats (UUID, IP, JSON, etc). Only formats with confidence
    /// >= this threshold are explored. Set to 1.0 to disable.
    #[arg(long, value_name = "THRESHOLD")]
    reinterpret_threshold: Option<f32>,

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

    /// Manage local usage analytics
    ///
    /// Commands:
    ///   status     - Show analytics status and summary
    ///   show       - Show full analytics data (TOML)
    ///   preview    - Preview what would be sent
    ///   contribute - Send anonymous usage data to TelemetryDeck
    ///   clear      - Clear all analytics data
    ///   enable     - Show how to enable analytics
    ///   disable    - Show how to disable analytics
    ///   path       - Show analytics file path
    #[arg(long, value_name = "COMMAND", default_missing_value = "status", num_args = 0..=1)]
    analytics: Option<String>,

    /// Show format conversion graph (without input data)
    ///
    /// Modes:
    ///   schema   - All formats and conversion edges (large graph)
    ///   category - Category-level relationships
    ///   FORMAT   - Show what a specific format can convert to/from
    ///
    /// Output format controlled by --dot (default: mermaid)
    #[arg(long, value_name = "MODE")]
    graph: Option<String>,

    /// Check for available updates
    #[arg(long)]
    check_updates: bool,

    /// List or manage plugins (requires --features plugins)
    ///
    /// Without arguments, lists all loaded plugins.
    ///
    /// Commands:
    ///   status  Show detailed status including load errors
    ///   path    Print plugin directory path
    ///
    /// Plugins are Python files in ~/.config/forb/plugins/ (Linux/macOS)
    /// or %APPDATA%\forb\plugins\ (Windows).
    ///
    /// See PLUGINS.md for documentation on creating plugins.
    #[cfg(feature = "plugins")]
    #[arg(long, value_name = "COMMAND", default_missing_value = "list", num_args = 0..=1, verbatim_doc_comment)]
    plugins: Option<String>,

    /// Target currency for expression functions like USD(100), EUR(50)
    ///
    /// Without a value, shows current target and available currencies.
    /// With a value, sets the target currency for this invocation.
    ///
    /// Examples:
    ///   forb --currency          Show current target currency
    ///   forb --currency EUR      Set target to EUR for this run
    ///   forb --currency SEK "USD(100)"  Convert 100 USD to SEK
    ///
    /// Priority: --currency flag > FORB_TARGET_CURRENCY env > config > locale > USD
    #[arg(long, value_name = "CODE", default_missing_value = "", num_args = 0..=1, verbatim_doc_comment)]
    currency: Option<String>,
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
    /// Binary data from file (raw bytes, core handles encoding)
    Binary { data: Vec<u8>, path: String },
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
                Ok(InputData::Binary {
                    data: e.into_bytes(),
                    path: url.to_string(),
                })
            }
        }
    } else {
        // Binary content
        Ok(InputData::Binary {
            data: buffer,
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

        // Binary data
        return Ok(InputData::Binary {
            data: buffer,
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

    // Binary data
    Ok(InputData::Binary {
        data: buffer,
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

    // Handle --check-updates (explicit update check)
    if cli.check_updates {
        handle_check_updates();
        return;
    }

    // Handle --plugins (plugin management)
    #[cfg(feature = "plugins")]
    if let Some(ref cmd) = cli.plugins {
        handle_plugins_command(cmd);
        return;
    }

    // Handle --currency (show info or set target)
    if let Some(ref code) = cli.currency {
        if code.is_empty() {
            // Show current target currency and available currencies
            handle_currency_info();
            return;
        }
        // Set target currency for this run (will be applied after config loading)
    }

    // Handle --graph (static format graph, no input needed)
    if let Some(ref mode) = cli.graph {
        let forb = Formatorbit::new();
        handle_graph_command(&forb, mode, cli.dot);
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

    // Create Formatorbit with optional conversion config from file and plugins
    let forb = {
        let mut conv_config = file_config.conversion_config().unwrap_or_default();

        // Apply CLI override for reinterpret threshold
        if let Some(threshold) = cli.reinterpret_threshold {
            conv_config.reinterpret_threshold = threshold;
            if cli.verbose > 0 {
                tracing::debug!("reinterpret_threshold = {} (from CLI)", threshold);
            }
        }

        #[cfg(feature = "plugins")]
        let base = {
            if file_config.plugins_enabled() {
                match Formatorbit::with_plugins() {
                    Ok((forb, report)) => {
                        if report.has_plugins() {
                            tracing::debug!(
                                "Loaded {} plugin(s): {} decoders, {} expr_vars, {} expr_funcs, {} traits",
                                report.total_loaded(),
                                report.decoders.len(),
                                report.expr_vars.len(),
                                report.expr_funcs.len(),
                                report.traits.len()
                            );
                        }
                        if report.has_errors() {
                            for (path, err) in &report.errors {
                                tracing::warn!("Plugin error in {}: {}", path.display(), err);
                            }
                        }
                        forb
                    }
                    Err(e) => {
                        tracing::warn!("Failed to initialize plugins: {}", e);
                        Formatorbit::new()
                    }
                }
            } else {
                tracing::debug!("Plugins disabled via config");
                Formatorbit::new()
            }
        };

        #[cfg(not(feature = "plugins"))]
        let base = Formatorbit::new();

        if conv_config.is_customized() || cli.reinterpret_threshold.is_some() {
            if cli.verbose > 0 && conv_config.is_customized() {
                tracing::debug!("Using custom priority/blocking config from config file");
            }
            base.set_config(conv_config)
        } else {
            base
        }
    };

    // Set target currency for expression functions
    // Priority: CLI flag > env > config > locale > default
    {
        use formatorbit_core::formats::currency_expr;

        if let Some(ref code) = cli.currency {
            if !code.is_empty() {
                // CLI flag takes highest priority
                currency_expr::set_target_currency(Some(code.to_uppercase()));
                tracing::debug!("target_currency = {} (from CLI)", code.to_uppercase());
            }
        } else if let Some(target) = file_config.target_currency() {
            // Config/env sets it
            let source = if std::env::var("FORB_TARGET_CURRENCY").is_ok() {
                "env FORB_TARGET_CURRENCY"
            } else {
                "config file"
            };
            currency_expr::set_target_currency(Some(target.clone()));
            tracing::debug!("target_currency = {} (from {})", target, source);
        }
        // Otherwise leave it as None and let currency_expr use locale detection
    }

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

    // Check if we should run in tee mode
    // Tee mode passes through stdin while annotating interesting values
    let stdin_is_pipe = !std::io::stdin().is_terminal();
    if cli.tee || cli.force_tee {
        if !stdin_is_pipe && !cli.force_tee {
            eprintln!(
                "{}: --tee requires piped input (e.g., cat file | forb --tee)",
                "error".red().bold()
            );
            std::process::exit(1);
        }
        tracker.record_pipe_mode(); // Keep analytics name for compatibility

        let tee_config = pipe::PipeModeConfig {
            threshold,
            highlight: cli.highlight,
            max_tokens,
            json: cli.json,
            format_filter: cli.only.clone().unwrap_or_default(),
            packet_mode,
        };

        if let Err(e) = pipe::run_pipe_mode(&forb, &tee_config) {
            eprintln!("{}: Failed to read stdin: {}", "error".red().bold(), e);
            std::process::exit(1);
        }
        return;
    }

    // Handle piped input (not tee mode) - read and process as single input
    // We'll set raw_input based on what we read, then let the normal flow handle it
    let (raw_input, piped_binary_data) = if stdin_is_pipe && cli.input.is_none() {
        let mut buffer = Vec::new();
        if let Err(e) = io::stdin().read_to_end(&mut buffer) {
            eprintln!("{}: Failed to read stdin: {}", "error".red().bold(), e);
            std::process::exit(1);
        }

        // Detect if binary or text
        let is_binary = buffer.contains(&0) || std::str::from_utf8(&buffer).is_err();

        if is_binary {
            // Binary data - will be processed via convert_bytes
            ("(stdin)".to_string(), Some(buffer))
        } else {
            // Text - trim and use as input string
            let text = String::from_utf8_lossy(&buffer);
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                eprintln!("{}: Empty input", "error".red().bold());
                std::process::exit(1);
            }
            (trimmed, None)
        }
    } else if let Some(input) = cli.input {
        (input, None)
    } else {
        // No input provided
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
        eprintln!("File/pipe input:");
        eprintln!("  forb @image.jpg            Read and analyze file");
        eprintln!("  echo hello | forb          Pipe text input");
        eprintln!("  cat data.bin | forb        Pipe binary data");
        eprintln!();
        eprintln!("Tee mode (pass-through with annotations):");
        eprintln!("  tail -f app.log | forb -T  Annotate log lines live");
        eprintln!("  cat logs.txt | forb -T -H  With highlighting");
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

    // Process input (handle @path syntax for file reading, or use piped binary)
    // Track file/URL input for analytics
    if raw_input.starts_with("@http://") || raw_input.starts_with("@https://") {
        tracker.record_url_fetch();
    } else if raw_input.starts_with('@') {
        tracker.record_file_input();
    }

    let (input, binary_data, file_path) = if let Some(data) = piped_binary_data {
        // Piped binary data - already read
        (String::new(), Some(data), Some("(stdin)".to_string()))
    } else if raw_input.starts_with('@')
        || raw_input.starts_with("http://")
        || raw_input.starts_with("https://")
    {
        // File or URL input - use read_input
        match read_input(&raw_input, url_timeout, url_max_size) {
            Ok(InputData::Text(text)) => (text, None, None),
            Ok(InputData::Binary { data, path }) => (String::new(), Some(data), Some(path)),
            Err(e) => {
                eprintln!("{}: {}", "error".red().bold(), e);
                std::process::exit(1);
            }
        }
    } else {
        // Direct text input (including piped text)
        (raw_input.clone(), None, None)
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
        verbose: cli.verbose > 0,
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

    let results = if let Some(ref data) = binary_data {
        // Binary data - use convert_bytes
        if let Some(ref from_format) = cli.from {
            forb.convert_bytes_filtered(data, std::slice::from_ref(from_format))
        } else {
            forb.convert_bytes_filtered(data, &format_filter)
        }
    } else if let Some(ref from_format) = cli.from {
        // Force specific format interpretation
        forb.convert_all_filtered(&input, std::slice::from_ref(from_format))
    } else {
        forb.convert_all_filtered(&input, &format_filter)
    };

    // Track format usage, conversion targets, and paths for analytics
    for result in &results {
        tracker.record_format_usage(&result.interpretation.source_format);
        for conv in &result.conversions {
            tracker.record_conversion_target(&conv.target_format);
            // Track full conversion paths (e.g., ["hex", "int-be", "epoch-seconds"])
            if conv.path.len() >= 2 {
                tracker.record_conversion_path(&conv.path);
            }
        }
    }

    if results.is_empty() {
        if cli.raw {
            // Silent failure for raw mode
            std::process::exit(1);
        }
        let display_input = if let Some(ref path) = file_path {
            path.as_str()
        } else if binary_data.is_some() {
            "(binary data)"
        } else {
            input.as_str()
        };

        // If a specific format was requested, try to show a validation error
        // (only for text input - binary validation not supported)
        if binary_data.is_none() {
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

    // For graph display, use file path if binary, otherwise input text
    let graph_label = if let Some(ref path) = file_path {
        path.clone()
    } else {
        input.clone()
    };

    // Handle --dot output
    if cli.dot {
        print_dot_graph(&graph_label, &results_to_show);
        return;
    }

    // Handle --mermaid output
    if cli.mermaid {
        print_mermaid_graph(&graph_label, &results_to_show);
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
            // Filter out hidden conversions (internal-only, don't add display value)
            let displayable_conversions: Vec<_> =
                result.conversions.iter().filter(|c| !c.hidden).collect();

            // Hash format IDs - shown at the bottom
            const HASH_FORMATS: &[&str] = &[
                "crc32",
                "md5",
                "sha1",
                "sha256",
                "sha512",
                "blake2b-256",
                "blake3",
            ];

            // Partition into: traits, hashes, and primary conversions
            let (traits, non_traits): (Vec<_>, Vec<_>) = displayable_conversions
                .iter()
                .partition(|c| c.kind == ConversionKind::Trait);

            let (hashes, primary): (Vec<&&Conversion>, Vec<&&Conversion>) = non_traits
                .into_iter()
                .partition(|c: &&&Conversion| HASH_FORMATS.contains(&c.target_format.as_str()));

            // Helper to display a single conversion
            let display_conversion = |conv: &Conversion| {
                let path_str = if conv.path.len() > 1 {
                    format!(" (via {})", conv.path.join(" → "))
                } else {
                    String::new()
                };

                let block_path_str = if pretty_config.show_paths && !conv.path.is_empty() {
                    format!(" {}", format!("[{}]", conv.path.join(":")).dimmed())
                } else {
                    String::new()
                };

                let display = format_conversion_display(
                    &conv.value,
                    &conv.display,
                    &conv.rich_display,
                    &pretty_config,
                );

                let kind_symbol = match conv.kind {
                    ConversionKind::Conversion => "→".cyan(),
                    ConversionKind::Representation => "≈".blue(),
                    ConversionKind::Trait => "✓".magenta(),
                };

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
            };

            // 1. Display traits first - grouped on one line unless verbose
            if !traits.is_empty() {
                if pretty_config.verbose {
                    for conv in &traits {
                        let path_str = if conv.path.len() > 1 {
                            format!(" (via {})", conv.path.join(" → "))
                        } else {
                            String::new()
                        };
                        let block_path_str = if pretty_config.show_paths && !conv.path.is_empty() {
                            format!(" {}", format!("[{}]", conv.path.join(":")).dimmed())
                        } else {
                            String::new()
                        };
                        println!(
                            "  {} {}: {}{}{}",
                            "✓".magenta(),
                            conv.target_format.yellow(),
                            conv.display,
                            path_str.dimmed(),
                            block_path_str
                        );
                    }
                } else {
                    let trait_displays: Vec<String> =
                        traits.iter().map(|c| c.display.clone()).collect();
                    println!("  {} {}", "✓".magenta(), trait_displays.join(", "));
                }
            }

            // 2. Display primary conversions (non-traits, non-hashes)
            let primary_to_show: Vec<_> = if limit == 0 {
                primary
            } else {
                // Reserve some slots for hashes if limit is applied
                let primary_limit = if limit > 3 { limit - 3 } else { limit };
                primary.into_iter().take(primary_limit).collect()
            };

            for conv in &primary_to_show {
                display_conversion(conv);
            }

            // 3. Display hashes last
            let hashes_to_show: Vec<_> = if limit == 0 {
                hashes
            } else {
                // Show remaining slots for hashes
                let used = primary_to_show.len();
                let remaining = limit.saturating_sub(used);
                hashes.into_iter().take(remaining).collect()
            };

            for conv in &hashes_to_show {
                display_conversion(conv);
            }

            // Show how many more are hidden (use hidden field, not hardcoded format names)
            let total_primary = result
                .conversions
                .iter()
                .filter(|c| {
                    !c.hidden
                        && c.kind != ConversionKind::Trait
                        && !HASH_FORMATS.contains(&c.target_format.as_str())
                })
                .count();
            let total_hashes = result
                .conversions
                .iter()
                .filter(|c| !c.hidden && HASH_FORMATS.contains(&c.target_format.as_str()))
                .count();
            let shown = primary_to_show.len() + hashes_to_show.len();
            let hidden_count = (total_primary + total_hashes).saturating_sub(shown);
            if hidden_count > 0 {
                println!(
                    "  {} {}",
                    "…".dimmed(),
                    format!("({} more, use -l 0 to show all)", hidden_count).dimmed()
                );
            }
        }
        println!();
    }

    // Background update check (after output, to stderr)
    // Skip for JSON output, raw output, pipe mode, or when updates are disabled
    if file_config.updates_enabled() && !cli.json && !cli.raw {
        if let Some(new_version) = check_for_updates_background() {
            use updates::{InstallMethod, VERSION};
            let hint = InstallMethod::detect().upgrade_hint();
            eprintln!(
                "Update available: v{} → v{} ({})",
                VERSION, new_version, hint
            );
        }
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

/// Handle --check-updates command.
fn handle_check_updates() {
    use colored::Colorize;
    use updates::{InstallMethod, VERSION};

    println!("Checking for updates...");

    match updates::check_for_update() {
        Ok(Some(new_version)) => {
            let hint = InstallMethod::detect().upgrade_hint();
            println!(
                "{} Update available: {} (you have {})",
                "✓".green().bold(),
                format!("v{}", new_version).green().bold(),
                format!("v{}", VERSION).dimmed()
            );
            println!("  Upgrade: {}", hint.cyan());

            // Update the cache with this result
            let mut data = analytics::AnalyticsData::load();
            data.last_version_check = Some(chrono::Utc::now());
            data.latest_known_version = Some(new_version);
            let _ = data.save();
        }
        Ok(None) => {
            println!(
                "{} You're on the latest version ({})",
                "✓".green().bold(),
                format!("v{}", VERSION).green()
            );

            // Update the cache
            let mut data = analytics::AnalyticsData::load();
            data.last_version_check = Some(chrono::Utc::now());
            data.latest_known_version = Some(VERSION.to_string());
            let _ = data.save();
        }
        Err(e) => {
            eprintln!(
                "{}: Failed to check for updates: {}",
                "error".red().bold(),
                e
            );
            std::process::exit(1);
        }
    }
}

/// Check for updates in background (cached, silent on errors).
///
/// Returns `Some(new_version)` if an update is available and should be shown.
fn check_for_updates_background() -> Option<String> {
    use updates::VERSION;

    let data = analytics::AnalyticsData::load();

    // Check if we should fetch (24h since last check)
    if !updates::should_check(data.last_version_check) {
        // Use cached result if available
        if let Some(ref cached) = data.latest_known_version {
            return updates::compare_versions(VERSION, cached);
        }
        return None;
    }

    // Fetch latest version (silently fail on errors)
    let latest = updates::fetch_latest_version().ok()?;

    // Update cache
    let mut data = data;
    data.last_version_check = Some(chrono::Utc::now());
    data.latest_known_version = Some(latest.clone());
    let _ = data.save();

    // Return if newer
    updates::compare_versions(VERSION, &latest)
}

/// Handle --graph command for static format graphs.
fn handle_graph_command(forb: &Formatorbit, mode: &str, use_dot: bool) {
    use colored::Colorize;

    match mode {
        "schema" => {
            let (infos, edges) = graph::build_schema_graph(forb);
            if use_dot {
                println!("{}", graph::schema_to_dot(&infos, &edges));
            } else {
                println!("{}", graph::schema_to_mermaid(&infos, &edges));
            }
        }
        "category" => {
            let edges = graph::build_category_graph(forb);
            if use_dot {
                println!("{}", graph::category_to_dot(&edges));
            } else {
                println!("{}", graph::category_to_mermaid(&edges));
            }
        }
        format_id => {
            // Treat as a format ID
            if !forb.is_valid_format(format_id) {
                eprintln!("{}: Unknown format '{}'\n", "error".red().bold(), format_id);
                eprintln!("Use --formats to see available formats, or try:");
                eprintln!("  --graph schema   - Show all formats");
                eprintln!("  --graph category - Show category relationships");
                std::process::exit(1);
            }
            let (related, incoming, outgoing) = graph::build_format_graph(forb, format_id);
            if use_dot {
                println!(
                    "{}",
                    graph::format_to_dot(format_id, &related, &incoming, &outgoing)
                );
            } else {
                println!(
                    "{}",
                    graph::format_to_mermaid(format_id, &related, &incoming, &outgoing)
                );
            }
        }
    }
}

/// Handle --plugins command.
#[cfg(feature = "plugins")]
fn handle_plugins_command(cmd: &str) {
    use colored::Colorize;
    use formatorbit_core::plugin::{discovery, PluginRegistry, PythonRuntime};
    use std::collections::HashMap;

    match cmd {
        "list" | "" => {
            // Try to load plugins and list them
            let mut registry = PluginRegistry::new();
            match registry.load_default() {
                Ok(report) => {
                    if report.total_loaded() == 0 {
                        println!("{}", "Plugins".bold().underline());
                        println!();
                        println!("{}", "No plugins loaded. To get started:".dimmed());
                        println!();
                        println!("  {} Create plugin directory:", "1.".bold());
                        println!(
                            "     {}",
                            format!(
                                "mkdir -p \"{}\"",
                                discovery::default_plugin_dir()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|| "~/.config/forb/plugins/".to_string())
                            )
                            .cyan()
                        );
                        println!();
                        println!("  {} Copy a sample plugin:", "2.".bold());
                        println!(
                            "     {}",
                            "cp sample-plugins/math_ext.py.sample ~/.config/forb/plugins/math_ext.py"
                                .cyan()
                        );
                        println!();
                        println!("  {} Try it out:", "3.".bold());
                        println!("     {}", "forb \"factorial(10)\"".cyan());
                        println!("     {}", "forb \"PI * 2\"".cyan());
                        println!();
                        println!(
                            "See {} for documentation on creating plugins.",
                            "PLUGINS.md".yellow()
                        );
                        return;
                    }

                    // Group plugins by source file
                    let mut by_file: HashMap<String, Vec<_>> = HashMap::new();
                    for info in &report.plugins {
                        let file_name = info
                            .source_file
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        by_file.entry(file_name).or_default().push(info);
                    }

                    println!("{}", "Loaded Plugins".bold().underline());
                    println!();

                    for (_file_name, plugins) in &by_file {
                        // Get plugin metadata from first plugin in the file
                        let meta = &plugins[0].plugin_meta;
                        let source_path = &plugins[0].source_file;
                        println!(
                            "  {} {} {}",
                            "▶".blue(),
                            meta.name.bold(),
                            format!("v{}", meta.version).dimmed()
                        );
                        println!("    {}", source_path.display().to_string().dimmed());
                        if let Some(ref author) = meta.author {
                            println!("    {}", format!("by {}", author).dimmed());
                        }
                        if let Some(ref desc) = meta.description {
                            println!("    {}", desc.dimmed());
                        }
                        println!();

                        for info in plugins {
                            let type_label = if report.decoders.contains(&info.id) {
                                "decoder"
                            } else if report.expr_vars.contains(&info.id) {
                                "var"
                            } else if report.expr_funcs.contains(&info.id) {
                                "func"
                            } else if report.traits.contains(&info.id) {
                                "trait"
                            } else if report.visualizers.contains(&info.id) {
                                "visualizer"
                            } else if report.currencies.contains(&info.id) {
                                "currency"
                            } else {
                                "plugin"
                            };

                            print!("      {} {} ", "→".cyan(), info.name.yellow());
                            print!("{}", format!("[{}]", type_label).dimmed());
                            if let Some(ref desc) = info.description {
                                print!(" {}", desc.dimmed());
                            }
                            println!();
                        }
                        println!();
                    }

                    // Show errors if any
                    if !report.errors.is_empty() {
                        println!("  {} Errors:", "✗".red().bold());
                        for (path, err) in &report.errors {
                            println!(
                                "    {} {}",
                                path.file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .yellow(),
                                err.to_string().red()
                            );
                        }
                        println!();
                    }

                    println!(
                        "{} {} item(s) from {} file(s)",
                        "✓".green().bold(),
                        report.total_loaded(),
                        by_file.len()
                    );

                    // Show sample plugins (*.py.sample files)
                    if let Some(plugin_dir) = discovery::default_plugin_dir() {
                        let samples: Vec<_> = std::fs::read_dir(&plugin_dir)
                            .into_iter()
                            .flatten()
                            .flatten()
                            .filter_map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                if name.ends_with(".py.sample") {
                                    Some(name.strip_suffix(".py.sample")?.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if !samples.is_empty() {
                            println!();
                            println!(
                                "{}",
                                "Samples (run --plugins toggle <name> to enable):".dimmed()
                            );
                            for name in samples {
                                println!("  {} {}", "○".dimmed(), name.dimmed());
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}: Failed to load plugins: {}", "error".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
        "status" => {
            // Initialize Python runtime
            if let Err(e) = PythonRuntime::init() {
                eprintln!("{}: {}", "Python runtime".red().bold(), e);
                std::process::exit(1);
            }
            println!("{} Python runtime initialized", "✓".green().bold());

            // Load plugins and show detailed status
            let mut registry = PluginRegistry::new();
            match registry.load_default() {
                Ok(report) => {
                    println!();
                    println!("{}", "Plugin Status".bold().underline());
                    println!();

                    // Show plugin directories
                    let dirs = discovery::discover_plugin_dirs();
                    println!("  {} Plugin directories:", "▶".blue());
                    for dir in &dirs {
                        let exists = dir.exists();
                        let marker = if exists { "✓".green() } else { "✗".red() };
                        println!("    {} {}", marker, dir.display());
                    }
                    println!();

                    // Show loaded plugins count
                    println!(
                        "  Decoders:    {}",
                        report.decoders.len().to_string().green()
                    );
                    println!(
                        "  Expr vars:   {}",
                        report.expr_vars.len().to_string().green()
                    );
                    println!(
                        "  Expr funcs:  {}",
                        report.expr_funcs.len().to_string().green()
                    );
                    println!("  Traits:      {}", report.traits.len().to_string().green());
                    println!(
                        "  Visualizers: {}",
                        report.visualizers.len().to_string().green()
                    );
                    println!(
                        "  Currencies:  {}",
                        report.currencies.len().to_string().green()
                    );

                    // Show errors
                    if !report.errors.is_empty() {
                        println!();
                        println!("  {} Errors:", "✗".red().bold());
                        for (path, err) in &report.errors {
                            println!("    {}", path.display().to_string().yellow());
                            println!("      {}", err.to_string().red());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}: Failed to load plugins: {}", "error".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
        "path" => match discovery::default_plugin_dir() {
            Some(path) => println!("{}", path.display()),
            None => {
                eprintln!(
                    "{}: Cannot determine plugin directory",
                    "error".red().bold()
                );
                std::process::exit(1);
            }
        },
        other if other.starts_with("toggle ") || other.starts_with("toggle\t") => {
            // Handle toggle command
            let name = other.strip_prefix("toggle").unwrap().trim();
            handle_plugin_toggle(name);
        }
        other => {
            eprintln!(
                "{}: Unknown plugins command '{}'\n",
                "error".red().bold(),
                other
            );
            eprintln!("Available commands:");
            eprintln!("  --plugins              List loaded plugins");
            eprintln!("  --plugins status       Show detailed status with errors");
            eprintln!("  --plugins path         Show plugin directory path");
            eprintln!("  --plugins toggle NAME  Enable/disable a plugin");
            std::process::exit(1);
        }
    }
}

/// Handle --plugins toggle <name> command.
#[cfg(feature = "plugins")]
fn handle_plugin_toggle(name: &str) {
    use colored::Colorize;
    use formatorbit_core::plugin::discovery;

    // Normalize the name (strip .py, .sample suffixes)
    let base_name = name
        .strip_suffix(".py.sample")
        .or_else(|| name.strip_suffix(".sample"))
        .or_else(|| name.strip_suffix(".py"))
        .unwrap_or(name);

    let Some(plugin_dir) = discovery::default_plugin_dir() else {
        eprintln!(
            "{}: Cannot determine plugin directory",
            "error".red().bold()
        );
        std::process::exit(1);
    };

    // Look for the plugin in either state
    let active_path = plugin_dir.join(format!("{}.py", base_name));
    let sample_path = plugin_dir.join(format!("{}.py.sample", base_name));

    if active_path.exists() {
        // Currently active, disable it (add .sample)
        match std::fs::rename(&active_path, &sample_path) {
            Ok(()) => {
                println!(
                    "{} Disabled {} (renamed to {})",
                    "✓".green().bold(),
                    base_name.yellow(),
                    sample_path.file_name().unwrap().to_string_lossy().dimmed()
                );
            }
            Err(e) => {
                eprintln!("{}: Failed to rename plugin: {}", "error".red().bold(), e);
                std::process::exit(1);
            }
        }
    } else if sample_path.exists() {
        // Currently sample, enable it (remove .sample)
        match std::fs::rename(&sample_path, &active_path) {
            Ok(()) => {
                println!(
                    "{} Enabled {} (renamed to {})",
                    "✓".green().bold(),
                    base_name.yellow(),
                    active_path.file_name().unwrap().to_string_lossy().dimmed()
                );
            }
            Err(e) => {
                eprintln!("{}: Failed to rename plugin: {}", "error".red().bold(), e);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("{}: Plugin '{}' not found", "error".red().bold(), base_name);
        eprintln!();
        eprintln!("Looked for:");
        eprintln!("  {}", active_path.display());
        eprintln!("  {}", sample_path.display());
        eprintln!();
        eprintln!("Run {} to see available plugins.", "--plugins".cyan());
        std::process::exit(1);
    }
}

/// Handle --currency (show current target and available currencies).
fn handle_currency_info() {
    use colored::Colorize;
    use formatorbit_core::formats::{currency_expr, currency_rates};

    // Initialize plugins to get plugin currencies
    #[cfg(feature = "plugins")]
    {
        use formatorbit_core::plugin::PluginRegistry;
        let mut registry = PluginRegistry::new();
        let _ = registry.load_default(); // Ignore errors, just want to load currencies
    }

    // Get current target currency and source
    let (target, source) = currency_expr::get_target_currency_with_source();

    println!("{}", "Currency Expression Functions".bold().underline());
    println!();
    println!(
        "  {} {} (from {})",
        "Target currency:".cyan(),
        target.yellow().bold(),
        source.dimmed()
    );
    println!();
    println!(
        "Use {} to convert amounts to your target currency.",
        "USD(100), EUR(50), BTC(0.5)".yellow()
    );
    println!();
    println!("{}", "Available currencies:".cyan());

    // Built-in currencies
    let builtin = currency_expr::builtin_currency_codes();
    let builtin_display: Vec<_> = builtin.iter().take(15).copied().collect();
    let remaining = builtin.len().saturating_sub(15);
    print!("  Built-in: ");
    print!("{}", builtin_display.join(", "));
    if remaining > 0 {
        print!(" {}", format!("(+{} more)", remaining).dimmed());
    }
    println!();

    // Plugin currencies
    let plugin_codes = currency_rates::plugin_currency_codes();
    if !plugin_codes.is_empty() {
        print!("  Plugins:  ");
        println!("{}", plugin_codes.join(", ").yellow());
    }

    println!();
    println!("{}", "Configuration:".cyan());
    println!("  CLI:     {} \"USD(100)\"", "--currency EUR".yellow());
    println!(
        "  Env:     {} forb \"USD(100)\"",
        "FORB_TARGET_CURRENCY=EUR".yellow()
    );
    println!(
        "  Config:  {} {} in config file",
        "[currency]".dimmed(),
        "target = \"EUR\"".yellow()
    );
    println!();
    println!(
        "Without explicit config, currency is detected from system locale or defaults to {}.",
        "USD".yellow()
    );
}
