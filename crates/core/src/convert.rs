//! Conversion graph traversal using BFS.
//!
//! This module finds all possible conversions from a value by traversing
//! a graph where nodes are values and edges are format conversions.

use std::collections::VecDeque;

use crate::format::Format;
use crate::types::{Conversion, ConversionPriority, CoreValue};

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
];

/// Check if a source→target conversion should be blocked.
fn is_blocked_path(source_format: &str, target_format: &str) -> bool {
    BLOCKED_PATHS
        .iter()
        .any(|(src, tgt)| source_format == *src && target_format == *tgt)
}

/// Find all possible conversions from a value using BFS.
///
/// This traverses the conversion graph, collecting all reachable formats.
/// The path is tracked to show how we got from the source to each target.
/// If `exclude_format` is provided, skip conversions to that format (to avoid hex→hex etc.)
pub fn find_all_conversions(
    formats: &[Box<dyn Format>],
    initial: &CoreValue,
    exclude_format: Option<&str>,
) -> Vec<Conversion> {
    let mut results = Vec::new();
    let mut seen_formats: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Pre-exclude the source format if specified
    if let Some(excluded) = exclude_format {
        seen_formats.insert(excluded.to_string());
    }

    // Queue holds (value, path_so_far)
    let mut queue: VecDeque<(CoreValue, Vec<String>)> = VecDeque::new();
    queue.push_back((initial.clone(), vec![]));

    // Also format the initial value with all applicable formats
    for format in formats {
        if format.can_format(initial) {
            if let Some(display) = format.format(initial) {
                let format_id = format.id().to_string();
                if seen_formats.insert(format_id.clone()) {
                    results.push(Conversion {
                        value: initial.clone(),
                        target_format: format_id.clone(),
                        display,
                        path: vec![format_id],
                        is_lossy: false,
                        priority: ConversionPriority::default(),
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
            let Some((current_value, current_path)) = queue.pop_front() else {
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

                    // Build the full path
                    let mut full_path = current_path.clone();
                    full_path.extend(conv.path.clone());

                    results.push(Conversion {
                        value: conv.value.clone(),
                        target_format: conv.target_format.clone(),
                        display: conv.display,
                        path: full_path.clone(),
                        is_lossy: conv.is_lossy,
                        priority: conv.priority,
                    });

                    // Add to queue for further exploration
                    queue.push_back((conv.value, full_path));
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
        let conversions = find_all_conversions(&formats, &bytes, None);

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

        let conversions = find_all_conversions(&formats, &value, None);

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
        let conversions = find_all_conversions(&formats, &bytes, None);

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
