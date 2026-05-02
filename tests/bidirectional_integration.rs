//! Integration tests for BidirectionalStream types.
//!
//! These tests only validate the Rust-side logic and type safety.
//! Actual Cronet stream operations require libcronet and are #[ignore]'d.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cronet_rs::bidirectional::{BidirectionalStreamCallback, BidirectionalStreamEngine};
use cronet_rs::error::{CronetError, NetError};

/// A mock callback for testing the trait.
#[derive(Default)]
struct MockCallback {
    events: Arc<Mutex<Vec<String>>>,
}

impl BidirectionalStreamCallback for MockCallback {
    fn on_stream_ready(&mut self, stream: u64) {
        self.events.lock().unwrap().push(format!("ready:{}", stream));
    }
    fn on_response_headers_received(
        &mut self, stream: u64, headers: HashMap<String, String>, proto: String,
    ) {
        self.events.lock().unwrap().push(format!("headers:{}:{:?}:{}", stream, headers, proto));
    }
    fn on_read_completed(&mut self, stream: u64, bytes_read: i32) {
        self.events.lock().unwrap().push(format!("read:{}:{}", stream, bytes_read));
    }
    fn on_write_completed(&mut self, stream: u64) {
        self.events.lock().unwrap().push(format!("write:{}", stream));
    }
    fn on_response_trailers_received(&mut self, stream: u64, trailers: HashMap<String, String>) {
        self.events.lock().unwrap().push(format!("trailers:{}:{:?}", stream, trailers));
    }
    fn on_succeeded(&mut self, stream: u64) {
        self.events.lock().unwrap().push(format!("succeeded:{}", stream));
    }
    fn on_failed(&mut self, stream: u64, net_error: i32) {
        self.events.lock().unwrap().push(format!("failed:{}:{}", stream, net_error));
    }
    fn on_canceled(&mut self, stream: u64) {
        self.events.lock().unwrap().push(format!("canceled:{}", stream));
    }
}

/// Helper to build a sample header map.
fn sample_headers() -> HashMap<String, String> {
    let mut h = HashMap::new();
    h.insert(":status".into(), "200".into());
    h.insert("content-type".into(), "text/plain".into());
    h.insert("content-length".into(), "42".into());
    h
}

#[test]
fn test_bidirectional_stream_callback_trait() {
    let callback = MockCallback::default();
    let events = callback.events.clone();

    // Simulate lifecycle
    {
        let mut cb = callback;
        cb.on_stream_ready(1);
        cb.on_response_headers_received(1, sample_headers(), "h2".into());
        cb.on_read_completed(1, 100);
        cb.on_write_completed(1);
        cb.on_succeeded(1);
    }

    let events = events.lock().unwrap();
    assert_eq!(events.len(), 5);
    assert_eq!(events[0], "ready:1");
    assert!(events[1].contains("headers:1"));
    assert_eq!(events[2], "read:1:100");
    assert_eq!(events[3], "write:1");
    assert_eq!(events[4], "succeeded:1");
}

#[test]
fn test_callback_failed_event() {
    let callback = MockCallback::default();
    let events = callback.events.clone();
    {
        let mut cb = callback;
        cb.on_failed(1, -34); // CONNECTION_RESET
    }
    let events = events.lock().unwrap();
    assert_eq!(events[0], "failed:1:-34");
}

#[test]
fn test_callback_canceled_event() {
    let callback = MockCallback::default();
    let events = callback.events.clone();
    {
        let mut cb = callback;
        cb.on_canceled(42);
    }
    let events = events.lock().unwrap();
    assert_eq!(events[0], "canceled:42");
}

#[test]
fn test_callback_trailers() {
    let callback = MockCallback::default();
    let events = callback.events.clone();
    let mut trailers = HashMap::new();
    trailers.insert("x-final".into(), "true".into());

    {
        let mut cb = callback;
        cb.on_response_trailers_received(1, trailers);
    }
    let events = events.lock().unwrap();
    assert!(events[0].contains("trailers"));
    assert!(events[0].contains("x-final"));
}

#[test]
fn test_bidirectional_stream_engine_create() {
    // BidirectionalStreamEngine::from_raw is pub(crate), so we test
    // that the type exists and is Sync+Send
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BidirectionalStreamEngine>();
}

#[test]
fn test_cronet_error_from_net_error_conversion() {
    let net_err = NetError::from(-34);
    let err: CronetError = net_err.into();
    assert!(matches!(err, CronetError::Net(ref n) if n.0 == -34));
}

/// Headers with CString boundary issues.
#[test]
fn test_header_array_large_headers() {
    let mut headers: HashMap<String, String> = HashMap::new();
    // Keys and values with various lengths
    headers.insert("a".into(), "b".into());
    headers.insert("x-custom-header".into(), "some-long-value-here".into());
    headers.insert(String::new(), String::new());

    // Just verify these don't panic at Rust level — CString handles nulls.
    for (k, v) in &headers {
        let _ = std::ffi::CString::new(k.as_str());
        let _ = std::ffi::CString::new(v.as_str());
    }
}

#[test]
fn test_bidirectional_stream_engine_derive() {
    // Verify Debug is implemented
    #[derive(Debug)]
    struct Wrapper {
        _engine: BidirectionalStreamEngine,
    }
    let _w = Wrapper { _engine: unsafe { std::mem::zeroed() } };
}
