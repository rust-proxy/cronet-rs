//! Bidirectional stream types for Cronet.
//!
//! Maps to cronet-go's bidirectional_stream_*.go files. Provides the low-level
//! stream interface and the higher-level connection wrapper.
//!
//! ## Callback / lifetime model
//!
//! Cronet's `bidirectional_stream` API is asynchronous: completions are
//! delivered on the engine's network thread via a `bidirectional_stream_callback`
//! table. The C library retains both the callback table pointer and the stream's
//! `annotation` for the lifetime of the stream, so neither may dangle.
//!
//! We satisfy that with:
//! - a single **`'static` callback table** ([`CALLBACKS`]) of `extern "C"`
//!   trampolines — its address is valid forever;
//! - a per-stream [`Shared`] state behind an `Arc`. One `Arc` ref is handed to
//!   the C side via the stream `annotation` ([`Arc::into_raw`]); the terminal
//!   callback (`on_succeeded`/`on_failed`/`on_canceled`) reclaims it. Because the
//!   C side holds a strong ref until a terminal event, the state can never be
//!   freed while a callback might still fire — no use-after-free even if the Rust
//!   `BidirectionalStream` is dropped early.
//!
//! Synchronous methods (`wait_for_headers`, `read`, `write`) block on a condvar
//! until the matching callback fires (or the stream terminates / times out).

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::io::{Read, Write};
use std::os::raw::c_char;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::error::{CronetError, NetError};
use crate::sys;

/// Default time the blocking helpers wait for a callback before giving up.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

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

    /// Create a new bidirectional stream that forwards events to `callback`.
    pub fn create_stream(
        &self,
        callback: Arc<Mutex<dyn BidirectionalStreamCallback + Send>>,
    ) -> BidirectionalStream {
        BidirectionalStream::new(self.raw, Some(callback))
    }
}

// ---------------------------------------------------------------------------
// BidirectionalStreamCallback trait
// ---------------------------------------------------------------------------

/// Callback trait for bidirectional stream events.
///
/// Implementors are invoked on the engine's **network thread**, so they must not
/// block or re-enter the stream. The `stream` argument is the raw stream handle
/// value (for identification only).
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
// Shared per-stream state
// ---------------------------------------------------------------------------

/// Terminal outcome of a stream — at most one is ever set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Terminal {
    Succeeded,
    Failed(i32),
    Canceled,
}

/// Mutable state guarded by `Shared::mutex`.
struct Inner {
    ready: bool,
    headers: Option<HashMap<String, String>>,
    negotiated_protocol: String,
    trailers: Option<HashMap<String, String>>,
    /// Set when an `on_read_completed` arrives; `last_read` carries its count.
    read_completed: bool,
    last_read: i32,
    /// Set when an `on_write_completed` arrives.
    write_completed: bool,
    terminal: Option<Terminal>,
    /// Optional user observer, invoked (outside the lock) after state updates.
    user_cb: Option<Arc<Mutex<dyn BidirectionalStreamCallback + Send>>>,
}

/// State shared between the Rust side and the C callbacks for one stream.
struct Shared {
    mutex: Mutex<Inner>,
    cond: Condvar,
}

impl Shared {
    fn new(user_cb: Option<Arc<Mutex<dyn BidirectionalStreamCallback + Send>>>) -> Self {
        Self {
            mutex: Mutex::new(Inner {
                ready: false,
                headers: None,
                negotiated_protocol: String::new(),
                trailers: None,
                read_completed: false,
                last_read: 0,
                write_completed: false,
                terminal: None,
                user_cb,
            }),
            cond: Condvar::new(),
        }
    }
}

fn terminal_to_err(t: Terminal) -> CronetError {
    match t {
        Terminal::Succeeded => CronetError::ConnectionClosed,
        Terminal::Failed(net) => CronetError::Net(NetError(net)),
        Terminal::Canceled => CronetError::Canceled,
    }
}

// ---------------------------------------------------------------------------
// C callback trampolines
// ---------------------------------------------------------------------------

