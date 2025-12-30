//! Pipe mode for processing stdin line by line.
//!
//! Reads lines from stdin, tokenizes them, and shows inline annotations
//! for interesting values like UUIDs, timestamps, hex blobs, etc.

use std::io::{self, BufRead, Write};

use colored::Colorize;
use formatorbit_core::{ConversionResult, Formatorbit, RichDisplay};

use crate::pretty::{self, PacketMode, PrettyConfig};
use crate::tokenizer::{is_interesting_candidate, tokenize, Token};

/// Configuration for pipe mode.
pub struct PipeModeConfig {
    /// Minimum confidence threshold for showing annotations (0.0-1.0)
    pub threshold: f32,
    /// Highlight interesting values inline with color
    pub highlight: bool,
    /// Maximum tokens to analyze per line (performance guard)
    pub max_tokens: usize,
    /// Output as JSON instead of human-readable
    pub json: bool,
    /// Filter to specific formats (empty = all formats)
    pub format_filter: Vec<String>,
    /// Packet layout mode for binary formats
    pub packet_mode: PacketMode,
}

/// A token with its interpretation results.
struct AnnotatedToken {
    token: Token,
    result: ConversionResult,
}

/// Run pipe mode, processing stdin line by line.
pub fn run_pipe_mode(forb: &Formatorbit, config: &PipeModeConfig) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let handle = stdin.lock();
    let mut out = stdout.lock();

    for line_result in handle.lines() {
        let line = line_result?;
        let annotations = process_line(forb, &line, config);
        print_line_result(&mut out, &line, &annotations, config)?;
    }

    Ok(())
}

/// Process a single line and find interesting tokens.
fn process_line(forb: &Formatorbit, line: &str, config: &PipeModeConfig) -> Vec<AnnotatedToken> {
    let tokens = tokenize(line);

    // Filter to interesting candidates and limit count
    let candidates: Vec<_> = tokens
        .into_iter()
        .filter(|t| is_interesting_candidate(&t.text))
        .take(config.max_tokens)
        .collect();

    let mut annotated = Vec::new();

    for token in candidates {
        let results = forb.convert_all_filtered(&token.text, &config.format_filter);

        // Find best interpretation above threshold
        let best = results
            .into_iter()
            .filter(|r| r.interpretation.confidence >= config.threshold)
            .max_by(|a, b| {
                a.interpretation
                    .confidence
                    .total_cmp(&b.interpretation.confidence)
            });

        if let Some(result) = best {
            annotated.push(AnnotatedToken { token, result });
        }
    }

    annotated
}

/// Print a line with its annotations.
fn print_line_result(
    out: &mut impl Write,
    line: &str,
    annotations: &[AnnotatedToken],
    config: &PipeModeConfig,
) -> io::Result<()> {
    if config.json {
        return print_json_line(out, line, annotations);
    }

    // Print the original line (with optional highlighting)
    if config.highlight && !annotations.is_empty() {
        print_highlighted_line(out, line, annotations)?;
    } else {
        writeln!(out, "{}", line)?;
    }

    // Print annotations below interesting tokens
    for annotated in annotations {
        print_annotation(out, annotated, config)?;
    }

    Ok(())
}

/// Print the line with matched tokens highlighted.
fn print_highlighted_line(
    out: &mut impl Write,
    line: &str,
    annotations: &[AnnotatedToken],
) -> io::Result<()> {
    // Sort annotations by position
    let mut sorted: Vec<_> = annotations.iter().collect();
    sorted.sort_by_key(|a| a.token.start);

    let mut last_end = 0;

    for annotated in sorted {
        let token = &annotated.token;

        // Print text before this token
        if token.start > last_end {
            write!(out, "{}", &line[last_end..token.start])?;
        }

        // Print highlighted token
        let highlighted = line[token.start..token.end].on_bright_yellow().black();
        write!(out, "{}", highlighted)?;

        last_end = token.end;
    }

    // Print remaining text
    if last_end < line.len() {
        write!(out, "{}", &line[last_end..])?;
    }

    writeln!(out)?;
    Ok(())
}

/// Print annotation for a token.
fn print_annotation(
    out: &mut impl Write,
    annotated: &AnnotatedToken,
    config: &PipeModeConfig,
) -> io::Result<()> {
    let interp = &annotated.result.interpretation;

    // Calculate indentation to align with token position
    let indent = " ".repeat(annotated.token.display_col);

    // Build conversions summary (limit to 1 for cleaner pipe output)
    // Conversions are already sorted by priority (Structured > Semantic > Encoding > Raw)
    let conv_summary: Vec<String> = annotated
        .result
        .conversions
        .iter()
        .take(1)
        .map(|c| {
            // Check if we should use packet layout for this conversion
            let display = if config.packet_mode != PacketMode::None {
                let packet_layout = c.rich_display.iter().find_map(|opt| {
                    if let RichDisplay::PacketLayout { segments, .. } = &opt.preferred {
                        Some(segments)
                    } else {
                        None
                    }
                });
                if let Some(segments) = packet_layout {
                    let pretty_config = PrettyConfig {
                        color: true,
                        indent: "  ",
                        compact: false,
                        packet_mode: config.packet_mode,
                    };
                    match config.packet_mode {
                        PacketMode::Compact => {
                            pretty::pretty_packet_compact(segments, &pretty_config)
                        }
                        PacketMode::Detailed => {
                            pretty::pretty_packet_detailed(segments, &pretty_config)
                        }
                        PacketMode::None => c.display.clone(),
                    }
                } else {
                    c.display.clone()
                }
            } else {
                c.display.clone()
            };
            format!("{}: {}", c.target_format.yellow(), display)
        })
        .collect();

    let conversions_str = if conv_summary.is_empty() {
        interp.description.clone()
    } else {
        conv_summary.join(", ")
    };

    writeln!(
        out,
        "{}{} {}: {}",
        indent,
        "\u{21b3}".cyan(), // ↳
        interp.source_format.green().bold(),
        conversions_str
    )?;

    Ok(())
}

/// Print JSON output for a line.
fn print_json_line(
    out: &mut impl Write,
    line: &str,
    annotations: &[AnnotatedToken],
) -> io::Result<()> {
    use serde_json::json;

    let json_annotations: Vec<_> = annotations
        .iter()
        .map(|a| {
            json!({
                "token": a.token.text,
                "position": {
                    "start": a.token.start,
                    "end": a.token.end,
                    "display_col": a.token.display_col,
                },
                "interpretation": {
                    "format": a.result.interpretation.source_format,
                    "confidence": a.result.interpretation.confidence,
                    "description": a.result.interpretation.description,
                },
                "conversions": a.result.conversions.iter().take(5).map(|c| {
                    json!({
                        "format": c.target_format,
                        "display": c.display,
                        "is_lossy": c.is_lossy,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();

    let output = json!({
        "line": line,
        "annotations": json_annotations,
    });

    writeln!(out, "{}", serde_json::to_string(&output).unwrap())?;
    Ok(())
}
