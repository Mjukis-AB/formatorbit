//! Conversion graph traversal using BFS.
//!
//! This module finds all possible conversions from a value by traversing
//! a graph where nodes are values and edges are format conversions.

use std::collections::VecDeque;

use crate::format::Format;
use crate::types::{
    BlockingConfig, Conversion, ConversionConfig, ConversionKind, ConversionPriority,
    ConversionStep, CoreValue, PriorityConfig,
};

/// Maximum BFS depth to prevent infinite loops in conversion graph traversal.
const MAX_BFS_DEPTH: usize = 5;

/// Unit format IDs that shouldn't cross-convert to each other.
const UNIT_FORMATS: &[&str] = &[
    "length",
    "weight",
    "volume",
    "speed",
    "pressure",
    "angle",
    "area",
    "energy",
    "temperature",
];

/// Target format IDs produced by unit conversions.
const UNIT_TARGETS: &[&str] = &[
    // Length
    "meters",
    "kilometers",
    "centimeters",
    "millimeters",
    "feet",
    "miles",
    "inches",
    // Weight
    "grams",
    "kilograms",
    "milligrams",
    "pounds",
    "ounces",
    // Volume
    "milliliters",
    "liters",
    "gallons",
    "fluid ounces",
    "cups",
    // Speed
    "m/s",
    "km/h",
    "mph",
    "knots",
    // Pressure
    "pascals",
    "kilopascals",
    "megapascals",
    "bar",
    "psi",
    "atmospheres",
    // Angle
    "degrees",
    "radians",
    "gradians",
    "turns",
    // Area
    "square meters",
    "square kilometers",
    "square centimeters",
    "square feet",
    "acres",
    "hectares",
    // Energy
    "joules",
    "kilojoules",
    "megajoules",
    "calories",
    "kilocalories",
    "kilowatt-hours",
    // Temperature
    "celsius",
    "fahrenheit",
    "kelvin",
];

/// Root-based blocking: targets that should never be reached from a given root interpretation.
/// Unlike BLOCKED_PATHS which blocks immediate source→target, this blocks the target
/// regardless of the path taken (e.g., "text" blocks ipv4 via text→bytes→ipv4).
const ROOT_BLOCKED_TARGETS: &[(&str, &str)] = &[
    // Text bytes shouldn't be interpreted as IP addresses
    // (4 bytes of ASCII like "test" aren't an IPv4 address)
    ("text", "ipv4"),
    ("text", "ipv6"),
    // Text bytes shouldn't be interpreted as colors
    ("text", "color-rgb"),
    ("text", "color-hex"),
    ("text", "color-hsl"),
    // Text bytes shouldn't be interpreted as integers or timestamps
    // (already blocked via BLOCKED_PATHS for immediate, but this catches all paths)
    ("text", "int-be"),
    ("text", "int-le"),
    ("text", "epoch-seconds"),
    ("text", "epoch-millis"),
    ("text", "apple-cocoa"),
    ("text", "filetime"),
    ("text", "duration"),
    ("text", "duration-ms"),
    ("text", "datasize"),
    ("text", "datasize-iec"),
    ("text", "datasize-si"),
    // Text bytes shouldn't be interpreted as UUIDs
    // (any 16 bytes can be formatted as UUID, but "🏳️‍🌈oj" isn't a UUID)
    ("text", "uuid"),
    // Hex bytes shouldn't be interpreted as IP addresses
    // (DEADBEEF as bytes isn't an IP like 222.173.190.239)
    ("hex", "ipv4"),
    ("hex", "ipv6"),
    ("hex", "ip"),
    // Hex bytes shouldn't be interpreted as colors
    // (use #DEADBEEF explicitly for color interpretation)
    ("hex", "color-rgb"),
    ("hex", "color-hsl"),
];