/// Recover the `*const Shared` stored in the stream's `annotation` field.
///
/// `bidirectional_stream` is laid out as `{ void* obj; void* annotation; }`, so
/// the annotation we passed to `bidirectional_stream_create` lives one pointer
/// past the stream address.
#[inline]
fn shared_ptr(stream: sys::RawBidirectionalStream) -> *const Shared {
    unsafe {
        let base = (stream.0 as usize) as *const *const std::ffi::c_void;
        *base.add(1) as *const Shared
    }
}

/// Borrow the shared state for a non-terminal callback. Safe because the C side
/// holds a strong `Arc` ref until a terminal callback runs.
#[inline]
unsafe fn shared<'a>(stream: sys::RawBidirectionalStream) -> &'a Shared {
    unsafe { &*shared_ptr(stream) }
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

unsafe fn headers_to_map(arr: *const sys::RawBidirectionalStreamHeaderArray) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if arr.is_null() {
        return map;
    }
    let arr = unsafe { &*arr };
    if arr.headers.is_null() || arr.count == 0 {
        return map;
    }
    let slice = unsafe { std::slice::from_raw_parts(arr.headers, arr.count) };
    for h in slice {
        let key = unsafe { cstr(h.key) };
        if !key.is_empty() {
            map.insert(key, unsafe { cstr(h.value) });
        }
    }
    map
}

/// Take a clone of the user callback under the lock so it can be invoked after
/// the guard is released (avoids re-entrancy deadlocks).
fn take_user_cb(inner: &Inner) -> Option<Arc<Mutex<dyn BidirectionalStreamCallback + Send>>> {
    inner.user_cb.clone()
}

extern "C" fn on_stream_ready(stream: sys::RawBidirectionalStream) {
    let sh = unsafe { shared(stream) };
    let cb = {
        let mut g = sh.mutex.lock().unwrap();
        g.ready = true;
        take_user_cb(&g)
    };
    sh.cond.notify_all();
    if let Some(cb) = cb {
        cb.lock().unwrap().on_stream_ready(stream.0);
    }
}

extern "C" fn on_response_headers_received(
    stream: sys::RawBidirectionalStream,
    headers: *const sys::RawBidirectionalStreamHeaderArray,
    negotiated_protocol: *const c_char,
) {
    let map = unsafe { headers_to_map(headers) };
    let proto = unsafe { cstr(negotiated_protocol) };
    let sh = unsafe { shared(stream) };
    let cb = {
        let mut g = sh.mutex.lock().unwrap();
        g.headers = Some(map.clone());
        g.negotiated_protocol = proto.clone();
        take_user_cb(&g)
    };
    sh.cond.notify_all();
    if let Some(cb) = cb {
        cb.lock()
            .unwrap()
            .on_response_headers_received(stream.0, map, proto);
    }
}

extern "C" fn on_read_completed(
    stream: sys::RawBidirectionalStream,
    _data: *mut c_char,
    bytes_read: i32,
) {
    // `_data` aliases the buffer we handed to bidirectional_stream_read(); the
    // bytes are already in it. We only need the count.
    let sh = unsafe { shared(stream) };
    let cb = {
        let mut g = sh.mutex.lock().unwrap();
        g.last_read = bytes_read;
        g.read_completed = true;
        take_user_cb(&g)
    };
    sh.cond.notify_all();
    if let Some(cb) = cb {
        cb.lock().unwrap().on_read_completed(stream.0, bytes_read);
    }
}

extern "C" fn on_write_completed(stream: sys::RawBidirectionalStream, _data: *const c_char) {
    let sh = unsafe { shared(stream) };
    let cb = {
        let mut g = sh.mutex.lock().unwrap();
        g.write_completed = true;
        take_user_cb(&g)
    };
    sh.cond.notify_all();
    if let Some(cb) = cb {
        cb.lock().unwrap().on_write_completed(stream.0);
    }
}

