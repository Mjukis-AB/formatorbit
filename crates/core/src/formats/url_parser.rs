//! URL parsing format with component breakdown and tracking removal.
//!
//! Parses URLs and provides:
//! - Structured breakdown of URL components (scheme, host, path, query params, fragment)
//! - Cleaned URL with tracking parameters removed (utm_*, fbclid, gclid, etc.)

use url::Url;

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
    RichDisplay, RichDisplayOption,
};

/// Tracking parameters to remove from URLs.
const TRACKING_PARAMS: &[&str] = &[
    // Google Analytics
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_content",
    "utm_term",
    "utm_id",
    // Click IDs
    "fbclid",
    "gclid",
    "msclkid",
    "yclid",
    "twclid",
    "ttclid",
    // Mailchimp
    "mc_eid",
    "mc_cid",
    // Google Analytics cookies
    "_ga",
    "_gl",
    // Other common tracking
    "ref_src",
];

pub struct UrlParserFormat;

impl UrlParserFormat {
    /// Check if a parameter is a tracking parameter.
    fn is_tracking_param(name: &str) -> bool {
        TRACKING_PARAMS.contains(&name.to_lowercase().as_str())
    }

    /// Try to parse input as a URL, adding https:// if no scheme is present.
    fn try_parse_url(input: &str) -> Option<Url> {
        // First, try to parse as-is
        if let Ok(url) = Url::parse(input) {
            return Some(url);
        }

        // If it looks like a URL without scheme, prepend https://
        if Self::looks_like_url_without_scheme(input) {
            if let Ok(url) = Url::parse(&format!("https://{}", input)) {
                return Some(url);
            }
        }

        None
    }

    /// Check if input looks like a URL without a scheme.
    fn looks_like_url_without_scheme(input: &str) -> bool {
        // Must contain a dot (for domain)
        if !input.contains('.') {
            return false;
        }

        // Shouldn't contain spaces
        if input.contains(' ') {
            return false;
        }

        // Shouldn't start with @ (email)
        if input.starts_with('@') {
            return false;
        }

        // If it has @ but no scheme, it's likely an email, not a URL
        if input.contains('@') && !input.contains('/') {
            return false;
        }

        // Check if it starts with something that looks like a domain
        let first_part = input.split('/').next().unwrap_or(input);
        let first_part = first_part.split('?').next().unwrap_or(first_part);
        let first_part = first_part.split('#').next().unwrap_or(first_part);

        // Remove port if present
        let domain_part = first_part.split(':').next().unwrap_or(first_part);

        // Check for TLD-like ending
        let parts: Vec<&str> = domain_part.split('.').collect();
        if parts.len() < 2 {
            return false;
        }

        let tld = parts.last().unwrap_or(&"");

        // Common TLDs (not exhaustive, but covers most cases)
        let common_tlds = [
            "com", "org", "net", "io", "co", "edu", "gov", "mil", "app", "dev", "ai", "me", "us",
            "uk", "de", "fr", "jp", "cn", "ru", "in", "br", "au", "ca", "it", "es", "nl", "se",
            "no", "fi", "dk", "pl", "cz", "at", "ch", "be", "pt", "gr", "ie", "nz", "za", "mx",
            "ar", "cl", "info", "biz", "xyz", "online", "site", "tech", "store", "blog",
        ];

        common_tlds.contains(tld)
    }

    /// Calculate confidence score for URL interpretation.
    fn calculate_confidence(input: &str, url: &Url) -> f32 {
        // High confidence if has recognized scheme
        if input.starts_with("http://")
            || input.starts_with("https://")
            || input.starts_with("ftp://")
        {
            return 0.95;
        }

        // Medium-high confidence if has tracking params (strong signal)
        let has_tracking = url.query_pairs().any(|(k, _)| Self::is_tracking_param(&k));
        if has_tracking {
            return 0.85;
        }

        // Check for path or query string (stronger signal than bare domain)
        let has_path = url.path() != "/" && !url.path().is_empty();
        let has_query = url.query().is_some();

        if has_path || has_query {
            return 0.75;
        }

        // Bare domain only - lower confidence
        0.60
    }

