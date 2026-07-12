//! Pipe mode for processing stdin line by line.
//!
//! Reads lines from stdin, tokenizes them, and shows inline annotations
//! for interesting values like UUIDs, timestamps, hex blobs, etc.

use std::io::{self, BufRead, Write};

use colored::Colorize;
use formatorbit_core::{ConversionResult, Formatorbit, RichDisplay};

use crate::pretty::{self, PacketMode, PrettyConfig};
use crate::tokenizer::{is_interesting_candidate, strip_ansi_codes, tokenize, Token};

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
        // Strip ANSI escape codes from colorized output (e.g., from log colorization scripts)
        let clean_line = strip_ansi_codes(&line);
        let annotations = process_line(forb, &clean_line, config);
        print_line_result(&mut out, &clean_line, &annotations, config)?;
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

/// Whether an interpretation's `description` is a generic size placeholder
/// rather than a semantic summary.
///
/// Byte-ish formats (hex, base64, raw bytes) describe themselves as e.g.
/// `"4 bytes"` or `"125 chars (ASCII)"` — those carry no meaning, so the
/// annotation should use the top conversion instead. Semantic formats (uuid,
/// ulid, ipv4, color, epoch, ...) put the meaning in the description, and we
/// keep it verbatim.
fn is_generic_description(desc: &str) -> bool {
    let desc = desc.trim();
    // Strip a leading count like "4 " / "125 " and check the remaining noun.
    let rest = match desc.split_once(' ') {
        Some((count, rest)) if count.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => return false,
    };
    // "bytes", "chars", "chars (ASCII)", "char", "byte", ...
    let noun = rest.split_whitespace().next().unwrap_or("");
    matches!(
        noun,
        "byte" | "bytes" | "char" | "chars" | "character" | "characters"
    )
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
                        show_paths: false, // Not needed in pipe mode
                        verbose: false,
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

    // Decide what to annotate the token with. Two sources compete:
    //   1. The interpretation's own `description` — for many semantic formats
    //      (uuid, ulid, ipv4, color, epoch) this IS the most meaningful summary
    //      (e.g. "UUID v4 (random)", "IPv4: 10.0.0.1 (Private)").
    //   2. The top-ranked conversion — for byte-ish formats (hex, base64) the
    //      description is a generic "N bytes" placeholder and the value lives in
    //      the first conversion (e.g. int/epoch).
    // Prefer the description when it carries semantics; otherwise use the top
    // conversion (which, post-ranking, is the highest-priority / most-semantic
    // one). This keeps hex bytes annotating as int/epoch while a UUID annotates
    // with its version/variant instead of a nonsense re-encoding.
    let conversions_str = if is_generic_description(&interp.description) {
        // Description is uninformative — use the top conversion, falling back to
        // the description only if there are no conversions at all.
        if conv_summary.is_empty() {
            interp.description.clone()
        } else {
            conv_summary.join(", ")
        }
    } else {
        // Description is semantic — lead with it.
        interp.description.clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_descriptions_detected() {
        assert!(is_generic_description("4 bytes"));
        assert!(is_generic_description("1 byte"));
        assert!(is_generic_description("125 chars (ASCII)"));
        assert!(is_generic_description("5 chars"));
    }

    #[test]
    fn semantic_descriptions_kept() {
        // These carry meaning and must NOT be treated as generic, so the tee
        // annotation leads with them instead of a nonsense conversion.
        assert!(!is_generic_description("UUID v4 (random)"));
        assert!(!is_generic_description(
            "ULID (created: 2016-07-30T23:54:10.259+00:00)"
        ));
        assert!(!is_generic_description("IPv4: 192.168.1.1 (Private)"));
        assert!(!is_generic_description(
            "2023-12-24T22:26:29+00:00 (2 years ago)"
        ));
        assert!(!is_generic_description("RGB: RGB(255, 87, 51)"));
        // A number alone is not "<count> <noun>".
        assert!(!is_generic_description("42"));
    }
}
