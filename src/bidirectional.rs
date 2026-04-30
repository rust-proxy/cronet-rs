//! Bidirectional stream types for Cronet.
//!
//! Maps to cronet-go's bidirectional_stream_*.go files. Provides the low-level
//! stream interface and the higher-level connection wrapper.

use std::collections::HashMap;
use std::ffi::CString;
use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::error::{CronetError, NetError};
use crate::sys;

// ---------------------------------------------------------------------------
// BidirectionalStreamEngine
// ---------------------------------------------------------------------------

/// The StreamEngine, obtained from an Engine. Used to create bidirectional streams.
#[derive(Clone, Copy, Debug)]
pub struct BidirectionalStreamEngine {
    raw: sys::RawStreamEngine,
}

impl BidirectionalStreamEngine {
    pub(crate) fn from_raw(raw: sys::RawStreamEngine) -> Self {
        Self { raw }
    }

    /// Create a new bidirectional stream with the given callback.
    pub fn create_stream(
        &self,
        _callback: Arc<Mutex<dyn BidirectionalStreamCallback + Send>>,
    ) -> BidirectionalStream {
        BidirectionalStream::new(self.raw, _callback)
    }
}

// ---------------------------------------------------------------------------
// BidirectionalStreamCallback trait
// ---------------------------------------------------------------------------

/// Callback trait for bidirectional stream events.
pub trait BidirectionalStreamCallback {
    fn on_stream_ready(&mut self, stream: u64);
    fn on_response_headers_received(
        &mut self,
        stream: u64,
        headers: HashMap<String, String>,
        negotiated_protocol: String,
    );
    fn on_read_completed(&mut self, stream: u64, bytes_read: i32);
    fn on_write_completed(&mut self, stream: u64);
    fn on_response_trailers_received(&mut self, stream: u64, trailers: HashMap<String, String>);
    fn on_succeeded(&mut self, stream: u64);
    fn on_failed(&mut self, stream: u64, net_error: i32);
    fn on_canceled(&mut self, stream: u64);
}

// ---------------------------------------------------------------------------
// StreamState
// ---------------------------------------------------------------------------

/// State shared between Rust side and C callbacks for a single stream.
#[allow(dead_code)]
pub(crate) struct StreamState {
    pub callback: Arc<Mutex<dyn BidirectionalStreamCallback + Send>>,
    pub destroyed: AtomicBool,
}

// ---------------------------------------------------------------------------
// BidirectionalStream
// ---------------------------------------------------------------------------

/// A bidirectional stream over HTTP/2 or QUIC.
#[derive(Debug)]
pub struct BidirectionalStream {
    pub(crate) raw: sys::RawBidirectionalStream,
    destroyed: bool,
}

impl BidirectionalStream {
    fn new(
        raw_engine: sys::RawStreamEngine,
        _callback: Arc<Mutex<dyn BidirectionalStreamCallback + Send>>,
    ) -> Self {
        // TODO: create with proper C callback registration.
        let callback = &sys::RawBidirectionalStreamCallback {
            on_stream_ready: None,
            on_response_headers_received: None,
            on_read_completed: None,
            on_write_completed: None,
            on_response_trailers_received: None,
            on_succeeded: None,
            on_failed: None,
            on_canceled: None,
        };
        let raw = unsafe { sys::bidirectional_stream_create(raw_engine, 0, callback) };
        Self { raw, destroyed: false }
    }

    /// Start the stream.
    pub fn start(
        &mut self,
        method: &str,
        url: &str,
        headers: &HashMap<String, String>,
        priority: i32,
        end_of_stream: bool,
    ) -> std::result::Result<(), CronetError> {
        let c_method = CString::new(method)
            .map_err(|_| CronetError::Config("method contains null".into()))?;
        let c_url = CString::new(url)
            .map_err(|_| CronetError::Config("url contains null".into()))?;

        // Build C header array
        let mut c_headers: Vec<sys::RawBidirectionalStreamHeader> = Vec::new();
        let mut _c_keys: Vec<CString> = Vec::new();
        let mut _c_vals: Vec<CString> = Vec::new();

        for (key, value) in headers {
            let c_key = CString::new(key.as_str())
                .map_err(|_| CronetError::Config("header key contains null".into()))?;
            let c_val = CString::new(value.as_str())
                .map_err(|_| CronetError::Config("header value contains null".into()))?;
            c_headers.push(sys::RawBidirectionalStreamHeader {
                key: c_key.as_ptr(),
                value: c_val.as_ptr(),
            });
            _c_keys.push(c_key);
            _c_vals.push(c_val);
        }

        let result = if !c_headers.is_empty() {
            let header_array = sys::RawBidirectionalStreamHeaderArray {
                count: c_headers.len(),
                capacity: c_headers.len(),
                headers: c_headers.as_ptr(),
            };
            unsafe {
                sys::bidirectional_stream_start(
                    self.raw,
                    c_url.as_ptr(),
                    priority,
                    c_method.as_ptr(),
                    &header_array as *const _,
                    end_of_stream,
                )
            }
        } else {
            unsafe {
                sys::bidirectional_stream_start(
                    self.raw,
                    c_url.as_ptr(),
                    priority,
                    c_method.as_ptr(),
                    std::ptr::null(),
                    end_of_stream,
                )
            }
        };

        if result != 0 {
            return Err(CronetError::Net(NetError(result)));
        }
        Ok(())
    }