    /// Remove tracking parameters and return the cleaned URL.
    fn clean_url(url: &Url) -> (String, Vec<String>) {
        let mut cleaned = url.clone();
        let mut removed = Vec::new();

        // Collect non-tracking query parameters
        if url.query().is_some() {
            let remaining: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(k, _)| {
                    if Self::is_tracking_param(k) {
                        removed.push(k.to_string());
                        false
                    } else {
                        true
                    }
                })
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();

            // Update query string
            if remaining.is_empty() {
                cleaned.set_query(None);
            } else {
                let query: String = remaining
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                cleaned.set_query(Some(&query));
            }
        }

        (cleaned.to_string(), removed)
    }

    /// Build rich display info for URL interpretation.
    fn build_rich_display(url: &Url, had_scheme: bool) -> Vec<RichDisplayOption> {
        let mut pairs = Vec::new();

        pairs.push(("Scheme".to_string(), url.scheme().to_string()));

        if let Some(host) = url.host_str() {
            pairs.push(("Host".to_string(), host.to_string()));
        }

        if let Some(port) = url.port() {
            pairs.push(("Port".to_string(), port.to_string()));
        }

        if url.path() != "/" && !url.path().is_empty() {
            pairs.push(("Path".to_string(), url.path().to_string()));
        }

        // Show query parameters
        let query_pairs: Vec<_> = url.query_pairs().collect();
        if !query_pairs.is_empty() {
            let query_str = query_pairs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            pairs.push(("Query".to_string(), query_str));
        }

        if let Some(fragment) = url.fragment() {
            pairs.push(("Fragment".to_string(), fragment.to_string()));
        }

        // Note if we added the scheme
        if !had_scheme {
            pairs.push(("Note".to_string(), "https:// scheme assumed".to_string()));
        }

        vec![RichDisplayOption::new(RichDisplay::KeyValue { pairs })]
    }

    /// Build description for URL interpretation.
    fn build_description(url: &Url) -> String {
        let mut parts = Vec::new();

        if let Some(host) = url.host_str() {
            parts.push(format!("Host: {}", host));
        }

        if url.path() != "/" && !url.path().is_empty() {
            parts.push(format!("Path: {}", url.path()));
        }

        let query_count = url.query_pairs().count();
        if query_count > 0 {
            parts.push(format!("{} query params", query_count));
        }

        if url.fragment().is_some() {
            parts.push("has fragment".to_string());
        }

        parts.join(", ")
    }
}

