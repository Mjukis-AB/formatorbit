use clap::Parser;
use colored::Colorize;
use formatorbit_core::Formatorbit;

const LONG_ABOUT: &str = r##"
Formatorbit automatically detects and converts data between formats.

Paste in data and see all possible interpretations - hex, base64, timestamps,
UUIDs, IP addresses, colors, MessagePack, and more.

SUPPORTED FORMATS:
  Encoding:     hex, base64, url-encoding
  Numbers:      decimal, big-endian int, little-endian int
  Time:         Unix epoch (sec/ms), Apple/Cocoa epoch, ISO 8601, RFC 2822
  Identifiers:  UUID (v1-v8 detection)
  Network:      IPv4, IPv6
  Colors:       #RGB, #RRGGBB, #RRGGBBAA, 0xAARRGGBB (Android)
  Data:         JSON, MessagePack, UTF-8

HEX INPUT FORMATS:
  forb supports multiple hex paste styles:

    691E01B8                    Continuous hex
    0x691E01B8                  With 0x prefix
    69 1E 01 B8                 Space-separated (hex dump style)
    69:1E:01:B8                 Colon-separated (MAC address style)
    69-1E-01-B8                 Dash-separated
    0x69, 0x1E, 0x01, 0xB8      Comma-separated
    {0x69, 0x1E, 0x01, 0xB8}    C array style
    [0x69, 0x1E, 0x01, 0xB8]    Bracket array style

EXAMPLES:
  forb 691E01B8                 Interpret hex bytes
  forb "87 A3 69 6E 74 01"      Parse space-separated hex (from hex dump)
  forb aR4BuA==                 Decode base64
  forb 1703456789               Interpret as Unix timestamp
  forb 192.168.1.1              Parse IP address
  forb '#FF5733'                Parse color (use quotes for #)
  forb 0x80FF5733               Android ARGB color
  forb 550e8400-e29b-41d4-a716-446655440000   Parse UUID

OUTPUT:
  Shows all possible interpretations ranked by confidence.
  Each interpretation shows available conversions (hex, base64, int, etc.)
  Use --json for machine-readable output."##;

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
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    /// Output results as JSON (for scripting/piping)
    #[arg(long, short = 'j')]
    json: bool,

    /// List all supported formats
    #[arg(long)]
    formats: bool,
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
        "Timestamps",
        "Identifiers",
        "Network",
        "Colors",
        "Data",
    ];

    for category in categories {
        let formats_in_cat: Vec<_> = infos
            .iter()
            .filter(|f| f.category == category)
            .collect();

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
                let examples: Vec<_> = info.examples.iter().take(3).map(|e| e.green().to_string()).collect();
                println!("      {}", format!("e.g. {}", examples.join(", ")).dimmed());
            }
        }
        println!();
    }

    println!("{}", "Hex Input Styles".bold().underline());
    println!("  The {} format accepts multiple common paste styles:", "hex".yellow());
    println!();
    println!("    {}           {}", "691E01B8".green(), "Continuous".dimmed());
    println!("    {}         {}", "0x691E01B8".green(), "With 0x prefix".dimmed());
    println!("    {}        {}", "69 1E 01 B8".green(), "Space-separated (hex dumps)".dimmed());
    println!("    {}        {}", "69:1E:01:B8".green(), "Colon-separated (MAC address)".dimmed());
    println!("    {}        {}", "69-1E-01-B8".green(), "Dash-separated".dimmed());
    println!("    {}   {}", "0x69, 0x1E, 0x01".green(), "Comma-separated".dimmed());
    println!("    {}  {}", "{0x69, 0x1E, 0x01}".green(), "C/C++ array style".dimmed());
}

fn main() {
    let cli = Cli::parse();

    if cli.formats {
        print_formats();
        return;
    }

    let Some(input) = cli.input else {
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
        eprintln!("Run {} for more information.", "forb --help".bold());
        std::process::exit(1);
    };

    let forb = Formatorbit::new();
    let results = forb.convert_all(&input);

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
        return;
    }

    if results.is_empty() {
        println!("No interpretations found for: {input}");
        return;
    }

    // Filter to show only high-confidence interpretations (skip utf8 fallback for hex-like input)
    let meaningful_results: Vec<_> = results
        .iter()
        .filter(|r| r.interpretation.confidence > 0.2)
        .collect();

    let results_to_show = if meaningful_results.is_empty() {
        results.iter().collect()
    } else {
        meaningful_results
    };

    for result in results_to_show {
        let conf = (result.interpretation.confidence * 100.0) as u32;
        println!(
            "{} {} ({}% confidence)",
            "▶".blue(),
            result.interpretation.source_format.green().bold(),
            conf
        );
        println!(
            "  {}",
            result.interpretation.description.dimmed()
        );

        if result.conversions.is_empty() {
            println!("  {}", "(no conversions available)".dimmed());
        } else {
            for conv in &result.conversions {
                let path_str = if conv.path.len() > 1 {
                    format!(" (via {})", conv.path.join(" → "))
                } else {
                    String::new()
                };

                println!(
                    "  {} {}: {}{}",
                    "→".cyan(),
                    conv.target_format.yellow(),
                    conv.display,
                    path_str.dimmed()
                );
            }
        }
        println!();
    }
}
