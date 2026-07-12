//! Conversion graph traversal using BFS.
//!
//! This module finds all possible conversions from a value by traversing
//! a graph where nodes are values and edges are format conversions.

use std::collections::VecDeque;

use crate::format::Format;
use crate::types::{
    BlockingConfig, Conversion, ConversionConfig, ConversionKind, ConversionPriority,
    ConversionStep, CoreValue, PriorityConfig, RichDisplay,
};

/// Maximum BFS depth to prevent infinite loops in conversion graph traversal.
const MAX_BFS_DEPTH: usize = 5;

/// Broad category of a format or conversion-target id, used to express blocking
/// policy as a handful of category-level rules instead of ~90 hardcoded pairs.
///
/// A category is assigned to any id via [`category_of`] - both source format ids
/// (e.g. `hex`, `text`) and fine-grained conversion-target ids (e.g.
/// `epoch-seconds`, `datasize-iec`). Unclassified ids fall into `Other` and are
/// never blocked by category rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    /// Raw-byte / notation renderings: hex, base64, binary, octal, url-encoded,
    /// escape sequences, hexdump, msgpack/protobuf/plist, char, *-int notations.
    Encoding,
    /// Human text: text, utf8.
    Text,
    /// Meaningful identifiers derived from structure, not raw bytes: IP
    /// addresses (+ CIDR sub-fields), UUIDs, colors, MAC addresses, geo codes.
    Identifier,
    /// Points in time: epoch bases, Apple/Cocoa, FILETIME, datetime, relative.
    Timestamp,
    /// Elapsed time: duration, duration-ms.
    Duration,
    /// Data sizes: datasize and its IEC/SI variants.
    DataSize,
    /// Dimensional unit quantities (length, weight, temperature, ...).
    Unit,
    /// Plain numbers: int-be/int-le/decimal.
    Number,
    /// Expression evaluation result.
    Expression,
    /// Anything not participating in category-level blocking.
    Other,
}

/// Classify a format id or conversion-target id into a [`Category`].
///
/// This is the single source of truth for category-based blocking; adding a new
/// format usually means adding its id (and any target ids it emits) to one arm
/// here rather than writing new (source, target) pairs.
fn category_of(id: &str) -> Category {
    match id {
        // --- Encodings / notations / binary containers ---
        "hex" | "base64" | "binary" | "octal" | "url-encoded" | "hexdump" | "bytes" | "escape"
        | "escape-hex" | "escape-unicode" | "hex-int" | "binary-int" | "octal-int"
        | "utf8-bytes" | "ascii-decimal" | "codepoints" | "msgpack" | "plist" | "protobuf"
        | "char" => Category::Encoding,

        // --- Human text ---
        "text" | "utf8" => Category::Text,

        // --- Structural identifiers ---
        "ip" | "ipv4" | "ipv6" | "cidr" | "netmask" | "wildcard" | "network-class"
        | "private-network" | "broadcast" | "host-range" | "host-count" | "address-count"
        | "uuid" | "ulid" | "color" | "color-hex" | "color-rgb" | "color-hsl" | "mac-address"
        | "geohash" | "coords" | "plus-code" | "mgrs" | "utm" | "dms" | "ddm" | "dd" => {
            Category::Identifier
        }

        // --- Time ---
        "epoch" | "epoch-seconds" | "epoch-millis" | "epoch-micros" | "epoch-nanos"
        | "apple-cocoa" | "filetime" | "datetime" | "relative-time" => Category::Timestamp,
        "duration" | "duration-ms" => Category::Duration,
        "datasize" | "datasize-iec" | "datasize-si" => Category::DataSize,

        // --- Dimensional units ---
        "length" | "weight" | "volume" | "speed" | "pressure" | "angle" | "area" | "energy"
        | "temperature" | "currency" => Category::Unit,

        // --- Plain numbers ---
        "int-be" | "int-le" | "decimal" => Category::Number,

        // --- Expression ---
        "expr" | "result" => Category::Expression,

        _ => Category::Other,
    }
}

