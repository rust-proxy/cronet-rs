//! Raw FFI bindings for the Cronet C API.
//!
//! These live in the separate [`cronet-sys`] crate (which also owns native
//! library resolution and the optional prebuilt-binary download). This module
//! re-exports them so existing `cronet_rs::sys::*` / `crate::sys::*` paths keep
//! working unchanged.
//!
//! [`cronet-sys`]: https://crates.io/crates/cronet-sys
pub use cronet_sys::*;
