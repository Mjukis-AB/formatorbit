//! CIDR notation format: parse network ranges and show detailed info.
//!
//! Input: `192.168.1.0/24`
//! Output: Network address, netmask, broadcast, host range, host count

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, ConversionStep, CoreValue, Interpretation,
    RichDisplay, RichDisplayOption,
};

// =============================================================================
// CIDR Parsing and Calculation
// =============================================================================

/// Parsed CIDR notation for IPv4.
#[derive(Debug, Clone)]
struct CidrV4 {
    /// Network address
    network: Ipv4Addr,
    /// Prefix length (0-32)
    prefix: u8,
}

impl CidrV4 {
    /// Parse CIDR notation string.
    fn parse(input: &str) -> Option<Self> {
        let parts: Vec<&str> = input.split('/').collect();
        if parts.len() != 2 {
            return None;
        }

        let addr: Ipv4Addr = parts[0].parse().ok()?;
        let prefix: u8 = parts[1].parse().ok()?;

        if prefix > 32 {
            return None;
        }

        // Calculate the actual network address (mask off host bits)
        let network = Self::apply_mask(addr, prefix);

        Some(Self { network, prefix })
    }

    /// Apply netmask to get network address.
    fn apply_mask(addr: Ipv4Addr, prefix: u8) -> Ipv4Addr {
        let mask = Self::prefix_to_mask(prefix);
        let addr_u32 = u32::from(addr);
        Ipv4Addr::from(addr_u32 & mask)
    }

    /// Convert prefix length to netmask.
    fn prefix_to_mask(prefix: u8) -> u32 {
        if prefix == 0 {
            0
        } else {
            !0u32 << (32 - prefix)
        }
    }

    /// Get the netmask as an IP address.
    fn netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(Self::prefix_to_mask(self.prefix))
    }

    /// Get the wildcard mask (inverse of netmask).
    fn wildcard(&self) -> Ipv4Addr {
        Ipv4Addr::from(!Self::prefix_to_mask(self.prefix))
    }

    /// Get the broadcast address.
    fn broadcast(&self) -> Ipv4Addr {
        let network_u32 = u32::from(self.network);
        let wildcard_u32 = u32::from(self.wildcard());
        Ipv4Addr::from(network_u32 | wildcard_u32)
    }

    /// Get the first usable host address.
    fn first_host(&self) -> Option<Ipv4Addr> {
        match self.prefix {
            32 => Some(self.network), // Single host
            31 => Some(self.network), // Point-to-point (both usable)
            _ => {
                let network_u32 = u32::from(self.network);
                Some(Ipv4Addr::from(network_u32 + 1))
            }
        }
    }

    /// Get the last usable host address.
    fn last_host(&self) -> Option<Ipv4Addr> {
        match self.prefix {
            32 => Some(self.network),     // Single host
            31 => Some(self.broadcast()), // Point-to-point (both usable)
            _ => {
                let broadcast_u32 = u32::from(self.broadcast());
                Some(Ipv4Addr::from(broadcast_u32 - 1))
            }
        }
    }

    /// Get the number of usable hosts.
    fn host_count(&self) -> u64 {
        match self.prefix {
            32 => 1, // Single host
            31 => 2, // Point-to-point
            _ => {
                let total = 1u64 << (32 - self.prefix);
                total.saturating_sub(2) // Subtract network and broadcast
            }
        }
    }

    /// Get the total number of addresses in the range.
    fn total_addresses(&self) -> u64 {
        1u64 << (32 - self.prefix)
    }

    /// Check if this is a private network.
    fn is_private(&self) -> bool {
        let octets = self.network.octets();
        // 10.0.0.0/8
        if octets[0] == 10 {
            return true;
        }
        // 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return true;
        }
        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return true;
        }
        false
    }

    /// Get network class (for informational purposes).
    fn network_class(&self) -> &'static str {
        let first = self.network.octets()[0];
        match first {
            0..=127 => "Class A",
            128..=191 => "Class B",
            192..=223 => "Class C",
            224..=239 => "Class D (Multicast)",
            240..=255 => "Class E (Reserved)",
        }
    }
}

/// Parsed CIDR notation for IPv6.
#[derive(Debug, Clone)]
struct CidrV6 {
    /// Network address
    network: Ipv6Addr,
    /// Prefix length (0-128)
    prefix: u8,
}

