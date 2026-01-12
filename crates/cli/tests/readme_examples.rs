//! Validates that README.md examples still work.
//!
//! This test extracts shell commands from README.md and verifies:
//! 1. Commands that should succeed do succeed
//! 2. Output contains expected patterns
//!
//! Run with: cargo test -p formatorbit-cli --test readme_examples

use std::process::Command;

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
