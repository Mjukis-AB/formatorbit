//! Local usage analytics with opt-in anonymous contribution.
//!
//! Privacy-first design:
//! - Local tracking enabled by default (stored in human-readable TOML)
//! - Anonymous contribution is explicitly opt-in
//! - No PII collected (no input data, filenames, URLs)
//! - Users can inspect, clear, and disable tracking at any time

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Current analytics data version.
const ANALYTICS_VERSION: u32 = 1;

/// Main analytics data structure.
///
/// Stored at `~/.config/forb/analytics.toml` (or platform equivalent).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyticsData {
    /// Schema version for forward compatibility.
    #[serde(default)]
    pub version: u32,

    /// When tracking started.
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,

    /// Last update time.
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,

    /// Format usage counts (source formats detected).
    #[serde(default)]
    pub format_usage: HashMap<String, u64>,

    /// Conversion target counts.
    #[serde(default)]
    pub conversion_targets: HashMap<String, u64>,

    /// Config customization tracking.
    #[serde(default)]
    pub config_changes: ConfigChangeStats,

    /// Session statistics.
    #[serde(default)]
    pub session_stats: SessionStats,
}

/// Statistics about config customization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigChangeStats {
    /// Times limit was customized (non-default).
    #[serde(default)]
    pub limit_customized: u64,

    /// Times threshold was customized.
    #[serde(default)]
    pub threshold_customized: u64,

    /// Times --only filter was used.
    #[serde(default)]
    pub only_filter_used: u64,

    /// Times priority config was used.
    #[serde(default)]
    pub priority_customized: u64,

    /// Times blocking config was used.
    #[serde(default)]
    pub blocking_customized: u64,

    /// Common --only filters (format -> count).
    #[serde(default)]
    pub common_only_filters: HashMap<String, u64>,

    /// Common blocked formats (format -> count).
    #[serde(default)]
    pub common_blocked_formats: HashMap<String, u64>,
}

/// Session-level statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    /// Total CLI invocations.
    #[serde(default)]
    pub total_invocations: u64,

    /// Times pipe mode was used.
    #[serde(default)]
    pub pipe_mode_uses: u64,

    /// Times file input (@path) was used.
    #[serde(default)]
    pub file_input_uses: u64,

    /// Times URL fetch (@http...) was used.
    #[serde(default)]
    pub url_fetch_uses: u64,

    /// Times --json output was used.
    #[serde(default)]
    pub json_output_uses: u64,

    /// Times --raw output was used.
    #[serde(default)]
    pub raw_output_uses: u64,

    /// Times --dot output was used.
    #[serde(default)]
    pub dot_output_uses: u64,

    /// Times --mermaid output was used.
    #[serde(default)]
    pub mermaid_output_uses: u64,
}

impl AnalyticsData {
    /// Get the analytics file path.
    ///
    /// - Linux/macOS: `~/.config/forb/analytics.toml`
    /// - Windows: `%APPDATA%\forb\analytics.toml`
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("forb").join("analytics.toml"))
    }

    /// Load analytics data from file. Returns default if file doesn't exist.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::new();
        };

        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::new();
        };

        toml::from_str(&contents).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse analytics file: {}", e);
            Self::new()
        })
    }

    /// Create a new analytics data instance.
    fn new() -> Self {
        Self {
            version: ANALYTICS_VERSION,
            started_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            ..Default::default()
        }
    }

    /// Save analytics data to file.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("Cannot determine config directory")?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let contents =
            toml::to_string_pretty(self).map_err(|e| format!("Failed to serialize: {}", e))?;

        fs::write(&path, contents).map_err(|e| format!("Failed to write file: {}", e))
    }

    /// Clear all analytics data (reset to fresh state).
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

/// Analytics tracker for recording events.
///
/// Collects events during a CLI invocation and saves on drop (if enabled).
pub struct AnalyticsTracker {
    data: AnalyticsData,
    enabled: bool,
    dirty: bool,
}

impl AnalyticsTracker {
    /// Create a new tracker with the given enabled state.
    pub fn new(enabled: bool) -> Self {
        let data = if enabled {
            AnalyticsData::load()
        } else {
            AnalyticsData::default()
        };

        Self {
            data,
            enabled,
            dirty: false,
        }
    }

    /// Check if analytics is enabled.
    ///
    /// Reserved for Phase 3 (contribution) functionality.
    #[must_use]
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Record a CLI invocation.
    pub fn record_invocation(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.total_invocations += 1;
        self.data.updated_at = Some(Utc::now());
        self.dirty = true;
    }

