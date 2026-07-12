//! Handlers for standalone CLI subcommands that run and exit without processing
//! an input value: `--formats`, `--analytics`, `--check-updates`, `--graph`,
//! `--plugins`, and `--currency` (info mode). Also hosts the background update
//! check used after normal output.

use colored::Colorize;
use formatorbit_core::Formatorbit;

use crate::config::Config;
use crate::{analytics, graph, updates};

/// Print the list of all supported formats grouped by category.
pub fn print_formats() {
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
        "Scheduling",
        "Hashing",
        "Identifiers",
        "Network",
        "Web",
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

/// Handle analytics subcommand.
pub fn handle_analytics_command(cmd: &str) {
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
pub fn handle_check_updates() {
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
pub fn check_for_updates_background() -> Option<String> {
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
pub fn handle_graph_command(forb: &Formatorbit, mode: &str, use_dot: bool) {
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
pub fn handle_plugins_command(cmd: &str) {
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

                    for plugins in by_file.values() {
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
pub fn handle_currency_info() {
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
