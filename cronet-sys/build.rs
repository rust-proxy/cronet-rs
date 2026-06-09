//! Build script for `cronet-sys`.
//!
//! Responsibilities:
//!   1. (`download` feature) Fetch the matching prebuilt `libcronet` for the
//!      target from github.com/rust-proxy/cronet-binaries into `OUT_DIR`, with
//!      mandatory SHA-256 verification against a pinned checksum. No network is
//!      touched unless this feature is enabled.
//!   2. (`static-link` feature) Emit `cargo:rustc-link-*` directives so the
//!      linker finds `libcronet.a` — either from `CRONET_LIB_DIR` or from the
//!      file just downloaded.
//!
//! In the default (dynamic) mode without `download`, this script does nothing
//! but register its rerun triggers.
//!
//! ## Relevant environment variables
//!
//! - `CRONET_LIB_DIR`         — dir containing `libcronet.a` (static mode).
//! - `CRONET_STATIC_NAME`     — static lib stem (default `cronet`).
//! - `CRONET_DOWNLOAD_TAG`    — release tag to fetch (default `latest`).
//! - `CRONET_DOWNLOAD_ASSET`  — override the auto-detected asset file name.
//! - `CRONET_DOWNLOAD_SHA256` — expected SHA-256 for a custom/unpinned asset.
//! - `CRONET_DOWNLOAD_DIR`    — cache dir for downloads (default `OUT_DIR`).

use std::env;
use std::path::PathBuf;

/// Base URL for release downloads.
#[cfg(feature = "download")]
const RELEASE_BASE: &str = "https://github.com/rust-proxy/cronet-binaries/releases/download";

/// Pinned SHA-256 checksums for the `latest` release assets.
///
/// If upstream re-rolls the rolling `latest` tag, these stop matching and the
/// build fails loudly — which is the intended safety property. Update this
/// table (and re-test) when intentionally bumping to a new upstream build.
#[cfg(feature = "download")]
const CHECKSUMS: &[(&str, &str)] = &[
    ("libcronet-android-386.a", "fe78e73d352a1b2bafaba474b017ebc7ecf4a892f30dea1838f9f6c2ce49e8ac"),
    ("libcronet-android-amd64.a", "4b83989eec6e2d1fe14bc5a8ae180d3e6fe98797eb232e7b83b062020dda7a3e"),
    ("libcronet-android-arm.a", "6160cb2194f0bf3cc70098f0f310f6dd5b19ba75e9556a2d67f5d24c06d9d96d"),
    ("libcronet-android-arm64.a", "254e8b23a5ec3f554d2e963463d96f631582fb3ae7b1f236cbdefe2cb108c53f"),
    ("libcronet-darwin-amd64.a", "2728f2d8cd1792bc8fdb9c720d1d49fcd8d48ea608e5d444a81be2620f432b40"),
    ("libcronet-darwin-arm64.a", "828f9ce5595d3aa2c9be90e64e4e1b5f9120be70c101e1dbbdde4ef81cf67eed"),
    ("libcronet-ios-amd64_simulator.a", "2a542644a687090b31d64904cc71fae3dfa488f7377e3684036273179c0ab227"),
    ("libcronet-ios-arm64.a", "efe1aeb08c390fd3ee452eca7ac8062fc5c4abe5aec402796fa45d8119d72c6f"),
    ("libcronet-ios-arm64_simulator.a", "4d3d68e4ecaeff1b2d416733e875ef8cdd97c5fcd79e6f7e7211c7bb3197699e"),
    ("libcronet-linux-386.so", "d2ab76c35092e41c78c64296dc8bcdd861ccd7846a9f671664257c468b1dff8a"),
    ("libcronet-linux-amd64.so", "436fbbea68293ca4b844e0d24bd61dc7e7d55ef08af47ab85b6d0f99f05f2129"),
    ("libcronet-linux-arm.so", "d82e7eafb94664f9ca9a14dee38b8a5c569e31a77e6c3728ab3d80856b8e6146"),
    ("libcronet-linux-arm64.so", "288341d94a086696e586002044f3167b4930867458d96741e19e8046429ae45c"),
    ("libcronet-linux-loong64.so", "94db3312d2b70afca26ebc780df72573a8e39cdfe26e714a1c48ee0c7b73f0d4"),
    ("libcronet-linux-mips64le.so", "5e811191d4b819bf7fae0a0cced3d566284b482b555b8cd06bfbb616f4e7e5dd"),
    ("libcronet-linux-mipsle.so", "447829433242383b9bcfe145c8cc1b106b46ab85ad6beb27f15f91440cde777e"),
    ("libcronet-linux-riscv64.so", "d23eb04668e5a6192a26633c503ce0b569f6357576a5a6bdcd64acc9b0824a99"),
    ("libcronet-tvos-amd64_simulator.a", "588e307e6dda7e9bc03f05cdd4c10d6bf6eda5f3c765f8bb0db9b2c5201560b0"),
    ("libcronet-tvos-arm64.a", "4ad132d40d764ea898d1cf06f8d90fbb6303a9840a1fc85af029a7eec1cc163e"),
    ("libcronet-tvos-arm64_simulator.a", "df22bb6c9ea9f5cf0b1310475a0cf579f112de0c97dae1c5c154088bc87f8234"),
    ("libcronet-windows-amd64.dll", "cff72d7636b40be5253c04ab35655f5f21b08b0fa8706936150e753ea6964774"),
    ("libcronet-windows-arm64.dll", "0f290db12a5cf36018dbbf878171c285096a3d99ff069032870cca1b44c28c1c"),
]; // keep sorted by name

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for var in [
        "CRONET_LIB_DIR",
        "CRONET_STATIC_NAME",
        "CRONET_DOWNLOAD_TAG",
        "CRONET_DOWNLOAD_ASSET",
        "CRONET_DOWNLOAD_SHA256",
        "CRONET_DOWNLOAD_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }

    let static_link = env::var_os("CARGO_FEATURE_STATIC_LINK").is_some();

    // 1. Optionally download (and verify) the prebuilt for this target.
    let downloaded = maybe_download();

    // Expose the on-disk path to the crate so dynamic-mode users can load it.
    if let Some(path) = &downloaded {
        println!("cargo:rustc-env=CRONET_DOWNLOADED_LIB_PATH={}", path.display());
    }

    // 2. Static-link wiring.
    if static_link {
        configure_static_link(downloaded.as_deref());
    }
}

