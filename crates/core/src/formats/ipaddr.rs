//! IP address format.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::format::{Format, FormatInfo};
use crate::types::{
    Conversion, ConversionKind, ConversionPriority, CoreValue, Interpretation, RichDisplay,
    RichDisplayOption,
};

pub struct IpAddrFormat;

impl IpAddrFormat {
    /// Get classification info for an IPv4 address.
    fn ipv4_info(addr: Ipv4Addr) -> Vec<(String, String)> {
        let octets = addr.octets();
        let mut info = vec![];

        // Address class (classful networking)
        let class = if octets[0] < 128 {
            "A"
        } else if octets[0] < 192 {
            "B"
        } else if octets[0] < 224 {
            "C"
        } else if octets[0] < 240 {
            "D (multicast)"
        } else {
            "E (reserved)"
        };
        info.push(("Class".to_string(), class.to_string()));

        // Scope/type
        let scope = if addr.is_loopback() {
            "Loopback"
        } else if addr.is_private() {
            "Private"
        } else if addr.is_link_local() {
            "Link-local"
        } else if addr.is_broadcast() {
            "Broadcast"
        } else if addr.is_multicast() {
            "Multicast"
        } else if addr.is_unspecified() {
            "Unspecified"
        } else if addr.is_documentation() {
            "Documentation"
        } else {
            "Public"
        };
        info.push(("Scope".to_string(), scope.to_string()));

        // Binary representation
        let binary = format!(
            "{:08b}.{:08b}.{:08b}.{:08b}",
            octets[0], octets[1], octets[2], octets[3]
        );
        info.push(("Binary".to_string(), binary));

        // Integer representation
        let int_val = u32::from(addr);
        info.push(("Integer".to_string(), int_val.to_string()));

        // Hex representation
        info.push(("Hex".to_string(), format!("0x{:08X}", int_val)));

        info
    }

    /// Get classification info for an IPv6 address.
    fn ipv6_info(addr: Ipv6Addr) -> Vec<(String, String)> {
        let mut info = vec![];

        // Scope/type
        let scope = if addr.is_loopback() {
            "Loopback"
        } else if addr.is_multicast() {
            "Multicast"
        } else if addr.is_unspecified() {
            "Unspecified"
        } else {
            // Check for common prefixes
            let segments = addr.segments();
            if segments[0] == 0xfe80 {
                "Link-local"
            } else if segments[0] & 0xfe00 == 0xfc00 {
                // fc00::/7 - Unique local addresses (includes fc00 and fd00)
                "Unique local"
            } else if segments[0] == 0x2001 && segments[1] == 0xdb8 {
                "Documentation"
            } else if segments[0] & 0xff00 == 0x2000 {
                "Global unicast"
            } else {
                "Other"
            }
        };
        info.push(("Scope".to_string(), scope.to_string()));

        // Full expanded form
        let segments = addr.segments();
        let expanded = format!(
            "{:04x}:{:04x}:{:04x}:{:04x}:{:04x}:{:04x}:{:04x}:{:04x}",
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7]
        );
        info.push(("Expanded".to_string(), expanded));

        // Check if it's an IPv4-mapped address
        if let Some(ipv4) = addr.to_ipv4_mapped() {
            info.push(("IPv4-mapped".to_string(), ipv4.to_string()));
        }

        info
    }
}

