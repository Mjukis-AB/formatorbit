//! Static format conversion graph generation.
//!
//! Generates graphs showing format relationships without requiring input data.
//! Uses representative sample inputs to probe conversion edges.

use formatorbit_core::{ConversionKind, FormatInfo, Formatorbit};
use std::collections::{HashMap, HashSet};

/// Escape a string to be a valid DOT identifier.
/// DOT identifiers can't contain special chars like `-`, `/`, ` `, etc.
/// Also handles reserved keywords by prefixing with underscore.
fn dot_escape(s: &str) -> String {
    let escaped = s.replace([' ', '-', '/', '.', '(', ')', '[', ']', '\'', '"'], "_");
    // DOT reserved keywords that can't be used as identifiers
    match escaped.as_str() {
        "graph" | "digraph" | "subgraph" | "node" | "edge" | "strict" => {
            format!("_{}", escaped)
        }
        _ => escaped,
    }
}

/// Escape a string to be a valid Mermaid identifier.
/// Mermaid identifiers can't contain special chars like `-`, `/`, etc.
/// Also handles reserved keywords by prefixing with underscore.
fn mermaid_escape(s: &str) -> String {
    let escaped = s.replace(
        [' ', '-', '/', '.', '(', ')', '[', ']', '\'', '"', ':'],
        "_",
    );
    // Mermaid reserved keywords that can't be used as identifiers
    match escaped.as_str() {
        "graph" | "subgraph" | "end" | "direction" | "click" | "style" | "class" | "linkStyle"
        | "classDef" => {
            format!("_{}", escaped)
        }
        _ => escaped,
    }
}

/// Edge in the format conversion graph.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FormatEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

/// Kind of conversion edge.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum EdgeKind {
    Conversion,
    Representation,
    Trait,
}

impl From<ConversionKind> for EdgeKind {
    fn from(kind: ConversionKind) -> Self {
        match kind {
            ConversionKind::Conversion => EdgeKind::Conversion,
            ConversionKind::Representation => EdgeKind::Representation,
            ConversionKind::Trait => EdgeKind::Trait,
        }
    }
}

/// Generate sample input strings for probing conversion edges.
///
/// Uses representative inputs that parse as various formats:
/// - Hex strings, base64, binary
/// - Timestamps in different formats
/// - UUIDs, IPs, colors
/// - Unit values
fn sample_inputs() -> Vec<&'static str> {
    vec![
        // Hex (parses as hex, bytes, color)
        "691E01B8",
        "0x691E01B8",
        // Base64
        "SGVsbG8gV29ybGQ=",
        // Binary
        "0b10101010",
        // Integers/epoch
        "1735689600",
        "65",
        "256",
        // UUID
        "550e8400-e29b-41d4-a716-446655440000",
        // IP addresses
        "192.168.1.1",
        "::1",
        // Color
        "#FF5733",
        "rgb(255, 87, 51)",
        // JSON
        r#"{"key": "value"}"#,
        "[1, 2, 3]",
        // DateTime strings
        "2025-01-01T00:00:00Z",
        // Duration
        "1h30m",
        "P1DT2H",
        // Data size
        "1MiB",
        "500KB",
        // Temperature
        "20C",
        "68F",
        // Length
        "5km",
        "100m",
        // Weight
        "5kg",
        "100lbs",
        // Speed
        "60mph",
        "100km/h",
        // Coordinates
        "51.5074, -0.1278",
        // Currency
        "100USD",
        "$50",
        // Math expression
        "2 + 2",
        "0xFF + 1",
        // Text
        "Hello World",
        // ULID
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        // Escape sequences
        r"\x48\x65\x6c\x6c\x6f",
    ]
}

/// Build the complete format conversion graph by probing with sample inputs.
pub fn build_schema_graph(forb: &Formatorbit) -> (Vec<FormatInfo>, Vec<FormatEdge>) {
    let infos = forb.format_infos();
    let mut edges: HashSet<FormatEdge> = HashSet::new();

    // For each sample input, parse and find conversions
    for input in sample_inputs() {
        let results = forb.convert_all(input);
        for result in results {
            let source_format = &result.interpretation.source_format;

            for conv in &result.conversions {
                // Skip hidden conversions
                if conv.hidden {
                    continue;
                }

                // Skip self-loops (format converting to itself)
                if source_format == &conv.target_format {
                    continue;
                }

                edges.insert(FormatEdge {
                    source: source_format.clone(),
                    target: conv.target_format.clone(),
                    kind: conv.kind.into(),
                });
            }
        }
    }

    (infos, edges.into_iter().collect())
}

