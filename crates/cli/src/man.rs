//! Man-page generation/installation and paging output.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

use clap::CommandFactory;

use crate::cli::Cli;

/// Generate man page content from the CLI definition.
pub fn generate_man_page() -> String {
    use clap_mangen::Man;
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buffer = Vec::new();
    man.render(&mut buffer).expect("Failed to render man page");
    String::from_utf8(buffer).expect("Man page is not valid UTF-8")
}

/// Get the man page installation path.
fn man_page_path() -> Option<std::path::PathBuf> {
    // Use ~/.local/share/man on all platforms (standard XDG location)
    dirs::home_dir().map(|p| {
        p.join(".local")
            .join("share")
            .join("man")
            .join("man1")
            .join("forb.1")
    })
}

/// Install the man page to the local man directory.
pub fn install_man_page() -> Result<std::path::PathBuf, String> {
    let path = man_page_path().ok_or("Cannot determine data directory")?;

    // Create directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create directory: {}", e))?;
    }

    // Write man page
    let content = generate_man_page();
    fs::write(&path, content).map_err(|e| format!("Cannot write man page: {}", e))?;

    Ok(path)
}

/// Display text through a pager (less/more) if stdout is a TTY.
pub fn show_with_pager(text: &str) {
    // Only use pager if stdout is a terminal
    if !std::io::stdout().is_terminal() {
        // Not a TTY, just print (ignore broken pipe errors when piped to head/etc)
        let _ = io::stdout().write_all(text.as_bytes());
        return;
    }

    // Try to find a pager: $PAGER, then less, then more
    let pager = std::env::var("PAGER").ok();
    let pager_cmd = pager.as_deref().unwrap_or("less");

    // Try to spawn the pager
    let result = Command::new(pager_cmd)
        .arg("-R") // Enable ANSI colors in less
        .stdin(Stdio::piped())
        .spawn();

    match result {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
        Err(_) => {
            // Pager not available, just print
            let _ = io::stdout().write_all(text.as_bytes());
        }
    }
}
