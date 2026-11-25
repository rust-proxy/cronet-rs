//! Cronet Engine - manages the Cronet network stack lifecycle.
//!
//! Maps to cronet-go's engine_*.go files. The Engine is the core object
//! that manages network sessions, HTTP/2 and QUIC connections.

use std::ffi::CStr;

use crate::engine_params::EngineParams;
use crate::error::{CronetError, CronetResult};
use crate::sys;
use crate::BidirectionalStreamEngine;

/// A dialer function for TCP connections. Receives address and port, returns
/// a file descriptor (>= 0) or a net_error code (< 0).
pub type Dialer = Box<dyn Fn(&str, u16) -> i32 + Send + 'static>;

/// A dialer function for UDP connections. Returns (fd, local_address, local_port)
/// or (net_error, "", 0).
pub type UdpDialer = Box<dyn Fn(&str, u16) -> (i32, String, u16) + Send + 'static>;

/// The Cronet Engine. Manages the network stack.
#[derive(Debug)]
pub struct Engine {
    pub(crate) raw: sys::RawEngine,
    started: bool,
    destroyed: bool,
    dialer_context: Option<Box<crate::socket_dialer::DialerContext>>,
}

impl Engine {
    /// Create a new Cronet Engine instance.
    pub fn new() -> Self {
        let raw = unsafe { sys::Cronet_Engine_Create() };
        Self {
            raw,
            started: false,
            destroyed: false,
            dialer_context: None,
        }
    }

    pub(crate) fn install_socket_dialer(
        &mut self,
        dialer: std::sync::Arc<dyn crate::socket_dialer::SocketDialer>,
    ) {
        // The allocation stays at a fixed address until after native engine destruction.
        let mut context = Box::new(crate::socket_dialer::DialerContext(dialer));
        let ptr = std::ptr::from_mut(&mut *context).cast();
        unsafe {
            sys::Cronet_Engine_SetDialer(self.raw, Some(crate::socket_dialer::tcp), ptr);
            sys::Cronet_Engine_SetUdpDialer(self.raw, Some(crate::socket_dialer::udp), ptr, None);
        }
        self.dialer_context = Some(context);
    }

    /// Start the engine with the given parameters.
    pub fn start_with_params(&mut self, params: &EngineParams) -> std::result::Result<(), CronetError> {
        let r = unsafe { sys::Cronet_Engine_StartWithParams(self.raw, params.raw) };
        if r != 0 {
            return Err(CronetError::Result(CronetResult::from(r)));
        }
        self.started = true;
        Ok(())
    }

    /// Shutdown the engine. Blocks until resources are cleaned up.
    pub fn shutdown(&mut self) -> std::result::Result<(), CronetError> {
        if self.started {
            let r = unsafe { sys::Cronet_Engine_Shutdown(self.raw) };
            if r != 0 {
                return Err(CronetError::Result(CronetResult::from(r)));
            }
            self.started = false;
        }
        Ok(())
    }

    /// Close all connections managed by this engine.
    pub fn close_all_connections(&self) {
        unsafe { sys::Cronet_Engine_CloseAllConnections(self.raw) }
    }

    /// Get the engine version string.
    pub fn version(&self) -> String {
        let ptr = unsafe { sys::Cronet_Engine_GetVersionString(self.raw) };
        if ptr.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
        }
    }

    /// Get the default user agent string.
    pub fn default_user_agent(&self) -> String {
        let ptr = unsafe { sys::Cronet_Engine_GetDefaultUserAgent(self.raw) };
        if ptr.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
        }
    }

    /// Set custom trusted root certificates (PEM format). Must be called before start.
    pub fn set_trusted_root_certificates(&mut self, pem: &str) -> bool {
        let c_pem = std::ffi::CString::new(pem).unwrap_or_default();
        let verifier = unsafe { sys::Cronet_CreateCertVerifierWithRootCerts(c_pem.as_ptr()) };
        if verifier.0 == 0 {
            return false;
        }
        unsafe { sys::Cronet_Engine_SetMockCertVerifierForTesting(self.raw, verifier) }
        true
    }

    /// Set a custom TCP dialer callback.
    pub fn set_dialer(&mut self, _dialer: Option<Dialer>) {
        match _dialer {
            None => unsafe { sys::Cronet_Engine_SetDialer(self.raw, None, std::ptr::null_mut()) },
            Some(_d) => {
                log::warn!("SetDialer: not fully implemented in purego mode yet");
            }
        }
    }

    /// Set a custom UDP dialer callback.
    pub fn set_udp_dialer(&mut self, _dialer: Option<UdpDialer>) {
        match _dialer {
            None => unsafe {
                sys::Cronet_Engine_SetUdpDialer(self.raw, None, std::ptr::null_mut(), None)
            },
            Some(_d) => {
                log::warn!("SetUdpDialer: not fully implemented in purego mode yet");
            }
        }
    }

    /// Get the BidirectionalStreamEngine for creating streams.
    pub fn stream_engine(&self) -> BidirectionalStreamEngine {
        let raw = unsafe { sys::Cronet_Engine_GetStreamEngine(self.raw) };
        BidirectionalStreamEngine::from_raw(raw)
    }

    /// Destroy the engine, freeing underlying C resources.
    pub fn destroy(&mut self) {
        if !self.destroyed {
            self.destroyed = true;
            unsafe { sys::Cronet_Engine_Destroy(self.raw) }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if !self.destroyed {
            if self.started {
                let _ = self.shutdown();
            }
            self.destroy();
        }
    }
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires libcronet shared library"]
    fn test_create_engine() {
        let engine = Engine::new();
        assert_ne!(engine.raw.0, 0);
        assert_eq!(engine.version(), "154.0.8037.49");
    }
}
