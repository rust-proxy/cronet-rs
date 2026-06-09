//! Raw FFI bindings for the Chromium Cronet C API (`libcronet`).
//!
//! Two backends, selected by feature:
//! - **dynamic** (default, Linux/Windows/Android): load `libcronet` at runtime
//!   via `dlopen`/`LoadLibrary`.
//! - **static-link** (macOS/iOS, also others): link `libcronet` at compile time.
//!
//! On macOS and iOS, use `--features static-link` (or `default-features = false`),
//! since `dlopen` of the bundled static archive is not applicable there.
//!
//! ## Obtaining the native library
//!
//! The native library is not vendored. Provide it one of three ways:
//! - point `CRONET_LIB_DIR` at a directory containing it (static mode), or
//! - place a loadable `libcronet.{so,dll}` on the system search path (dynamic
//!   mode), or
//! - enable the **`download`** feature to have `build.rs` fetch and checksum a
//!   prebuilt from <https://github.com/rust-proxy/cronet-binaries> into `OUT_DIR`.
//!   Then load it via [`load_downloaded_library`] (dynamic) — the path is also
//!   available via [`downloaded_library_path`].

#![allow(clippy::missing_safety_doc)]

// ---------------------------------------------------------------------------
// Common opaque pointer types — always compiled
// ---------------------------------------------------------------------------

/// Opaque handle to a `Cronet_Engine` object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawEngine(pub u64);

/// Opaque handle to a `Cronet_EngineParams` object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawEngineParams(pub u64);

/// Opaque handle to a `stream_engine` (bidirectional stream engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawStreamEngine(pub u64);

/// Opaque handle to a `bidirectional_stream`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawBidirectionalStream(pub u64);

/// Opaque handle to a `CertVerifier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawCertVerifier(pub u64);

// ---------------------------------------------------------------------------
// C callback types — always compiled
// ---------------------------------------------------------------------------

/// TCP dialer callback: returns fd (or negative net_error).
pub type RawDialer =
    extern "C" fn(context: u64, address: *const std::os::raw::c_char, port: u16) -> i32;

/// UDP dialer callback: returns fd, writes back local address/port.
pub type RawUdpDialer = extern "C" fn(
    context: u64,
    address: *const std::os::raw::c_char,
    port: u16,
    out_local_address: *mut std::os::raw::c_char,
    out_local_port: *mut u16,
) -> i32;

/// Raw callback struct for `bidirectional_stream`.
///
/// Field order and signatures mirror `bidirectional_stream_callback` in
/// `bidirectional_stream_c.h` exactly. The C API does **not** pass a user-data
/// argument — application state is associated through the stream's `annotation`
/// (passed to `bidirectional_stream_create`). `on_read_completed` and
/// `on_write_completed` carry the data buffer pointer the C API was operating on.
#[repr(C)]
pub struct RawBidirectionalStreamCallback {
    /// `void (*on_stream_ready)(bidirectional_stream* stream)`
    pub on_stream_ready: Option<extern "C" fn(stream: RawBidirectionalStream)>,
    /// `void (*on_response_headers_received)(bidirectional_stream*, const header_array*, const char*)`
    pub on_response_headers_received: Option<
        extern "C" fn(
            stream: RawBidirectionalStream,
            headers: *const RawBidirectionalStreamHeaderArray,
            negotiated_protocol: *const std::os::raw::c_char,
        ),
    >,
    /// `void (*on_read_completed)(bidirectional_stream*, char* data, int bytes_read)`
    pub on_read_completed: Option<
        extern "C" fn(
            stream: RawBidirectionalStream,
            data: *mut std::os::raw::c_char,
            bytes_read: i32,
        ),
    >,
    /// `void (*on_write_completed)(bidirectional_stream*, const char* data)`
    pub on_write_completed: Option<
        extern "C" fn(stream: RawBidirectionalStream, data: *const std::os::raw::c_char),
    >,
    /// `void (*on_response_trailers_received)(bidirectional_stream*, const header_array*)`
    pub on_response_trailers_received: Option<
        extern "C" fn(
            stream: RawBidirectionalStream,
            trailers: *const RawBidirectionalStreamHeaderArray,
        ),
    >,
    /// `void (*on_succeded)(bidirectional_stream* stream)` (note: C header spelling)
    pub on_succeeded: Option<extern "C" fn(stream: RawBidirectionalStream)>,
    /// `void (*on_failed)(bidirectional_stream* stream, int net_error)`
    pub on_failed: Option<extern "C" fn(stream: RawBidirectionalStream, net_error: i32)>,
    /// `void (*on_canceled)(bidirectional_stream* stream)`
    pub on_canceled: Option<extern "C" fn(stream: RawBidirectionalStream)>,
}

/// Header struct for `bidirectional_stream`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawBidirectionalStreamHeader {
    pub key: *const std::os::raw::c_char,
    pub value: *const std::os::raw::c_char,
}

/// Header array struct.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawBidirectionalStreamHeaderArray {
    pub count: usize,
    pub capacity: usize,
    pub headers: *const RawBidirectionalStreamHeader,
}

// ---------------------------------------------------------------------------
// Prebuilt-library discovery (the `download` feature)
// ---------------------------------------------------------------------------

/// Filesystem path to the prebuilt libcronet that `build.rs` downloaded into
/// `OUT_DIR`, or `None` if the `download` feature was not enabled.
///
/// In dynamic mode this is a loadable `.so`/`.dll`; pass it to
/// [`load_library`] (or just call [`load_downloaded_library`]).
pub fn downloaded_library_path() -> Option<&'static str> {
    option_env!("CRONET_DOWNLOADED_LIB_PATH")
}

/// Load the prebuilt library that `build.rs` downloaded (dynamic mode).
///
/// Returns an error if the crate was built without the `download` feature.
///
/// # Safety
/// Same contract as [`load_library`] — must be called before any other FFI
/// function, and only once.
#[cfg(feature = "dynamic")]
pub unsafe fn load_downloaded_library() -> Result<(), Box<dyn std::error::Error>> {
    match downloaded_library_path() {
        Some(path) => unsafe { load_library(path) },
        None => Err("cronet-sys was built without the `download` feature; \
                     no prebuilt library path is available"
            .into()),
    }
}

// ---------------------------------------------------------------------------
// Pull in the correct backend
// ---------------------------------------------------------------------------

#[cfg(feature = "dynamic")]
mod dynamic;
#[cfg(feature = "dynamic")]
pub use dynamic::*;

#[cfg(not(feature = "dynamic"))]
mod static_link;
#[cfg(not(feature = "dynamic"))]
pub use static_link::*;

#[cfg(all(test, feature = "download"))]
mod download_tests {
    /// With the `download` feature on, build.rs must have fetched a prebuilt
    /// and exposed a real, existing path.
    #[test]
    fn downloaded_path_points_at_a_real_file() {
        let path = super::downloaded_library_path()
            .expect("`download` feature should set CRONET_DOWNLOADED_LIB_PATH");
        assert!(
            std::path::Path::new(path).exists(),
            "downloaded library should exist on disk: {path}"
        );
    }
}
