//! Command-line argument definitions and small argument helpers.

use clap::Parser;

pub const LONG_ABOUT: &str = r##"
Formatorbit automatically detects and converts data between formats.

Paste in data and see all possible interpretations - hex, base64, timestamps,
UUIDs, colors, math expressions, currencies, units, and more.

SUPPORTED FORMATS:
  Encoding:     hex, base64, binary, octal, url-encoding, escape sequences
  Numbers:      decimal, data sizes (1MB, 1MiB), temperature (30°C, 86°F)
  Math:         expressions (2 + 2, 0xFF + 1, 1 << 8)
  Units:        length, weight, volume, speed, pressure, energy, angle, area
  Currency:     100 USD, $50, 5kEUR, 2.5MSEK (with live exchange rates)
  Time:         Unix epoch (sec/ms), durations (1h30m), ISO 8601, cron (*/5 * * * *)
  Hashing:      MD5, SHA-1, SHA-256, SHA-512, Blake2b, Blake3, CRC32
  Identifiers:  UUID (v1-v8), ULID, NanoID, CUID2, JWT
  Network:      IPv4, IPv6, MAC address (with OUI vendor lookup)
  Web:          URL parsing (with tracking parameter removal)
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
  forb '00:1A:2B:3C:4D:5E'      MAC address with vendor lookup
  forb '*/5 * * * *'            Cron expression schedule
  forb 'https://x.com?utm_source=ads'  URL with tracking removal

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
#[command(after_help = "For more information, visit: https://github.com/mjukis-ab/formatorbit")]
pub struct Cli {
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
    pub input: Option<String>,

    /// Output results as JSON (for scripting/piping)
    #[arg(long, short = 'j')]
    pub json: bool,

    /// List all supported formats
    #[arg(long)]
    pub formats: bool,

    // === Tee mode options ===
    /// Tee mode: pass through stdin while annotating interesting values
    ///
    /// Like Unix `tee`, but annotates timestamps, UUIDs, hex, etc. inline.
    /// Useful for live log monitoring: tail -f app.log | forb --tee
    #[arg(long, short = 'T')]
    pub tee: bool,

    /// Minimum confidence threshold for showing annotations (0.0-1.0)
    ///
    /// In tee mode, only values with confidence >= threshold are annotated.
    #[arg(long, short = 't')]
    pub threshold: Option<f32>,

    /// Highlight interesting values inline with color
    ///
    /// In tee mode, highlights matched tokens with background color.
    #[arg(long, short = 'H')]
    pub highlight: bool,

    /// Only use specific formats (comma-separated, supports aliases)
    ///
    /// Examples: --only uuid,hex,ts  or  -o b64,ip
    /// Use --formats to see available format IDs and aliases.
    #[arg(long, short = 'o', value_delimiter = ',')]
    pub only: Option<Vec<String>>,

    /// Maximum conversions to show per interpretation (0 = unlimited)
    ///
    /// With priority sorting, the most valuable conversions come first.
    #[arg(long, short = 'l')]
    pub limit: Option<usize>,

    /// Force tee mode even when stdin is a TTY (for testing)
    #[arg(long, hide = true)]
    pub force_tee: bool,

    /// Maximum tokens to analyze per line in tee mode
    #[arg(long, hide = true)]
    pub max_tokens: Option<usize>,

    /// Disable colored output
    #[arg(long, short = 'C')]
    pub no_color: bool,

    /// Compact output for structured data (single line)
    #[arg(long, short = 'c')]
    pub compact: bool,

    /// Output only raw converted values (for scripting)
    ///
    /// Outputs just the conversion values without labels or formatting.
    /// Combine with --only to get specific format output.
    #[arg(long, short = 'r')]
    pub raw: bool,

    /// Show only the highest-confidence interpretation
    #[arg(long, short = '1')]
    pub first: bool,

    /// Force input to be interpreted as a specific format
    ///
    /// Skip auto-detection and treat input as the specified format.
    /// Use --formats to see available format IDs.
    #[arg(long, short = 'f', value_name = "FORMAT")]
    pub from: Option<String>,

    /// Output conversion graph in Graphviz DOT format
    ///
    /// Pipe to dot to render: forb --dot INPUT | dot -Tpng > graph.png
    #[arg(long)]
    pub dot: bool,

    /// Output conversion graph in Mermaid format (renders in GitHub/GitLab)
    #[arg(long)]
    pub mermaid: bool,

    /// Show blockable path for each conversion (for configuring blocking)
    ///
    /// Displays the path in [source:target] format that can be copied
    /// directly into config.toml [blocking] paths array.
    #[arg(long)]
    pub show_paths: bool,

    /// Minimum confidence for reinterpreting decoded strings (0.0-1.0)
    ///
    /// When hex/base64 decodes to a string, try parsing that string as
    /// other formats (UUID, IP, JSON, etc). Only formats with confidence
    /// >= this threshold are explored. Set to 1.0 to disable.
    #[arg(long, value_name = "THRESHOLD")]
    pub reinterpret_threshold: Option<f32>,

    /// Show packet layout for binary formats (protobuf, msgpack)
    ///
    /// Displays byte-level structure with offsets, lengths, and decoded values.
    /// Use --packet=compact for inline horizontal format or --packet=detailed for table format.
    #[arg(long, short = 'p', value_name = "MODE", num_args = 0..=1, default_missing_value = "compact")]
    pub packet: Option<String>,

    /// Enable verbose logging (use multiple times for more detail)
    ///
    /// -v shows debug messages, -vv shows trace messages.
    /// Useful for understanding why something was or wasn't matched.
    #[arg(long, short = 'v', action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Timeout for URL fetches in seconds
    #[arg(long, value_name = "SECS")]
    pub url_timeout: Option<u64>,

    /// Maximum response size for URL fetches (e.g., 10M, 50M, 1G)
    #[arg(long, value_name = "SIZE")]
    pub url_max_size: Option<String>,

    /// Show config file path
    #[arg(long)]
    pub config_path: bool,

    /// Generate default config file (see --config-path for location)
    #[arg(long)]
    pub config_init: bool,

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
    pub analytics: Option<String>,

    /// Show format conversion graph (without input data)
    ///
    /// Modes:
    ///   schema   - All formats and conversion edges (large graph)
    ///   category - Category-level relationships
    ///   FORMAT   - Show what a specific format can convert to/from
    ///
    /// Output format controlled by --dot (default: mermaid)
    #[arg(long, value_name = "MODE")]
    pub graph: Option<String>,

    /// Check for available updates
    #[arg(long)]
    pub check_updates: bool,

    /// Output man page to stdout (pipe to less or save to file)
    #[arg(long, hide = true)]
    pub man: bool,

    /// Install man page to ~/.local/share/man/man1/forb.1
    #[arg(long, hide = true)]
    pub install_man: bool,

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
    pub plugins: Option<String>,

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
    pub currency: Option<String>,
}

/// Parse size string like "10M", "50M", "1G" into bytes.
pub fn parse_size(s: &str) -> Result<u64, String> {
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
