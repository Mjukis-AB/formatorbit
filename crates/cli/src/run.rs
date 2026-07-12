//! Top-level run orchestration: parse args, resolve configuration, acquire
//! input, run the conversion, and render output.

use std::io::{self, IsTerminal, Read};

use clap::{CommandFactory, Parser};
use colored::{control::set_override, Colorize};
use formatorbit_core::{Conversion, ConversionKind, Formatorbit};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

use crate::cli::{parse_size, Cli};
use crate::commands;
use crate::config::{self, Config};
use crate::input::{read_input, InputData};
use crate::man::{generate_man_page, install_man_page, show_with_pager};
use crate::pretty::{PacketMode, PrettyConfig};
use crate::render::{format_conversion_display, print_dot_graph, print_mermaid_graph};
use crate::{analytics, pipe};

/// Entry point body: everything `main` does after the manual `--help`/`--version`
/// short-circuit.
pub fn run() {
    // Handle --help and --version manually to support pager
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        let help = Cli::command().render_long_help().to_string();
        show_with_pager(&help);
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

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
        commands::handle_analytics_command(cmd);
        return;
    }

    // Handle --check-updates (explicit update check)
    if cli.check_updates {
        commands::handle_check_updates();
        return;
    }

    // Handle --man (output man page)
    if cli.man {
        let man_page = generate_man_page();
        show_with_pager(&man_page);
        return;
    }

    // Handle --install-man (install man page)
    if cli.install_man {
        match install_man_page() {
            Ok(path) => {
                println!("Installed man page to: {}", path.display());
                println!();
                println!("To use it, add to your shell config:");
                println!("  export MANPATH=\"$HOME/.local/share/man:$MANPATH\"");
                println!();
                println!("Then run: man forb");
            }
            Err(e) => {
                eprintln!("{}: {}", "error".red().bold(), e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Handle --plugins (plugin management)
    #[cfg(feature = "plugins")]
    if let Some(ref cmd) = cli.plugins {
        commands::handle_plugins_command(cmd);
        return;
    }

    // Handle --currency (show info or set target)
    if let Some(ref code) = cli.currency {
        if code.is_empty() {
            // Show current target currency and available currencies
            commands::handle_currency_info();
            return;
        }
        // Set target currency for this run (will be applied after config loading)
    }

    // Handle --graph (static format graph, no input needed)
    if let Some(ref mode) = cli.graph {
        let forb = Formatorbit::new();
        commands::handle_graph_command(&forb, mode, cli.dot);
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
        commands::print_formats();
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
            // Bare piped multi-line input is analyzed as a single text blob.
            // Users who piped a log likely wanted per-line annotations, so
            // hint at --tee. Non-breaking: behavior is unchanged, hint goes to
            // stderr, and only when the output is not machine-readable.
            if trimmed.contains('\n') && !cli.json && !cli.raw {
                eprintln!(
                    "{}: multi-line input is analyzed as one blob; use {} for per-line annotations (e.g. {})",
                    "hint".yellow().bold(),
                    "--tee".bold(),
                    "cat file | forb --tee".bold()
                );
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
                // Reserve slots for hashes only if any actually exist, and never
                // reserve more than there are hashes (up to 3). This keeps the
                // high-value primary conversions visible instead of wasting slots
                // on hashes that don't exist (e.g. `forb 691E01B8`).
                let reserved = hashes.len().min(3);
                let primary_limit = limit.saturating_sub(reserved).max(1);
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
        if let Some(new_version) = commands::check_for_updates_background() {
            use crate::updates::{InstallMethod, VERSION};
            let hint = InstallMethod::detect().upgrade_hint();
            eprintln!(
                "Update available: v{} → v{} ({})",
                VERSION, new_version, hint
            );
        }
    }
}