/// Build category-level graph showing which categories can convert to which.
pub fn build_category_graph(forb: &Formatorbit) -> Vec<(String, String)> {
    let (infos, edges) = build_schema_graph(forb);

    // Create format_id -> category mapping
    let format_to_category: HashMap<&str, &str> =
        infos.iter().map(|info| (info.id, info.category)).collect();

    // Aggregate edges by category
    let mut category_edges: HashSet<(String, String)> = HashSet::new();
    for edge in &edges {
        let source_cat = format_to_category
            .get(edge.source.as_str())
            .unwrap_or(&"Other");
        let target_cat = format_to_category
            .get(edge.target.as_str())
            .unwrap_or(&"Other");

        // Skip self-loops at category level
        if source_cat != target_cat {
            category_edges.insert((source_cat.to_string(), target_cat.to_string()));
        }
    }

    category_edges.into_iter().collect()
}

/// Build graph for a specific format showing what it converts to/from.
pub fn build_format_graph(
    forb: &Formatorbit,
    format_id: &str,
) -> (Vec<String>, Vec<FormatEdge>, Vec<FormatEdge>) {
    let (_, all_edges) = build_schema_graph(forb);

    // Collect formats that convert TO this format (exclude self-loops)
    let incoming: Vec<FormatEdge> = all_edges
        .iter()
        .filter(|e| e.target == format_id && e.source != format_id)
        .cloned()
        .collect();

    // Collect formats this format converts TO (exclude self-loops)
    let outgoing: Vec<FormatEdge> = all_edges
        .iter()
        .filter(|e| e.source == format_id && e.target != format_id)
        .cloned()
        .collect();

    // Collect all related format IDs
    let mut related: HashSet<String> = HashSet::new();
    related.insert(format_id.to_string());
    for e in &incoming {
        related.insert(e.source.clone());
    }
    for e in &outgoing {
        related.insert(e.target.clone());
    }

    (related.into_iter().collect(), incoming, outgoing)
}

/// Render schema graph as Mermaid.
pub fn schema_to_mermaid(infos: &[FormatInfo], edges: &[FormatEdge]) -> String {
    let mut out = String::new();
    out.push_str("graph LR\n");

    // Group formats by category
    let mut by_category: HashMap<&str, Vec<&FormatInfo>> = HashMap::new();
    for info in infos {
        by_category.entry(info.category).or_default().push(info);
    }

    // Output subgraphs for each category
    for (category, formats) in &by_category {
        out.push_str(&format!("  subgraph {}\n", mermaid_escape(category)));
        for info in formats {
            let node_id = mermaid_escape(info.id);
            out.push_str(&format!("    {}[\"{}\"]\n", node_id, info.id));
        }
        out.push_str("  end\n");
    }

    // Output edges
    for edge in edges {
        let source = mermaid_escape(&edge.source);
        let target = mermaid_escape(&edge.target);
        let arrow = match edge.kind {
            EdgeKind::Conversion => "-->",
            EdgeKind::Representation => "-.->",
            EdgeKind::Trait => "-..->",
        };
        out.push_str(&format!("  {} {} {}\n", source, arrow, target));
    }

    out
}

/// Render category graph as Mermaid.
pub fn category_to_mermaid(edges: &[(String, String)]) -> String {
    let mut out = String::new();
    out.push_str("graph LR\n");

    // Collect all categories
    let mut categories: HashSet<&str> = HashSet::new();
    for (src, tgt) in edges {
        categories.insert(src);
        categories.insert(tgt);
    }

    // Output nodes
    for cat in &categories {
        let id = mermaid_escape(cat);
        out.push_str(&format!("  {}[\"{}\"]\n", id, cat));
    }

    // Output edges
    for (src, tgt) in edges {
        let src_id = mermaid_escape(src);
        let tgt_id = mermaid_escape(tgt);
        out.push_str(&format!("  {} --> {}\n", src_id, tgt_id));
    }

    out
}

/// Render format-specific graph as Mermaid.
pub fn format_to_mermaid(
    format_id: &str,
    _related: &[String],
    incoming: &[FormatEdge],
    outgoing: &[FormatEdge],
) -> String {
    let mut out = String::new();
    out.push_str("graph LR\n");

    let center_id = mermaid_escape(format_id);
    out.push_str(&format!("  {}((\"{}\")){}\n", center_id, format_id, ""));

    // Incoming edges
    if !incoming.is_empty() {
        out.push_str("  subgraph Converts_FROM\n");
        for edge in incoming {
            let src = mermaid_escape(&edge.source);
            out.push_str(&format!("    {}[\"{}\"]\n", src, edge.source));
        }
        out.push_str("  end\n");

        for edge in incoming {
            let src = mermaid_escape(&edge.source);
            out.push_str(&format!("  {} --> {}\n", src, center_id));
        }
    }

    // Outgoing edges
    if !outgoing.is_empty() {
        out.push_str("  subgraph Converts_TO\n");
        for edge in outgoing {
            let tgt = mermaid_escape(&edge.target);
            out.push_str(&format!("    {}[\"{}\"]\n", tgt, edge.target));
        }
        out.push_str("  end\n");

        for edge in outgoing {
            let tgt = mermaid_escape(&edge.target);
            let arrow = match edge.kind {
                EdgeKind::Conversion => "-->",
                EdgeKind::Representation => "-.->",
                EdgeKind::Trait => "-..->",
            };
            out.push_str(&format!("  {} {} {}\n", center_id, arrow, tgt));
        }
    }

    out
}

