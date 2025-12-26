//! Conversion graph traversal using BFS.
//!
//! This module finds all possible conversions from a value by traversing
//! a graph where nodes are values and edges are format conversions.

use std::collections::VecDeque;

use crate::format::Format;
use crate::types::{Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue};

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
];

/// Check if a source→target conversion should be blocked.
fn is_blocked_path(source_format: &str, target_format: &str) -> bool {
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

/// Find all possible conversions from a value using BFS.
///
/// This traverses the conversion graph, collecting all reachable formats.
/// The path is tracked to show how we got from the source to each target.
/// If `exclude_format` is provided, skip conversions to that format (to avoid hex→hex etc.)
/// If `source_format` is provided, it's included as the first element in the path.
pub fn find_all_conversions(
    formats: &[Box<dyn Format>],
    initial: &CoreValue,
    exclude_format: Option<&str>,
    source_format: Option<&str>,
) -> Vec<Conversion> {
    let mut results = Vec::new();
    let mut seen_formats: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Pre-exclude the source format if specified
    if let Some(excluded) = exclude_format {
        seen_formats.insert(excluded.to_string());
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
                if seen_formats.insert(format_id.clone()) {
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
                        metadata: None,
                    });
                }
            }
        }
    }

    // BFS through conversions
    let max_depth = 5; // Prevent infinite loops
    let mut depth = 0;

    while !queue.is_empty() && depth < max_depth {
        let level_size = queue.len();

        for _ in 0..level_size {
            let Some((current_value, current_path, current_steps)) = queue.pop_front() else {
                break;
            };

            // Get conversions from all formats
            for format in formats {
                for conv in format.conversions(&current_value) {
                    // Skip if we've already seen this format
                    if seen_formats.contains(&conv.target_format) {
                        continue;
                    }

                    seen_formats.insert(conv.target_format.clone());

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

                    results.push(Conversion {
                        value: conv.value.clone(),
                        target_format: conv.target_format.clone(),
                        display: conv.display,
                        path: full_path.clone(),
                        steps: full_steps.clone(),
                        is_lossy: conv.is_lossy,
                        priority: conv.priority,
                        kind: conv.kind,
                        display_only: conv.display_only,
                        metadata: conv.metadata,
                    });

                    // Add to queue for further exploration (unless terminal)
                    if !conv.display_only {
                        queue.push_back((conv.value, full_path, full_steps));
                    }
                }
            }
        }

        depth += 1;
    }

    // Filter out blocked source→target combinations
    if let Some(source) = exclude_format {
        results.retain(|conv| !is_blocked_path(source, &conv.target_format));
    }

    // Sort by priority (Structured first), then by path length (shorter = more direct)
    results.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.path.len().cmp(&b.path.len()))
    });

    results
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
        let conversions = find_all_conversions(&formats, &bytes, None, None);

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

        let conversions = find_all_conversions(&formats, &value, None, None);

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
        let conversions = find_all_conversions(&formats, &bytes, None, None);

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