/// Nonsensical source→target combinations to filter out.
/// These are conversions that technically work but are never useful.
const BLOCKED_PATHS: &[(&str, &str)] = &[
    // IP addresses aren't msgpack-encoded data
    ("ipv4", "msgpack"),
    ("ipv6", "msgpack"),
    // UUIDs aren't msgpack-encoded data
    ("uuid", "msgpack"),
    // IP addresses aren't timestamps
    ("ipv4", "epoch-seconds"),
    ("ipv4", "epoch-millis"),
    ("ipv4", "apple-cocoa"),
    ("ipv4", "filetime"),
    // UUIDs aren't timestamps (except v1, but that's handled separately)
    ("uuid", "epoch-seconds"),
    ("uuid", "epoch-millis"),
    ("uuid", "apple-cocoa"),
    ("uuid", "filetime"),
    // Expression results - filter noise, keep primary result and hex/binary/octal representations
    ("expr", "msgpack"),
    ("expr", "octal"),
    ("expr", "datasize"),
    ("expr", "datasize-iec"),
    ("expr", "datasize-si"),
    ("expr", "duration"),
    ("expr", "duration-ms"),
    ("expr", "decimal"),
    // Data sizes aren't durations
    ("datasize", "duration"),
    ("datasize", "duration-ms"),
    // Durations aren't data sizes or re-interpreted as different time scales
    ("duration", "datasize"),
    ("duration", "datasize-iec"),
    ("duration", "datasize-si"),
    ("duration", "duration-ms"),
    // Colors aren't timestamps or data sizes
    ("color-hex", "duration"),
    ("color-hex", "duration-ms"),
    ("color-hex", "datasize"),
    ("color-hex", "datasize-iec"),
    ("color-hex", "datasize-si"),
    ("color-rgb", "duration"),
    ("color-rgb", "duration-ms"),
    ("color-rgb", "datasize"),
    ("color-rgb", "datasize-iec"),
    ("color-rgb", "datasize-si"),
    ("color-hsl", "duration"),
    ("color-hsl", "duration-ms"),
    ("color-hsl", "datasize"),
    ("color-hsl", "datasize-iec"),
    ("color-hsl", "datasize-si"),
    // Hexdump output is for display only - don't re-encode it
    ("hexdump", "bytes"),
    ("hexdump", "url-encoded"),
    ("hexdump", "escape-unicode"),
    ("hexdump", "escape-hex"),
    ("hexdump", "msgpack"),
    // URL-encoded shouldn't chain further (double/triple encoding is noise)
    ("url-encoded", "url-encoded"),
    ("url-encoded", "bytes"),
    ("url-encoded", "escape-unicode"),
    ("url-encoded", "escape-hex"),
    // Plain text shouldn't produce noisy intermediate conversions
    ("text", "url-encoded"),
    ("text", "graph"),
    ("text", "text"),
    ("text", "msgpack"),
    ("text", "escape-unicode"),
    // Escape sequences are terminal display formats
    ("escape-hex", "bytes"),
    ("escape-hex", "url-encoded"),
    ("escape-unicode", "bytes"),
    ("escape-unicode", "url-encoded"),
    // Text bytes shouldn't be interpreted as integers (the bytes represent characters, not numbers)
    ("text", "int-be"),
    ("text", "int-le"),
    // Circular: text → bytes → utf8 just produces the original text again
    ("text", "utf8"),
    // Text bytes shouldn't be interpreted as IP addresses or colors
    // (4 bytes of ASCII text like "test" aren't an IPv4 address or RGBA color)
    ("text", "ipv4"),
    ("text", "ipv6"),
    ("text", "color-rgb"),
    ("text", "color-hex"),
    ("text", "color-hsl"),
];

/// Check if a source→target conversion should be blocked (hardcoded rules only).
fn is_blocked_path_builtin(source_format: &str, target_format: &str) -> bool {
    // Check explicit blocked paths
    if BLOCKED_PATHS
        .iter()
        .any(|(src, tgt)| source_format == *src && target_format == *tgt)
    {
        return true;
    }

    // Block unit format cross-conversions
    // (e.g., length -> temperature targets like "celsius")
    if UNIT_FORMATS.contains(&source_format) && UNIT_TARGETS.contains(&target_format) {
        // Check if target belongs to a different unit type
        // Allow same-type conversions (length -> meters, etc.)
        let source_owns_target = match source_format {
            "length" => matches!(
                target_format,
                "meters"
                    | "kilometers"
                    | "centimeters"
                    | "millimeters"
                    | "feet"
                    | "miles"
                    | "inches"
            ),
            "weight" => matches!(
                target_format,
                "grams" | "kilograms" | "milligrams" | "pounds" | "ounces"
            ),
            "volume" => matches!(
                target_format,
                "milliliters" | "liters" | "gallons" | "fluid ounces" | "cups"
            ),
            "speed" => matches!(target_format, "m/s" | "km/h" | "mph" | "knots"),
            "pressure" => matches!(
                target_format,
                "pascals" | "kilopascals" | "megapascals" | "bar" | "psi" | "atmospheres"
            ),
            "angle" => matches!(target_format, "degrees" | "radians" | "gradians" | "turns"),
            "area" => matches!(
                target_format,
                "square meters"
                    | "square kilometers"
                    | "square centimeters"
                    | "square feet"
                    | "acres"
                    | "hectares"
            ),
            "energy" => matches!(
                target_format,
                "joules"
                    | "kilojoules"
                    | "megajoules"
                    | "calories"
                    | "kilocalories"
                    | "kilowatt-hours"
            ),
            "temperature" => matches!(target_format, "celsius" | "fahrenheit" | "kelvin"),
            _ => false,
        };
        if !source_owns_target {
            return true;
        }
    }

    false
}