/// Structural identifiers require real syntax (dotted quads, hyphenated UUID
/// groups, `#`-prefixed colours), so they must never be read out of an arbitrary
/// byte blob - a run of ASCII text or a hex dump is not an IP / colour / UUID
/// just because it has the right length. This applies to *any* raw-byte root
/// regardless of path (e.g. `hex → bytes → ipv4`). Identifier roots are excluded
/// here so that same-family conversions (colour→colour space, CIDR sub-fields)
/// stay open; cross-family cases (e.g. mac→ip) are the residual pair list.
fn root_blocks_identifier(root_cat: Category) -> bool {
    matches!(root_cat, Category::Encoding | Category::Text)
}

/// Categories a raw-byte value can legitimately be *read as* a number/quantity:
/// interpreting hex bytes as an integer, a timestamp, or a data size is the
/// whole point of the tool. Plain human text, however, is none of these - "test"
/// is not the integer 1952805748 or an epoch - so text roots block them too.
fn is_numeric_interpretation(cat: Category) -> bool {
    matches!(
        cat,
        Category::Number | Category::Timestamp | Category::Duration | Category::DataSize
    )
}

/// A "quantity" is a number that measures something in a specific dimension.
fn is_quantity_category(cat: Category) -> bool {
    matches!(
        cat,
        Category::Timestamp | Category::Duration | Category::DataSize | Category::Unit
    )
}

/// Category-level blocking policy. Returns true when a conversion from
/// `source_cat` (immediate source) to `target_cat` is never useful.
///
/// Replaces the bulk of the old BLOCKED_PATHS pairs with two rules:
///
/// - A structural identifier (IP, UUID, colour, MAC) is a *decoded value*. Its
///   underlying integer can still be shown (Identifier → Number is fine - a
///   colour's `0xFF5733`, an IP's integer), but re-reading it as a *quantity*
///   (a timestamp, duration, or data size) is nonsense: an IP is not an epoch,
///   a colour is not "16 MB".
/// - A quantity in one dimension is never a quantity in another dimension - a
///   duration is not a data size, a temperature is not a length. (Same-dimension
///   conversions like `length → length` are produced inside one format and never
///   reach this cross-category check.)
fn category_rule_blocks(source_cat: Category, target_cat: Category) -> bool {
    // Identifiers must not be re-read as a measured quantity.
    if source_cat == Category::Identifier && is_quantity_category(target_cat) {
        return true;
    }

    // No cross-dimension quantity conversions.
    if is_quantity_category(source_cat)
        && is_quantity_category(target_cat)
        && source_cat != target_cat
    {
        return true;
    }

    false
}

/// Small residual list of (source, target) blocks that are genuinely specific
/// and not worth generalising into a category rule.
///
/// Kept deliberately short - each entry is a concrete "this exact chain is
/// noise" case that a category rule would either miss or over-apply:
/// - terminal display/encoding formats that must not chain into further
///   encodings (hexdump/url-encoded/escape-* are for viewing, not re-encoding);
/// - identifiers and expression results that should not be re-serialised as
///   binary containers or re-derived as numbers/sizes/durations;
/// - a couple of `text` circular/noise edges.
const RESIDUAL_BLOCKED_PATHS: &[(&str, &str)] = &[
    // Structural identifiers / expression results aren't binary-container data.
    ("ipv4", "msgpack"),
    ("ipv6", "msgpack"),
    ("uuid", "msgpack"),
    ("expr", "msgpack"),
    // Expression results: keep the primary result + hex/binary notations, drop
    // the speculative numeric re-readings (these are Number/Unit/Duration cats
    // but expr is Expression, so no category rule covers them).
    ("expr", "octal"),
    ("expr", "decimal"),
    ("expr", "datasize"),
    ("expr", "datasize-iec"),
    ("expr", "datasize-si"),
    ("expr", "duration"),
    ("expr", "duration-ms"),
    // Terminal display / encoding formats must not chain into more encodings.
    ("hexdump", "bytes"),
    ("hexdump", "url-encoded"),
    ("hexdump", "escape-unicode"),
    ("hexdump", "escape-hex"),
    ("hexdump", "msgpack"),
    ("url-encoded", "url-encoded"),
    ("url-encoded", "bytes"),
    ("url-encoded", "escape-unicode"),
    ("url-encoded", "escape-hex"),
    ("escape-hex", "bytes"),
    ("escape-hex", "url-encoded"),
    ("escape-unicode", "bytes"),
    ("escape-unicode", "url-encoded"),
    // Plain-text noise edges.
    ("text", "url-encoded"),
    ("text", "graph"),
    ("text", "msgpack"),
    ("text", "escape-unicode"),
    // Circular: text → bytes → utf8 just reproduces the original text.
    ("text", "utf8"),
    // A duration shown in a different time scale is a redundant re-scaling.
    ("duration", "duration-ms"),
];

