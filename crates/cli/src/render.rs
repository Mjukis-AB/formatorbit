//! Rendering helpers for a single input's results: conversion-value display and
//! per-input DOT/Mermaid conversion graphs.

use formatorbit_core::{truncate_str, CoreValue, RichDisplay, RichDisplayOption};

use crate::pretty::{self, PacketMode, PrettyConfig};

/// Format a conversion's display string, applying pretty-printing for structured data.
pub fn format_conversion_display(
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
pub fn print_dot_graph(input: &str, results: &[&formatorbit_core::ConversionResult]) {
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
pub fn print_mermaid_graph(input: &str, results: &[&formatorbit_core::ConversionResult]) {
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