/// Check if a target is blocked based on root interpretation (builtin rules).
fn is_root_blocked_builtin(root_format: &str, target_format: &str) -> bool {
    ROOT_BLOCKED_TARGETS
        .iter()
        .any(|(root, target)| root_format == *root && target_format == *target)
}

/// Check if a conversion should be blocked (builtin rules + user config).
fn is_blocked(
    source_format: &str,
    target_format: &str,
    root_format: Option<&str>,
    path: &[String],
    blocking: Option<&BlockingConfig>,
) -> bool {
    // Check builtin blocked paths (immediate source→target)
    if is_blocked_path_builtin(source_format, target_format) {
        return true;
    }

    // Check builtin root-based blocking (root→...→target)
    if let Some(root) = root_format {
        if is_root_blocked_builtin(root, target_format) {
            return true;
        }
    }

    // Check user-configured blocking
    if let Some(config) = blocking {
        // Check if target format is blocked
        if config.is_format_blocked(target_format) {
            return true;
        }
        // Check if this path is blocked
        if config.is_path_blocked(path) {
            return true;
        }
        // Check root-based blocking from user config
        if let Some(root) = root_format {
            if config.is_root_blocked(root, target_format) {
                return true;
            }
        }
    }

    false
}

/// Find all possible conversions from a value using BFS.
///
/// This traverses the conversion graph, collecting all reachable formats.
/// The path is tracked to show how we got from the source to each target.
/// If `exclude_format` is provided, skip conversions to that format (to avoid hex→hex etc.)
/// If `source_format` is provided, it's included as the first element in the path.
/// If `config` is provided, user-configured blocking and priority settings are applied.
pub fn find_all_conversions(
    formats: &[Box<dyn Format>],
    initial: &CoreValue,
    exclude_format: Option<&str>,
    source_format: Option<&str>,
    config: Option<&ConversionConfig>,
) -> Vec<Conversion> {
    let blocking = config.map(|c| &c.blocking);
    let priority = config.map(|c| &c.priority);
    let mut results = Vec::new();
    // Track seen conversions by (target_format, display) to allow different values
    // for the same format (e.g., int-be → epoch vs int-le → epoch with different dates)
    let mut seen_results: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    // Track seen formats for BFS exploration (to prevent infinite loops)
    // We use a separate set here because we still want to explore from a format only once
    // per unique value, but we want to show all unique results.
    let mut seen_for_bfs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    // Pre-exclude the source format if specified
    if let Some(excluded) = exclude_format {
        // For the source format, we block all display values
        seen_results.insert((excluded.to_string(), String::new()));
    }

    // Queue holds (value, path_so_far, steps_so_far)
    let mut queue: VecDeque<(CoreValue, Vec<String>, Vec<ConversionStep>)> = VecDeque::new();

    // Initialize with source format if provided, so paths show the full chain
    let initial_path = source_format
        .map(|s| vec![s.to_string()])
        .unwrap_or_default();
    queue.push_back((initial.clone(), initial_path, vec![]));

    // Also format the initial value with all applicable formats
    for format in formats {
        if format.can_format(initial) {
            if let Some(display) = format.format(initial) {
                let format_id = format.id().to_string();
                let result_key = (format_id.clone(), display.clone());
                if seen_results.insert(result_key) {
                    // Build path including source format if provided
                    let mut path = source_format
                        .map(|s| vec![s.to_string()])
                        .unwrap_or_default();
                    path.push(format_id.clone());

                    results.push(Conversion {
                        value: initial.clone(),
                        target_format: format_id.clone(),
                        display: display.clone(),
                        path,
                        steps: vec![ConversionStep {
                            format: format_id,
                            value: initial.clone(),
                            display,
                        }],
                        is_lossy: false,
                        priority: ConversionPriority::default(),
                        display_only: false,
                        kind: ConversionKind::default(),
                        hidden: false,
                        rich_display: vec![],
                    });
                }
            }
        }
    }

    // Get reinterpret threshold from config (default 0.7)
    let reinterpret_threshold = config.map(|c| c.reinterpret_threshold()).unwrap_or(0.7);

    // BFS through conversions
    let mut depth = 0;

    while !queue.is_empty() && depth < MAX_BFS_DEPTH {
        let level_size = queue.len();

        for _ in 0..level_size {
            let Some((current_value, current_path, current_steps)) = queue.pop_front() else {
                break;
            };

            // Get the immediate source format (last element of current path, or root)
            let immediate_source = current_path.last().map(|s| s.as_str()).unwrap_or("");

            // String reinterpretation: when we have a decoded string (not from original input),
            // try parsing it as other formats (UUID, IP, JSON, datetime, etc.)
            if let CoreValue::String(s) = &current_value {
                // Only reinterpret if this came from a conversion (not original input)
                // and reinterpretation is enabled (threshold < 1.0)
                if !current_path.is_empty() && reinterpret_threshold < 1.0 {
                    for format in formats {
                        // Skip text format to prevent infinite loops
                        if format.id() == "text" {
                            continue;
                        }

                        for interp in format.parse(s) {
                            // Only consider high-confidence interpretations
                            if interp.confidence < reinterpret_threshold {
                                continue;
                            }

                            let target_format = interp.source_format.clone();

                            // Check blocking - but skip root-based blocking for string reinterpretation
                            // because we're now in a different semantic domain (text content, not raw bytes).
                            // E.g., hex bytes→ipv4 is blocked, but hex→utf8("192.168.1.1")→ipv4 should be allowed.
                            if is_blocked(
                                immediate_source,
                                &target_format,
                                None, // Skip root blocking for string reinterpretation
                                &current_path,
                                blocking,
                            ) {
                                continue;
                            }

                            // Format the interpreted value for display
                            let display = format
                                .format(&interp.value)
                                .unwrap_or_else(|| interp.description.clone());

                            let result_key = (target_format.clone(), display.clone());
                            let bfs_key = (target_format.clone(), display.clone());

                            // Build path
                            let mut full_path = current_path.clone();
                            full_path.push(target_format.clone());

                            // Build steps
                            let mut full_steps = current_steps.clone();
                            full_steps.push(ConversionStep {
                                format: target_format.clone(),
                                value: interp.value.clone(),
                                display: display.clone(),
                            });

                            // Add to results
                            if seen_results.insert(result_key) {
                                results.push(Conversion {
                                    value: interp.value.clone(),
                                    target_format: target_format.clone(),
                                    display: display.clone(),
                                    path: full_path.clone(),
                                    steps: full_steps.clone(),
                                    is_lossy: false,
                                    priority: ConversionPriority::Structured,
                                    kind: ConversionKind::Conversion,
                                    display_only: false,
                                    hidden: false,
                                    rich_display: interp.rich_display.clone(),
                                });
                            }

                            // Add to queue for further exploration
                            if seen_for_bfs.insert(bfs_key) {
                                queue.push_back((interp.value, full_path, full_steps));
                            }
                        }
                    }
                }
            }

            // Get conversions from all formats
            for format in formats {
                for conv in format.conversions(&current_value) {
                    // Check blocking early (before adding to results or queue)
                    if is_blocked(
                        immediate_source,
                        &conv.target_format,
                        source_format,
                        &current_path,
                        blocking,
                    ) {
                        continue;
                    }

                    let result_key = (conv.target_format.clone(), conv.display.clone());
                    let bfs_key = (conv.target_format.clone(), conv.display.clone());

                    // Build the full path (format IDs only, for backwards compat)
                    let mut full_path = current_path.clone();
                    full_path.extend(conv.path.clone());

                    // Build the full steps (with values)
                    let mut full_steps = current_steps.clone();
                    // Add any intermediate steps from this conversion
                    for step in &conv.steps {
                        full_steps.push(step.clone());
                    }
                    // Add the final step if not already included
                    if full_steps.is_empty()
                        || full_steps.last().map(|s| &s.format) != Some(&conv.target_format)
                    {
                        full_steps.push(ConversionStep {
                            format: conv.target_format.clone(),
                            value: conv.value.clone(),
                            display: conv.display.clone(),
                        });
                    }

                    // Add to results if we haven't seen this exact (format, display) pair
                    if seen_results.insert(result_key) {
                        results.push(Conversion {
                            value: conv.value.clone(),
                            target_format: conv.target_format.clone(),
                            display: conv.display.clone(),
                            path: full_path.clone(),
                            steps: full_steps.clone(),
                            is_lossy: conv.is_lossy,
                            priority: conv.priority,
                            kind: conv.kind,
                            display_only: conv.display_only,
                            hidden: conv.hidden,
                            rich_display: conv.rich_display.clone(),
                        });
                    }

                    // Add to queue for further exploration (unless terminal or already explored)
                    if !conv.display_only && seen_for_bfs.insert(bfs_key) {
                        queue.push_back((conv.value, full_path, full_steps));
                    }
                }
            }
        }

        depth += 1;
    }

    // Filter out blocked source→target combinations (builtin + user config)
    // This catches any that slipped through (e.g., from initial format() calls)
    if let Some(source) = exclude_format {
        results.retain(|conv| {
            !is_blocked(
                source,
                &conv.target_format,
                source_format,
                &conv.path,
                blocking,
            )
        });
    }

    // Sort by priority, respecting user configuration
    sort_conversions(&mut results, priority);

    results
}