    /// Record format usage (when a format is detected as source).
    pub fn record_format_usage(&mut self, format: &str) {
        if !self.enabled {
            return;
        }
        *self
            .data
            .format_usage
            .entry(format.to_string())
            .or_insert(0) += 1;
        self.dirty = true;
    }

    /// Record a conversion target.
    pub fn record_conversion_target(&mut self, target: &str) {
        if !self.enabled {
            return;
        }
        *self
            .data
            .conversion_targets
            .entry(target.to_string())
            .or_insert(0) += 1;
        self.dirty = true;
    }

    /// Record pipe mode usage.
    pub fn record_pipe_mode(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.pipe_mode_uses += 1;
        self.dirty = true;
    }

    /// Record file input usage.
    pub fn record_file_input(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.file_input_uses += 1;
        self.dirty = true;
    }

    /// Record URL fetch usage.
    pub fn record_url_fetch(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.url_fetch_uses += 1;
        self.dirty = true;
    }

    /// Record JSON output usage.
    pub fn record_json_output(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.json_output_uses += 1;
        self.dirty = true;
    }

    /// Record raw output usage.
    pub fn record_raw_output(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.raw_output_uses += 1;
        self.dirty = true;
    }

    /// Record DOT output usage.
    pub fn record_dot_output(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.dot_output_uses += 1;
        self.dirty = true;
    }

    /// Record Mermaid output usage.
    pub fn record_mermaid_output(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.session_stats.mermaid_output_uses += 1;
        self.dirty = true;
    }

    /// Record --only filter usage.
    pub fn record_only_filter(&mut self, formats: &[String]) {
        if !self.enabled || formats.is_empty() {
            return;
        }
        self.data.config_changes.only_filter_used += 1;
        for format in formats {
            *self
                .data
                .config_changes
                .common_only_filters
                .entry(format.clone())
                .or_insert(0) += 1;
        }
        self.dirty = true;
    }

    /// Record limit customization.
    pub fn record_limit_customized(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.config_changes.limit_customized += 1;
        self.dirty = true;
    }

    /// Record threshold customization.
    pub fn record_threshold_customized(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.config_changes.threshold_customized += 1;
        self.dirty = true;
    }

    /// Record priority config usage.
    pub fn record_priority_customized(&mut self) {
        if !self.enabled {
            return;
        }
        self.data.config_changes.priority_customized += 1;
        self.dirty = true;
    }

    /// Record blocking config usage.
    pub fn record_blocking_customized(&mut self, blocked_formats: &[String]) {
        if !self.enabled {
            return;
        }
        self.data.config_changes.blocking_customized += 1;
        for format in blocked_formats {
            *self
                .data
                .config_changes
                .common_blocked_formats
                .entry(format.clone())
                .or_insert(0) += 1;
        }
        self.dirty = true;
    }

    /// Get read-only access to the data (for display commands).
    ///
    /// Note: Currently unused by CLI but useful for testing and Phase 3.
    #[must_use]
    #[cfg(test)]
    pub fn data(&self) -> &AnalyticsData {
        &self.data
    }

    /// Get mutable access to data (for clear command).
    pub fn data_mut(&mut self) -> &mut AnalyticsData {
        self.dirty = true;
        &mut self.data
    }

    /// Save analytics data if dirty.
    pub fn save(&self) {
        if !self.enabled || !self.dirty {
            return;
        }
        if let Err(e) = self.data.save() {
            tracing::debug!("Failed to save analytics: {}", e);
        }
    }
}

impl Drop for AnalyticsTracker {
    fn drop(&mut self) {
        self.save();
    }
}