impl CidrV6 {
    /// Parse CIDR notation string.
    fn parse(input: &str) -> Option<Self> {
        let parts: Vec<&str> = input.split('/').collect();
        if parts.len() != 2 {
            return None;
        }

        let addr: Ipv6Addr = parts[0].parse().ok()?;
        let prefix: u8 = parts[1].parse().ok()?;

        if prefix > 128 {
            return None;
        }

        // Calculate the actual network address
        let network = Self::apply_mask(addr, prefix);

        Some(Self { network, prefix })
    }

    /// Apply netmask to get network address.
    fn apply_mask(addr: Ipv6Addr, prefix: u8) -> Ipv6Addr {
        let segments = addr.segments();
        let mut result = [0u16; 8];

        for (i, segment) in segments.iter().enumerate() {
            let bit_start = i * 16;
            let bit_end = bit_start + 16;

            if prefix as usize >= bit_end {
                // Entire segment is in network part
                result[i] = *segment;
            } else if prefix as usize <= bit_start {
                // Entire segment is in host part
                result[i] = 0;
            } else {
                // Segment spans the boundary
                let bits_to_keep = prefix as usize - bit_start;
                let mask = !0u16 << (16 - bits_to_keep);
                result[i] = segment & mask;
            }
        }

        Ipv6Addr::from(result)
    }

    /// Get total addresses as a string (may be very large).
    fn total_addresses_str(&self) -> String {
        if self.prefix >= 64 {
            let count = 1u128 << (128 - self.prefix);
            format!("{}", count)
        } else {
            // Too large to represent, use 2^n notation
            format!("2^{}", 128 - self.prefix)
        }
    }
}

/// Build rich display for IPv4 CIDR info.
fn build_rich_display_v4(cidr: &CidrV4) -> RichDisplayOption {
    let mut pairs = vec![
        (
            "network".to_string(),
            format!("{}/{}", cidr.network, cidr.prefix),
        ),
        ("netmask".to_string(), cidr.netmask().to_string()),
        ("wildcard".to_string(), cidr.wildcard().to_string()),
        ("broadcast".to_string(), cidr.broadcast().to_string()),
    ];

    if let (Some(first), Some(last)) = (cidr.first_host(), cidr.last_host()) {
        pairs.push(("first host".to_string(), first.to_string()));
        pairs.push(("last host".to_string(), last.to_string()));
    }

    pairs.push((
        "usable hosts".to_string(),
        format_host_count(cidr.host_count()),
    ));
    pairs.push((
        "total addresses".to_string(),
        format_host_count(cidr.total_addresses()),
    ));

    if cidr.is_private() {
        pairs.push(("scope".to_string(), "Private".to_string()));
    }

    RichDisplayOption::new(RichDisplay::KeyValue { pairs })
}

/// Build rich display for IPv6 CIDR info.
fn build_rich_display_v6(cidr: &CidrV6) -> RichDisplayOption {
    let pairs = vec![
        (
            "network".to_string(),
            format!("{}/{}", cidr.network, cidr.prefix),
        ),
        ("total addresses".to_string(), cidr.total_addresses_str()),
    ];

    RichDisplayOption::new(RichDisplay::KeyValue { pairs })
}