impl Format for IpAddrFormat {
    fn id(&self) -> &'static str {
        "ip"
    }

    fn name(&self) -> &'static str {
        "IP Address"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Network",
            description: "IPv4 and IPv6 address parsing",
            examples: &["192.168.1.1", "::1", "2001:db8::1"],
            aliases: self.aliases(),
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let mut results = vec![];

        // Try IPv4
        if let Ok(addr) = input.parse::<Ipv4Addr>() {
            let info = Self::ipv4_info(addr);
            let scope = info
                .iter()
                .find(|(k, _)| k == "Scope")
                .map(|(_, v)| v.as_str())
                .unwrap_or("Unknown");

            results.push(Interpretation {
                value: CoreValue::Bytes(addr.octets().to_vec()),
                source_format: "ipv4".to_string(),
                confidence: 0.9,
                description: format!("IPv4: {addr} ({scope})"),
                rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue {
                    pairs: info,
                })],
            });
        }

        // Try IPv6
        if let Ok(addr) = input.parse::<Ipv6Addr>() {
            let info = Self::ipv6_info(addr);
            let scope = info
                .iter()
                .find(|(k, _)| k == "Scope")
                .map(|(_, v)| v.as_str())
                .unwrap_or("Unknown");

            results.push(Interpretation {
                value: CoreValue::Bytes(addr.octets().to_vec()),
                source_format: "ipv6".to_string(),
                confidence: 0.9,
                description: format!("IPv6: {addr} ({scope})"),
                rich_display: vec![RichDisplayOption::new(RichDisplay::KeyValue {
                    pairs: info,
                })],
            });
        }

        results
    }

    fn can_format(&self, value: &CoreValue) -> bool {
        match value {
            CoreValue::Bytes(bytes) => bytes.len() == 4 || bytes.len() == 16,
            _ => false,
        }
    }

    fn format(&self, value: &CoreValue) -> Option<String> {
        match value {
            CoreValue::Bytes(bytes) if bytes.len() == 4 => {
                let addr = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
                Some(addr.to_string())
            }
            CoreValue::Bytes(bytes) if bytes.len() == 16 => {
                let arr: [u8; 16] = bytes.as_slice().try_into().ok()?;
                let addr = Ipv6Addr::from(arr);
                Some(addr.to_string())
            }
            _ => None,
        }
    }

    fn conversions(&self, value: &CoreValue) -> Vec<Conversion> {
        let CoreValue::Bytes(bytes) = value else {
            return vec![];
        };

        match bytes.len() {
            4 => {
                let addr = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
                vec![Conversion {
                    value: CoreValue::String(addr.to_string()),
                    target_format: "ipv4".to_string(),
                    display: addr.to_string(),
                    path: vec!["ipv4".to_string()],
                    is_lossy: false,
                    steps: vec![],
                    priority: ConversionPriority::Semantic,
                    display_only: false,
                    kind: ConversionKind::default(),
                    hidden: false,
                    rich_display: vec![],
                }]
            }
            16 => {
                let arr: [u8; 16] = match bytes.as_slice().try_into() {
                    Ok(a) => a,
                    Err(_) => return vec![],
                };
                let addr = Ipv6Addr::from(arr);

                let conversions = vec![Conversion {
                    value: CoreValue::String(addr.to_string()),
                    target_format: "ipv6".to_string(),
                    display: addr.to_string(),
                    path: vec!["ipv6".to_string()],
                    is_lossy: false,
                    steps: vec![],
                    priority: ConversionPriority::Semantic,
                    display_only: false,
                    kind: ConversionKind::default(),
                    hidden: false,
                    rich_display: vec![],
                }];

                // Also try as UUID since both are 16 bytes
                // (UUID format will handle this separately)

                conversions
            }
            _ => vec![],
        }
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ipv4", "ipv6"]
    }

    fn validate(&self, input: &str) -> Option<String> {
        // Try IPv4
        if let Err(e) = input.parse::<Ipv4Addr>() {
            // Try IPv6
            if let Err(e6) = input.parse::<Ipv6Addr>() {
                // Return the more informative error
                if input.contains(':') {
                    return Some(format!("invalid IPv6 address: {}", e6));
                } else {
                    return Some(format!("invalid IPv4 address: {}", e));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4() {
        let format = IpAddrFormat;
        let results = format.parse("192.168.1.1");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "ipv4");

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes, &[192, 168, 1, 1]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_parse_ipv6() {
        let format = IpAddrFormat;
        let results = format.parse("::1");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_format, "ipv6");

        if let CoreValue::Bytes(bytes) = &results[0].value {
            assert_eq!(bytes.len(), 16);
            assert_eq!(bytes[15], 1); // Last byte is 1 for ::1
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_format_ipv4() {
        let format = IpAddrFormat;
        let value = CoreValue::Bytes(vec![8, 8, 8, 8]);
        assert_eq!(format.format(&value), Some("8.8.8.8".to_string()));
    }

    #[test]
    fn test_bytes_to_ipv4_conversion() {
        let format = IpAddrFormat;
        let value = CoreValue::Bytes(vec![10, 0, 0, 1]);
        let conversions = format.conversions(&value);

        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].target_format, "ipv4");
        assert_eq!(conversions[0].display, "10.0.0.1");
    }

    #[test]
    fn test_invalid_ip() {
        let format = IpAddrFormat;
        assert!(format.parse("not.an.ip").is_empty());
        assert!(format.parse("256.1.1.1").is_empty());
    }
}