/// Format analytics data for human-readable display.
pub fn format_status(data: &AnalyticsData, enabled: bool) -> String {
    let mut out = String::new();

    // Status
    out.push_str(&format!(
        "Analytics: {}\n",
        if enabled { "enabled" } else { "disabled" }
    ));

    if let Some(path) = AnalyticsData::path() {
        out.push_str(&format!("Data file: {}\n", path.display()));
    }

    if let Some(started) = data.started_at {
        out.push_str(&format!("Tracking since: {}\n", started.format("%Y-%m-%d")));
    }

    out.push('\n');

    // Summary stats
    out.push_str(&format!(
        "Total invocations: {}\n",
        data.session_stats.total_invocations
    ));

    let feature_count = data.session_stats.pipe_mode_uses
        + data.session_stats.file_input_uses
        + data.session_stats.url_fetch_uses
        + data.session_stats.json_output_uses;

    if feature_count > 0 {
        out.push_str("\nFeature usage:\n");
        if data.session_stats.pipe_mode_uses > 0 {
            out.push_str(&format!(
                "  Pipe mode: {}\n",
                data.session_stats.pipe_mode_uses
            ));
        }
        if data.session_stats.file_input_uses > 0 {
            out.push_str(&format!(
                "  File input: {}\n",
                data.session_stats.file_input_uses
            ));
        }
        if data.session_stats.url_fetch_uses > 0 {
            out.push_str(&format!(
                "  URL fetch: {}\n",
                data.session_stats.url_fetch_uses
            ));
        }
        if data.session_stats.json_output_uses > 0 {
            out.push_str(&format!(
                "  JSON output: {}\n",
                data.session_stats.json_output_uses
            ));
        }
    }

    // Top formats
    if !data.format_usage.is_empty() {
        out.push_str("\nTop source formats:\n");
        let mut formats: Vec<_> = data.format_usage.iter().collect();
        formats.sort_by(|a, b| b.1.cmp(a.1));
        for (format, count) in formats.iter().take(10) {
            out.push_str(&format!("  {}: {}\n", format, count));
        }
    }

    out
}

/// Format analytics data as full TOML (for show command).
pub fn format_full(data: &AnalyticsData) -> String {
    toml::to_string_pretty(data).unwrap_or_else(|_| "Failed to serialize data".to_string())
}

// =============================================================================
// Anonymous Contribution (Phase 3)
// =============================================================================

/// TelemetryDeck app ID for forb.
const TELEMETRY_APP_ID: &str = "DAE80104-B36F-4556-96FB-7288CD467265";

/// TelemetryDeck ingest endpoint.
const TELEMETRY_ENDPOINT: &str = "https://nom.telemetrydeck.com/v2/";

/// Anonymized payload for contribution.
///
/// This is what gets sent to TelemetryDeck. Contains only aggregate
/// counts and percentages - never any input data or PII.
#[derive(Debug, Clone, Serialize)]
pub struct ContributionPayload {
    /// CLI version.
    pub version: String,
    /// Platform (darwin/linux/windows).
    pub platform: String,
    /// Top 10 source formats (comma-separated).
    pub top_formats: String,
    /// Top 10 conversion targets (comma-separated).
    pub top_targets: String,
    /// Total invocations.
    pub total_invocations: u64,
    /// Pipe mode usage count.
    pub pipe_mode_uses: u64,
    /// File input usage count.
    pub file_input_uses: u64,
    /// URL fetch usage count.
    pub url_fetch_uses: u64,
    /// JSON output usage count.
    pub json_output_uses: u64,
    /// Days since tracking started.
    pub tracking_days: u64,
}

impl ContributionPayload {
    /// Create a contribution payload from analytics data.
    #[must_use]
    pub fn from_data(data: &AnalyticsData) -> Self {
        // Get top 10 formats by usage
        let mut formats: Vec<_> = data.format_usage.iter().collect();
        formats.sort_by(|a, b| b.1.cmp(a.1));
        let top_formats: Vec<_> = formats.iter().take(10).map(|(k, _)| k.as_str()).collect();

        // Get top 10 conversion targets
        let mut targets: Vec<_> = data.conversion_targets.iter().collect();
        targets.sort_by(|a, b| b.1.cmp(a.1));
        let top_targets: Vec<_> = targets.iter().take(10).map(|(k, _)| k.as_str()).collect();

        // Calculate days since tracking started
        let tracking_days = data
            .started_at
            .map(|started| {
                let now = Utc::now();
                now.signed_duration_since(started).num_days().max(0) as u64
            })
            .unwrap_or(0);

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            top_formats: top_formats.join(","),
            top_targets: top_targets.join(","),
            total_invocations: data.session_stats.total_invocations,
            pipe_mode_uses: data.session_stats.pipe_mode_uses,
            file_input_uses: data.session_stats.file_input_uses,
            url_fetch_uses: data.session_stats.url_fetch_uses,
            json_output_uses: data.session_stats.json_output_uses,
            tracking_days,
        }
    }

    /// Format as human-readable preview.
    #[must_use]
    pub fn format_preview(&self) -> String {
        let mut out = String::new();
        out.push_str("Contribution preview (exactly what will be sent):\n\n");
        out.push_str(&format!("  version: {}\n", self.version));
        out.push_str(&format!("  platform: {}\n", self.platform));
        out.push_str(&format!("  tracking_days: {}\n", self.tracking_days));
        out.push_str(&format!(
            "  total_invocations: {}\n",
            self.total_invocations
        ));
        out.push_str(&format!("  pipe_mode_uses: {}\n", self.pipe_mode_uses));
        out.push_str(&format!("  file_input_uses: {}\n", self.file_input_uses));
        out.push_str(&format!("  url_fetch_uses: {}\n", self.url_fetch_uses));
        out.push_str(&format!("  json_output_uses: {}\n", self.json_output_uses));
        out.push_str(&format!("  top_formats: {}\n", self.top_formats));
        out.push_str(&format!("  top_targets: {}\n", self.top_targets));
        out.push_str(
            "\nNo input data, filenames, or personally identifiable information is included.\n",
        );
        out
    }
}

