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
//!   PROXY_QUIC      — set to 1/true to use QUIC (HTTP/3) instead of HTTP/2
//!   TARGET          — destination to dial through the proxy (default: httpbin.org:80)

use std::env;
use std::io::{Read, Write};

use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load a local `.env` (searches the current dir and its ancestors), so
    // PROXY_* / TARGET can be set there instead of exporting them in the shell.
    // Real shell env vars still win over `.env`.
    match dotenvy::dotenv() {
        Ok(path) => eprintln!("loaded env from {}", path.display()),
        Err(e) if e.not_found() => {}
        Err(e) => eprintln!("warning: failed to read .env: {e}"),
    }

    // Load the Cronet shared library (no-op in static-link mode)
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("dylibs")?;
    }

    // Read config from environment
    let server_address = env::var("PROXY_SERVER").unwrap_or_else(|_| "example.com:443".into());
    let username = env::var("PROXY_USERNAME").ok();
    let password = env::var("PROXY_PASSWORD").ok();
    let quic_enabled = matches!(
        env::var("PROXY_QUIC").unwrap_or_default().as_str(),
        "1" | "true" | "TRUE" | "yes"
    );

    // Configure the NaiveClient
    let config = NaiveClientConfig {
        server_address,
        username,
        password,
        concurrency: 2,
        quic_enabled,
        extra_headers: [("X-Custom".into(), "cronet-rs".into())].into(),
        ..Default::default()
    };

    // Create and start the client
    let mut client = NaiveClient::new(config)?;
    client.start()?;
    println!("NaiveClient started");

    // Dial target through the proxy
    let target = env::var("TARGET").unwrap_or_else(|_| "httpbin.org:80".into());
    let mut conn = client.dial_and_handshake(&target)?;
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
