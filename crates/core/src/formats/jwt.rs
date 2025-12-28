//! JWT (JSON Web Token) format.

use base64::Engine;
use chrono::{TimeZone, Utc};

use crate::format::{Format, FormatInfo};
use crate::types::{CoreValue, Interpretation};

pub struct JwtFormat;

impl JwtFormat {
    /// Decode base64url (URL-safe base64 without padding).
    fn base64url_decode(input: &str) -> Option<Vec<u8>> {
        // Replace URL-safe chars with standard base64 chars
        let standard: String = input
            .chars()
            .map(|c| match c {
                '-' => '+',
                '_' => '/',
                c => c,
            })
            .collect();

        // Add padding if needed
        let padded = match standard.len() % 4 {
            2 => format!("{standard}=="),
            3 => format!("{standard}="),
            _ => standard,
        };

        base64::engine::general_purpose::STANDARD
            .decode(&padded)
            .ok()
    }

    /// Parse a JWT and extract header and payload.
    fn parse_jwt(input: &str) -> Option<(serde_json::Value, serde_json::Value)> {
        let parts: Vec<&str> = input.trim().split('.').collect();

        // JWT must have exactly 3 parts: header.payload.signature
        if parts.len() != 3 {
            return None;
        }

        // Decode header
        let header_bytes = Self::base64url_decode(parts[0])?;
        let header: serde_json::Value = serde_json::from_slice(&header_bytes).ok()?;

        // Verify it looks like a JWT header (must have "alg")
        header.get("alg")?;

        // Decode payload
        let payload_bytes = Self::base64url_decode(parts[1])?;
        let payload: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;

        Some((header, payload))
    }

    /// Format a Unix timestamp claim as human-readable datetime.
    fn format_timestamp(ts: i64) -> Option<String> {
        Utc.timestamp_opt(ts, 0).single().map(|dt| dt.to_rfc3339())
    }

    /// Build a description string with algorithm and key claims.
    fn build_description(header: &serde_json::Value, payload: &serde_json::Value) -> String {
        let alg = header
            .get("alg")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let mut parts = vec![format!("JWT ({alg})")];

        // Add expiry info if present
        if let Some(exp) = payload.get("exp").and_then(|v| v.as_i64()) {
            if let Some(dt) = Self::format_timestamp(exp) {
                let now = Utc::now().timestamp();
                let status = if exp < now { "expired" } else { "valid" };
                parts.push(format!("expires: {dt} ({status})"));
            }
        }

        // Add subject if present
        if let Some(sub) = payload.get("sub").and_then(|v| v.as_str()) {
            if sub.len() <= 50 {
                parts.push(format!("sub: {sub}"));
            }
        }

        parts.join(", ")
    }
}

impl Format for JwtFormat {
    fn id(&self) -> &'static str {
        "jwt"
    }

    fn name(&self) -> &'static str {
        "JWT"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Identifiers",
            description: "JSON Web Token (decoded header and payload)",
            examples: &["eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.xxx"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((header, payload)) = Self::parse_jwt(input) else {
            return vec![];
        };

        // Build combined JSON for display
        let combined = serde_json::json!({
            "header": header,
            "payload": payload
        });

        // Determine confidence
        let has_typ_jwt = header
            .get("typ")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("jwt"))
            .unwrap_or(false);

        let confidence = if has_typ_jwt { 0.98 } else { 0.92 };

        let description = Self::build_description(&header, &payload);

        vec![Interpretation {
            value: CoreValue::Json(combined),
            source_format: "jwt".to_string(),
            confidence,
            description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false // We don't encode JWTs (would need a secret)
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["token"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Standard test JWT: {"alg":"HS256","typ":"JWT"}.{"sub":"1234567890","name":"John Doe","iat":1516239022}
    const TEST_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

    #[test]
    fn test_parse_jwt() {
        let format = JwtFormat;
        let results = format.parse(TEST_JWT);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "jwt");
        assert!(results[0].confidence > 0.9);

        if let CoreValue::Json(json) = &results[0].value {
            assert_eq!(json["header"]["alg"], "HS256");
            assert_eq!(json["header"]["typ"], "JWT");
            assert_eq!(json["payload"]["sub"], "1234567890");
            assert_eq!(json["payload"]["name"], "John Doe");
        } else {
            panic!("Expected Json");
        }
    }

    #[test]
    fn test_jwt_without_typ() {
        let format = JwtFormat;
        // JWT with just alg, no typ
        // {"alg":"HS256"}.{"sub":"test"}
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0In0.signature";
        let results = format.parse(jwt);

        assert_eq!(results.len(), 1);
        // Lower confidence without typ:JWT
        assert!(results[0].confidence < 0.95);
    }

    #[test]
    fn test_base64url_decode() {
        // Test URL-safe base64 decoding
        let decoded = JwtFormat::base64url_decode("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9").unwrap();
        let json: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(json["alg"], "HS256");
        assert_eq!(json["typ"], "JWT");
    }

    #[test]
    fn test_not_jwt() {
        let format = JwtFormat;

        // Not enough parts
        assert!(format.parse("just.two").is_empty());

        // Not valid base64
        assert!(format.parse("not.valid.jwt!!!").is_empty());

        // Valid base64 but not JSON
        assert!(format.parse("aGVsbG8.d29ybGQ.dGVzdA").is_empty());

        // JSON but no alg field
        assert!(format
            .parse("eyJ0eXAiOiJKV1QifQ.eyJzdWIiOiJ0ZXN0In0.sig")
            .is_empty());
    }

    #[test]
    fn test_jwt_with_expiry() {
        let format = JwtFormat;
        // JWT with exp claim (expired: 2018-01-18)
        // {"alg":"HS256","typ":"JWT"}.{"sub":"test","exp":1516239022}
        let jwt =
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwiZXhwIjoxNTE2MjM5MDIyfQ.sig";
        let results = format.parse(jwt);

        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("expired"));
    }

    #[test]
    fn test_description_includes_algorithm() {
        let format = JwtFormat;
        let results = format.parse(TEST_JWT);

        assert!(results[0].description.contains("HS256"));
    }
}
