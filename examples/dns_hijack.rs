//! DNS hijacking for NaiveProxy ECH support.
//!
//! Demonstrates DNS query parsing, synthetic response building,
//! and the naive DNS resolver that hijacks A/AAAA queries for
//! the proxy server name.
//!
//! Run:
//!   cargo run --example dns_hijack

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use cronet_rs::dns::{create_naive_dns_resolver, DnsConfig};

/// Build a raw DNS A-record query for testing.
fn build_a_query(name: &str) -> Vec<u8> {
    let mut query = Vec::new();
    query.extend_from_slice(&[0x12, 0x34]); // ID
    query.extend_from_slice(&[0x01, 0x00]); // flags (standard query)
    query.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1
    query.extend_from_slice(&[0x00, 0x00]); // ANCOUNT = 0
    query.extend_from_slice(&[0x00, 0x00]); // NSCOUNT = 0
    query.extend_from_slice(&[0x00, 0x00]); // ARCOUNT = 0
    for label in name.split('.') {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.push(0); // end of name
    query.extend_from_slice(&[0x00, 0x01]); // QTYPE = A
    query.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN
    query
}

/// Build a raw DNS AAAA-record query.
fn main() {
    println!("=== DNS Hijacking Demo ===\n");

    // --- 1. DNS query parsing ---
    let query = build_a_query("example.com");
    let parsed = cronet_rs::dns::parse_dns_query(&query);
    match parsed {
        Some((id, qname, qtype)) => {
            println!("Parsed DNS query: id=0x{:04x}, name={}, qtype={}",
                id, qname, qtype);
        }
        None => eprintln!("Failed to parse DNS query"),
    }

    // --- 2. Synthetic response building ---
    let ips = vec![
        IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
        IpAddr::V4(Ipv4Addr::new(93, 184, 216, 35)),
    ];
    let response = cronet_rs::dns::build_dns_response(0x1234, "example.com", 1, &ips);
    println!("\nSynthetic A response ({} bytes): header id=0x{:04x}{:04x}, ancount={}",
        response.len(),
        u16::from_be_bytes([response[0], response[1]]),
        u16::from_be_bytes([response[2], response[3]]),
        u16::from_be_bytes([response[6], response[7]]),
    );

    // --- 3. AAAA record ---
    let ipv6s = vec![
        IpAddr::V6(Ipv6Addr::new(0x2606, 0x2800, 0x220, 0x1, 0x248, 0x1893, 0x25c8, 0x1946)),
    ];
    let resp6 = cronet_rs::dns::build_dns_response(0x5678, "example.com", 28, &ipv6s);
    println!("Synthetic AAAA response: {} bytes, ancount={}",
        resp6.len(),
        u16::from_be_bytes([resp6[6], resp6[7]]),
    );

    // --- 4. Naive DNS resolver with hijacking ---
    let config = DnsConfig {
        server_name: "proxy.example.com".into(),
        server_address: "93.184.216.34".into(),
        ech_enabled: false,
        ech_config_list: None,
        ech_query_server_name: None,
        quic_enabled: false,
    };

    let resolver = create_naive_dns_resolver(config);
    let hijack_query = build_a_query("proxy.example.com");
    let hijack_response = resolver(&hijack_query);

    if hijack_response != hijack_query {
        println!("\nDNS hijacked! Response differs from query ({} vs {} bytes)",
            hijack_response.len(), hijack_query.len());
        println!("  ANCOUNT = {} (should be >= 1 for hijacked response)",
            u16::from_be_bytes([hijack_response[6], hijack_response[7]]));
    } else {
        println!("\nDNS query passed through (no hijacking needed)");
    }

    // Normal query (should pass through)
    let normal_query = build_a_query("example.com");
    let normal_response = resolver(&normal_query);
    println!("Normal query response: {} bytes (query was {} bytes)",
        normal_response.len(), normal_query.len());
}