/// Residual root-based blocks: a target that must not be reached from a given
/// root on *any* path, for cases a category rule would over- or under-apply.
///
/// These are all cross-family identifier confusion: both the root and the target
/// are structural identifiers (so `root_blocks_identifier` doesn't fire and the
/// Identifier→Identifier category rule deliberately stays open for same-family
/// space conversions like colour→colour), yet the two families are unrelated:
/// - a MAC address's 6 bytes are not an IP address or a colour;
/// - an IP address's 4/16 bytes are not an RGB(A) colour — `192.168.1.1` is not
///   `#C0A80101`. Colour readings of an IP were pure noise.
const RESIDUAL_ROOT_BLOCKED: &[(&str, &str)] = &[
    ("mac-address", "ipv4"),
    ("mac-address", "ipv6"),
    ("mac-address", "color-rgb"),
    ("mac-address", "color-hsl"),
    ("mac-address", "color-hex"),
    ("ipv4", "color-rgb"),
    ("ipv4", "color-hsl"),
    ("ipv4", "color-hex"),
    ("ipv6", "color-rgb"),
    ("ipv6", "color-hsl"),
    ("ipv6", "color-hex"),
];

/// Check if a source→target conversion should be blocked (builtin rules only).
fn is_blocked_path_builtin(source_format: &str, target_format: &str) -> bool {
    let source_cat = category_of(source_format);
    let target_cat = category_of(target_format);

    // Category-level policy.
    if category_rule_blocks(source_cat, target_cat) {
        return true;
    }

    // Residual specific pairs.
    RESIDUAL_BLOCKED_PATHS
        .iter()
        .any(|(src, tgt)| source_format == *src && target_format == *tgt)
}

/// Check if a target is blocked based on root interpretation (builtin rules).
///
/// Two category rules replace the entire hand-written ROOT_BLOCKED_TARGETS list:
///
/// 1. Structural identifiers (IP, colour, UUID) can't be read out of an
///    arbitrary byte blob - block them for encoding / text / identifier roots,
///    on any path.
/// 2. Plain human text is not a number, timestamp, duration, or data size, so a
///    `text` root additionally blocks those numeric interpretations. (Encoding
///    roots like `hex` deliberately keep them - hex → int → epoch is the point.)
fn is_root_blocked_builtin(root_format: &str, target_format: &str) -> bool {
    let root_cat = category_of(root_format);
    let target_cat = category_of(target_format);

    if target_cat == Category::Identifier && root_blocks_identifier(root_cat) {
        return true;
    }

    if root_cat == Category::Text && is_numeric_interpretation(target_cat) {
        return true;
    }

    // Residual cross-family identifier cases (e.g. mac-address → ipv4).
    RESIDUAL_ROOT_BLOCKED
        .iter()
        .any(|(root, target)| root_format == *root && target_format == *target)
}

/// Epoch/timestamp formats that produce a `DateTime` from an integer offset
/// against some reference epoch (Unix, Apple/Cocoa, Windows FILETIME, ...).
///
/// These are the formats involved in datetime→epoch→datetime cycles: once a
/// value is already a `DateTime`, re-deriving another epoch integer from it
/// against a *different* reference and re-showing that as a timestamp produces
/// nonsense (e.g. an Apple timestamp re-read as a Unix epoch). The `datetime`
/// format is deliberately excluded - it is the human-readable ISO rendering of
/// a `DateTime`, not a new epoch integer, so `epoch-seconds → datetime` stays.
const EPOCH_FORMATS: &[&str] = &[
    "epoch-seconds",
    "epoch-millis",
    "epoch-micros",
    "epoch-nanos",
    "apple-cocoa",
    "filetime",
];

