//! Input acquisition: `@path` file reading, `@-` stdin, and URL fetching.

use std::fs;
use std::io::{self, Read};
use std::path::Path;

/// Result of processing input (either direct string or file contents)
pub enum InputData {
    /// Text input to be parsed as string
    Text(String),
    /// Binary data from file (raw bytes, core handles encoding)
    Binary { data: Vec<u8>, path: String },
}

/// Fetch content from a URL with timeout and size limits.
fn fetch_url(url: &str, timeout_secs: u64, max_size: u64) -> Result<InputData, String> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .call()
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("timed out") || msg.contains("Timeout") {
                format!(
                    "Request timed out after {}s. Use --url-timeout to increase the limit.",
                    timeout_secs
                )
            } else {
                format!("Failed to fetch URL '{}': {}", url, e)
            }
        })?;

    let content_type = response
        .header("Content-Type")
        .unwrap_or("application/octet-stream")
        .to_string();

    // Read response body with size limit
    let mut buffer = Vec::new();
    response
        .into_reader()
        .take(max_size)
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Check if we hit the size limit
    if buffer.len() as u64 >= max_size {
        let size_mb = max_size / (1024 * 1024);
        return Err(format!(
            "Response exceeds {} MB limit. Use --url-max-size to increase (e.g., --url-max-size 50M).",
            size_mb
        ));
    }

    // Determine if it's text or binary based on content-type
    let is_text = content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("xml")
        || content_type.contains("javascript");

    if is_text {
        // Try to interpret as UTF-8 text
        match String::from_utf8(buffer) {
            Ok(text) => Ok(InputData::Text(text)),
            Err(e) => {
                // Fall back to binary if not valid UTF-8
                Ok(InputData::Binary {
                    data: e.into_bytes(),
                    path: url.to_string(),
                })
            }
        }
    } else {
        // Binary content
        Ok(InputData::Binary {
            data: buffer,
            path: url.to_string(),
        })
    }
}

/// Read input, handling @path syntax for file reading and URL fetching.
///
/// For URL fetching, uses the provided timeout and size limits.
pub fn read_input(input: &str, url_timeout: u64, url_max_size: u64) -> Result<InputData, String> {
    if !input.starts_with('@') {
        return Ok(InputData::Text(input.to_string()));
    }

    let path = &input[1..];

    // Handle URLs (http:// or https://)
    if path.starts_with("http://") || path.starts_with("https://") {
        return fetch_url(path, url_timeout, url_max_size);
    }

    // Handle @- for stdin
    if path == "-" {
        let mut buffer = Vec::new();
        io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;

        // Try to interpret as UTF-8 text first
        if let Ok(text) = String::from_utf8(buffer.clone()) {
            // If it's valid UTF-8 and doesn't look like binary, treat as text
            if !text.contains('\0') {
                return Ok(InputData::Text(text));
            }
        }

        // Binary data
        return Ok(InputData::Binary {
            data: buffer,
            path: "stdin".to_string(),
        });
    }

    // Check if file exists
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Read file contents
    let buffer =
        fs::read(file_path).map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

    // Try to interpret as UTF-8 text first
    if let Ok(text) = String::from_utf8(buffer.clone()) {
        // If it's valid UTF-8 and doesn't look like binary, treat as text
        if !text.contains('\0') {
            return Ok(InputData::Text(text));
        }
    }

    // Binary data
    Ok(InputData::Binary {
        data: buffer,
        path: path.to_string(),
    })
}
