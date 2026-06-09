//! Live integration tests against the real `libcronet.dll` shipped in
//! `dylibs/windows-amd64/`.
//!
//! Unlike the other integration suites (which only validate Rust-side logic),
//! these tests actually `LoadLibrary` the bundled Cronet build and call into it,
//! exercising the FFI bindings end-to-end. This is what catches ABI mismatches
//! between the Rust declarations and the C headers.
//!
//! The tests only run on Windows with the default `dynamic` feature and only
//! when the DLL is present; otherwise they skip gracefully so cross-platform
//! `cargo test` stays green.

#![cfg(all(windows, feature = "dynamic"))]

use std::path::PathBuf;
use std::sync::Once;

use cronet_rs::engine_params::{EngineParamsBuilder, HttpCacheMode};
use cronet_rs::Engine;

/// Root `dylibs/` tree. The real DLL lives at `dylibs/windows-amd64/libcronet.dll`;
/// we hand the *root* to `load_library` so the recursive platform-best-match
/// resolution is exercised end-to-end against the real binary.
fn dylibs_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dylibs")
}

static LOAD: Once = Once::new();

/// Load `libcronet` exactly once for this test binary. Returns `false` if the
/// DLL is not on disk (so the caller can skip).
///
/// `load_library` stores the handle in a process-global `OnceLock`, so this is
/// safe to call from every test; only the first call actually loads.
fn ensure_loaded() -> bool {
    let root = dylibs_root();
    let dll = root.join("windows-amd64").join("libcronet.dll");
    if !dll.exists() {
        eprintln!("skipping: {} not found", dll.display());
        return false;
    }
    LOAD.call_once(|| {
        // Pass the *root* dir: load_library should recurse into windows-amd64/,
        // pick libcronet.dll, and ignore the sibling .h files. Errors (incl.
        // "already loaded") are intentionally ignored — another thread may have
        // won the race.
        let _ = unsafe { cronet_rs::sys::load_library(root.to_str().unwrap()) };
    });
    true
}

/// Build, start, and tear down a real engine; check version / user-agent.
#[test]
fn real_engine_lifecycle() {
    if !ensure_loaded() {
        return;
    }

    let mut engine = Engine::new();
    let mut params = EngineParamsBuilder::new().http2(true).quic(true).build().unwrap();
    engine.start_with_params(&params).expect("engine start");
    params.destroy();

    let version = engine.version();
    assert!(
        version.contains('.'),
        "version string should look like x.y.z.w, got {version:?}"
    );

    let ua = engine.default_user_agent();
    assert!(!ua.is_empty(), "default user agent must not be empty");

    // GetStreamEngine returns a handle owned by the engine — must be non-null
    // for a started engine.
    let stream_engine = engine.stream_engine();
    let _ = stream_engine; // handle validity is enough; creation is exercised below

    engine.close_all_connections();
    engine.shutdown().expect("engine shutdown");
}

/// The storage-free `HttpCacheMode`s must be accepted by the real engine.
///
/// This is the live counterpart to the discriminant unit test. We only exercise
/// `Disabled` and `InMemory` here: the disk-backed modes (`DiskNoHttp`, `Disk`)
/// require a storage path — which is not part of the bound API — and Cronet
/// intentionally aborts if a disk cache is requested without one. The full
/// discriminant contract (including the previously-swapped disk values) is
/// pinned by `engine_integration::test_http_cache_mode_discriminants`.
#[test]
fn real_engine_accepts_memory_cache_modes() {
    if !ensure_loaded() {
        return;
    }

    for mode in [HttpCacheMode::Disabled, HttpCacheMode::InMemory] {
        let mut engine = Engine::new();
        let mut params = EngineParamsBuilder::new()
            .http2(true)
            .http_cache_mode(mode)
            .build()
            .unwrap();

        let r = engine.start_with_params(&params);
        params.destroy();
        assert!(r.is_ok(), "start failed for cache mode {mode:?}: {r:?}");
        engine.shutdown().expect("shutdown");
    }
}

/// Create and destroy a real bidirectional stream through the corrected
/// `RawBidirectionalStreamCallback` layout. A wrong struct size/field order
/// here would corrupt the callback table the C side reads.
#[test]
fn real_bidirectional_stream_create_destroy() {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use cronet_rs::bidirectional::BidirectionalStreamCallback;

    if !ensure_loaded() {
        return;
    }

    /// Minimal no-op callback handler.
    struct Noop;
    impl BidirectionalStreamCallback for Noop {
        fn on_stream_ready(&mut self, _stream: u64) {}
        fn on_response_headers_received(
            &mut self,
            _stream: u64,
            _headers: HashMap<String, String>,
            _proto: String,
        ) {
        }
        fn on_read_completed(&mut self, _stream: u64, _bytes_read: i32) {}
        fn on_write_completed(&mut self, _stream: u64) {}
        fn on_response_trailers_received(&mut self, _stream: u64, _trailers: HashMap<String, String>) {}
        fn on_succeeded(&mut self, _stream: u64) {}
        fn on_failed(&mut self, _stream: u64, _net_error: i32) {}
        fn on_canceled(&mut self, _stream: u64) {}
    }

    let mut engine = Engine::new();
    let mut params = EngineParamsBuilder::new().http2(true).build().unwrap();
    engine.start_with_params(&params).expect("engine start");
    params.destroy();

    let stream_engine = engine.stream_engine();
    {
        let mut stream = stream_engine.create_stream(Arc::new(Mutex::new(Noop)));
        // Drop runs bidirectional_stream_destroy through the corrected ABI.
        stream.destroy();
    }

    engine.shutdown().expect("engine shutdown");
}

/// Drive a real HTTP/2 request through the full async callback bridge and
/// confirm response headers come back. This is the regression test for the
/// dangling-callback access violation: a stream created with a stack-local
/// callback table would crash here once the network thread fired a callback.
///
/// Network-dependent: if the host can't reach the internet the request errors
/// out (or times out) and the test skips the assertion. The point is that the
/// callback path completes without crashing.
#[test]
fn real_bidirectional_get_receives_status_header() {
    use std::collections::HashMap;

    use cronet_rs::BidirectionalConn;

    if !ensure_loaded() {
        return;
    }

    let mut engine = Engine::new();
    let mut params = EngineParamsBuilder::new().http2(true).build().unwrap();
    engine.start_with_params(&params).expect("engine start");
    params.destroy();

    {
        let stream_engine = engine.stream_engine();
        let mut conn = BidirectionalConn::new(&stream_engine);
        let headers = HashMap::new();
        // GET with end_of_stream = true (no request body).
        if conn
            .start("GET", "https://example.com", &headers, 0, true)
            .is_ok()
        {
            match conn.wait_for_headers() {
                Ok(h) => assert!(
                    h.contains_key(":status"),
                    "expected a :status pseudo-header, got keys {:?}",
                    h.keys().collect::<Vec<_>>()
                ),
                Err(e) => eprintln!("skipping assertion (no network / request failed): {e}"),
            }
        }
        let _ = conn.close();
        // conn drops here, before engine shutdown.
    }

    engine.shutdown().expect("engine shutdown");
}
