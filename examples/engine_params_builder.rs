//! EngineParamsBuilder example — demonstrate all builder options.
//!
//! Shows how to configure the Cronet engine with various parameters
//! including QUIC, HTTP/2, Brotli, cache, and custom experimental options.
//!
//! Run:
//!   cargo run --example engine_params_builder

use cronet_rs::engine_params::{EngineParamsBuilder, HttpCacheMode};
use cronet_rs::error::CronetResult;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== EngineParamsBuilder Examples ===\n");

    // --- QUIC with BBR congestion control ---
    let params = EngineParamsBuilder::new()
        .quic(true)
        .http2(false)
        .brotli(true)
        .experimental_options(
            r#"{"QUIC":{"connection_options":"TBBR","receive_stream_window_size":16777216}}"#,
        )
        .build()?;
    println!("QUIC + BBR params: valid");
    drop(params);

    // --- HTTP/2 with large window ---
    let params = EngineParamsBuilder::new()
        .http2(true)
        .quic(false)
        .user_agent("cronet-rs-h2/0.1")
        .experimental_options(
            r#"{"SPDY":{"recv_window_size":16777216,"header_table_size":8388608}}"#,
        )
        .build()?;
    println!("HTTP/2 params: valid");
    drop(params);

    // --- Disk cache ---
    let params = EngineParamsBuilder::new()
        .http2(true)
        .http_cache_mode(HttpCacheMode::Disk)
        .http_cache_max_size(100 * 1024 * 1024) // 100 MB
        .build()?;
    println!("Disk cache params (100MB): valid");
    drop(params);

    // --- Custom UA + Brotli ---
    let params = EngineParamsBuilder::new()
        .http2(true)
        .brotli(true)
        .user_agent("MyApp/1.0 cronet-rs")
        .build()?;
    println!("Custom UA params: valid");
    drop(params);

    // --- Default (no extras) ---
    let params = EngineParamsBuilder::new().build()?;
    println!("Default params: valid");

    // --- Check CronetResult error codes ---
    println!("\n--- CronetResult codes ---");
    for code in &[0i32, -100, -101, -999] {
        let result = CronetResult::from(*code);
        println!("  {:>4} → {}", code, result);
    }

    drop(params);
    println!("\nAll params created and dropped successfully");

    Ok(())
}
