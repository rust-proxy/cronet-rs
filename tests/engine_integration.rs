//! Integration tests for Engine and EngineParams.
//!
//! Tests that do NOT require libcronet:
//!   - Builder defaults and chaining
//!   - Error type conversions
//!   - Type safety checks
//!   - HttpCacheMode discriminants
//!
//! Tests requiring libcronet are #[ignore]'d.
//!
//! NOTE: In dynamic mode even EngineParams::new() calls FFI (dlopen),
//! so all tests creating Engine/EngineParams require libcronet.

use cronet_rs::engine::{Dialer, UdpDialer};
use cronet_rs::engine_params::HttpCacheMode;

// ---------------------------------------------------------------------------
// Tests that don't need libcronet (pure Rust type checks)
// ---------------------------------------------------------------------------

#[test]
fn test_http_cache_mode_discriminants() {
    // Discriminants must match Cronet_EngineParams_HTTP_CACHE_MODE in cronet.idl_c.h:
    //   DISABLED=0, IN_MEMORY=1, DISK_NO_HTTP=2, DISK=3.
    assert_eq!(HttpCacheMode::Disabled as i32, 0);
    assert_eq!(HttpCacheMode::InMemory as i32, 1);
    assert_eq!(HttpCacheMode::DiskNoHttp as i32, 2);
    assert_eq!(HttpCacheMode::Disk as i32, 3);
}

#[test]
fn test_dialer_type_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Dialer>();
    assert_send::<UdpDialer>();
}

// ---------------------------------------------------------------------------
// Tests requiring libcronet — must be manually enabled
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires libcronet shared library; set CRONET_LIB_DIR or LD_LIBRARY_PATH"]
fn test_builder_default() {
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut params = cronet_rs::engine_params::EngineParamsBuilder::new().build().unwrap();
    params.destroy();
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_builder_with_quic() {
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut params = cronet_rs::engine_params::EngineParamsBuilder::new()
        .quic(true)
        .http2(false)
        .build()
        .unwrap();
    params.destroy();
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_builder_with_all_options() {
    use cronet_rs::engine_params::{EngineParamsBuilder, HttpCacheMode};
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut params = EngineParamsBuilder::new()
        .quic(true)
        .http2(true)
        .brotli(true)
        .user_agent("cronet-rs-test/0.1")
        .experimental_options(r#"{"QUIC":{"connection_options":"TBBR"}}"#)
        .http_cache_mode(HttpCacheMode::InMemory)
        .http_cache_max_size(50_000_000)
        .build()
        .unwrap();
    params.destroy();
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_builder_cache_disk() {
    use cronet_rs::engine_params::{EngineParamsBuilder, HttpCacheMode};
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut params = EngineParamsBuilder::new()
        .http2(true)
        .http_cache_mode(HttpCacheMode::Disk)
        .http_cache_max_size(100_000_000)
        .build()
        .unwrap();
    params.destroy();
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_builder_custom_user_agent() {
    use cronet_rs::engine_params::EngineParamsBuilder;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut params = EngineParamsBuilder::new()
        .user_agent("TestApp/1.0")
        .build()
        .unwrap();
    params.destroy();
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_engine_params_drop_safety() {
    use cronet_rs::engine_params::EngineParamsBuilder;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let params = EngineParamsBuilder::new().build().unwrap();
    drop(params);
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_engine_params_double_destroy() {
    use cronet_rs::engine_params::EngineParamsBuilder;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut params = EngineParamsBuilder::new().build().unwrap();
    params.destroy();
    params.destroy();
}

#[test]
#[ignore = "requires libcronet shared library; set CRONET_LIB_DIR or LD_LIBRARY_PATH"]
fn test_engine_lifecycle_dynamic() {
    use cronet_rs::engine_params::EngineParamsBuilder;
    use cronet_rs::Engine;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut engine = Engine::new();
    let mut params = EngineParamsBuilder::new()
        .http2(true)
        .build()
        .unwrap();
    engine.start_with_params(&params).unwrap();
    params.destroy();

    assert!(!engine.version().is_empty());
    assert!(!engine.default_user_agent().is_empty());

    let _stream_engine = engine.stream_engine();
    engine.close_all_connections();
    engine.shutdown().unwrap();
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_engine_version_string() {
    use cronet_rs::Engine;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let engine = Engine::new();
    let version = engine.version();
    assert!(version.contains("."), "version should contain dots: {}", version);
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_engine_default_user_agent() {
    use cronet_rs::Engine;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let engine = Engine::new();
    let ua = engine.default_user_agent();
    assert!(!ua.is_empty(), "user agent should not be empty");
    assert!(ua.contains("Cronet"), "user agent should contain 'Cronet': {}", ua);
}

#[test]
#[ignore = "requires libcronet shared library"]
fn test_set_dialer_disable() {
    use cronet_rs::Engine;
    #[cfg(feature = "dynamic")]
    unsafe {
        cronet_rs::sys::load_library("libcronet.so.119").unwrap();
    }
    let mut engine = Engine::new();
    engine.set_dialer(None);
    engine.set_udp_dialer(None);
}
