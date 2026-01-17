//! MAC address format with OUI vendor lookup.

use super::mac_oui_data::lookup_vendor;
use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct MacAddressFormat;

/// Detected MAC address format notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacNotation {
    /// Colon-separated: `00:1A:2B:3C:4D:5E`
    Colon,
    /// Hyphen-separated: `00-1A-2B-3C-4D-5E`
    Hyphen,
    /// Cisco dot notation: `001A.2B3C.4D5E`
    Cisco,
    /// Space-separated: `00 1A 2B 3C 4D 5E`
    Space,
    /// Raw hex (no separator): `001A2B3C4D5E`
    Raw,
}

impl MacNotation {
    fn name(self) -> &'static str {
        match self {
            Self::Colon => "Colon-separated",
            Self::Hyphen => "Hyphen-separated",
            Self::Cisco => "Cisco notation",
            Self::Space => "Space-separated",
            Self::Raw => "Raw hex",
        }
    }

    fn confidence(self) -> f32 {
        match self {
            Self::Colon | Self::Hyphen | Self::Cisco => 0.95,
            Self::Space => 0.85,
            Self::Raw => 0.65,
        }
    }
}

/// Determine the address type based on the first byte.
fn address_type(bytes: &[u8; 6]) -> &'static str {
    // Check for broadcast address (all 1s)
    if bytes == &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
        return "Broadcast";
    }

    // Check for all zeros (invalid/unspecified)
    if bytes == &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00] {
        return "Unspecified";
    }

    let first_byte = bytes[0];

    // Bit 0 (LSB): 0 = unicast, 1 = multicast
    // Bit 1: 0 = globally unique (OUI), 1 = locally administered
    let is_multicast = first_byte & 0x01 != 0;
    let is_local = first_byte & 0x02 != 0;

    match (is_multicast, is_local) {
        (true, _) => "Multicast",
        (false, true) => "Locally administered",
        (false, false) => "Unicast",
    }
}

/// Parse a MAC address string into 6 bytes and detect the notation.
fn parse_mac(input: &str) -> Option<([u8; 6], MacNotation)> {
    let input = input.trim();

    // Try colon-separated: 00:1A:2B:3C:4D:5E
    if let Some(bytes) = parse_separated(input, ':') {
        return Some((bytes, MacNotation::Colon));
    }

    // Try hyphen-separated: 00-1A-2B-3C-4D-5E
    if let Some(bytes) = parse_separated(input, '-') {
        return Some((bytes, MacNotation::Hyphen));
    }

    // Try space-separated: 00 1A 2B 3C 4D 5E
    if let Some(bytes) = parse_separated(input, ' ') {
        return Some((bytes, MacNotation::Space));
    }

    // Try Cisco notation: 001A.2B3C.4D5E
    if let Some(bytes) = parse_cisco(input) {
        return Some((bytes, MacNotation::Cisco));
    }

    // Try raw hex: 001A2B3C4D5E
    if let Some(bytes) = parse_raw_hex(input) {
        return Some((bytes, MacNotation::Raw));
    }

    None
}

/// Parse MAC with a single-character separator (6 groups of 2 hex chars).
fn parse_separated(input: &str, sep: char) -> Option<[u8; 6]> {
    let parts: Vec<&str> = input.split(sep).collect();
    if parts.len() != 6 {
        return None;
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        if part.len() != 2 {
            return None;
        }
        bytes[i] = u8::from_str_radix(part, 16).ok()?;
    }

    Some(bytes)
}

/// Parse Cisco notation: 001A.2B3C.4D5E (3 groups of 4 hex chars).
fn parse_cisco(input: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        if part.len() != 4 {
            return None;
        }
        let val = u16::from_str_radix(part, 16).ok()?;
        bytes[i * 2] = (val >> 8) as u8;
        bytes[i * 2 + 1] = val as u8;
    }

    Some(bytes)
}

/// Parse raw hex without separators: 001A2B3C4D5E (12 hex chars).
fn parse_raw_hex(input: &str) -> Option<[u8; 6]> {
    if input.len() != 12 {
        return None;
    }

    // Check all characters are hex digits
    if !input.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let mut bytes = [0u8; 6];
    for i in 0..6 {
        bytes[i] = u8::from_str_radix(&input[i * 2..i * 2 + 2], 16).ok()?;
    }

    Some(bytes)
}

