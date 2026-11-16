//! # cronet-rs
//!
//! Rust bindings for Chromium Cronet (naiveproxy).
//!
//! Port of [cronet-go](https://github.com/SagerNet/cronet-go) — provides a
//! high-level client that wraps the Cronet C API with a cleaner interface
//! supporting HTTP/2 and QUIC CONNECT proxies with NaiveProxy's padding
//! protocol.
//!
//! ## Feature flags
//!
//! | Feature | Default | Platforms | Description |
//! |---|---|---|---|
//! | `dynamic` | ✅ | Linux, Windows, Android | Load `libcronet` at runtime via dlopen/LoadLibrary |
//! | `static-link` | ❌ | macOS, iOS, Android, Linux, Windows | Link `libcronet.a` at compile time |
//! | `android-static` | ❌ | Android | Alias for `static-link` |
//! | `download-static` | ❌ | static targets | Download and verify a static archive only when `static-link` is also enabled |
//! | `download` | ❌ | all | Have `cronet-sys`'s build script download + SHA-256 verify a prebuilt `libcronet` for the target into `OUT_DIR` |
//!
//! `static-link` takes precedence if Cargo also enables `dynamic`.
//! Linux archives must match the target libc (glibc or musl); Windows static
//! linking requires MSVC and `cronet.lib` plus its generated `link_flags.txt`.
//! The native Windows build uses `/MT`; compile Rust with matching
//! `RUSTFLAGS="-C target-feature=+crt-static"` and an explicit `--target`.
//! `CRONET_LIB_DIR` bypasses downloading in static builds, including with `download-static`.
//! `download-static` alone keeps dynamic loading and never downloads. The legacy
//! `download` feature retains its download behavior for either backend.
//! New Linux/Windows static assets are not checksum-pinned yet: provide a local
//! archive until a verified release is available.
//!
//! **macOS/iOS must use `static-link`** — dlopen is not available.
//! **Android should prefer `static-link`** for stable production builds.
//!
//! With `download`, the native library is fetched at build time (no manual
//! setup). In dynamic mode, load it via
//! [`sys::load_downloaded_library`](crate::sys::load_downloaded_library)
//! instead of passing an explicit path; in static mode it links automatically.
//!
//! ```toml
//! [dependencies]
//! cronet-rs = { version = "0.1", default-features = false, features = ["static-link"] }
//! ```
//!
//! ## Quick Start (dynamic mode)
//!
//! ```rust,no_run
//! use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig};
//! use cronet_rs::sys::load_library;
//! use std::io::{Read, Write};
//!
//! unsafe { load_library("libcronet.so").unwrap(); }
//!
//! let mut client = NaiveClient::new(NaiveClientConfig {
//!     server_address: "example.com:443".to_string(),
//!     username: Some("user".to_string()),
//!     password: Some("pass".to_string()),
//!     ..Default::default()
//! }).unwrap();
//! client.start().unwrap();
//!
//! let mut conn = client.dial_and_handshake("httpbin.org:80").unwrap();
//! write!(conn, "GET /get HTTP/1.1\r\nHost: httpbin.org\r\nConnection: close\r\n\r\n").unwrap();
//! conn.flush().unwrap();
//!
//! let mut response = String::new();
//! conn.read_to_string(&mut response).unwrap();
//! println!("{}", response);
//! client.close().unwrap();
//! ```
//!
//! ## Quick Start (static mode, macOS/iOS)
//!
//! ```rust,no_run
//! // Set: CRONET_LIB_DIR=/path/to/libcronet cargo build
//! use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig};
//! use std::io::{Read, Write};
//!
//! let mut client = NaiveClient::new(NaiveClientConfig {
//!     server_address: "example.com:443".to_string(),
//!     ..Default::default()
//! }).unwrap();
//! client.start().unwrap();
//!
//! let mut conn = client.dial_and_handshake("httpbin.org:80").unwrap();
//! write!(conn, "GET /get HTTP/1.1\r\nHost: httpbin.org\r\nConnection: close\r\n\r\n").unwrap();
//! conn.flush().unwrap();
//! let mut resp = String::new();
//! conn.read_to_string(&mut resp).unwrap();
//! println!("{}", resp);
//! client.close().unwrap();
//! ```

pub mod bidirectional;
pub mod dns;
pub mod engine;
pub mod engine_params;
pub mod error;
pub mod naive_client;
pub mod naive_conn;
pub mod padding;
pub mod sys;

// Re-export main public types at the crate root.
pub use bidirectional::{BidirectionalConn, BidirectionalStream, BidirectionalStreamEngine};
pub use engine::Engine;
pub use engine_params::{EngineParams, EngineParamsBuilder, HttpCacheMode};
pub use error::{CronetError, CronetResult, ErrorCode, NetError};
pub use naive_client::{NaiveClient, NaiveClientConfig, QuicCongestionControl};
pub use naive_conn::NaiveConn;