/// Whether a conversion is a redundant round-trip that should never be emitted.
///
/// Two generic rules, independent of any hardcoded (source, target) pair list:
///
/// (a) A conversion may not target the same format that produced the value it
///     converts. Re-emitting a value in the format it already has (an adjacent
///     duplicate like `epoch-seconds → epoch-seconds` or `decimal → decimal`)
///     carries no information.
///
/// (b) Once a value is a timestamp/`DateTime`, don't re-derive a *different*
///     epoch integer from it and present that as another timestamp. This kills
///     datetime→epoch→datetime cycles across epoch bases (e.g.
///     `apple-cocoa → epoch-seconds`, `epoch-seconds → epoch-millis`) while
///     leaving the useful `epoch → datetime` rendering intact.
fn is_redundant_roundtrip(source_format: &str, target_format: &str) -> bool {
    // (a) no adjacent duplicate / self-conversion
    if source_format == target_format {
        return true;
    }

    // (b) no epoch → other-epoch re-derivation
    EPOCH_FORMATS.contains(&source_format) && EPOCH_FORMATS.contains(&target_format)
}

/// Check if a conversion should be blocked (builtin rules + user config).
fn is_blocked(
    source_format: &str,
    target_format: &str,
    root_format: Option<&str>,
    path: &[String],
    blocking: Option<&BlockingConfig>,
) -> bool {
    // Generic round-trip rules (adjacent duplicates + epoch cycles)
    if is_redundant_roundtrip(source_format, target_format) {
        return true;
    }

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

    // Call source_conversions() for the source format only.
    // These are conversions specific to the format that parsed the input,
    // not applicable to values from other sources during BFS.
    if let Some(source_fmt) = source_format {
        if let Some(format) = formats.iter().find(|f| f.id() == source_fmt) {
            for mut conv in format.source_conversions(initial) {
                // Build path including source format
                let mut path = vec![source_fmt.to_string()];
                path.push(conv.target_format.clone());
                conv.path = path;

                // Build steps
                conv.steps = vec![ConversionStep {
                    format: conv.target_format.clone(),
                    value: conv.value.clone(),
                    display: conv.display.clone(),
                }];

                let result_key = (conv.target_format.clone(), conv.display.clone());
                if seen_results.insert(result_key) {
                    results.push(conv);
                }
            }
        }
    }

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

    // Collapse conversions that a viewer would see as byte-identical duplicates
    // of one already kept (same target family, same value, same rendered output).
    dedup_identical_display(&mut results);

    results
}

/// Set of format ids that render the *same* value into the *same* text and are
/// therefore interchangeable renderings of one another. Only conversions whose
/// target is in the same group here are eligible to be deduped against each
/// other, so we never merge conceptually distinct conversions (like `utf8` vs
/// `text`, or `int-be` vs `decimal`) that merely coincide in text for one input.
fn render_group(format_id: &str) -> Option<u8> {
    match format_id {
        // Two ways to pretty-print the same JSON value.
        "json" | "json-formatted" => Some(0),
        _ => None,
    }
}

/// Remove conversions that are byte-identical redundant renderings of one
/// already kept, keeping the first (highest-priority) occurrence.
///
/// The target case is a value that two formats in the same [`render_group`]
/// render identically - the classic example is `json` (the `format()`
/// rendering) and `json-formatted` (the conversion), which pretty-print the same
/// value byte-for-byte. Showing both is pure redundancy. Restricting to a render
/// group keeps genuinely distinct interpretations that merely coincide in text
/// (e.g. `utf8` vs `text`, `int-be` vs `decimal`) fully visible.
fn dedup_identical_display(results: &mut Vec<Conversion>) {
    let mut seen: std::collections::HashSet<(u8, String)> = std::collections::HashSet::new();
    results.retain(|conv| {
        let Some(group) = render_group(&conv.target_format) else {
            return true;
        };
        let rich_sig = conv
            .rich_display
            .first()
            .map(|opt| format!("{:?}", opt.preferred))
            .unwrap_or_default();
        seen.insert((group, format!("{}\u{0}{}", conv.display, rich_sig)))
    });
}