/// TelemetryDeck signal format.
#[derive(Debug, Serialize)]
struct TelemetrySignal {
    #[serde(rename = "appID")]
    app_id: String,
    #[serde(rename = "clientUser")]
    client_user: String,
    #[serde(rename = "type")]
    signal_type: String,
    payload: ContributionPayload,
}

/// Send contribution to TelemetryDeck.
///
/// Returns Ok(()) on success, Err with message on failure.
pub fn send_contribution(data: &AnalyticsData) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    // Generate a random client ID (not persisted = no tracking across contributions)
    let random_id = uuid::Uuid::new_v4().to_string();
    let mut hasher = Sha256::new();
    hasher.update(random_id.as_bytes());
    let client_user = format!("{:x}", hasher.finalize());

    let payload = ContributionPayload::from_data(data);

    let signal = TelemetrySignal {
        app_id: TELEMETRY_APP_ID.to_string(),
        client_user,
        signal_type: "forb.contribution".to_string(),
        payload,
    };

    // TelemetryDeck expects an array of signals
    let signals = vec![signal];
    let body =
        serde_json::to_string(&signals).map_err(|e| format!("Failed to serialize: {}", e))?;

    let response = ureq::post(TELEMETRY_ENDPOINT)
        .set("Content-Type", "application/json; charset=utf-8")
        .timeout(std::time::Duration::from_secs(10))
        .send_string(&body)
        .map_err(|e| format!("Failed to send: {}", e))?;

    if response.status() >= 200 && response.status() < 300 {
        Ok(())
    } else {
        Err(format!("Server returned status {}", response.status()))
    }
}

/// Format the contribution payload as JSON (for --analytics preview).
#[allow(dead_code)]
pub fn format_contribution_json(data: &AnalyticsData) -> String {
    let payload = ContributionPayload::from_data(data);
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "Failed to serialize".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_analytics_data() {
        let data = AnalyticsData::load();
        assert_eq!(data.version, ANALYTICS_VERSION);
        assert!(data.started_at.is_some());
    }

    #[test]
    fn test_tracker_disabled() {
        let mut tracker = AnalyticsTracker::new(false);
        tracker.record_invocation();
        tracker.record_format_usage("hex");

        // Should not record anything when disabled
        assert_eq!(tracker.data().session_stats.total_invocations, 0);
        assert!(tracker.data().format_usage.is_empty());
    }

    #[test]
    fn test_tracker_records_formats() {
        let mut tracker = AnalyticsTracker::new(true);
        // Clear any existing data to make test deterministic
        tracker.data_mut().format_usage.clear();

        tracker.record_format_usage("hex");
        tracker.record_format_usage("hex");
        tracker.record_format_usage("uuid");

        assert_eq!(tracker.data().format_usage.get("hex"), Some(&2));
        assert_eq!(tracker.data().format_usage.get("uuid"), Some(&1));
    }

    #[test]
    fn test_tracker_records_sessions() {
        let mut tracker = AnalyticsTracker::new(true);
        // Reset session stats to make test deterministic
        tracker.data_mut().session_stats = SessionStats::default();

        tracker.record_invocation();
        tracker.record_pipe_mode();
        tracker.record_json_output();

        assert_eq!(tracker.data().session_stats.total_invocations, 1);
        assert_eq!(tracker.data().session_stats.pipe_mode_uses, 1);
        assert_eq!(tracker.data().session_stats.json_output_uses, 1);
    }

    #[test]
    fn test_clear_data() {
        let mut tracker = AnalyticsTracker::new(true);
        tracker.record_invocation();
        tracker.record_format_usage("hex");

        tracker.data_mut().clear();

        assert_eq!(tracker.data().session_stats.total_invocations, 0);
        assert!(tracker.data().format_usage.is_empty());
    }
}
