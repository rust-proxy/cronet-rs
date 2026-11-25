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
//! | `dynamic` | ❌ | Linux, Windows, Android | Load `libcronet` at runtime via dlopen/LoadLibrary |
//! | `static-link` | ❌ | macOS, iOS, Android, Linux, Windows | Link `libcronet.a` at compile time |
//! | `android-static` | ❌ | Android | Alias for `static-link` |
//! | `download-static` | ❌ | static targets | Download and verify a static archive when the static backend is selected |
//! | `download` | ❌ | all | Have `cronet-sys`'s build script download + SHA-256 verify a prebuilt `libcronet` for the target into `OUT_DIR` |
//!
//! Desktop builds load Cronet dynamically by default. Request `static-link` to embed it;
//! explicitly enabling `dynamic` selects runtime loading even with `static-link`.
//! Apple targets always link statically. On Android an explicit `static-link` wins.
//! Linux archives must match the target libc (glibc or musl); Windows static
//! linking requires MSVC and `cronet.lib` plus its generated `link_flags.txt`.
//! Windows downloads select `/MD` for Rust's default CRT without extra flags,
//! or `/MT` when `+crt-static` is enabled. Manually supplied archives must match
//! the application's CRT. Both variants statically link Cronet; `/MD` uses the
//! MSVC runtime DLLs.
//! `CRONET_LIB_DIR` bypasses downloading in static builds, including with `download-static`.
//! On desktop, `download-static` alone keeps dynamic loading and skips downloading. The legacy
//! `download` feature retains its download behavior for either backend.
//! Downloads use the fixed `v154.0.8037.49` release with pinned SHA-256
//! checksums. Cronet's private BoringSSL symbols are isolated from other TLS libraries.
//!
//! **macOS/iOS always link statically** — dlopen is not available.
//! **Android should prefer `static-link`** for stable production builds.
//!
//! ## Custom socket dialer
//!
//! [`socket_dialer::SocketDialer`] lets the embedder create every physical TCP
//! and UDP socket itself, for example to bind Cronet's connections to a
//! physical interface before connecting. The hook is available on Unix and
//! Windows and must be installed with
//! [`NaiveClient::start_with_dialer`](crate::naive_client::NaiveClient::start_with_dialer)
//! before the engine starts.
//!
//! With `download`, the native library is fetched at build time (no manual
//! setup). In dynamic mode, load it via
//! [`sys::load_downloaded_library`](crate::sys::load_downloaded_library)
//! instead of passing an explicit path; in static mode it links automatically.
//!
//! ## Host resolver rules
//!
//! [`NaiveClientConfig::host_resolver_rules`](crate::naive_client::NaiveClientConfig::host_resolver_rules)
//! maps the proxy server name to a fixed address (`MAP host ip`) while the
//! name still drives SNI and certificate verification:
//!
//! ```rust,no_run
//! use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig};
//!
//! let client = NaiveClient::new(NaiveClientConfig {
//!     server_address: "proxy.example.com:443".to_string(),
//!     host_resolver_rules: Some("MAP proxy.example.com 203.0.113.7".to_string()),
//!     ..Default::default()
//! })?;
//! # Ok::<(), cronet_rs::error::CronetError>(())
//! ```
//!
//! Rules are ignored for network-bound contexts.
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
pub mod socket_dialer;
pub mod sys;

// Re-export main public types at the crate root.
pub use bidirectional::{BidirectionalConn, BidirectionalStream, BidirectionalStreamEngine};
pub use engine::Engine;
pub use engine_params::{EngineParams, EngineParamsBuilder, HttpCacheMode};
pub use error::{CronetError, CronetResult, ErrorCode, NetError};
pub use naive_client::{NaiveClient, NaiveClientConfig, QuicCongestionControl};
pub use naive_conn::NaiveConn;