extern "C" fn on_response_trailers_received(
    stream: sys::RawBidirectionalStream,
    trailers: *const sys::RawBidirectionalStreamHeaderArray,
) {
    let map = unsafe { headers_to_map(trailers) };
    let sh = unsafe { shared(stream) };
    let cb = {
        let mut g = sh.mutex.lock().unwrap();
        g.trailers = Some(map.clone());
        take_user_cb(&g)
    };
    sh.cond.notify_all();
    if let Some(cb) = cb {
        cb.lock()
            .unwrap()
            .on_response_trailers_received(stream.0, map);
    }
}

/// Shared tail for the three terminal callbacks: record the outcome, wake any
/// waiter, forward to the observer, then reclaim the C side's `Arc` ref.
fn finish(stream: sys::RawBidirectionalStream, outcome: Terminal) {
    let ptr = shared_ptr(stream);
    let cb = {
        let sh = unsafe { &*ptr };
        let cb = {
            let mut g = sh.mutex.lock().unwrap();
            // Only the first terminal event counts.
            if g.terminal.is_none() {
                g.terminal = Some(outcome);
            }
            take_user_cb(&g)
        };
        sh.cond.notify_all();
        cb
    };
    if let Some(cb) = cb {
        let mut guard = cb.lock().unwrap();
        match outcome {
            Terminal::Succeeded => guard.on_succeeded(stream.0),
            Terminal::Failed(net) => guard.on_failed(stream.0, net),
            Terminal::Canceled => guard.on_canceled(stream.0),
        }
    }
    // Balance the Arc::into_raw() done at creation. After a terminal event the C
    // library makes no further callbacks, so dropping this ref is safe.
    unsafe { drop(Arc::from_raw(ptr)) };
}

extern "C" fn on_succeeded(stream: sys::RawBidirectionalStream) {
    finish(stream, Terminal::Succeeded);
}

extern "C" fn on_failed(stream: sys::RawBidirectionalStream, net_error: i32) {
    finish(stream, Terminal::Failed(net_error));
}

extern "C" fn on_canceled(stream: sys::RawBidirectionalStream) {
    finish(stream, Terminal::Canceled);
}

/// The single, `'static` callback table shared by every stream.
static CALLBACKS: sys::RawBidirectionalStreamCallback = sys::RawBidirectionalStreamCallback {
    on_stream_ready: Some(on_stream_ready),
    on_response_headers_received: Some(on_response_headers_received),
    on_read_completed: Some(on_read_completed),
    on_write_completed: Some(on_write_completed),
    on_response_trailers_received: Some(on_response_trailers_received),
    on_succeeded: Some(on_succeeded),
    on_failed: Some(on_failed),
    on_canceled: Some(on_canceled),
};

// ---------------------------------------------------------------------------
// BidirectionalStream
// ---------------------------------------------------------------------------

/// A bidirectional stream over HTTP/2 or QUIC.
pub struct BidirectionalStream {
    pub(crate) raw: sys::RawBidirectionalStream,
    shared: Arc<Shared>,
    started: bool,
    destroyed: bool,
}

impl BidirectionalStream {
    fn new(
        raw_engine: sys::RawStreamEngine,
        user_cb: Option<Arc<Mutex<dyn BidirectionalStreamCallback + Send>>>,
    ) -> Self {
        let shared = Arc::new(Shared::new(user_cb));
        // Hand one strong ref to the C side as the stream annotation. It is
        // reclaimed by the terminal callback (see `finish`).
        let annotation = Arc::into_raw(Arc::clone(&shared)) as usize as u64;
        let raw = unsafe { sys::bidirectional_stream_create(raw_engine, annotation, &CALLBACKS) };
        Self {
            raw,
            shared,
            started: false,
            destroyed: false,
        }
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
        let c_url =
            CString::new(url).map_err(|_| CronetError::Config("url contains null".into()))?;

        // Keep the CStrings alive until the call returns.
        let mut c_keys: Vec<CString> = Vec::with_capacity(headers.len());
        let mut c_vals: Vec<CString> = Vec::with_capacity(headers.len());
        let mut c_headers: Vec<sys::RawBidirectionalStreamHeader> = Vec::with_capacity(headers.len());
        for (key, value) in headers {
            let c_key = CString::new(key.as_str())
                .map_err(|_| CronetError::Config("header key contains null".into()))?;
            let c_val = CString::new(value.as_str())
                .map_err(|_| CronetError::Config("header value contains null".into()))?;
            c_headers.push(sys::RawBidirectionalStreamHeader {
                key: c_key.as_ptr(),
                value: c_val.as_ptr(),
            });
            c_keys.push(c_key);
            c_vals.push(c_val);
        }

        let header_array = sys::RawBidirectionalStreamHeaderArray {
            count: c_headers.len(),
            capacity: c_headers.len(),
            headers: c_headers.as_ptr(),
        };
        let header_ptr = if c_headers.is_empty() {
            std::ptr::null()
        } else {
            &header_array as *const _
        };

        let result = unsafe {
            sys::bidirectional_stream_start(
                self.raw,
                c_url.as_ptr(),
                priority,
                c_method.as_ptr(),
                header_ptr,
                end_of_stream,
            )
        };
        if result != 0 {
            return Err(CronetError::Net(NetError(result)));
        }
        self.started = true;
        Ok(())
    }

