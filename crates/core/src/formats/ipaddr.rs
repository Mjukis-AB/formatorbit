//! IP address format.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::format::{Format, FormatInfo};
use crate::types::{Conversion, ConversionPriority, CoreValue, Interpretation};

pub struct IpAddrFormat;

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
            results.push(Interpretation {
                value: CoreValue::Bytes(addr.octets().to_vec()),
                source_format: "ipv4".to_string(),
                confidence: 0.9,
                description: format!("IPv4: {addr}"),
            });
        }

        // Try IPv6
        if let Ok(addr) = input.parse::<Ipv6Addr>() {
            results.push(Interpretation {
                value: CoreValue::Bytes(addr.octets().to_vec()),
                source_format: "ipv6".to_string(),
                confidence: 0.9,
                description: format!("IPv6: {addr}"),
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
                    metadata: None,
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
                    metadata: None,
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
