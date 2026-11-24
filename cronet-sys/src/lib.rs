//! Raw FFI bindings for the Chromium Cronet C API (`libcronet`).
//!
//! Desktop targets load Cronet dynamically by default. Enable `static-link` to
//! embed the library, or `dynamic` to select runtime loading when both are enabled.
//! Apple targets use static linking. Android uses runtime loading unless
//! `static-link` is enabled.
//!
//! ## Obtaining the native library
//!
//! The native library is not vendored. Provide it one of three ways:
//! - point `CRONET_LIB_DIR` at a directory containing it (static mode), or
//! - place a loadable `libcronet.{so,dll}` on the system search path (dynamic
//!   mode), or
//! - enable **`static-link,download-static`** for automatic static downloads, or
//!   the legacy **`download`** feature for either backend, to fetch and checksum a
//!   prebuilt from <https://github.com/rust-proxy/cronet-binaries> into `OUT_DIR`.
//!   Then load it via [`load_downloaded_library`] (dynamic) — the path is also
//!   available via [`downloaded_library_path`].

// The upstream header declares bidirectional_stream_is_done, but the native
// library does not implement it. Observe the terminal callbacks instead; binding
// the missing symbol prevents Windows static consumers from linking.

#![allow(clippy::missing_safety_doc)]

// ---------------------------------------------------------------------------
// Common opaque pointer types — always compiled
// ---------------------------------------------------------------------------

/// Opaque handle to a `Cronet_Engine` object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawEngine(pub usize);

/// Opaque handle to a `Cronet_EngineParams` object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawEngineParams(pub usize);

/// Opaque handle to a `stream_engine` (bidirectional stream engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawStreamEngine(pub usize);

/// Opaque handle to a `bidirectional_stream`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawBidirectionalStream(pub usize);

/// Opaque handle to a `CertVerifier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawCertVerifier(pub usize);

// ---------------------------------------------------------------------------
// C callback types — always compiled
// ---------------------------------------------------------------------------

/// TCP dialer callback: returns fd (or negative net_error).
pub type RawDialer =
    extern "C" fn(context: *mut std::ffi::c_void, address: *const std::os::raw::c_char, port: u16) -> i32;

/// UDP dialer callback: returns fd, writes back local address/port.
pub type RawUdpDialer = extern "C" fn(
    context: *mut std::ffi::c_void,
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

/// Whether this build links Cronet into the executable.
pub const STATIC_LINKED: bool = cfg!(cronet_static_link);

/// Path to the verified archive or shared library selected by the downloader,
/// or `None` when the build used no downloaded asset.
///
/// In dynamic mode this is a loadable `.so`/`.dll`; pass it to
/// [`load_library`] (or just call [`load_downloaded_library`]).
pub fn downloaded_library_path() -> Option<&'static str> {
    option_env!("CRONET_DOWNLOADED_LIB_PATH")
}

/// Load the prebuilt library that `build.rs` downloaded (dynamic mode).
///
/// Returns an error in dynamic mode without `download`. In static mode this
/// is a no-op: the library is already linked.
///
/// # Safety
/// Same contract as [`load_library`] — must be called before any other FFI
/// function, and only once.
pub unsafe fn load_downloaded_library() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(cronet_static_link)]
    {
        Ok(())
    }
    #[cfg(not(cronet_static_link))]
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

#[cfg(not(cronet_static_link))]
mod dynamic;
#[cfg(not(cronet_static_link))]
pub use dynamic::*;

#[cfg(cronet_static_link)]
mod static_link;
#[cfg(cronet_static_link)]
pub use static_link::*;

#[cfg(all(
    test,
    any(
        feature = "download",
        all(feature = "download-static", cronet_static_link)
    )
))]
mod download_tests {
    /// With the `download` feature on, build.rs must have fetched a prebuilt
    /// and exposed a real, existing path.
    #[test]
    fn downloaded_path_points_at_a_real_file() {
        if std::env::var_os("CRONET_LIB_DIR").is_some() && super::STATIC_LINKED {
            return;
        }
        let path = super::downloaded_library_path()
            .expect("`download` feature should set CRONET_DOWNLOADED_LIB_PATH");
        assert!(
            std::path::Path::new(path).exists(),
            "downloaded library should exist on disk: {path}"
        );
    }
}

#[cfg(all(test, cronet_static_link))]
mod static_tests {
    #[test]
    fn static_backend_does_not_load_a_runtime_library() {
        assert!(unsafe { super::load_library("/nonexistent/cronet-library") }.is_ok());
        assert!(unsafe { super::load_downloaded_library() }.is_ok());
    }
}

#[cfg(all(test, not(cronet_static_link), not(feature = "download")))]
mod dynamic_tests {
    #[test]
    fn dynamic_backend_uses_runtime_library_without_static_download() {
        assert!(super::downloaded_library_path().is_none());
        assert!(unsafe { super::load_library("/nonexistent/cronet-library") }.is_err());
    }
}