    /// Read data into the buffer.
    pub fn read(&mut self, buf: &mut [u8]) -> std::result::Result<i32, CronetError> {
        let len = buf.len() as i32;
        let result = unsafe {
            if buf.is_empty() {
                sys::bidirectional_stream_read(self.raw, std::ptr::null_mut(), 0)
            } else {
                sys::bidirectional_stream_read(self.raw, buf.as_mut_ptr(), len)
            }
        };
        if result < 0 {
            return Err(CronetError::Net(NetError(result)));
        }
        Ok(result)
    }

    /// Write data.
    pub fn write(&mut self, buf: &[u8], end_of_stream: bool) -> std::result::Result<i32, CronetError> {
        let len = buf.len() as i32;
        let result = unsafe {
            if buf.is_empty() {
                sys::bidirectional_stream_write(self.raw, std::ptr::null(), 0, end_of_stream)
            } else {
                sys::bidirectional_stream_write(self.raw, buf.as_ptr(), len, end_of_stream)
            }
        };
        if result < 0 {
            return Err(CronetError::Net(NetError(result)));
        }
        Ok(result)
    }

    /// Flush pending writes.
    pub fn flush(&self) {
        unsafe { sys::bidirectional_stream_flush(self.raw) }
    }

    /// Cancel the stream.
    pub fn cancel(&self) {
        unsafe { sys::bidirectional_stream_cancel(self.raw) }
    }

    /// Destroy the stream.
    pub fn destroy(&mut self) {
        if !self.destroyed {
            self.destroyed = true;
            unsafe { sys::bidirectional_stream_destroy(self.raw); }
        }
    }
}

impl Drop for BidirectionalStream {
    fn drop(&mut self) {
        self.destroy();
    }
}

unsafe impl Send for BidirectionalStream {}

// ---------------------------------------------------------------------------
// BidirectionalConn
// ---------------------------------------------------------------------------

/// A connection-oriented wrapper around `BidirectionalStream`.
///
/// Implements `Read` and `Write` for use as an I/O stream.
pub struct BidirectionalConn {
    stream: Option<BidirectionalStream>,
    closed: bool,
    headers: Option<HashMap<String, String>>,
    error: Option<CronetError>,
}

impl BidirectionalConn {
    pub fn new(_stream_engine: &BidirectionalStreamEngine) -> Self {
        let callback = Arc::new(Mutex::new(ConnCallbackHandler::new()));
        let stream = _stream_engine.create_stream(callback);
        Self {
            stream: Some(stream),
            closed: false,
            headers: None,
            error: None,
        }
    }

    /// Start the connection.
    pub fn start(
        &mut self,
        method: &str,
        url: &str,
        headers: &HashMap<String, String>,
        priority: i32,
        end_of_stream: bool,
    ) -> std::result::Result<(), CronetError> {
        if let Some(ref mut stream) = self.stream {
            stream.start(method, url, headers, priority, end_of_stream)
        } else {
            Err(CronetError::ConnectionClosed)
        }
    }

    /// Wait for response headers (handshake).
    pub fn wait_for_headers(&mut self) -> std::result::Result<&HashMap<String, String>, CronetError> {
        if let Some(ref headers) = self.headers {
            return Ok(headers);
        }
        if self.error.is_some() {
            return Err(CronetError::Config("previous stream error".into()));
        }
        Err(CronetError::Config("headers not received yet".into()))
    }

    /// Close the connection.
    pub fn close(&mut self) -> std::result::Result<(), CronetError> {
        self.closed = true;
        if let Some(ref mut stream) = self.stream {
            stream.cancel();
        }
        Ok(())
    }

    /// Flush the underlying stream.
    pub fn flush(&self) {
        if let Some(ref stream) = self.stream {
            stream.flush();
        }
    }
}

impl Read for BidirectionalConn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.closed {
            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "connection closed"));
        }
        if let Some(ref mut stream) = self.stream {
            let n = stream.read(buf).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
            Ok(n as usize)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "stream not available"))
        }
    }
}

impl Write for BidirectionalConn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.closed {
            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "connection closed"));
        }
        if let Some(ref mut stream) = self.stream {
            stream.write(buf, false).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
            Ok(buf.len())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "stream not available"))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref stream) = self.stream {
            stream.flush();
        }
        Ok(())
    }
}

impl Drop for BidirectionalConn {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Internal callback handler for connection-level events.
struct ConnCallbackHandler {
    ready: bool,
    headers: Option<HashMap<String, String>>,
    error: Option<CronetError>,
}

impl ConnCallbackHandler {
    fn new() -> Self {
        Self { ready: false, headers: None, error: None }
    }
}

impl BidirectionalStreamCallback for ConnCallbackHandler {
    fn on_stream_ready(&mut self, _stream: u64) { self.ready = true; }

    fn on_response_headers_received(
        &mut self, _stream: u64, headers: HashMap<String, String>, _negotiated_protocol: String,
    ) { self.headers = Some(headers); }

    fn on_read_completed(&mut self, _stream: u64, _bytes_read: i32) {}
    fn on_write_completed(&mut self, _stream: u64) {}
    fn on_response_trailers_received(&mut self, _stream: u64, _trailers: HashMap<String, String>) {}
    fn on_succeeded(&mut self, _stream: u64) {}

    fn on_failed(&mut self, _stream: u64, net_error: i32) {
        self.error = Some(CronetError::Net(NetError(net_error)));
    }

    fn on_canceled(&mut self, _stream: u64) { self.error = Some(CronetError::Canceled); }
}
