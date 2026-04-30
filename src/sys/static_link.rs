//! Static linking backend — link libcronet at compile time.
//!
//! Required on macOS and iOS where dlopen is unavailable.
//! The build.rs script emits the correct `cargo:rustc-link-lib` directives.
//!
//! # Environment
//!
//! - `CRONET_LIB_DIR` — directory containing `libcronet.a` / `libcronet.tbd`
//! - `CRONET_STATIC_NAME` — library name (default: `cronet` → libcronet.a)

#![allow(non_snake_case, dead_code, improper_ctypes, unsafe_op_in_unsafe_fn)]

/// In static-link mode, no library handle is needed.
/// This type exists for API compatibility with the dynamic backend.
pub struct Library;

impl Library {
    /// No-op. libcronet is already linked at compile time.
    /// # Safety
    ///
    /// Always safe. Calling this is unnecessary but harmless.
    pub unsafe fn new(_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Library)
    }
}

/// No-op in static mode — the library is already linked.
/// # Safety
///
    /// Always safe in this mode.
pub unsafe fn load_library(_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal namespace for the extern "C" declarations
// ---------------------------------------------------------------------------
mod ffi {
    use super::super::{
        RawBidirectionalStream, RawBidirectionalStreamCallback, RawBidirectionalStreamHeaderArray,
        RawCertVerifier, RawDialer, RawEngine, RawEngineParams, RawStreamEngine, RawUdpDialer,
    };

    unsafe extern "C" {
        // ---- Engine ----
        pub fn Cronet_Engine_Create() -> RawEngine;
        pub fn Cronet_Engine_Destroy(engine: RawEngine);
        pub fn Cronet_Engine_StartWithParams(engine: RawEngine, params: RawEngineParams) -> i32;
        pub fn Cronet_Engine_Shutdown(engine: RawEngine) -> i32;
        pub fn Cronet_Engine_GetVersionString(engine: RawEngine) -> *const std::os::raw::c_char;
        pub fn Cronet_Engine_GetDefaultUserAgent(engine: RawEngine) -> *const std::os::raw::c_char;
        pub fn Cronet_Engine_GetStreamEngine(engine: RawEngine) -> RawStreamEngine;
        pub fn Cronet_Engine_CloseAllConnections(engine: RawEngine);
        pub fn Cronet_CreateCertVerifierWithRootCerts(
            pem: *const std::os::raw::c_char,
        ) -> RawCertVerifier;
        pub fn Cronet_Engine_SetMockCertVerifierForTesting(engine: RawEngine, verifier: RawCertVerifier);
        pub fn Cronet_Engine_SetDialer(
            engine: RawEngine,
            callback: Option<RawDialer>,
            context: u64,
        );
        pub fn Cronet_Engine_SetUdpDialer(
            engine: RawEngine,
            callback: Option<RawUdpDialer>,
            context: u64,
        );

        // ---- EngineParams ----
        pub fn Cronet_EngineParams_Create() -> RawEngineParams;
        pub fn Cronet_EngineParams_Destroy(params: RawEngineParams);
        pub fn Cronet_EngineParams_enable_quic_set(params: RawEngineParams, enable: bool);
        pub fn Cronet_EngineParams_enable_http2_set(params: RawEngineParams, enable: bool);
        pub fn Cronet_EngineParams_enable_brotli_set(params: RawEngineParams, enable: bool);
        pub fn Cronet_EngineParams_user_agent_set(
            params: RawEngineParams, ua: *const std::os::raw::c_char,
        );
        pub fn Cronet_EngineParams_experimental_options_set(
            params: RawEngineParams, opts: *const std::os::raw::c_char,
        );
        pub fn Cronet_EngineParams_http_cache_mode_set(params: RawEngineParams, mode: i32);
        pub fn Cronet_EngineParams_http_cache_max_size_set(params: RawEngineParams, size: i64);

        // ---- BidirectionalStream ----
        pub fn bidirectional_stream_create(
            engine: RawStreamEngine,
            annotation: u64,
            callback: *const RawBidirectionalStreamCallback,
        ) -> RawBidirectionalStream;
        pub fn bidirectional_stream_destroy(stream: RawBidirectionalStream) -> i32;
        pub fn bidirectional_stream_start(
            stream: RawBidirectionalStream,
            url: *const std::os::raw::c_char,
            priority: i32,
            method: *const std::os::raw::c_char,
            headers: *const RawBidirectionalStreamHeaderArray,
            end_of_stream: bool,
        ) -> i32;
        pub fn bidirectional_stream_read(
            stream: RawBidirectionalStream, buffer: *mut u8, size: i32,
        ) -> i32;
        pub fn bidirectional_stream_write(
            stream: RawBidirectionalStream, buffer: *const u8, size: i32, end_of_stream: bool,
        ) -> i32;
        pub fn bidirectional_stream_flush(stream: RawBidirectionalStream);
        pub fn bidirectional_stream_cancel(stream: RawBidirectionalStream);
        pub fn bidirectional_stream_disable_auto_flush(stream: RawBidirectionalStream, disable: bool);
    }
}

// ---------------------------------------------------------------------------
// Engine — thin wrappers
// ---------------------------------------------------------------------------

pub unsafe fn Cronet_Engine_Create() -> super::RawEngine {
    ffi::Cronet_Engine_Create()
}

pub unsafe fn Cronet_Engine_Destroy(engine: super::RawEngine) {
    ffi::Cronet_Engine_Destroy(engine)
}

pub unsafe fn Cronet_Engine_StartWithParams(
    engine: super::RawEngine, params: super::RawEngineParams,
) -> i32 {
    ffi::Cronet_Engine_StartWithParams(engine, params)
}

pub unsafe fn Cronet_Engine_Shutdown(engine: super::RawEngine) -> i32 {
    ffi::Cronet_Engine_Shutdown(engine)
}

pub unsafe fn Cronet_Engine_GetVersionString(engine: super::RawEngine) -> *const std::os::raw::c_char {
    ffi::Cronet_Engine_GetVersionString(engine)
}

pub unsafe fn Cronet_Engine_GetDefaultUserAgent(engine: super::RawEngine) -> *const std::os::raw::c_char {
    ffi::Cronet_Engine_GetDefaultUserAgent(engine)
}

pub unsafe fn Cronet_Engine_GetStreamEngine(engine: super::RawEngine) -> super::RawStreamEngine {
    ffi::Cronet_Engine_GetStreamEngine(engine)
}

pub unsafe fn Cronet_Engine_CloseAllConnections(engine: super::RawEngine) {
    ffi::Cronet_Engine_CloseAllConnections(engine)
}

pub unsafe fn Cronet_CreateCertVerifierWithRootCerts(
    pem: *const std::os::raw::c_char,
) -> super::RawCertVerifier {
    ffi::Cronet_CreateCertVerifierWithRootCerts(pem)
}

pub unsafe fn Cronet_Engine_SetMockCertVerifierForTesting(
    engine: super::RawEngine, verifier: super::RawCertVerifier,
) {
    ffi::Cronet_Engine_SetMockCertVerifierForTesting(engine, verifier)
}

pub unsafe fn Cronet_Engine_SetDialer(
    engine: super::RawEngine,
    callback: Option<super::RawDialer>,
    context: u64,
) {
    ffi::Cronet_Engine_SetDialer(engine, callback, context)
}

pub unsafe fn Cronet_Engine_SetUdpDialer(
    engine: super::RawEngine,
    callback: Option<super::RawUdpDialer>,
    context: u64,
) {
    ffi::Cronet_Engine_SetUdpDialer(engine, callback, context)
}

// ---------------------------------------------------------------------------
// EngineParams — thin wrappers
// ---------------------------------------------------------------------------

pub unsafe fn Cronet_EngineParams_Create() -> super::RawEngineParams {
    ffi::Cronet_EngineParams_Create()
}

pub unsafe fn Cronet_EngineParams_Destroy(params: super::RawEngineParams) {
    ffi::Cronet_EngineParams_Destroy(params)
}

pub unsafe fn Cronet_EngineParams_enable_quic_set(params: super::RawEngineParams, enable: bool) {
    ffi::Cronet_EngineParams_enable_quic_set(params, enable)
}

pub unsafe fn Cronet_EngineParams_enable_http2_set(params: super::RawEngineParams, enable: bool) {
    ffi::Cronet_EngineParams_enable_http2_set(params, enable)
}

pub unsafe fn Cronet_EngineParams_enable_brotli_set(params: super::RawEngineParams, enable: bool) {
    ffi::Cronet_EngineParams_enable_brotli_set(params, enable)
}

pub unsafe fn Cronet_EngineParams_user_agent_set(
    params: super::RawEngineParams, ua: *const std::os::raw::c_char,
) {
    ffi::Cronet_EngineParams_user_agent_set(params, ua)
}

pub unsafe fn Cronet_EngineParams_experimental_options_set(
    params: super::RawEngineParams, opts: *const std::os::raw::c_char,
) {
    ffi::Cronet_EngineParams_experimental_options_set(params, opts)
}

pub unsafe fn Cronet_EngineParams_http_cache_mode_set(params: super::RawEngineParams, mode: i32) {
    ffi::Cronet_EngineParams_http_cache_mode_set(params, mode)
}

pub unsafe fn Cronet_EngineParams_http_cache_max_size_set(params: super::RawEngineParams, size: i64) {
    ffi::Cronet_EngineParams_http_cache_max_size_set(params, size)
}

// ---------------------------------------------------------------------------
// BidirectionalStream — thin wrappers
// ---------------------------------------------------------------------------

pub unsafe fn bidirectional_stream_create(
    engine: super::RawStreamEngine,
    annotation: u64,
    callback: *const super::RawBidirectionalStreamCallback,
) -> super::RawBidirectionalStream {
    ffi::bidirectional_stream_create(engine, annotation, callback)
}

pub unsafe fn bidirectional_stream_destroy(stream: super::RawBidirectionalStream) -> i32 {
    ffi::bidirectional_stream_destroy(stream)
}

pub unsafe fn bidirectional_stream_start(
    stream: super::RawBidirectionalStream,
    url: *const std::os::raw::c_char,
    priority: i32,
    method: *const std::os::raw::c_char,
    headers: *const super::RawBidirectionalStreamHeaderArray,
    end_of_stream: bool,
) -> i32 {
    ffi::bidirectional_stream_start(stream, url, priority, method, headers, end_of_stream)
}

pub unsafe fn bidirectional_stream_read(
    stream: super::RawBidirectionalStream, buffer: *mut u8, size: i32,
) -> i32 {
    ffi::bidirectional_stream_read(stream, buffer, size)
}

pub unsafe fn bidirectional_stream_write(
    stream: super::RawBidirectionalStream, buffer: *const u8, size: i32, end_of_stream: bool,
) -> i32 {
    ffi::bidirectional_stream_write(stream, buffer, size, end_of_stream)
}

pub unsafe fn bidirectional_stream_flush(stream: super::RawBidirectionalStream) {
    ffi::bidirectional_stream_flush(stream)
}

pub unsafe fn bidirectional_stream_cancel(stream: super::RawBidirectionalStream) {
    ffi::bidirectional_stream_cancel(stream)
}

pub unsafe fn bidirectional_stream_disable_auto_flush(
    stream: super::RawBidirectionalStream, disable: bool,
) {
    ffi::bidirectional_stream_disable_auto_flush(stream, disable)
}
