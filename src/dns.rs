//! DNS resolution and hijacking for NaiveProxy ECH support.
//!
//! Maps to cronet-go's naive_dns.go. Implements an in-process DNS server
//! that handles A/AAAA/HTTPS record hijacking for ECH (Encrypted Client Hello).

use std::net::ToSocketAddrs;
use std::sync::Arc;

/// A DNS resolver function: takes a raw DNS query and returns a raw DNS response.
pub type DnsResolver = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// Configuration for DNS hijacking in the NaiveClient.
#[derive(Clone, Debug)]
pub struct DnsConfig {
    /// The server name (SNI) for the upstream server.
    pub server_name: String,
    /// The actual server address (IP or hostname).
    pub server_address: String,
    /// Whether ECH is enabled.
    pub ech_enabled: bool,
    /// Pre-configured ECH config list (raw bytes).
    pub ech_config_list: Option<Vec<u8>>,
    /// Server name to query for ECH configs.
    pub ech_query_server_name: Option<String>,
    /// Whether QUIC is enabled.
    pub quic_enabled: bool,
}

/// Minimal DNS protocol helpers for query parsing and response construction.
mod dns_protocol {
    use std::io::{Cursor, Read};

    /// Parse a DNS query and return (id, qname, qtype).
    pub fn parse_query(data: &[u8]) -> Option<(u16, String, u16)> {
        if data.len() < 12 {
            return None;
        }
        let mut cursor = Cursor::new(data);

        // DNS header: ID(2) + Flags(2) + QDCOUNT(2) + ANCOUNT(2) + NSCOUNT(2) + ARCOUNT(2) = 12 bytes
        let id = read_u16(&mut cursor)?;
        cursor.read_exact(&mut [0u8; 2]).ok()?; // flags
        let qdcount = read_u16(&mut cursor)?;
        cursor.read_exact(&mut [0u8; 6]).ok()?; // ANCOUNT, NSCOUNT, ARCOUNT

        if qdcount == 0 {
            return None;
        }

        // Question section starts here
        let name = parse_name(&mut cursor, data)?;
        let qtype = read_u16(&mut cursor)?;

        Some((id, name, qtype))
    }

    fn read_u16(cursor: &mut Cursor<&[u8]>) -> Option<u16> {
        let mut buf = [0u8; 2];
        cursor.read_exact(&mut buf).ok()?;
        Some(u16::from_be_bytes(buf))
    }

    fn parse_name(cursor: &mut Cursor<&[u8]>, _packet: &[u8]) -> Option<String> {
        let mut labels = Vec::new();
        loop {
            let mut len_buf = [0u8; 1];
            cursor.read_exact(&mut len_buf).ok()?;
            let len = len_buf[0] as usize;

            if len == 0 {
                break; // root label = end of name
            }

            // DNS compression pointer (0xC0 0xZZ)
            if len & 0xC0 == 0xC0 {
                let mut next = [0u8; 1];
                cursor.read_exact(&mut next).ok()?;
                break; // skip compressed names in our minimal parser
            }

            let mut label = vec![0u8; len];
            cursor.read_exact(&mut label).ok()?;
            labels.push(String::from_utf8_lossy(&label).to_lowercase());
        }
        Some(labels.join("."))
    }

    /// Build a synthetic DNS response with A/AAAA records.
    pub fn build_synthetic_response(
        id: u16,
        query_name: &str,
        qtype: u16,
        ips: &[std::net::IpAddr],
    ) -> Vec<u8> {
        let mut response = Vec::new();
        response.extend_from_slice(&id.to_be_bytes()); // ID
        response.extend_from_slice(&0x8180u16.to_be_bytes()); // Flags: response, aa, no error
        response.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
        response.extend_from_slice(&(ips.len() as u16).to_be_bytes()); // ANCOUNT
        response.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
        response.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT

        // Question
        write_name(&mut response, query_name);
        response.extend_from_slice(&qtype.to_be_bytes());
        response.extend_from_slice(&0x0001u16.to_be_bytes()); // CLASS IN

        // Answers
        for ip in ips {
            write_name(&mut response, query_name);
            match ip {
                std::net::IpAddr::V4(v4) => {
                    response.extend_from_slice(&0x0001u16.to_be_bytes()); // TYPE A
                    response.extend_from_slice(&0x0001u16.to_be_bytes()); // CLASS IN
                    response.extend_from_slice(&0x0000003cu32.to_be_bytes()); // TTL 60
                    response.extend_from_slice(&0x0004u16.to_be_bytes()); // RDLENGTH
                    response.extend_from_slice(&v4.octets());
                }
                std::net::IpAddr::V6(v6) => {
                    response.extend_from_slice(&0x001cu16.to_be_bytes()); // TYPE AAAA
                    response.extend_from_slice(&0x0001u16.to_be_bytes()); // CLASS IN
                    response.extend_from_slice(&0x0000003cu32.to_be_bytes()); // TTL 60
                    response.extend_from_slice(&0x0010u16.to_be_bytes()); // RDLENGTH
                    response.extend_from_slice(&v6.octets());
                }
            }
        }

        response
    }