/// Map the current target to the release asset file name.
///
/// Returns `None` for targets we don't have a mapping for — callers should then
/// require `CRONET_DOWNLOAD_ASSET` to be set explicitly.
#[cfg(feature = "download")]
fn asset_name() -> Option<String> {
    if let Ok(explicit) = env::var("CRONET_DOWNLOAD_ASSET") {
        return Some(explicit);
    }

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let abi = env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default();

    let rel_arch = match arch.as_str() {
        "x86" => "386",
        "x86_64" => "amd64",
        "arm" => "arm",
        "aarch64" => "arm64",
        "loongarch64" => "loong64",
        "riscv64" => "riscv64",
        "mips64" => "mips64le",
        "mips" => "mipsle",
        _ => return None,
    };

    let name = match os.as_str() {
        "linux" => format!("libcronet-linux-{rel_arch}.so"),
        "windows" => format!("libcronet-windows-{rel_arch}.dll"),
        "android" => format!("libcronet-android-{rel_arch}.a"),
        "macos" => format!("libcronet-darwin-{rel_arch}.a"),
        "ios" | "tvos" => {
            // Simulator targets: `*-apple-ios-sim` sets target_abi=sim, and the
            // x86_64 Apple-mobile target is always a simulator.
            let is_sim = abi == "sim" || arch == "x86_64";
            let slot = if is_sim {
                format!("{rel_arch}_simulator")
            } else {
                rel_arch.to_string()
            };
            format!("libcronet-{os}-{slot}.a")
        }
        _ => return None,
    };
    Some(name)
}

#[cfg(feature = "download")]
fn expected_sha(asset: &str) -> Option<&'static str> {
    CHECKSUMS
        .iter()
        .find(|(name, _)| *name == asset)
        .map(|(_, sha)| *sha)
}

#[cfg(not(feature = "download"))]
fn maybe_download() -> Option<PathBuf> {
    None
}

