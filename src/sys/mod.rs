//! Raw FFI bindings for Chromium Cronet C API (libcronet).
//!
//! Two backends:
//! - **dynamic** (default, Linux/Windows/Android): load libcronet at runtime via dlopen
//! - **static** (macOS/iOS): link libcronet at compile time via `#[link]`
//!
//! On macOS and iOS, use `--features static-link` (or default-features = false).

// ---------------------------------------------------------------------------
// Common opaque pointer types — always compiled
// ---------------------------------------------------------------------------

/// Opaque handle to a Cronet_Engine object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawEngine(pub u64);

/// Opaque handle to a Cronet_EngineParams object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawEngineParams(pub u64);

/// Opaque handle to a StreamEngine (bidirectional_stream_engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawStreamEngine(pub u64);

/// Opaque handle to a BidirectionalStream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawBidirectionalStream(pub u64);

/// Opaque handle to a CertVerifier.
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

/// Raw callback struct for bidirectional_stream.
#[repr(C)]
pub struct RawBidirectionalStreamCallback {
    pub on_stream_ready: Option<
        extern "C" fn(stream: RawBidirectionalStream, user_data: u64),
    >,
    pub on_response_headers_received: Option<
        extern "C" fn(
            stream: RawBidirectionalStream,
            headers: *const RawBidirectionalStreamHeaderArray,
            negotiated_protocol: *const std::os::raw::c_char,
            user_data: u64,
        ),
    >,
    pub on_read_completed: Option<
        extern "C" fn(stream: RawBidirectionalStream, bytes_read: i32, user_data: u64),
    >,
    pub on_write_completed: Option<
        extern "C" fn(stream: RawBidirectionalStream, user_data: u64),
    >,
    pub on_response_trailers_received: Option<
        extern "C" fn(
            stream: RawBidirectionalStream,
            trailers: *const RawBidirectionalStreamHeaderArray,
            user_data: u64,
        ),
    >,
    pub on_succeeded: Option<
        extern "C" fn(stream: RawBidirectionalStream, user_data: u64),
    >,
    pub on_failed: Option<
        extern "C" fn(stream: RawBidirectionalStream, net_error: i32, user_data: u64),
    >,
    pub on_canceled: Option<
        extern "C" fn(stream: RawBidirectionalStream, user_data: u64),
    >,
}

/// Header struct for bidirectional_stream.
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
