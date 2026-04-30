//! Build script for cronet-rs.
//!
//! In static-link mode, emits `cargo:rustc-link-lib` directives so the linker
//! finds `libcronet.a` at compile time. Uses environment variables:
//!
//! - `CRONET_LIB_DIR` — path to the directory containing `libcronet.a`
//! - `CRONET_STATIC_NAME` — library stem (default `cronet`, resolves to `libcronet.a`)

use std::env;

fn main() {
    // Static-link mode: tell rustc where to find libcronet.a
    // In dynamic mode, the library is loaded at runtime — no build script needed.
    let has_static = env::var("CARGO_FEATURE_STATIC_LINK").is_ok();

    if !has_static {
        // In dynamic mode, just pass through. No linking needed.
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    println!("cargo:rerun-if-env-changed=CRONET_LIB_DIR");
    println!("cargo:rerun-if-env-changed=CRONET_STATIC_NAME");

    // Library search path
    if let Ok(lib_dir) = env::var("CRONET_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);
    }

    // Library name
    let lib_name = env::var("CRONET_STATIC_NAME").unwrap_or_else(|_| "cronet".to_string());
    println!("cargo:rustc-link-lib=static={}", lib_name);

    // On macOS, also link necessary frameworks
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" || target_os == "ios" {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
    }

    println!("cargo:rustc-cfg=cronet_static_link");
}
