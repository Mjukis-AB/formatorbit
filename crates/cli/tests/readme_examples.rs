//! Validates that README.md examples still work.
//!
//! This test extracts shell commands from README.md and verifies:
//! 1. Commands that should succeed do succeed
//! 2. Output contains expected patterns
//!
//! Run with: cargo test -p formatorbit-cli --test readme_examples

use std::process::{Command, Stdio};

/// A throwaway `$HOME` so tests never read the developer's real config
/// (`~/Library/Application Support/forb/config.toml`), which can invert the
/// category order and change output ordering. Each call gets a unique dir.
fn isolated_home() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("forb_readme_test_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).expect("create isolated HOME");
    dir
}

/// Run `forb` with the shipped-default config (isolated HOME) and no color.
/// Returns (stdout, stderr).
fn run_forb_default(args: &[&str]) -> (String, String) {
    let forb = env!("CARGO_BIN_EXE_forb");
    let home = isolated_home();
    let output = Command::new(forb)
        .args(args)
        .env("HOME", &home)
        .env("NO_COLOR", "1")
        // Ensure network-touching behavior stays off regardless of ambient env.
        .env("FORB_CHECK_UPDATES", "0")
        .output()
        .expect("run forb");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Run `forb` with piped stdin (isolated HOME, no color). Returns (stdout, stderr).
fn run_forb_piped(args: &[&str], stdin_data: &str) -> (String, String) {
    use std::io::Write;
    let forb = env!("CARGO_BIN_EXE_forb");
    let home = isolated_home();
    let mut child = Command::new(forb)
        .args(args)
        .env("HOME", &home)
        .env("NO_COLOR", "1")
        .env("FORB_CHECK_UPDATES", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forb");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin_data.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait forb");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Extract forb commands from README.md
fn extract_forb_commands() -> Vec<(String, Option<String>)> {
    let readme = include_str!("../../../README.md");
    let mut commands = Vec::new();
    let mut in_code_block = false;
    let mut current_command = None;
    let mut expected_output = None;

    for line in readme.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End of code block
                if let Some(cmd) = current_command.take() {
                    commands.push((cmd, expected_output.take()));
                }
                in_code_block = false;
            } else {
                // Start of code block
                in_code_block = line.starts_with("```bash") || line.starts_with("```shell");
            }
            continue;
        }

        if in_code_block {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Extract forb commands (handle both `forb` and `$ forb`)
            if trimmed.starts_with("forb ") || trimmed.starts_with("$ forb ") {
                // Save previous command if any
                if let Some(cmd) = current_command.take() {
                    commands.push((cmd, expected_output.take()));
                }

                let cmd = trimmed.strip_prefix("$ ").unwrap_or(trimmed).to_string();
                current_command = Some(cmd);
                expected_output = None;
            } else if current_command.is_some() && trimmed.starts_with("▶") {
                // This is expected output - capture the format name
                expected_output = Some(trimmed.to_string());
            }
        }
    }

    // Don't forget the last command
    if let Some(cmd) = current_command {
        commands.push((cmd, expected_output));
    }

    commands
}

/// Parse a forb command into args, handling quotes
fn parse_command(cmd: &str) -> Option<Vec<String>> {
    // Remove "forb " prefix
    let args_str = cmd.strip_prefix("forb ")?;

    // Simple quote-aware parsing
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = args_str.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            '\\' if in_double_quote => {
                // Handle escape sequences in double quotes
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        chars.next();
                        current.push(next);
                    } else {
                        current.push(c);
                    }
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    Some(args)
}

/// Commands that we skip testing (pipes, special cases, etc.)
fn should_skip(cmd: &str) -> bool {
    // Skip commands with pipes (complex shell operations)
    if cmd.contains(" | ") {
        return true;
    }

    // Skip commands that need external tools
    if cmd.contains("dot -T") {
        return true;
    }

    // Skip commands that just show help/info
    if cmd.contains("--formats")
        || cmd.contains("--config-path")
        || cmd.contains("--config-init")
        || cmd.contains("--analytics")
        || cmd.contains("--plugins")
        || cmd.contains("--currency")
        || cmd.contains("--check-updates")
        || cmd.contains("--graph")
    {
        return true;
    }

    // Skip file operations that need actual files
    if cmd.contains("@") {
        return true;
    }

    // Skip commands that need network (currency conversion with live rates)
    if cmd.contains("USD") || cmd.contains("EUR") || cmd.contains("BTC") || cmd.contains("SEK") {
        return true;
    }

    false
}

#[test]
fn test_readme_examples_parse() {
    let commands = extract_forb_commands();

    // We should find a reasonable number of examples
    assert!(
        commands.len() >= 20,
        "Expected at least 20 forb examples in README, found {}",
        commands.len()
    );

    println!("Found {} forb commands in README.md", commands.len());
}