/// Tiebreaker rank for a conversion kind within the same priority category.
///
/// Actual transformations (e.g. int → datetime) are the most valuable and rank
/// ahead of mere notation representations (e.g. 0x691E01B8) and observational
/// traits. Without this, low-value notation variants can bury a genuine semantic
/// result (like an epoch timestamp) below the default output limit.
fn kind_rank(kind: ConversionKind) -> u8 {
    match kind {
        ConversionKind::Conversion => 0,
        ConversionKind::Representation => 1,
        ConversionKind::Trait => 2,
    }
}

/// Sub-rank within the Semantic category, driven by the structured rich display.
///
/// A concrete calendar timestamp (`DateTime`) is the interpretation that an
/// integer's magnitude most strongly implies, so it ranks ahead of the more
/// speculative "reinterpret this number as a size / a duration" readings that
/// fire for essentially any integer. This is generic — it keys off the rich
/// display variant, not hardcoded format names — so any timestamp-producing
/// conversion benefits and any size/duration one is deprioritized.
fn semantic_subrank(conv: &Conversion) -> u8 {
    match conv.rich_display.first().map(|opt| &opt.preferred) {
        Some(RichDisplay::DateTime { .. }) => 0,
        // Size and duration are plausible for almost any integer, so they are
        // the weakest semantic signal and rank last within the category.
        Some(RichDisplay::DataSize { .. } | RichDisplay::Duration { .. }) => 2,
        _ => 1,
    }
}