    /// Block until response headers arrive, returning them. Errors if the stream
    /// terminates first or the wait times out.
    pub fn wait_for_headers(
        &self,
        timeout: Duration,
    ) -> std::result::Result<HashMap<String, String>, CronetError> {
        let deadline = Instant::now() + timeout;
        let mut g = self.shared.mutex.lock().unwrap();
        loop {
            if let Some(h) = &g.headers {
                return Ok(h.clone());
            }
            if let Some(t) = g.terminal {
                return Err(terminal_to_err(t));
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(CronetError::Config("timed out waiting for headers".into()));
            }
            let (ng, _) = self.shared.cond.wait_timeout(g, deadline - now).unwrap();
            g = ng;
        }
    }

    /// Read up to `buf.len()` bytes. Returns the number read (0 = end of stream).
    ///
    /// Issues an async `bidirectional_stream_read` and blocks until the matching
    /// `on_read_completed` (or a terminal event) fires. `buf` is borrowed for the
    /// whole call, so the buffer cronet writes into stays valid.
    pub fn read(&mut self, buf: &mut [u8]) -> std::result::Result<i32, CronetError> {
        {
            let g = self.shared.mutex.lock().unwrap();
            match g.terminal {
                Some(Terminal::Succeeded) => return Ok(0),
                Some(t) => return Err(terminal_to_err(t)),
                None => {}
            }
        }
        if buf.is_empty() {
            return Ok(0);
        }

        // Arm: clear the previous completion flag before issuing the read.
        {
            let mut g = self.shared.mutex.lock().unwrap();
            g.read_completed = false;
            g.last_read = 0;
        }

        let len = buf.len() as i32;
        let rc = unsafe { sys::bidirectional_stream_read(self.raw, buf.as_mut_ptr(), len) };
        if rc < 0 {
            return Err(CronetError::Net(NetError(rc)));
        }

        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        let mut g = self.shared.mutex.lock().unwrap();
        loop {
            if g.read_completed {
                return Ok(g.last_read);
            }
            match g.terminal {
                Some(Terminal::Succeeded) => return Ok(0),
                Some(t) => return Err(terminal_to_err(t)),
                None => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(CronetError::Config("timed out waiting for read".into()));
            }
            let (ng, _) = self.shared.cond.wait_timeout(g, deadline - now).unwrap();
            g = ng;
        }
    }

