//! Basic Cronet Engine example.
//!
//! Demonstrates engine lifecycle: create, start with params, inspect version,
//! shutdown, and cleanup.
//!
//! Usage (dynamic mode):
//!   cargo run --example engine_basic
//!
//! Usage (static-link mode):
//!   CRONET_LIB_DIR=/path/to/lib cargo run --example engine_basic --no-default-features --features static-link

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In dynamic mode we must load the library first.
    // In static-link mode load_library is a no-op.
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119")?;
    }

    // --- Create engine ---
    let mut engine = cronet_rs::Engine::new();
    println!("Engine created");

    // --- Build params ---
    use cronet_rs::EngineParamsBuilder;
    let mut params = EngineParamsBuilder::new()
        .http2(true)
        .brotli(true)
        .user_agent("cronet-rs/0.1.0")
        .experimental_options(r#"{"SPDY":{"recv_window_size":8388608}}"#)
        .build()?;

    // --- Start engine ---
    engine.start_with_params(&params)?;
    params.destroy(); // C object no longer needed after start
    println!("Engine started");

    // --- Inspect engine info ---
    println!("Version: {}", engine.version());
    println!("User-Agent: {}", engine.default_user_agent());

    // --- Get stream engine ---
    let _stream_engine = engine.stream_engine();
    println!("Stream engine obtained");

    // --- Close all connections ---
    engine.close_all_connections();

    // --- Shutdown ---
    engine.shutdown()?;
    println!("Engine shutdown complete");

    Ok(())
}