/// Sort conversions by priority, respecting user configuration.
fn sort_conversions(results: &mut [Conversion], priority_config: Option<&PriorityConfig>) {
    results.sort_by(|a, b| {
        if let Some(config) = priority_config {
            // User-configured category order
            let cat_a = config.category_sort_key(a.priority);
            let cat_b = config.category_sort_key(b.priority);

            // Within same category, apply format offsets
            if cat_a == cat_b {
                // Higher offset = shown earlier (so negate for comparison)
                let off_a = config.format_offset(&a.target_format);
                let off_b = config.format_offset(&b.target_format);
                // Higher offset comes first
                off_b
                    .cmp(&off_a)
                    .then_with(|| a.path.len().cmp(&b.path.len()))
            } else {
                cat_a.cmp(&cat_b)
            }
        } else {
            // Default: priority enum order, then path length
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.path.len().cmp(&b.path.len()))
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Base64Format, BytesToIntFormat, DateTimeFormat, HexFormat};

    #[test]
    fn test_bytes_to_multiple_formats() {
        let formats: Vec<Box<dyn Format>> = vec![
            Box::new(HexFormat),
            Box::new(Base64Format),
            Box::new(BytesToIntFormat),
        ];

        let bytes = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = find_all_conversions(&formats, &bytes, None, None, None);

        // Should have hex, base64, int-be, int-le
        let format_ids: Vec<_> = conversions
            .iter()
            .map(|c| c.target_format.as_str())
            .collect();

        assert!(format_ids.contains(&"hex"));
        assert!(format_ids.contains(&"base64"));
        assert!(format_ids.contains(&"int-be"));
        assert!(format_ids.contains(&"int-le"));
    }

    #[test]
    fn test_int_to_datetime() {
        let formats: Vec<Box<dyn Format>> = vec![Box::new(DateTimeFormat)];

        let value = CoreValue::Int {
            value: 1763574200,
            original_bytes: None,
        };

        let conversions = find_all_conversions(&formats, &value, None, None, None);

        let datetime_conv = conversions
            .iter()
            .find(|c| c.target_format == "epoch-seconds");
        assert!(datetime_conv.is_some());
        assert!(datetime_conv.unwrap().display.contains("2025"));
    }

    #[test]
    fn test_chained_conversions() {
        let formats: Vec<Box<dyn Format>> = vec![
            Box::new(HexFormat),
            Box::new(BytesToIntFormat),
            Box::new(DateTimeFormat),
        ];

        // Start with bytes that represent epoch 1763574200
        let bytes = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = find_all_conversions(&formats, &bytes, None, None, None);

        // Should find datetime via bytes -> int-be -> epoch-seconds
        let datetime_conv = conversions
            .iter()
            .find(|c| c.target_format == "epoch-seconds");

        assert!(
            datetime_conv.is_some(),
            "Should find epoch-seconds conversion"
        );
        let dt = datetime_conv.unwrap();
        assert!(dt.display.contains("2025"));
        assert!(!dt.path.is_empty()); // Has a path
    }
}