    /// Write `buf`, blocking until `on_write_completed` (or a terminal event).
    pub fn write(
        &mut self,
        buf: &[u8],
        end_of_stream: bool,
    ) -> std::result::Result<i32, CronetError> {
        {
            let g = self.shared.mutex.lock().unwrap();
            if let Some(t) = g.terminal {
                return Err(terminal_to_err(t));
            }
        }

        {
            let mut g = self.shared.mutex.lock().unwrap();
            g.write_completed = false;
        }

        let len = buf.len() as i32;
        let rc = unsafe {
            if buf.is_empty() {
                sys::bidirectional_stream_write(self.raw, std::ptr::null(), 0, end_of_stream)
            } else {
                sys::bidirectional_stream_write(self.raw, buf.as_ptr(), len, end_of_stream)
            }
        };
        if rc < 0 {
            return Err(CronetError::Net(NetError(rc)));
        }

        let deadline = Instant::now() + DEFAULT_TIMEOUT;
        let mut g = self.shared.mutex.lock().unwrap();
        loop {
            if g.write_completed {
                return Ok(buf.len() as i32);
            }
            match g.terminal {
                Some(Terminal::Succeeded) => return Ok(buf.len() as i32),
                Some(t) => return Err(terminal_to_err(t)),
                None => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(CronetError::Config("timed out waiting for write".into()));
            }
            let (ng, _) = self.shared.cond.wait_timeout(g, deadline - now).unwrap();
            g = ng;
        }
    }

    /// Flush pending writes.
    pub fn flush(&self) {
        unsafe { sys::bidirectional_stream_flush(self.raw) }
    }

    /// Cancel the stream.
    pub fn cancel(&self) {
        unsafe { sys::bidirectional_stream_cancel(self.raw) }
    }

    /// Whether a terminal event has been observed.
    fn is_terminal(&self) -> bool {
        self.shared.mutex.lock().unwrap().terminal.is_some()
    }

    /// Wait (best-effort) for any terminal event, up to `timeout`.
    fn wait_terminal(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut g = self.shared.mutex.lock().unwrap();
        while g.terminal.is_none() {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let (ng, _) = self.shared.cond.wait_timeout(g, deadline - now).unwrap();
            g = ng;
        }
        true
    }

    /// Destroy the stream.
    pub fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;

        // If still in flight, cancel and give the network thread a moment to
        // deliver the terminal callback (which reclaims the C-side Arc ref).
        if self.started && !self.is_terminal() {
            unsafe { sys::bidirectional_stream_cancel(self.raw) };
            let _ = self.wait_terminal(Duration::from_secs(2));
        }
        unsafe {
            sys::bidirectional_stream_destroy(self.raw);
        }
    }
}

impl Drop for BidirectionalStream {
    fn drop(&mut self) {
        self.destroy();
    }
}

// Safe: all interior state is either a pointer-sized handle or behind a Mutex.
unsafe impl Send for BidirectionalStream {}

// ---------------------------------------------------------------------------
// BidirectionalConn
// ---------------------------------------------------------------------------

/// A connection-oriented wrapper around `BidirectionalStream`.
///
/// Implements `Read` and `Write` for use as an I/O stream.
pub struct BidirectionalConn {
    stream: BidirectionalStream,
    closed: bool,
}

impl BidirectionalConn {
    pub fn new(stream_engine: &BidirectionalStreamEngine) -> Self {
        Self {
            stream: BidirectionalStream::new(stream_engine.raw, None),
            closed: false,
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
        self.stream.start(method, url, headers, priority, end_of_stream)
    }

    /// Wait for response headers (handshake).
    pub fn wait_for_headers(
        &mut self,
    ) -> std::result::Result<HashMap<String, String>, CronetError> {
        self.stream.wait_for_headers(DEFAULT_TIMEOUT)
    }

    /// Close the connection.
    pub fn close(&mut self) -> std::result::Result<(), CronetError> {
        if !self.closed {
            self.closed = true;
            self.stream.cancel();
        }
        Ok(())
    }

    /// Flush the underlying stream.
    pub fn flush(&self) {
        self.stream.flush();
    }
}

impl Read for BidirectionalConn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.closed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "connection closed",
            ));
        }
        match self.stream.read(buf) {
            Ok(n) => Ok(n as usize),
            // A clean remote close surfaces as EOF, not an error.
            Err(CronetError::ConnectionClosed) => Ok(0),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
        }
    }
}

impl Write for BidirectionalConn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.closed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "connection closed",
            ));
        }
        self.stream
            .write(buf, false)
            .map(|_| buf.len())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush();
        Ok(())
    }
}

impl Drop for BidirectionalConn {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
