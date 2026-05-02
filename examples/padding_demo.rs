//! NaiveProxy padding protocol demonstration.
//!
//! Shows how the PaddingManager wraps data with random padding headers
//! (first 8 chunks) and strips them on read. This obfuscates traffic
//! patterns against deep packet inspection.
//!
//! Run:
//!   cargo run --example padding_demo

use std::io::Cursor;

use cronet_rs::padding::PaddingManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NaiveProxy Padding Protocol Demo ===\n");

    // --- Writer side ---
    let mut payload = Vec::new();
    let mut pm = PaddingManager::new();

    // Write first chunk (gets framed with 3-byte header + random padding)
    let msg1 = b"Hello, this is chunk 1 with padding!";
    let n1 = pm.write_with_padding(&mut Cursor::new(&mut payload), msg1)?;
    println!("Wrote chunk 1: {} bytes → {} raw bytes", n1, payload.len());

    // Write second chunk
    let msg2 = b"Chunk 2 also gets padding";
    let n2 = pm.write_with_padding(&mut Cursor::new(&mut payload), msg2)?;
    println!("Wrote chunk 2: {} bytes → {} raw bytes", n2, payload.len());

    // Write chunks 3-9 (past PADDING_COUNT=8, no padding)
    for i in 3..=9 {
        let msg = format!("Chunk {} — plain (no padding after chunk 8)", i);
        pm.write_with_padding(&mut Cursor::new(&mut payload), msg.as_bytes())?;
    }
    println!("\nTotal payload size: {} bytes", payload.len());

    // --- Reader side ---
    let mut reader = Cursor::new(&payload);
    let mut pm2 = PaddingManager::new();
    let mut out = vec![0u8; 1024];

    for i in 1..=9 {
        let (n, had_frame) = pm2.read_with_padding(&mut reader, &mut out)?;
        let text = String::from_utf8_lossy(&out[..n]);
        println!("Read chunk {}: {} bytes (framed={}) — {}", i, n, had_frame, text);
    }

    // --- Check replaceable status ---
    println!("\nReader replaceable: {} (should be true after 8 framed reads)",
        pm2.reader_replaceable());
    println!("Writer replaceable: {}", pm.writer_replaceable());
    println!("Writer headroom: {} bytes", pm.front_headroom());
    println!("Writer rear headroom: {} bytes", pm.rear_headroom());

    Ok(())
}