/// Sort conversions by priority, respecting user configuration.
///
/// Ordering is: category (priority) → kind (Conversion > Representation > Trait)
/// → semantic sub-rank (timestamps > other > size/duration) → path depth
/// (shallower first) → user format offset. Priority always dominates so semantic
/// results (datetime, uuid, ip) rank above encodings, and within a category real
/// transformations beat notation variants and speculative numeric readings.
fn sort_conversions(results: &mut [Conversion], priority_config: Option<&PriorityConfig>) {
    results.sort_by(|a, b| {
        // Category key: user-configured order if present, else the enum order.
        let (cat_a, cat_b) = match priority_config {
            Some(config) => (
                config.category_sort_key(a.priority),
                config.category_sort_key(b.priority),
            ),
            None => (a.priority as usize, b.priority as usize),
        };

        if cat_a != cat_b {
            return cat_a.cmp(&cat_b);
        }

        // Within the same category: prefer real conversions over notation/traits,
        // then the semantic sub-rank, then shallower paths, then user offset.
        let off_a = priority_config.map_or(0, |c| c.format_offset(&a.target_format));
        let off_b = priority_config.map_or(0, |c| c.format_offset(&b.target_format));

        kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then_with(|| semantic_subrank(a).cmp(&semantic_subrank(b)))
            .then_with(|| a.path.len().cmp(&b.path.len()))
            .then_with(|| off_b.cmp(&off_a))
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

    #[test]
    fn test_redundant_roundtrip_rules() {
        // (a) self / adjacent-duplicate conversions are always redundant
        assert!(is_redundant_roundtrip("decimal", "decimal"));
        assert!(is_redundant_roundtrip("epoch-seconds", "epoch-seconds"));
        assert!(is_redundant_roundtrip("color-hex", "color-hex"));

        // (b) one epoch base must not be re-derived as another epoch base
        assert!(is_redundant_roundtrip("epoch-seconds", "epoch-millis"));
        assert!(is_redundant_roundtrip("apple-cocoa", "epoch-seconds"));
        assert!(is_redundant_roundtrip("filetime", "epoch-seconds"));

        // The useful directions stay open:
        //  - an integer/decimal can become a timestamp
        //  - a timestamp can be rendered as an ISO `datetime` (not an epoch int)
        assert!(!is_redundant_roundtrip("decimal", "epoch-seconds"));
        assert!(!is_redundant_roundtrip("datetime", "epoch-seconds"));
        assert!(!is_redundant_roundtrip("epoch-seconds", "datetime"));
        assert!(!is_redundant_roundtrip("hex", "base64"));
    }

    #[test]
    fn test_epoch_roundtrip_absent_from_graph() {
        let formats: Vec<Box<dyn Format>> = vec![
            Box::new(HexFormat),
            Box::new(BytesToIntFormat),
            Box::new(DateTimeFormat),
        ];

        // bytes for epoch 1763574200
        let bytes = CoreValue::Bytes(vec![0x69, 0x1E, 0x01, 0xB8]);
        let conversions = find_all_conversions(&formats, &bytes, None, None, None);

        // No conversion may re-derive an epoch integer from an already-derived
        // timestamp (e.g. ...epoch-seconds → epoch-seconds/epoch-millis) or hop
        // between epoch bases (apple-cocoa → epoch-seconds).
        for conv in &conversions {
            for window in conv.path.windows(2) {
                assert!(
                    !is_redundant_roundtrip(&window[0], &window[1]),
                    "redundant round-trip leaked into path: {:?}",
                    conv.path
                );
            }
        }
    }

    #[test]
    fn test_json_formatted_dedup() {
        use crate::formats::JsonFormat;

        // A JSON value reached from a non-json source (as JWT does) is rendered
        // by both `json` (format()) and `json-formatted` (conversions()) with
        // identical pretty output. Only one should survive.
        let formats: Vec<Box<dyn Format>> = vec![Box::new(JsonFormat)];
        let value = CoreValue::Json(serde_json::json!({"a": 1, "b": [2, 3]}));
        let conversions = find_all_conversions(&formats, &value, None, None, None);

        let json_renderings: Vec<_> = conversions
            .iter()
            .filter(|c| c.target_format == "json" || c.target_format == "json-formatted")
            .collect();

        assert_eq!(
            json_renderings.len(),
            1,
            "expected a single JSON rendering, got: {:?}",
            json_renderings
                .iter()
                .map(|c| &c.target_format)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_category_classification() {
        assert_eq!(category_of("hex"), Category::Encoding);
        assert_eq!(category_of("text"), Category::Text);
        assert_eq!(category_of("ipv4"), Category::Identifier);
        assert_eq!(category_of("color-hex"), Category::Identifier);
        assert_eq!(category_of("epoch-seconds"), Category::Timestamp);
        assert_eq!(category_of("duration-ms"), Category::Duration);
        assert_eq!(category_of("datasize-iec"), Category::DataSize);
        assert_eq!(category_of("length"), Category::Unit);
        assert_eq!(category_of("int-be"), Category::Number);
        assert_eq!(category_of("expr"), Category::Expression);
        assert_eq!(category_of("json"), Category::Other);
    }

    #[test]
    fn test_category_blocking_rules() {
        // Raw-byte roots may be read as numbers/timestamps (that's the point)...
        assert!(!is_root_blocked_builtin("hex", "epoch-seconds"));
        assert!(!is_root_blocked_builtin("hex", "int-be"));
        assert!(!is_root_blocked_builtin("hex", "datasize-iec"));
        // ...but never as structural identifiers.
        assert!(is_root_blocked_builtin("hex", "ipv4"));
        assert!(is_root_blocked_builtin("hex", "color-hex"));
        assert!(is_root_blocked_builtin("text", "uuid"));

        // Plain text is not a number/timestamp/size either.
        assert!(is_root_blocked_builtin("text", "int-be"));
        assert!(is_root_blocked_builtin("text", "epoch-seconds"));
        assert!(is_root_blocked_builtin("text", "datasize"));

        // Identifiers keep their raw integer but are not re-read as quantities.
        assert!(!is_blocked_path_builtin("ipv4", "decimal"));
        assert!(is_blocked_path_builtin("ipv4", "epoch-seconds"));
        assert!(is_blocked_path_builtin("uuid", "apple-cocoa"));
        assert!(is_blocked_path_builtin("color-hex", "datasize"));

        // No cross-dimension quantity conversions.
        assert!(is_blocked_path_builtin("duration", "datasize"));
        assert!(is_blocked_path_builtin("datasize", "duration"));

        // Same-family identifier conversions stay open (colour spaces, CIDR).
        assert!(!is_blocked_path_builtin("color-hex", "color-rgb"));
        assert!(!is_blocked_path_builtin("color-hex", "color-hsl"));

        // Cross-family identifiers are the residual root list (mac ≠ ip/colour).
        assert!(is_root_blocked_builtin("mac-address", "ipv4"));
        assert!(is_root_blocked_builtin("mac-address", "color-hex"));
    }
}