/// Format MAC address as colon-separated uppercase.
fn format_mac_colon(bytes: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

impl Format for MacAddressFormat {
    fn id(&self) -> &'static str {
        "mac-address"
    }

    fn name(&self) -> &'static str {
        "MAC Address"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Network",
            description: "MAC address with OUI vendor lookup",
            examples: &["00:1A:2B:3C:4D:5E", "00-1A-2B-3C-4D-5E", "001A.2B3C.4D5E"],
            aliases: self.aliases(),
            has_validation: true,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some((bytes, notation)) = parse_mac(input) else {
            return vec![];
        };

        let oui: [u8; 3] = [bytes[0], bytes[1], bytes[2]];
        let vendor = lookup_vendor(&oui);
        let addr_type = address_type(&bytes);
        let formatted = format_mac_colon(&bytes);

        // Build description
        let description = if let Some(v) = vendor {
            format!("{} ({})", formatted, v)
        } else {
            format!("{} (Unknown vendor)", formatted)
        };

        // Build rich display
        let mut pairs = vec![
            (
                "Vendor".to_string(),
                vendor.unwrap_or("Unknown").to_string(),
            ),
            (
                "OUI".to_string(),
                format!("{:02X}:{:02X}:{:02X}", oui[0], oui[1], oui[2]),
            ),
            ("Format".to_string(), notation.name().to_string()),
            ("Type".to_string(), addr_type.to_string()),
        ];

        // Add NIC-specific part
        pairs.push((
            "NIC".to_string(),
            format!("{:02X}:{:02X}:{:02X}", bytes[3], bytes[4], bytes[5]),
        ));

        vec![Interpretation {
            value: CoreValue::Bytes(bytes.to_vec()),
            source_format: "mac-address".to_string(),
            confidence: notation.confidence(),
            description,
            rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue { pairs })],
        }]
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        matches!(value, CoreValue::Bytes(bytes) if bytes.len() == 6)
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) if bytes.len() == 6 => {
                let arr: [u8; 6] = bytes.as_slice().try_into().ok()?;
                Some(format_mac_colon(&arr))
            }
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        if bytes.len() != 6 {
            return vec![];
        }

        let arr: [u8; 6] = match bytes.as_slice().try_into() {
            Ok(a) => a,
            Err(_) => return vec![],
        };

        let oui: [u8; 3] = [arr[0], arr[1], arr[2]];
        let vendor = lookup_vendor(&oui);
        let addr_type = address_type(&arr);
        let formatted = format_mac_colon(&arr);

        // Build description
        let description = if let Some(v) = vendor {
            format!("{} ({})", formatted, v)
        } else {
            format!("{} (Unknown vendor)", formatted)
        };

        // Build rich display
        let pairs = vec![
            (
                "Vendor".to_string(),
                vendor.unwrap_or("Unknown").to_string(),
            ),
            (
                "OUI".to_string(),
                format!("{:02X}:{:02X}:{:02X}", oui[0], oui[1], oui[2]),
            ),
            ("Type".to_string(), addr_type.to_string()),
            (
                "NIC".to_string(),
                format!("{:02X}:{:02X}:{:02X}", arr[3], arr[4], arr[5]),
            ),
        ];

        vec![Conversion {
            value: CoreValue::String(formatted.clone()),
            target_format: "mac-address".to_string(),
            display: description,
            path: vec!["mac-address".to_string()],
            is_lossy: false,
            steps: vec![],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Conversion,
            display_only: true, // Don't further convert MAC string
            hidden: false,
            rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue { pairs })],
        }]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["mac", "ethernet", "hw-address"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        if parse_mac(input).is_some() {
            return None;
        }

        let input = input.trim();

        // Provide helpful error messages
        if input.contains(':') {
            let parts: Vec<&str> = input.split(':').collect();
            if parts.len() != 6 {
                return Some(format!(
                    "Expected 6 colon-separated groups, found {}",
                    parts.len()
                ));
            }
            for (i, part) in parts.iter().enumerate() {
                if part.len() != 2 {
                    return Some(format!(
                        "Group {} should be 2 hex characters, found '{}'",
                        i + 1,
                        part
                    ));
                }
                if u8::from_str_radix(part, 16).is_err() {
                    return Some(format!("Invalid hex in group {}: '{}'", i + 1, part));
                }
            }
        }

        Some("Invalid MAC address format. Expected formats: 00:1A:2B:3C:4D:5E, 00-1A-2B-3C-4D-5E, 001A.2B3C.4D5E, or 001A2B3C4D5E".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_colon_separated() {
        let format = MacAddressFormat;
        let results = format.parse("00:1A:2B:3C:4D:5E");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mac-address");
        assert!((results[0].confidence - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_parse_hyphen_separated() {
        let format = MacAddressFormat;
        let results = format.parse("00-1A-2B-3C-4D-5E");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mac-address");
    }

    #[test]
    fn test_parse_cisco_notation() {
        let format = MacAddressFormat;
        let results = format.parse("001A.2B3C.4D5E");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "mac-address");

        // Verify the bytes are correct
        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);
        } else {
            panic!("Expected Bytes value");
        }
    }

    #[test]
    fn test_parse_space_separated() {
        let format = MacAddressFormat;
        let results = format.parse("00 1A 2B 3C 4D 5E");

        assert_eq!(results.len(), 1);
        assert!((results[0].confidence - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_parse_raw_hex() {
        let format = MacAddressFormat;
        let results = format.parse("001A2B3C4D5E");

        assert_eq!(results.len(), 1);
        assert!((results[0].confidence - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_parse_lowercase() {
        let format = MacAddressFormat;
        let results = format.parse("00:1a:2b:3c:4d:5e");

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_broadcast_address() {
        let format = MacAddressFormat;
        let results = format.parse("FF:FF:FF:FF:FF:FF");

        assert_eq!(results.len(), 1);
        assert!(
            results[0].description.contains("Broadcast") || {
                // Check rich display
                if let Some(opt) = results[0].rich_display.first() {
                    if let RichDisplay::KeyValue { pairs } = &opt.preferred {
                        pairs.iter().any(|(k, v)| k == "Type" && v == "Broadcast")
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        );
    }

    #[test]
    fn test_multicast_address() {
        let bytes = [0x01, 0x00, 0x5E, 0x00, 0x00, 0x01]; // IPv4 multicast
        assert_eq!(address_type(&bytes), "Multicast");
    }

    #[test]
    fn test_locally_administered() {
        let bytes = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]; // Locally administered bit set
        assert_eq!(address_type(&bytes), "Locally administered");
    }

    #[test]
    fn test_unicast_address() {
        let bytes = [0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E];
        assert_eq!(address_type(&bytes), "Unicast");
    }

    #[test]
    fn test_vendor_lookup() {
        // Cisco OUI: 00:00:0C
        let oui = [0x00, 0x00, 0x0C];
        let vendor = lookup_vendor(&oui);
        assert!(vendor.is_some());
        assert!(vendor.unwrap().contains("Cisco"));
    }

    #[test]
    fn test_invalid_mac() {
        let format = MacAddressFormat;

        // Too short
        assert!(format.parse("00:1A:2B").is_empty());

        // Too long
        assert!(format.parse("00:1A:2B:3C:4D:5E:FF").is_empty());

        // Invalid hex
        assert!(format.parse("00:GG:2B:3C:4D:5E").is_empty());

        // Wrong format
        assert!(format.parse("not a mac").is_empty());
    }

    #[test]
    fn test_format_bytes_to_mac() {
        let format = MacAddressFormat;
        let value = CoreValue::Bytes(vec![0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);

        let formatted = format.format(&value);
        assert_eq!(formatted, Some("00:1A:2B:3C:4D:5E".to_string()));
    }

    #[test]
    fn test_conversions_from_bytes() {
        let format = MacAddressFormat;
        let value = CoreValue::Bytes(vec![0x00, 0x00, 0x0C, 0x12, 0x34, 0x56]);

        let convs = format.conversions(&value);
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].target_format, "mac-address");
        assert!(convs[0].display.contains("Cisco"));
    }

    #[test]
    fn test_oui_database_size() {
        use crate::formats::mac_oui_data::OUI_COUNT;

        // Verify database has expected number of entries
        assert!(OUI_COUNT > 30000, "OUI database should have 30K+ entries");
        assert!(
            OUI_COUNT < 100000,
            "OUI database shouldn't exceed 100K entries"
        );
    }

    #[test]
    fn test_lookup_performance() {
        // Test that lookup is fast (binary search on sorted array)
        let test_ouis: &[[u8; 3]] = &[
            [0x00, 0x00, 0x0C], // Cisco (early in list)
            [0xAC, 0xDE, 0x48], // Apple (middle-ish)
            [0xFF, 0xFF, 0xFF], // End of range (not found)
            [0x00, 0x00, 0x00], // Start of range (Xerox)
        ];

        // Do many lookups to ensure performance is acceptable
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            for oui in test_ouis {
                let _ = lookup_vendor(oui);
            }
        }
        let elapsed = start.elapsed();

        // 40K lookups should complete in well under 100ms
        assert!(
            elapsed.as_millis() < 100,
            "OUI lookup too slow: {:?} for 40K lookups",
            elapsed
        );
    }

    #[test]
    fn test_no_heap_allocation_for_static_data() {
        // The OUI_DATA array is static - verify a lookup doesn't allocate
        // by ensuring the returned string is a &'static str
        let oui = [0x00, 0x00, 0x0C];
        let vendor: Option<&'static str> = lookup_vendor(&oui);
        assert!(vendor.is_some());

        // If this compiles, the data is truly static (no heap)
        let _static_ref: &'static str = vendor.unwrap();
    }
}
