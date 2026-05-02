//! NaiveClient example — proxy dial with CONNECT and HTTP GET.
//!
//! This demonstrates the full flow:
//!   1. Load libcronet
//!   2. Create a NaiveClient with proxy config
//!   3. Dial a target through the proxy
//!   4. Perform HTTP GET over the tunnel
//!   5. Print response and close
//!
//! Usage:
//!   cargo run --example naive_client_simple
//!
//! Environment variables:
//!   PROXY_SERVER    — proxy address (default: example.com:443)
//!   PROXY_USERNAME  — proxy auth username (optional)
//!   PROXY_PASSWORD  — proxy auth password (optional)

use std::env;
use std::io::{Read, Write};

use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the Cronet shared library (no-op in static-link mode)
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119")?;
    }

    // Read config from environment
    let server_address = env::var("PROXY_SERVER").unwrap_or_else(|_| "example.com:443".into());
    let username = env::var("PROXY_USERNAME").ok();
    let password = env::var("PROXY_PASSWORD").ok();

    // Configure the NaiveClient
    let config = NaiveClientConfig {
        server_address,
        username,
        password,
        concurrency: 2,
        quic_enabled: false,
        extra_headers: [("X-Custom".into(), "cronet-rs".into())].into(),
        ..Default::default()
    };

    // Create and start the client
    let mut client = NaiveClient::new(config)?;
    client.start()?;
    println!("NaiveClient started");

    // Dial target through the proxy
    let target = "httpbin.org:80";
    let mut conn = client.dial_and_handshake(target)?;
    println!("Connected to {} via proxy", target);

    // Send HTTP GET request
    write!(
        conn,
        "GET /get HTTP/1.1\r\nHost: httpbin.org\r\nConnection: close\r\n\r\n"
    )?;
    conn.flush()?;

    // Read response
    let mut response = String::new();
    conn.read_to_string(&mut response)?;
    println!("Response:\n{}", response);

    // Close client
    client.close()?;
    println!("NaiveClient closed");

    Ok(())
}