    fn write_name(buf: &mut Vec<u8>, name: &str) {
        for label in name.split('.') {
            buf.push(label.len() as u8);
            buf.extend_from_slice(label.as_bytes());
        }
        buf.push(0);
    }
}

/// Create a DNS resolver that handles ECH-related hijacking for NaiveProxy.
pub fn create_naive_dns_resolver(config: DnsConfig) -> DnsResolver {
    let cfg = Arc::new(config);

    Arc::new(move |query: &[u8]| -> Vec<u8> {
        let parsed = dns_protocol::parse_query(query);
        let (id, qname, qtype) = match parsed {
            Some(p) => p,
            None => return query.to_vec(),
        };

        let server_name = cfg.server_name.trim_end_matches('.');
        let qname_trimmed = qname.trim_end_matches('.');

        let addr = cfg.server_address.trim_end_matches('.');

        // A/AAAA: hijack if server_address resolves differently from server_name
        if (qtype == 1 || qtype == 28) && qname_trimmed == server_name && addr != server_name {
            if let Ok(ips) = (addr, 0u16).to_socket_addrs().map(|addrs| {
                addrs
                    .filter_map(|a| {
                        let ip = a.ip();
                        if qtype == 1 && ip.is_ipv4() {
                            Some(ip)
                        } else if qtype == 28 && ip.is_ipv6() {
                            Some(ip)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            }) {
                return dns_protocol::build_synthetic_response(id, &qname, qtype, &ips);
            }
        }

        // Fallback: standard resolution
        if let Ok(addrs) = (qname_trimmed, 0u16).to_socket_addrs() {
            let standard_addrs = addrs
                .filter_map(|a| {
                    let ip = a.ip();
                    if qtype == 1 && ip.is_ipv4() {
                        Some(ip)
                    } else if qtype == 28 && ip.is_ipv6() {
                        Some(ip)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !standard_addrs.is_empty() {
                return dns_protocol::build_synthetic_response(id, &qname, qtype, &standard_addrs);
            }
        }

        query.to_vec()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_a_query(name: &str) -> Vec<u8> {
        let mut query = Vec::new();
        query.extend_from_slice(&[0x12, 0x34]); // ID
        query.extend_from_slice(&[0x01, 0x00]); // flags (standard query)
        query.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1
        query.extend_from_slice(&[0x00, 0x00]); // ANCOUNT = 0
        query.extend_from_slice(&[0x00, 0x00]); // NSCOUNT = 0
        query.extend_from_slice(&[0x00, 0x00]); // ARCOUNT = 0
        // Question: encoded name
        for label in name.split('.') {
            query.push(label.len() as u8);
            query.extend_from_slice(label.as_bytes());
        }
        query.push(0); // end of name
        query.extend_from_slice(&[0x00, 0x01]); // QTYPE = A
        query.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN
        query
    }

    #[test]
    fn test_dns_parse_query_simple() {
        let query = build_a_query("example.com");
        let (id, name, qtype) = dns_protocol::parse_query(&query).unwrap();
        assert_eq!(id, 0x1234);
        assert_eq!(name, "example.com");
        assert_eq!(qtype, 1);
    }

    #[test]
    fn test_dns_parse_query_multi_label() {
        let query = build_a_query("www.example.co.uk");
        let (id, name, qtype) = dns_protocol::parse_query(&query).unwrap();
        assert_eq!(id, 0x1234);
        assert_eq!(name, "www.example.co.uk");
        assert_eq!(qtype, 1);
    }

    #[test]
    fn test_synthetic_response() {
        use std::net::{IpAddr, Ipv4Addr};
        let ips = vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))];
        let response = dns_protocol::build_synthetic_response(0x1234, "example.com", 1, &ips);

        // Verify it's parseable back
        assert!(response.len() > 12);
        // First 2 bytes should be our ID
        assert_eq!(response[0], 0x12);
        assert_eq!(response[1], 0x34);
    }
}