/// Format host count with commas for readability.
fn format_host_count(count: u64) -> String {
    if count >= 1_000_000_000 {
        format!("{:.2}B", count as f64 / 1_000_000_000.0)
    } else if count >= 1_000_000 {
        format!("{:.2}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

// =============================================================================
// Format Implementation
// =============================================================================

pub struct CidrFormat;

impl Format for CidrFormat {
    fn id(&self) -> &'static str {
        "cidr"
    }

    fn name(&self) -> &'static str {
        "CIDR Notation"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Network",
            description: "CIDR network ranges with netmask, broadcast, host count",
            examples: &["192.168.1.0/24", "10.0.0.0/8", "172.16.0.0/12"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let trimmed = input.trim();

        // Try IPv4 CIDR
        if let Some(cidr) = CidrV4::parse(trimmed) {
            let host_count = cidr.host_count();
            let description = format!(
                "{}/{} ({} usable hosts)",
                cidr.network,
                cidr.prefix,
                format_host_count(host_count)
            );

            return vec![Interpretation {
                value: CoreValue::String(format!("{}/{}", cidr.network, cidr.prefix)),
                source_format: "cidr".to_string(),
                confidence: 0.95,
                description,
                rich_display: vec![build_rich_display_v4(&cidr)],
            }];
        }

        // Try IPv6 CIDR
        if let Some(cidr) = CidrV6::parse(trimmed) {
            let description = format!(
                "{}/{} ({} addresses)",
                cidr.network,
                cidr.prefix,
                cidr.total_addresses_str()
            );

            return vec![Interpretation {
                value: CoreValue::String(format!("{}/{}", cidr.network, cidr.prefix)),
                source_format: "cidr".to_string(),
                confidence: 0.95,
                description,
                rich_display: vec![build_rich_display_v6(&cidr)],
            }];
        }

        vec![]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::String(cidr_str) = value else {
            return vec![];
        };

        // Only process if it looks like a CIDR
        if !cidr_str.contains('/') {
            return vec![];
        }

        // Try IPv4
        if let Some(cidr) = CidrV4::parse(cidr_str) {
            return build_conversions_v4(&cidr);
        }

        // Try IPv6
        if let Some(cidr) = CidrV6::parse(cidr_str) {
            return build_conversions_v6(&cidr);
        }

        vec![]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["subnet", "network"]
    }
}

/// Build conversions for IPv4 CIDR.
fn build_conversions_v4(cidr: &CidrV4) -> Vec<Conversion> {
    let mut conversions = Vec::new();

    // Netmask
    let netmask = cidr.netmask().to_string();
    conversions.push(Conversion {
        value: CoreValue::String(netmask.clone()),
        target_format: "netmask".to_string(),
        display: netmask.clone(),
        path: vec!["netmask".to_string()],
        steps: vec![ConversionStep {
            format: "netmask".to_string(),
            value: CoreValue::String(netmask.clone()),
            display: netmask,
        }],
        priority: ConversionPriority::Semantic,
        kind: ConversionKind::Representation,
        display_only: true,
        ..Default::default()
    });

    // Broadcast
    let broadcast = cidr.broadcast().to_string();
    conversions.push(Conversion {
        value: CoreValue::String(broadcast.clone()),
        target_format: "broadcast".to_string(),
        display: broadcast.clone(),
        path: vec!["broadcast".to_string()],
        steps: vec![ConversionStep {
            format: "broadcast".to_string(),
            value: CoreValue::String(broadcast.clone()),
            display: broadcast,
        }],
        priority: ConversionPriority::Semantic,
        kind: ConversionKind::Representation,
        display_only: true,
        ..Default::default()
    });

    // Host range
    if let (Some(first), Some(last)) = (cidr.first_host(), cidr.last_host()) {
        let range = format!("{} - {}", first, last);
        conversions.push(Conversion {
            value: CoreValue::String(range.clone()),
            target_format: "host-range".to_string(),
            display: range.clone(),
            path: vec!["host-range".to_string()],
            steps: vec![ConversionStep {
                format: "host-range".to_string(),
                value: CoreValue::String(range.clone()),
                display: range,
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Representation,
            display_only: true,
            ..Default::default()
        });
    }

    // Host count (as trait)
    let host_count = format!("{} usable hosts", format_host_count(cidr.host_count()));
    conversions.push(Conversion {
        value: CoreValue::String(host_count.clone()),
        target_format: "host-count".to_string(),
        display: host_count.clone(),
        path: vec!["host-count".to_string()],
        steps: vec![ConversionStep {
            format: "host-count".to_string(),
            value: CoreValue::String(host_count.clone()),
            display: host_count,
        }],
        priority: ConversionPriority::Semantic,
        kind: ConversionKind::Trait,
        display_only: true,
        ..Default::default()
    });

    // Wildcard mask
    let wildcard = cidr.wildcard().to_string();
    conversions.push(Conversion {
        value: CoreValue::String(wildcard.clone()),
        target_format: "wildcard".to_string(),
        display: wildcard.clone(),
        path: vec!["wildcard".to_string()],
        steps: vec![ConversionStep {
            format: "wildcard".to_string(),
            value: CoreValue::String(wildcard.clone()),
            display: wildcard,
        }],
        priority: ConversionPriority::Semantic,
        kind: ConversionKind::Representation,
        display_only: true,
        ..Default::default()
    });

    // Network class (as trait)
    let class = cidr.network_class();
    conversions.push(Conversion {
        value: CoreValue::String(class.to_string()),
        target_format: "network-class".to_string(),
        display: class.to_string(),
        path: vec!["network-class".to_string()],
        steps: vec![ConversionStep {
            format: "network-class".to_string(),
            value: CoreValue::String(class.to_string()),
            display: class.to_string(),
        }],
        priority: ConversionPriority::Semantic,
        kind: ConversionKind::Trait,
        display_only: true,
        ..Default::default()
    });

    // Private network trait
    if cidr.is_private() {
        conversions.push(Conversion {
            value: CoreValue::String("Private network".to_string()),
            target_format: "private-network".to_string(),
            display: "Private network (RFC 1918)".to_string(),
            path: vec!["private-network".to_string()],
            steps: vec![ConversionStep {
                format: "private-network".to_string(),
                value: CoreValue::String("Private network".to_string()),
                display: "Private network (RFC 1918)".to_string(),
            }],
            priority: ConversionPriority::Semantic,
            kind: ConversionKind::Trait,
            display_only: true,
            ..Default::default()
        });
    }

    conversions
}

/// Build conversions for IPv6 CIDR.
fn build_conversions_v6(cidr: &CidrV6) -> Vec<Conversion> {
    let mut conversions = Vec::new();

    // Total addresses
    let total = format!("{} addresses", cidr.total_addresses_str());
    conversions.push(Conversion {
        value: CoreValue::String(total.clone()),
        target_format: "address-count".to_string(),
        display: total.clone(),
        path: vec!["address-count".to_string()],
        steps: vec![ConversionStep {
            format: "address-count".to_string(),
            value: CoreValue::String(total.clone()),
            display: total,
        }],
        priority: ConversionPriority::Semantic,
        kind: ConversionKind::Trait,
        display_only: true,
        ..Default::default()
    });

    conversions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cidr_v4() {
        let format = CidrFormat;
        let results = format.parse("192.168.1.0/24");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("254 usable hosts"));
    }

    #[test]
    fn test_parse_cidr_v4_normalizes_network() {
        // Input 192.168.1.100/24 should normalize to 192.168.1.0/24
        let cidr = CidrV4::parse("192.168.1.100/24").unwrap();
        assert_eq!(cidr.network, Ipv4Addr::new(192, 168, 1, 0));
    }

    #[test]
    fn test_cidr_v4_calculations() {
        let cidr = CidrV4::parse("192.168.1.0/24").unwrap();
        assert_eq!(cidr.netmask(), Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(cidr.wildcard(), Ipv4Addr::new(0, 0, 0, 255));
        assert_eq!(cidr.broadcast(), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(cidr.first_host(), Some(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(cidr.last_host(), Some(Ipv4Addr::new(192, 168, 1, 254)));
        assert_eq!(cidr.host_count(), 254);
    }

    #[test]
    fn test_cidr_v4_slash_32() {
        let cidr = CidrV4::parse("192.168.1.1/32").unwrap();
        assert_eq!(cidr.host_count(), 1);
        assert_eq!(cidr.first_host(), Some(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(cidr.last_host(), Some(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn test_cidr_v4_slash_31() {
        let cidr = CidrV4::parse("192.168.1.0/31").unwrap();
        assert_eq!(cidr.host_count(), 2);
    }

    #[test]
    fn test_cidr_v4_slash_8() {
        let cidr = CidrV4::parse("10.0.0.0/8").unwrap();
        assert_eq!(cidr.netmask(), Ipv4Addr::new(255, 0, 0, 0));
        assert_eq!(cidr.host_count(), 16_777_214);
        assert!(cidr.is_private());
    }

    #[test]
    fn test_cidr_v4_private() {
        assert!(CidrV4::parse("10.0.0.0/8").unwrap().is_private());
        assert!(CidrV4::parse("172.16.0.0/12").unwrap().is_private());
        assert!(CidrV4::parse("192.168.0.0/16").unwrap().is_private());
        assert!(!CidrV4::parse("8.8.8.0/24").unwrap().is_private());
    }

    #[test]
    fn test_parse_cidr_v6() {
        let format = CidrFormat;
        let results = format.parse("2001:db8::/32");
        assert_eq!(results.len(), 1);
        assert!(results[0].description.contains("2^96 addresses"));
    }

    #[test]
    fn test_conversions_v4() {
        let format = CidrFormat;
        let value = CoreValue::String("192.168.1.0/24".to_string());
        let conversions = format.conversions(&value);

        assert!(conversions.iter().any(|c| c.target_format == "netmask"));
        assert!(conversions.iter().any(|c| c.target_format == "broadcast"));
        assert!(conversions.iter().any(|c| c.target_format == "host-range"));
        assert!(conversions.iter().any(|c| c.target_format == "host-count"));
    }

    #[test]
    fn test_no_match() {
        let format = CidrFormat;
        assert!(format.parse("192.168.1.1").is_empty()); // No prefix
        assert!(format.parse("192.168.1.0/33").is_empty()); // Invalid prefix
        assert!(format.parse("hello/24").is_empty()); // Invalid IP
    }
}
