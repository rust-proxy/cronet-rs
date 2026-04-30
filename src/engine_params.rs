//! EngineParams - configuration for the Cronet Engine.
//!
//! Maps to cronet-go's engine_params_*.go files. Provides a builder-style API
//! for configuring the engine before starting.

use std::ffi::CString;

use crate::error::CronetError;
use crate::sys;

/// HTTP cache mode for the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum HttpCacheMode {
    Disabled = 0,
    InMemory = 1,
    Disk = 2,
    DiskNoHttp = 3,
}

/// Configuration for a Cronet Engine. Created via [`EngineParamsBuilder`].
#[derive(Debug)]
pub struct EngineParams {
    pub(crate) raw: sys::RawEngineParams,
    destroyed: bool,
}

impl EngineParams {
    /// Create a new engine params object. Use [`EngineParamsBuilder`] for configuration.
    pub fn new() -> Self {
        let raw = unsafe { sys::Cronet_EngineParams_Create() };
        Self { raw, destroyed: false }
    }

    /// Destroy the underlying C object. Must be called to avoid memory leaks.
    pub fn destroy(&mut self) {
        if !self.destroyed {
            self.destroyed = true;
            unsafe { sys::Cronet_EngineParams_Destroy(self.raw) }
        }
    }

    // ------------------------------------------------------------------
    // Setters
    // ------------------------------------------------------------------

    pub fn set_enable_quic(&mut self, enable: bool) {
        unsafe { sys::Cronet_EngineParams_enable_quic_set(self.raw, enable) }
    }

    pub fn set_enable_http2(&mut self, enable: bool) {
        unsafe { sys::Cronet_EngineParams_enable_http2_set(self.raw, enable) }
    }

    pub fn set_enable_brotli(&mut self, enable: bool) {
        unsafe { sys::Cronet_EngineParams_enable_brotli_set(self.raw, enable) }
    }

    pub fn set_user_agent(&mut self, user_agent: &str) -> std::result::Result<(), CronetError> {
        let c = CString::new(user_agent)
            .map_err(|_| CronetError::Config("user_agent contains null".into()))?;
        unsafe { sys::Cronet_EngineParams_user_agent_set(self.raw, c.as_ptr()) }
        Ok(())
    }

    pub fn set_experimental_options(&mut self, json: &str) -> std::result::Result<(), CronetError> {
        let c = CString::new(json)
            .map_err(|_| CronetError::Config("experimental options contains null".into()))?;
        unsafe { sys::Cronet_EngineParams_experimental_options_set(self.raw, c.as_ptr()) }
        Ok(())
    }

    pub fn set_http_cache_mode(&mut self, mode: HttpCacheMode) {
        unsafe { sys::Cronet_EngineParams_http_cache_mode_set(self.raw, mode as i32) }
    }

    pub fn set_http_cache_max_size(&mut self, max_size: i64) {
        unsafe { sys::Cronet_EngineParams_http_cache_max_size_set(self.raw, max_size) }
    }
}

impl Drop for EngineParams {
    fn drop(&mut self) {
        if !self.destroyed {
            self.destroy();
        }
    }
}

/// Builder for EngineParams.
#[derive(Debug, Default)]
pub struct EngineParamsBuilder {
    enable_quic: bool,
    enable_http2: bool,
    enable_brotli: bool,
    user_agent: Option<String>,
    experimental_options: Option<String>,
    http_cache_mode: Option<HttpCacheMode>,
    http_cache_max_size: Option<i64>,
}

impl EngineParamsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn quic(mut self, enable: bool) -> Self {
        self.enable_quic = enable;
        self
    }

    pub fn http2(mut self, enable: bool) -> Self {
        self.enable_http2 = enable;
        self
    }

    pub fn brotli(mut self, enable: bool) -> Self {
        self.enable_brotli = enable;
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    pub fn experimental_options(mut self, json: impl Into<String>) -> Self {
        self.experimental_options = Some(json.into());
        self
    }

    pub fn http_cache_mode(mut self, mode: HttpCacheMode) -> Self {
        self.http_cache_mode = Some(mode);
        self
    }

    pub fn http_cache_max_size(mut self, size: i64) -> Self {
        self.http_cache_max_size = Some(size);
        self
    }

    /// Build the EngineParams, consuming the builder.
    pub fn build(self) -> std::result::Result<EngineParams, CronetError> {
        let mut params = EngineParams::new();

        params.set_enable_quic(self.enable_quic);
        params.set_enable_http2(self.enable_http2);
        params.set_enable_brotli(self.enable_brotli);

        if let Some(ref ua) = self.user_agent {
            params.set_user_agent(ua)?;
        }
        if let Some(ref json) = self.experimental_options {
            params.set_experimental_options(json)?;
        }
        if let Some(mode) = self.http_cache_mode {
            params.set_http_cache_mode(mode);
        }
        if let Some(size) = self.http_cache_max_size {
            params.set_http_cache_max_size(size);
        }

        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires libcronet shared library"]
    fn test_builder_defaults() {
        let params = EngineParamsBuilder::new().build().unwrap();
        assert!(!params.destroyed);
    }
}