impl Format for UrlParserFormat {
    fn id(&self) -> &'static str {
        "url-parser"
    }

    fn name(&self) -> &'static str {
        "URL"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Web",
            description: "URL parsing with component breakdown and tracking removal",
            examples: &[
                "https://example.com/page?id=123",
                "shop.com/item?fbclid=ABC&utm_source=google",
            ],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Check if it starts with a scheme
        let had_scheme = trimmed.contains("://");

        let url = match Self::try_parse_url(trimmed) {
            Some(u) => u,
            None => return vec![],
        };

        // Only accept http/https/ftp URLs
        if !["http", "https", "ftp", "mailto"].contains(&url.scheme()) {
            return vec![];
        }

        // Require a host
        if url.host_str().is_none() {
            return vec![];
        }

        let confidence = Self::calculate_confidence(trimmed, &url);
        let description = Self::build_description(&url);
        let rich_display = Self::build_rich_display(&url, had_scheme);

        vec![Interpretation {
            value: CoreValue::String(url.to_string()),
            source_format: "url-parser".to_string(),
            confidence,
            description,
            rich_display,
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::String(url_str) = value else {
            return vec![];
        };

        let Ok(url) = Url::parse(url_str) else {
            return vec![];
        };

        let mut conversions = Vec::new();

        // Check if there are tracking params to remove
        let (cleaned_url, removed) = Self::clean_url(&url);

        if !removed.is_empty() {
            let removed_str = removed.join(", ");
            let display = format!("{} (removed: {})", cleaned_url, removed_str);

            conversions.push(Conversion {
                value: CoreValue::String(cleaned_url.clone()),
                target_format: "url-cleaned".to_string(),
                display,
                path: vec!["url-cleaned".to_string()],
                steps: vec![ConversionStep {
                    format: "url-cleaned".to_string(),
                    value: CoreValue::String(cleaned_url.clone()),
                    display: cleaned_url,
                }],
                priority: ConversionPriority::Semantic,
                kind: ConversionKind::Representation,
                display_only: true,
                is_lossy: false,
                hidden: false,
                rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue {
                    pairs: vec![
                        ("Cleaned URL".to_string(), url_str.clone()),
                        ("Removed".to_string(), removed_str),
                    ],
                })],
            });
        }

        conversions
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["url-parse", "link"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_with_scheme() {
        let format = UrlParserFormat;
        let results = format.parse("https://example.com/page?id=123");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "url-parser");
        assert!(results[0].confidence >= 0.95);
    }

    #[test]
    fn test_parse_url_without_scheme() {
        let format = UrlParserFormat;
        let results = format.parse("example.com/page?id=123");

        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 0.70);
    }

    #[test]
    fn test_parse_url_with_tracking() {
        let format = UrlParserFormat;
        let results = format.parse("shop.com/item?fbclid=ABC&utm_source=google");

        assert_eq!(results.len(), 1);
        // Should have higher confidence due to tracking params
        assert!(results[0].confidence >= 0.85);
    }

    #[test]
    fn test_clean_url_removes_tracking() {
        let format = UrlParserFormat;
        let results = format.parse("https://example.com/page?id=123&utm_source=google&fbclid=ABC");

        assert_eq!(results.len(), 1);

        if let CoreValue::String(_url_str) = &results[0].value {
            let conversions = format.conversions(&results[0].value);

            // Should have a url-cleaned conversion
            assert!(!conversions.is_empty());
            let cleaned = &conversions[0];
            assert_eq!(cleaned.target_format, "url-cleaned");

            // The cleaned URL should not have tracking params
            if let CoreValue::String(cleaned_url) = &cleaned.value {
                assert!(!cleaned_url.contains("utm_source"));
                assert!(!cleaned_url.contains("fbclid"));
                assert!(cleaned_url.contains("id=123")); // Semantic param preserved
            } else {
                panic!("Expected String value");
            }
        } else {
            panic!("Expected String value");
        }
    }

    #[test]
    fn test_no_clean_conversion_without_tracking() {
        let format = UrlParserFormat;
        let results = format.parse("https://api.example.com/v1/users?limit=10");

        assert_eq!(results.len(), 1);
        let conversions = format.conversions(&results[0].value);

        // Should NOT have a url-cleaned conversion (no tracking params)
        assert!(conversions.is_empty());
    }

    #[test]
    fn test_not_url() {
        let format = UrlParserFormat;

        // Plain text should not parse as URL
        assert!(format.parse("hello world").is_empty());

        // Email should not parse as URL
        assert!(format.parse("user@example.com").is_empty());

        // Just numbers
        assert!(format.parse("12345").is_empty());
    }

    #[test]
    fn test_url_with_fragment() {
        let format = UrlParserFormat;
        let results = format.parse("https://example.com/page#section1");

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("has fragment"));
    }

    #[test]
    fn test_clean_url_preserves_fragment() {
        let format = UrlParserFormat;
        let results = format.parse("https://example.com/page?utm_source=google#section1");

        assert_eq!(results.len(), 1);
        let conversions = format.conversions(&results[0].value);

        assert!(!conversions.is_empty());
        if let CoreValue::String(cleaned_url) = &conversions[0].value {
            assert!(cleaned_url.contains("#section1"));
        }
    }
}
