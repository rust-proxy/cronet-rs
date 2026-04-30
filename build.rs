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

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    println!("cargo:rerun-if-env-changed=CRONET_LIB_DIR");
    println!("cargo:rerun-if-env-changed=CRONET_STATIC_NAME");

    // Library search path
    if let Ok(lib_dir) = env::var("CRONET_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);
    }

    // Library name
    let lib_name = env::var("CRONET_STATIC_NAME").unwrap_or_else(|_| "cronet".to_string());
    println!("cargo:rustc-link-lib=static={}", lib_name);

    // --- Platform-specific link dependencies ---

    match target_os.as_str() {
        "macos" | "ios" => {
            // Apple platforms: required system frameworks
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        }
        "android" => {
            // Android NDK: statically link libc++ so the .apk doesn't depend on
            // a specific NDK runtime version.
            println!("cargo:rustc-link-lib=static=c++_static");

            // Resolve NDK c++ library directory for the linker.
            // libc++_static.a lives under:
            //   $ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/<target>
            if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
                let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
                let target_vendor = env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default();
                let ndk_candidates: &[&str] = match (target_arch.as_str(), target_vendor.as_str()) {
                    ("aarch64", _) => &["aarch64-linux-android"],
                    ("x86_64", _) => &["x86_64-linux-android"],
                    ("arm", _) => &["armv7a-linux-androideabi", "arm-linux-androideabi"],
                    ("x86", _) => &["i686-linux-android"],
                    _ => &[],
                };
                let ndk_base = std::path::Path::new(&ndk_home)
                    .join("toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib");
                for candidate in ndk_candidates {
                    let cxx_path = ndk_base.join(candidate);
                    if cxx_path.exists() {
                        println!("cargo:rustc-link-search=native={}", cxx_path.display());
                        break;
                    }
                }
            }
        }
        _ => {
            // Linux, Windows: libcronet is built with C++ so we may need the
            // C++ standard library. On glibc Linux this is normally linked
            // automatically by the compiler driver; on musl or Windows static
            // mode it may need explicit handling.
            if target_os == "linux" {
                // For fully static musl builds, the user may need:
                //   RUSTFLAGS="-C target-feature=+crt-static"
                // libstdc++ is pulled in automatically by rustc.
            }
        }
    }

    println!("cargo:rustc-cfg=cronet_static_link");
}
