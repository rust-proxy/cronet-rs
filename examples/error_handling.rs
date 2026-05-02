//! Error handling patterns for cronet-rs.
//!
//! Demonstrates usage of CronetError, NetError, and CronetResult types.
//!
//! Run:
//!   cargo run --example error_handling

use cronet_rs::error::{CronetError, CronetResult, ErrorCode, NetError};

fn main() {
    println!("=== Cronet Error Handling Patterns ===\n");

    // --- CronetResult ---
    println!("--- CronetResult ---");
    let result = CronetResult::from(-101); // IllegalState
    match result {
        CronetResult::Success => println!("OK!"),
        CronetResult::IllegalArgument => println!("Bad argument"),
        CronetResult::IllegalState => println!("Illegal state — engine not started?"),
        _ => println!("Other error: {}", result),
    }

    // --- NetError ---
    println!("\n--- NetError ---");
    let net_err = NetError::from(-34); // CONNECTION_RESET
    println!("Error {}: {}", net_err.0, net_err.description());

    // Match on common network errors
    let errors = [
        NetError::CONNECTION_TIMED_OUT,
        NetError::CONNECTION_REFUSED,
        NetError::HOSTNAME_NOT_RESOLVED,
        NetError::CERT_AUTHORITY_INVALID,
        NetError::QUIC_PROTOCOL_FAILED,
    ];
    for err in &errors {
        println!("  {} → {}", err, err.description());
    }

    // --- CronetError ---
    println!("\n--- CronetError combinations ---");
    let scenarios: Vec<CronetError> = vec![
        CronetError::Result(CronetResult::IllegalState),
        CronetError::Net(NetError::CONNECTION_REFUSED),
        CronetError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
        CronetError::LibraryNotLoaded("libcronet.so not found on LD_LIBRARY_PATH".into()),
        CronetError::BadStatus("503 Service Unavailable".into()),
        CronetError::Config("server_address is required".into()),
        CronetError::ConnectionClosed,
        CronetError::Canceled,
    ];

    for err in &scenarios {
        match err {
            CronetError::Result(r) => println!("  API error: {}", r),
            CronetError::Net(n) => println!("  Network: {}", n),
            CronetError::Io(e) => println!("  IO: {}", e),
            CronetError::LibraryNotLoaded(msg) => println!("  Library: {}", msg),
            CronetError::BadStatus(s) => println!("  Bad status: {}", s),
            CronetError::Config(msg) => println!("  Config: {}", msg),
            CronetError::ConnectionClosed => println!("  Connection closed"),
            CronetError::Canceled => println!("  Canceled"),
        }
    }

    // --- ErrorCode ---
    println!("\n--- ErrorCode (from cronet_c.h) ---");
    let codes = [
        (ErrorCode::ErrorCallback, "callback"),
        (ErrorCode::ErrorHostnameNotResolved, "hostname not resolved"),
        (ErrorCode::ErrorConnectionRefused, "connection refused"),
        (ErrorCode::ErrorQuicProtocolFailed, "QUIC failed"),
    ];
    for (code, desc) in &codes {
        println!("  {:?}: {}", code, desc);
    }
}