#[cfg(feature = "download")]
fn maybe_download() -> Option<PathBuf> {
    use std::io::Read;

    let asset = asset_name().unwrap_or_else(|| {
        panic!(
            "cronet-sys: cannot determine prebuilt asset for target os={:?} arch={:?}; \
             set CRONET_DOWNLOAD_ASSET explicitly",
            env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
        )
    });

    let tag = env::var("CRONET_DOWNLOAD_TAG").unwrap_or_else(|_| "latest".to_string());

    // Determine the expected checksum: pinned table first, env override second.
    let expected = expected_sha(&asset)
        .map(str::to_string)
        .or_else(|| env::var("CRONET_DOWNLOAD_SHA256").ok())
        .unwrap_or_else(|| {
            panic!(
                "cronet-sys: no pinned SHA-256 for asset {asset:?} and \
                 CRONET_DOWNLOAD_SHA256 is unset; refusing to use an unverified binary"
            )
        });

    let cache_dir = env::var("CRONET_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("OUT_DIR").unwrap()));
    std::fs::create_dir_all(&cache_dir).expect("create download cache dir");
    let dest = cache_dir.join(&asset);

    // Reuse a previously-downloaded, still-valid copy.
    if dest.exists() {
        if let Ok(bytes) = std::fs::read(&dest) {
            if sha256_hex(&bytes) == expected {
                println!("cargo:warning=cronet-sys: using cached {asset}");
                return Some(dest);
            }
        }
        // stale/corrupt — fall through and re-download
    }

    let url = format!("{RELEASE_BASE}/{tag}/{asset}");
    println!("cargo:warning=cronet-sys: downloading {url}");

    let agent = ureq::AgentBuilder::new().redirects(10).build();
    let resp = agent
        .get(&url)
        .call()
        .unwrap_or_else(|e| panic!("cronet-sys: download of {url} failed: {e}"));

    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("cronet-sys: reading {url} failed: {e}"));

    let got = sha256_hex(&bytes);
    assert_eq!(
        got, expected,
        "cronet-sys: SHA-256 mismatch for {asset}\n  expected: {expected}\n  got:      {got}\n\
         The upstream release may have changed; verify and update the pinned checksum."
    );

    // Write atomically: temp file in the same dir, then rename.
    let tmp = dest.with_extension("download.tmp");
    std::fs::write(&tmp, &bytes).expect("write downloaded binary");
    std::fs::rename(&tmp, &dest).expect("finalize downloaded binary");
    Some(dest)
}

#[cfg(feature = "download")]
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Emit link directives for static linking.
fn configure_static_link(downloaded: Option<&std::path::Path>) {
    let lib_name = env::var("CRONET_STATIC_NAME").unwrap_or_else(|_| "cronet".to_string());

    // Resolve the directory that holds `lib<name>.a`.
    let lib_dir: Option<PathBuf> = if let Ok(dir) = env::var("CRONET_LIB_DIR") {
        Some(PathBuf::from(dir))
    } else if let Some(dl) = downloaded.filter(|p| p.extension().is_some_and(|e| e == "a")) {
        // Stage the downloaded archive as `lib<name>.a` in OUT_DIR so the
        // linker's `-l<name>` finds it.
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let staged = out_dir.join(format!("lib{lib_name}.a"));
        std::fs::copy(dl, &staged).expect("stage downloaded static lib");
        Some(out_dir)
    } else {
        None
    };

    if let Some(dir) = lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rustc-link-lib=static={lib_name}");

    // Platform-specific runtime/system dependencies of libcronet.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "macos" | "ios" | "tvos" => {
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        }
        "android" => {
            // Statically link libc++ so the artifact doesn't depend on a
            // specific NDK runtime version.
            println!("cargo:rustc-link-lib=static=c++_static");
            if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
                let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
                let candidates: &[&str] = match arch.as_str() {
                    "aarch64" => &["aarch64-linux-android"],
                    "x86_64" => &["x86_64-linux-android"],
                    "arm" => &["armv7a-linux-androideabi", "arm-linux-androideabi"],
                    "x86" => &["i686-linux-android"],
                    _ => &[],
                };
                let base = std::path::Path::new(&ndk_home)
                    .join("toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib");
                for c in candidates {
                    let p = base.join(c);
                    if p.exists() {
                        println!("cargo:rustc-link-search=native={}", p.display());
                        break;
                    }
                }
            }
        }
        _ => {}
    }
}
