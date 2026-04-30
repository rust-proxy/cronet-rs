//! Dynamic loading backend — load libcronet at runtime via dlopen/LoadLibrary.
//!
//! Used on Linux, Windows, and Android where dlopen is available.
//! NOT available on macOS/iOS.

#![allow(non_snake_case, dead_code, unsafe_op_in_unsafe_fn)]

use std::sync::OnceLock;

pub use libloading::Library;

static LIB: OnceLock<Library> = OnceLock::new();

/// Load the libcronet shared library at runtime.
///
/// Must be called **before** any other FFI function (engine creation, etc.).
/// On success, the library stays loaded for the lifetime of the process.
///
/// # Safety
///
/// Calling this twice will return an error. Only one library can be loaded.
pub unsafe fn load_library(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let lib = Library::new(path)?;
    LIB.set(lib).map_err(|_| "library already loaded".into())
}

// ---------------------------------------------------------------------------
// Helper: look up and call a C function
// ---------------------------------------------------------------------------

macro_rules! ffi_call {
    ($name:literal, $sig:ty, $($arg:expr),* $(,)?) => {{
        let lib = LIB.get().expect("libcronet not loaded; call load_library() first");
        let func: libloading::Symbol<$sig> = lib.get($name).unwrap_or_else(|e| {
            panic!("symbol {} not found: {}", String::from_utf8_lossy($name), e)
        });
        func($($arg),*)
    }};
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub unsafe fn Cronet_Engine_Create() -> super::RawEngine {
    ffi_call!(b"Cronet_Engine_Create", unsafe extern "C" fn() -> super::RawEngine,)
}

pub unsafe fn Cronet_Engine_Destroy(engine: super::RawEngine) {
    ffi_call!(b"Cronet_Engine_Destroy", unsafe extern "C" fn(super::RawEngine), engine)
}

pub unsafe fn Cronet_Engine_StartWithParams(
    engine: super::RawEngine, params: super::RawEngineParams,
) -> i32 {
    ffi_call!(b"Cronet_Engine_StartWithParams", unsafe extern "C" fn(super::RawEngine, super::RawEngineParams) -> i32, engine, params)
}

pub unsafe fn Cronet_Engine_Shutdown(engine: super::RawEngine) -> i32 {
    ffi_call!(b"Cronet_Engine_Shutdown", unsafe extern "C" fn(super::RawEngine) -> i32, engine)
}

pub unsafe fn Cronet_Engine_GetVersionString(engine: super::RawEngine) -> *const std::os::raw::c_char {
    ffi_call!(b"Cronet_Engine_GetVersionString", unsafe extern "C" fn(super::RawEngine) -> *const std::os::raw::c_char, engine)
}

pub unsafe fn Cronet_Engine_GetDefaultUserAgent(engine: super::RawEngine) -> *const std::os::raw::c_char {
    ffi_call!(b"Cronet_Engine_GetDefaultUserAgent", unsafe extern "C" fn(super::RawEngine) -> *const std::os::raw::c_char, engine)
}

pub unsafe fn Cronet_Engine_GetStreamEngine(engine: super::RawEngine) -> super::RawStreamEngine {
    ffi_call!(b"Cronet_Engine_GetStreamEngine", unsafe extern "C" fn(super::RawEngine) -> super::RawStreamEngine, engine)
}

pub unsafe fn Cronet_Engine_CloseAllConnections(engine: super::RawEngine) {
    ffi_call!(b"Cronet_Engine_CloseAllConnections", unsafe extern "C" fn(super::RawEngine), engine)
}

pub unsafe fn Cronet_CreateCertVerifierWithRootCerts(
    pem: *const std::os::raw::c_char,
) -> super::RawCertVerifier {
    ffi_call!(b"Cronet_CreateCertVerifierWithRootCerts", unsafe extern "C" fn(*const std::os::raw::c_char) -> super::RawCertVerifier, pem)
}

pub unsafe fn Cronet_Engine_SetMockCertVerifierForTesting(
    engine: super::RawEngine, verifier: super::RawCertVerifier,
) {
    ffi_call!(b"Cronet_Engine_SetMockCertVerifierForTesting", unsafe extern "C" fn(super::RawEngine, super::RawCertVerifier), engine, verifier)
}

pub unsafe fn Cronet_Engine_SetDialer(
    engine: super::RawEngine,
    callback: Option<super::RawDialer>,
    context: u64,
) {
    ffi_call!(b"Cronet_Engine_SetDialer", unsafe extern "C" fn(super::RawEngine, Option<super::RawDialer>, u64), engine, callback, context)
}

pub unsafe fn Cronet_Engine_SetUdpDialer(
    engine: super::RawEngine,
    callback: Option<super::RawUdpDialer>,
    context: u64,
) {
    ffi_call!(b"Cronet_Engine_SetUdpDialer", unsafe extern "C" fn(super::RawEngine, Option<super::RawUdpDialer>, u64), engine, callback, context)
}

// ---------------------------------------------------------------------------
// EngineParams
// ---------------------------------------------------------------------------

pub unsafe fn Cronet_EngineParams_Create() -> super::RawEngineParams {
    ffi_call!(b"Cronet_EngineParams_Create", unsafe extern "C" fn() -> super::RawEngineParams,)
}

pub unsafe fn Cronet_EngineParams_Destroy(params: super::RawEngineParams) {
    ffi_call!(b"Cronet_EngineParams_Destroy", unsafe extern "C" fn(super::RawEngineParams), params)
}

pub unsafe fn Cronet_EngineParams_enable_quic_set(params: super::RawEngineParams, enable: bool) {
    ffi_call!(b"Cronet_EngineParams_enable_quic_set", unsafe extern "C" fn(super::RawEngineParams, bool), params, enable)
}

pub unsafe fn Cronet_EngineParams_enable_http2_set(params: super::RawEngineParams, enable: bool) {
    ffi_call!(b"Cronet_EngineParams_enable_http2_set", unsafe extern "C" fn(super::RawEngineParams, bool), params, enable)
}

pub unsafe fn Cronet_EngineParams_enable_brotli_set(params: super::RawEngineParams, enable: bool) {
    ffi_call!(b"Cronet_EngineParams_enable_brotli_set", unsafe extern "C" fn(super::RawEngineParams, bool), params, enable)
}

pub unsafe fn Cronet_EngineParams_user_agent_set(
    params: super::RawEngineParams, ua: *const std::os::raw::c_char,
) {
    ffi_call!(b"Cronet_EngineParams_user_agent_set", unsafe extern "C" fn(super::RawEngineParams, *const std::os::raw::c_char), params, ua)
}

pub unsafe fn Cronet_EngineParams_experimental_options_set(
    params: super::RawEngineParams, opts: *const std::os::raw::c_char,
) {
    ffi_call!(b"Cronet_EngineParams_experimental_options_set", unsafe extern "C" fn(super::RawEngineParams, *const std::os::raw::c_char), params, opts)
}

pub unsafe fn Cronet_EngineParams_http_cache_mode_set(params: super::RawEngineParams, mode: i32) {
    ffi_call!(b"Cronet_EngineParams_http_cache_mode_set", unsafe extern "C" fn(super::RawEngineParams, i32), params, mode)
}

pub unsafe fn Cronet_EngineParams_http_cache_max_size_set(params: super::RawEngineParams, size: i64) {
    ffi_call!(b"Cronet_EngineParams_http_cache_max_size_set", unsafe extern "C" fn(super::RawEngineParams, i64), params, size)
}

// ---------------------------------------------------------------------------
// BidirectionalStream
// ---------------------------------------------------------------------------

pub unsafe fn bidirectional_stream_create(
    engine: super::RawStreamEngine,
    annotation: u64,
    callback: *const super::RawBidirectionalStreamCallback,
) -> super::RawBidirectionalStream {
    ffi_call!(b"bidirectional_stream_create", unsafe extern "C" fn(super::RawStreamEngine, u64, *const super::RawBidirectionalStreamCallback) -> super::RawBidirectionalStream, engine, annotation, callback)
}

pub unsafe fn bidirectional_stream_destroy(stream: super::RawBidirectionalStream) -> i32 {
    ffi_call!(b"bidirectional_stream_destroy", unsafe extern "C" fn(super::RawBidirectionalStream) -> i32, stream)
}

pub unsafe fn bidirectional_stream_start(
    stream: super::RawBidirectionalStream,
    url: *const std::os::raw::c_char,
    priority: i32,
    method: *const std::os::raw::c_char,
    headers: *const super::RawBidirectionalStreamHeaderArray,
    end_of_stream: bool,
) -> i32 {
    ffi_call!(b"bidirectional_stream_start", unsafe extern "C" fn(super::RawBidirectionalStream, *const std::os::raw::c_char, i32, *const std::os::raw::c_char, *const super::RawBidirectionalStreamHeaderArray, bool) -> i32, stream, url, priority, method, headers, end_of_stream)
}

pub unsafe fn bidirectional_stream_read(
    stream: super::RawBidirectionalStream,
    buffer: *mut u8,
    size: i32,
) -> i32 {
    ffi_call!(b"bidirectional_stream_read", unsafe extern "C" fn(super::RawBidirectionalStream, *mut u8, i32) -> i32, stream, buffer, size)
}

pub unsafe fn bidirectional_stream_write(
    stream: super::RawBidirectionalStream,
    buffer: *const u8,
    size: i32,
    end_of_stream: bool,
) -> i32 {
    ffi_call!(b"bidirectional_stream_write", unsafe extern "C" fn(super::RawBidirectionalStream, *const u8, i32, bool) -> i32, stream, buffer, size, end_of_stream)
}

pub unsafe fn bidirectional_stream_flush(stream: super::RawBidirectionalStream) {
    ffi_call!(b"bidirectional_stream_flush", unsafe extern "C" fn(super::RawBidirectionalStream), stream)
}

pub unsafe fn bidirectional_stream_cancel(stream: super::RawBidirectionalStream) {
    ffi_call!(b"bidirectional_stream_cancel", unsafe extern "C" fn(super::RawBidirectionalStream), stream)
}

pub unsafe fn bidirectional_stream_disable_auto_flush(stream: super::RawBidirectionalStream, disable: bool) {
    ffi_call!(b"bidirectional_stream_disable_auto_flush", unsafe extern "C" fn(super::RawBidirectionalStream, bool), stream, disable)
}