/// Render schema graph as Graphviz DOT.
pub fn schema_to_dot(infos: &[FormatInfo], edges: &[FormatEdge]) -> String {
    let mut out = String::new();
    out.push_str("digraph formats {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box];\n\n");

    // Group formats by category using subgraphs
    let mut by_category: HashMap<&str, Vec<&FormatInfo>> = HashMap::new();
    for info in infos {
        by_category.entry(info.category).or_default().push(info);
    }

    for (category, formats) in &by_category {
        let cluster_name = dot_escape(category);
        out.push_str(&format!("  subgraph cluster_{} {{\n", cluster_name));
        out.push_str(&format!("    label=\"{}\";\n", category));
        for info in formats {
            let node_id = dot_escape(info.id);
            out.push_str(&format!("    {} [label=\"{}\"];\n", node_id, info.id));
        }
        out.push_str("  }\n\n");
    }

    // Output edges
    for edge in edges {
        let source = dot_escape(&edge.source);
        let target = dot_escape(&edge.target);
        let style = match edge.kind {
            EdgeKind::Conversion => "",
            EdgeKind::Representation => " [style=dashed]",
            EdgeKind::Trait => " [style=dotted]",
        };
        out.push_str(&format!("  {} -> {}{}\n", source, target, style));
    }

    out.push_str("}\n");
    out
}

/// Render category graph as Graphviz DOT.
pub fn category_to_dot(edges: &[(String, String)]) -> String {
    let mut out = String::new();
    out.push_str("digraph categories {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box, style=filled, fillcolor=lightblue];\n\n");

    // Collect all categories
    let mut categories: HashSet<&str> = HashSet::new();
    for (src, tgt) in edges {
        categories.insert(src);
        categories.insert(tgt);
    }

    // Output nodes
    for cat in &categories {
        let id = dot_escape(cat);
        out.push_str(&format!("  {} [label=\"{}\"];\n", id, cat));
    }
    out.push('\n');

    // Output edges
    for (src, tgt) in edges {
        let src_id = dot_escape(src);
        let tgt_id = dot_escape(tgt);
        out.push_str(&format!("  {} -> {};\n", src_id, tgt_id));
    }

    out.push_str("}\n");
    out
}

/// Render format-specific graph as Graphviz DOT.
pub fn format_to_dot(
    format_id: &str,
    _related: &[String],
    incoming: &[FormatEdge],
    outgoing: &[FormatEdge],
) -> String {
    let mut out = String::new();
    out.push_str("digraph format {\n");
    out.push_str("  rankdir=LR;\n\n");

    let center_id = dot_escape(format_id);
    out.push_str(&format!(
        "  {} [shape=ellipse, style=filled, fillcolor=yellow, label=\"{}\"];\n\n",
        center_id, format_id
    ));

    // Incoming cluster
    if !incoming.is_empty() {
        out.push_str("  subgraph cluster_from {\n");
        out.push_str("    label=\"Converts FROM\";\n");
        for edge in incoming {
            let src = dot_escape(&edge.source);
            out.push_str(&format!("    {} [label=\"{}\"];\n", src, edge.source));
        }
        out.push_str("  }\n\n");

        for edge in incoming {
            let src = dot_escape(&edge.source);
            out.push_str(&format!("  {} -> {};\n", src, center_id));
        }
    }

    // Outgoing cluster
    if !outgoing.is_empty() {
        out.push_str("\n  subgraph cluster_to {\n");
        out.push_str("    label=\"Converts TO\";\n");
        for edge in outgoing {
            let tgt = dot_escape(&edge.target);
            out.push_str(&format!("    {} [label=\"{}\"];\n", tgt, edge.target));
        }
        out.push_str("  }\n\n");

        for edge in outgoing {
            let tgt = dot_escape(&edge.target);
            let style = match edge.kind {
                EdgeKind::Conversion => "",
                EdgeKind::Representation => " [style=dashed]",
                EdgeKind::Trait => " [style=dotted]",
            };
            out.push_str(&format!("  {} -> {}{}\n", center_id, tgt, style));
        }
    }

    out.push_str("}\n");
    out
}