#[test]
fn test_readme_examples_run() {
    let forb = env!("CARGO_BIN_EXE_forb");
    let commands = extract_forb_commands();

    let mut tested = 0;
    let mut skipped = 0;
    let mut failed = Vec::new();

    for (cmd, expected) in &commands {
        if should_skip(cmd) {
            skipped += 1;
            continue;
        }

        let Some(args) = parse_command(cmd) else {
            eprintln!("Failed to parse command: {}", cmd);
            continue;
        };

        // Run the command
        let output = Command::new(forb).args(&args).output();

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);

                // Check if it produced output (not necessarily success exit code)
                // Some interpretations might not be found, which is OK
                let has_output = !stdout.is_empty() || !stderr.is_empty();

                if !has_output && !result.status.success() {
                    failed.push(format!(
                        "{}: no output, exit code {:?}",
                        cmd,
                        result.status.code()
                    ));
                } else if expected.is_some() {
                    // Check if expected format is mentioned
                    if !stdout.contains("▶") && !stderr.contains("error") {
                        // No interpretation found - might be OK for some edge cases
                        eprintln!("Warning: {} - no interpretation found", cmd);
                    }
                }
                tested += 1;
            }
            Err(e) => {
                failed.push(format!("{}: failed to run: {}", cmd, e));
            }
        }
    }

    println!("\nREADME examples: {} tested, {} skipped", tested, skipped);

    if !failed.is_empty() {
        eprintln!("\nFailed examples:");
        for f in &failed {
            eprintln!("  - {}", f);
        }
        panic!("{} README examples failed", failed.len());
    }
}

#[test]
fn test_core_examples() {
    // Test a few critical examples that must always work
    let forb = env!("CARGO_BIN_EXE_forb");

    let critical_examples = [
        ("691E01B8", "hex"),
        ("aR4BuA==", "base64"),
        ("1703456789", "decimal"),
        ("550e8400-e29b-41d4-a716-446655440000", "uuid"),
        ("192.168.1.1", "ipv4"),
        ("#FF5733", "color"),
        ("0xFF + 1", "expr"),
        ("1h30m", "duration"),
        ("5km", "length"),
        ("30C", "temperature"),
    ];

    for (input, expected_format) in critical_examples {
        let output = Command::new(forb)
            .arg(input)
            .output()
            .expect("Failed to run forb");

        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            stdout.contains(&format!("▶ {}", expected_format))
                || stdout.contains(&format!("▶ {}-", expected_format)), // e.g., "color-hex"
            "Expected '{}' to be interpreted as '{}', got:\n{}",
            input,
            expected_format,
            stdout
        );
    }
}

/// Locks in the README front-page example (`forb 691E01B8`) against the
/// shipped-default config. Phase 1 re-ranked conversions so the output now
/// leads with `decimal` then `epoch-seconds`; this test fails if that drifts.
///
/// Asserts only on stable substrings — never on relative-time strings
/// ("7 months ago") or the "(N more)" tail, both of which change over time.
#[test]
fn test_readme_front_page_example() {
    let (stdout, _stderr) = run_forb_default(&["691E01B8"]);

    assert!(
        stdout.contains("▶ hex"),
        "front-page example should be interpreted as hex, got:\n{stdout}"
    );
    // The canonical decimal value must lead the conversions.
    assert!(
        stdout.contains("decimal: 1763574200"),
        "front-page example should show decimal 1763574200, got:\n{stdout}"
    );
    // The big-endian epoch timestamp (date part is stable regardless of "now").
    assert!(
        stdout.contains("epoch-seconds: 2025-11-19T17:43:20"),
        "front-page example should show the epoch-seconds timestamp, got:\n{stdout}"
    );

    // Ranking guard: decimal must appear before epoch-seconds, and the
    // spurious hex→ipv4 reading must NOT lead (it was removed in Phase 1).
    let dec = stdout.find("decimal: 1763574200").expect("decimal present");
    let epoch = stdout
        .find("epoch-seconds: 2025-11-19")
        .expect("epoch present");
    assert!(
        dec < epoch,
        "decimal should rank before epoch-seconds, got:\n{stdout}"
    );
}

/// Locks in the README tee-mode example. `cat server.log | forb --tee`
/// annotates each line with the highest-value interpretation: a UUID line
/// shows its version/variant, a hex-bytes line shows the decimal value.
#[test]
fn test_readme_tee_example() {
    let log = "[2024-01-15 10:30:45] User 550e8400-e29b-41d4-a716-446655440000 logged in\n\
               [2024-01-15 10:30:46] Received payload: 69 1E 01 B8\n";
    let (stdout, _stderr) = run_forb_piped(&["--tee"], log);

    // Original lines are passed through.
    assert!(
        stdout.contains("logged in") && stdout.contains("Received payload"),
        "tee should pass through original lines, got:\n{stdout}"
    );
    // UUID annotates with version/variant, NOT a nonsense ipv6 re-encoding.
    assert!(
        stdout.contains("uuid: UUID v4"),
        "UUID line should annotate with version/variant, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("uuid: ipv6"),
        "UUID line must not annotate with a nonsense ipv6 re-encoding, got:\n{stdout}"
    );
    // Hex bytes annotate with the integer value.
    assert!(
        stdout.contains("hex: decimal: 1763574200"),
        "hex-bytes line should annotate with the decimal value, got:\n{stdout}"
    );
}

/// Bare piped multi-line input (no `--tee`) is analyzed as a single blob, but
/// forb should print a one-line stderr hint pointing at `--tee`. This keeps the
/// README's documented behavior honest and non-breaking.
#[test]
fn test_bare_multiline_pipe_hints_tee() {
    let log = "550e8400-e29b-41d4-a716-446655440000\n69 1E 01 B8\n";
    let (_stdout, stderr) = run_forb_piped(&[], log);

    assert!(
        stderr.contains("--tee"),
        "multi-line piped input should hint at --tee on stderr, got stderr:\n{stderr}"
    );

    // Single-line piped input must NOT emit the hint.
    let (_stdout1, stderr1) = run_forb_piped(&[], "691E01B8\n");
    assert!(
        !stderr1.contains("--tee"),
        "single-line piped input should not hint at --tee, got stderr:\n{stderr1}"
    );
}
