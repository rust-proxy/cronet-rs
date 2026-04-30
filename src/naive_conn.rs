//! NaiveConn - a padded connection over Cronet's bidirectional stream.
//!
//! Maps to cronet-go's naive_conn.go. Wraps a BidirectionalConn with
//! the NaiveProxy padding protocol and provides the handshake logic.

use std::io::{Read, Write};

use crate::bidirectional::BidirectionalConn;
use crate::error::CronetError;
use crate::padding::PaddingManager;

/// The NaiveConn interface: a TCP-like connection over Cronet with padding.
pub struct NaiveConn {
    inner: BidirectionalConn,
    padding: PaddingManager,
    handshake_done: bool,
    handshake_status: Option<u16>,
}

impl NaiveConn {
    pub fn new(inner: BidirectionalConn) -> Self {
        Self {
            inner,
            padding: PaddingManager::new(),
            handshake_done: false,
            handshake_status: None,
        }
    }

    /// Perform the handshake by waiting for response headers.
    /// Returns the HTTP status code.
    pub fn handshake(&mut self) -> std::result::Result<u16, CronetError> {
        if self.handshake_done {
            return self
                .handshake_status
                .ok_or(CronetError::Config("handshake already done".into()));
        }

        let headers = self.inner.wait_for_headers()?;
        let status: u16 = headers
            .get(":status")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| CronetError::BadStatus("missing :status header".into()))?;

        self.handshake_done = true;
        self.handshake_status = Some(status);

        if status != 200 {
            return Err(CronetError::BadStatus(format!("{}", status)));
        }

        log::debug!("handshake succeeded with status 200");
        Ok(status)
    }

    /// Generate a random padding header value (string form for HTTP headers).
    pub fn generate_padding_header() -> Vec<u8> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let padding_len = rng.gen_range(30..62);
        let mut padding = vec![0u8; padding_len];
        let chars: &[u8] = b"!#$()+<>?@[]^`{}";
        for i in 0..16.min(padding_len) {
            padding[i] = chars[rng.gen_range(0..chars.len())];
        }
        for i in 16..padding_len {
            padding[i] = b'~';
        }
        padding
    }
}

impl Read for NaiveConn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.handshake_done {
            self.handshake().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
        }
        self.padding.read_with_padding(&mut self.inner, buf).map(|r| r.0)
    }
}

impl Write for NaiveConn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.handshake_done {
            self.handshake().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?;
        }
        self.padding.write_with_padding(&mut self.inner, buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush();
        Ok(())
    }
}

impl Drop for NaiveConn {
    fn drop(&mut self) {
        let _ = self.inner.close();
    }
}
