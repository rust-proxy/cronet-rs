//! Build script for `cronet-sys`.
//!
//! Responsibilities:
//!   1. (`download` feature) Fetch the matching prebuilt `libcronet` for the
//!      target from github.com/rust-proxy/cronet-binaries into `OUT_DIR`, with
//!      mandatory SHA-256 verification against a pinned checksum. No network is
//!      touched unless `download`, or both `download-static` and `static-link`,
//!      are enabled.
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
//! - `CRONET_DOWNLOAD_TAG`    — release tag to fetch (default `static-20261116.1`).
//! - `CRONET_DOWNLOAD_ASSET`  — override the auto-detected asset file name.
//! - `CRONET_DOWNLOAD_SHA256` — expected SHA-256 for a custom/unpinned asset.
//! - `CRONET_DOWNLOAD_LINK_FLAGS_SHA256` — expected SHA-256 for Windows link flags.
//! - `CRONET_DOWNLOAD_DIR`    — cache dir for downloads (default `OUT_DIR`).

mod build_target;

use std::env;
use std::path::PathBuf;

/// Base URL for release downloads.
#[cfg(any(feature = "download", feature = "download-static"))]
const RELEASE_BASE: &str = "https://github.com/rust-proxy/cronet-binaries/releases/download";

/// Pinned SHA-256 checksums for the fixed `static-20261116.1` release.
/// Update the release tag and this table together when adopting a new build.
#[cfg(any(feature = "download", feature = "download-static"))]
#[rustfmt::skip]
const CHECKSUMS: &[(&str, &str)] = &[
    ("libcronet-android-386.a", "edf35fc0c5c7f01230d0b8024bc735154528bc1f48dee59d8bd55f2f76a62fba"),
    ("libcronet-android-amd64.a", "e301077dc89f061fedeb9531cedd3041caac7fe21f3b1e06631773b289ce0f93"),
    ("libcronet-android-arm.a", "7e600d9e38f9343313d48b536f19739ca228cb096ff866eae3eb7b71086c2d66"),
    ("libcronet-android-arm64.a", "6970194fd468fbe92906c3e4cdbf14f053bdc59a7185ae7ee05442ed75e88229"),
    ("libcronet-darwin-amd64.a", "ddaa895794766013347330fe80edc59fe96e6a95fca2cee4d027497c8d75906b"),
    ("libcronet-darwin-arm64.a", "e901fe7956d6b92dd385c313f8695a6f4a1989ca69320fe71cd2c434481ada13"),
    ("libcronet-ios-arm64.a", "7b03c4f8cf864e52d92f6d4db92adc04afcb7d5b112cbdfdd13fa2622130fb21"),
    ("libcronet-ios-arm64_simulator.a", "3a2cab4a6e3fee750ccabb21d54d4cac146080ad692d5b134c08ab99b6a93581"),
    ("libcronet-linux-386.a", "51b9a27fa7c2bcff80734d7baf94afb8d2d6376bab73e945317ade8c66e39a6f"),
    ("libcronet-linux-386.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-386.so", "ba4cdbbaf787d5eca0c965979e485b6f8b1415112f9de900584a5768be05e516"),
    ("libcronet-linux-386_musl.a", "a63a516cba1f1a21bfa4e4e9ed1422561bfd21970e93064f2c515da97724c4c2"),
    ("libcronet-linux-386_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-amd64.a", "97392860f73550b284690f8cc28cb913b063a3cb68f47b101f0fd8462d70636f"),
    ("libcronet-linux-amd64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-amd64.so", "c0cdd7dcfa9101ff49f93918c754a87e3b4d0c47016cf5bd995ec4a251393a35"),
    ("libcronet-linux-amd64_musl.a", "d95aa89201b7830cb40f4bc29474c5a0c2b47d8083fb9cac9394ab210b8982f0"),
    ("libcronet-linux-amd64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-arm.a", "c3c9d13559283395405ba2eb5b811f9f41f3481bdf339f8ac95c5c6701199522"),
    ("libcronet-linux-arm.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-arm.so", "711f399ca4595eb2f113717ea5e80c29b89e571efd7773ad7524bbd139d2e3ce"),
    ("libcronet-linux-arm64.a", "b79dbe80d9af766b3b1d087b93e38aabda1661e06f6809962293f2f000e750e2"),
    ("libcronet-linux-arm64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-arm64.so", "7e39992f7c990db32b0651f864cb3686ad9529f3f6fe57d7e4fdac0befa8dbb9"),
    ("libcronet-linux-arm64_musl.a", "246b4144891191de6f6ea6e81e1e1105841bb3adea4a825839d569f5fd4327d9"),
    ("libcronet-linux-arm64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-arm_musl.a", "f730c859c02785b7652cbdf7367f48f39c7564f821298f7afdb0609cb42369e8"),
    ("libcronet-linux-arm_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-loong64.a", "5af1b7e4c80d20d8558059f6bef929233715f7cbebc04b58b7478d3dec8cd429"),
    ("libcronet-linux-loong64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-loong64.so", "aeba1019bc832ca0c570e341c83241c24232a51118382c7e1487130495bc7422"),
    ("libcronet-linux-loong64_musl.a", "3df3f3f701318c5e593c2079351f8a3e7426a8b135b8ce1f47d61bf64fb8b31b"),
    ("libcronet-linux-loong64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-mips64le.a", "3ae32852224bbd57d32c24a6a95f62bfe478795c2a91e0bb44a3c5c40c69084f"),
    ("libcronet-linux-mips64le.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-mips64le.so", "5485c699a07022ac2e16d152dcca48954e19c55846fa0de852caabaaf51a32f7"),
    ("libcronet-linux-mipsle.a", "caccfba6248b5f46e0352e36c2b1902d41365d87523f8770f10ae97f8b7aca8d"),
    ("libcronet-linux-mipsle.link-flags.txt", "2906b769664b8caba306bd9027ea645f7831df4a60accdac9380e0ca915b31c5"),
    ("libcronet-linux-mipsle.so", "5180527913818308bbbdd695be539f417fd888627e84e46579bd5fbc1ae171a1"),
    ("libcronet-linux-mipsle_musl.a", "3a9357c2482097746ccbcc7fe2b929ee2d6741f3b09359a1d6f5cc4d2147b59e"),
    ("libcronet-linux-mipsle_musl.link-flags.txt", "7c4f3c9ad180baeaf47dc3f42d868f648bf93749e28a235ffc3981cc08379e75"),
    ("libcronet-linux-riscv64.a", "21c84da1384d5a047013ab2a02e60d61d87cfa666c00e6968100e85299aaf471"),
    ("libcronet-linux-riscv64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-riscv64.so", "7e6cede91829a868753a8018963b5090ccd8a76c90333fff2da76b84ba944826"),
    ("libcronet-linux-riscv64_musl.a", "f3da36042182bb7ddce627b836ffb2823f1ce23a76b230c81359daca30573a73"),
    ("libcronet-linux-riscv64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-tvos-amd64_simulator.a", "250fbe5134fdd684196436472dc1ffb52217fe3897aab7d54ad93e29ea47d121"),
    ("libcronet-tvos-arm64.a", "79ee7beb3c11ffa44f9d9eb445947263feae5b1ebf99090bd0d6779500c107ee"),
    ("libcronet-tvos-arm64_simulator.a", "a911824e12eecf7d514b7b697e7f3bdcc9390ff05131b9bff9fd79191ef1e076"),
    ("libcronet-windows-amd64.dll", "9b42707fe83c8d58665e120f8f94f8bc16dadf2bd3b5a5bb228719f074181161"),
    ("libcronet-windows-amd64.lib", "4e1d00228c71976fa2b74fea402712de16aaf7af52bb6511c389c7e4c8fb9db1"),
    ("libcronet-windows-amd64.link-flags.txt", "5d75a21440c384731856c6226936f931c5dc47d26e94df5c907e77531ea383eb"),
    ("libcronet-windows-amd64_md.dll", "09542ec60e059443a5236704b768ca962cadb4506782468be226b058906fb67d"),
    ("libcronet-windows-amd64_md.lib", "53196dba93d3f7309f9dd1a09053a10d04909c25b9349e7209f33850fa50e455"),
    ("libcronet-windows-amd64_md.link-flags.txt", "8c1d69cc91de13a5f7e737bedeaee89f6dbcdd918001a35aea48d3bb253edb5c"),
    ("libcronet-windows-arm64.dll", "52a6e102dd3c6fd76c598ae26984f2f7b7c49c0d22eeee46e981bfdd66f64fbf"),
    ("libcronet-windows-arm64.lib", "0d1c1e216a83ab83bb87cc4c11c5c73f10a7d90da6a1d86adad235d633f79a46"),
    ("libcronet-windows-arm64.link-flags.txt", "5d75a21440c384731856c6226936f931c5dc47d26e94df5c907e77531ea383eb"),
    ("libcronet-windows-arm64_md.dll", "53ff4b83e37f7233bd6ab101ac4c391ee9475d210e19dd66a311a6a2c54655bc"),
    ("libcronet-windows-arm64_md.lib", "8925d2055ce0f7d28aa4b47f326cd79ebbc8580427434e1c8336d2f3e813a0eb"),
    ("libcronet-windows-arm64_md.link-flags.txt", "8c1d69cc91de13a5f7e737bedeaee89f6dbcdd918001a35aea48d3bb253edb5c"),
]; // keep sorted by name

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_target.rs");
    for var in [
        "CRONET_LIB_DIR",
        "CARGO_CFG_TARGET_FEATURE",
        "ANDROID_NDK_HOME",
        "CRONET_STATIC_NAME",
        "CRONET_DOWNLOAD_TAG",
        "CRONET_DOWNLOAD_ASSET",
        "CRONET_DOWNLOAD_SHA256",
        "CRONET_DOWNLOAD_LINK_FLAGS_SHA256",
        "CRONET_DOWNLOAD_DIR",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }

    let static_link = env::var_os("CARGO_FEATURE_STATIC_LINK").is_some()
        || env::var_os("CARGO_FEATURE_DYNAMIC").is_none();

    // 1. Optionally download (and verify) the prebuilt for this target.
    // A supplied native archive takes precedence over automatic downloads.
    let downloaded = if build_target::download_enabled(
        cfg!(feature = "download"),
        cfg!(feature = "download-static"),
        cfg!(feature = "static-link"),
    ) && !(static_link && env::var_os("CRONET_LIB_DIR").is_some())
    {
        maybe_download()
    } else {
        None
    };

    // Expose the on-disk path to the crate so dynamic-mode users can load it.
    if let Some(path) = &downloaded {
        println!(
            "cargo:rustc-env=CRONET_DOWNLOADED_LIB_PATH={}",
            path.display()
        );
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
#[cfg(any(feature = "download", feature = "download-static"))]
fn asset_name() -> Option<String> {
    if let Ok(explicit) = env::var("CRONET_DOWNLOAD_ASSET") {
        return Some(explicit);
    }

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let abi = env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default();

    build_target::asset_name(
        &os,
        &arch,
        &abi,
        &env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default(),
        &env::var("CARGO_CFG_TARGET_ENDIAN").unwrap_or_default(),
        env::var_os("CARGO_FEATURE_STATIC_LINK").is_some()
            || env::var_os("CARGO_FEATURE_DYNAMIC").is_none(),
    )
    .map(|asset| {
        build_target::select_crt_asset(
            asset,
            &env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default(),
        )
    })
}

#[cfg(any(feature = "download", feature = "download-static"))]
fn expected_sha(asset: &str) -> Option<&'static str> {
    CHECKSUMS
        .iter()
        .find(|(name, _)| *name == asset)
        .map(|(_, sha)| *sha)
}

#[cfg(not(any(feature = "download", feature = "download-static")))]
fn maybe_download() -> Option<PathBuf> {
    None
}

#[cfg(any(feature = "download", feature = "download-static"))]
fn maybe_download() -> Option<PathBuf> {
    let asset = asset_name().unwrap_or_else(|| {
        panic!(
            "cronet-sys: cannot determine prebuilt asset for target os={:?} arch={:?}; \
             set CRONET_DOWNLOAD_ASSET explicitly",
            env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
        )
    });

    let archive = download_asset(&asset, "CRONET_DOWNLOAD_SHA256");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && (env::var_os("CARGO_FEATURE_STATIC_LINK").is_some()
            || env::var_os("CARGO_FEATURE_DYNAMIC").is_none())
    {
        let flags_asset = format!(
            "{}.link-flags.txt",
            asset
                .strip_suffix(".lib")
                .expect("Windows static downloads must use a .lib archive")
        );
        let flags = download_asset(&flags_asset, "CRONET_DOWNLOAD_LINK_FLAGS_SHA256");
        let staged = PathBuf::from(env::var("OUT_DIR").unwrap()).join("link_flags.txt");
        std::fs::copy(flags, staged).expect("stage verified Windows system link flags");
    }
    Some(archive)
}

#[cfg(any(feature = "download", feature = "download-static"))]
fn download_asset(asset: &str, checksum_env: &str) -> PathBuf {
    use std::io::Read;

    let tag = env::var("CRONET_DOWNLOAD_TAG").unwrap_or_else(|_| "static-20261116.1".to_string());

    // Determine the expected checksum: pinned table first, env override second.
    let expected = expected_sha(asset)
        .map(str::to_string)
        .or_else(|| env::var(checksum_env).ok())
        .unwrap_or_else(|| {
            panic!(
                "cronet-sys: no pinned SHA-256 for asset {asset:?} and \
                 {checksum_env} is unset; refusing to use an unverified binary"
            )
        });

    let cache_dir = env::var("CRONET_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("OUT_DIR").unwrap()));
    std::fs::create_dir_all(&cache_dir).expect("create download cache dir");
    let dest = cache_dir.join(asset);

    // Reuse a previously-downloaded, still-valid copy.
    if let Ok(bytes) = std::fs::read(&dest)
        && sha256_hex(&bytes) == expected
    {
        println!("cargo:warning=cronet-sys: using cached {asset}");
        return dest;
    }
    // Missing, stale or corrupt cache entries must be downloaded and verified.

    let url = format!("{RELEASE_BASE}/{tag}/{asset}");
    println!("cargo:warning=cronet-sys: downloading {url}");

    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .redirects(10)
        .timeout(std::time::Duration::from_secs(120))
        .build();
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
    dest
}

#[cfg(any(feature = "download", feature = "download-static"))]
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
    } else if let Some(dl) =
        downloaded.filter(|p| p.extension().is_some_and(|e| e == "a" || e == "lib"))
    {
        // Stage the downloaded archive as `lib<name>.a` in OUT_DIR so the
        // linker's `-l<name>` finds it.
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let filename = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            format!("{lib_name}.lib")
        } else {
            format!("lib{lib_name}.a")
        };
        let staged = out_dir.join(filename);
        std::fs::copy(dl, &staged).expect("stage downloaded static lib");
        Some(out_dir)
    } else {
        None
    };

    if let Some(dir) = &lib_dir {
        println!("cargo:rustc-link-search=native={}", dir.display());
        let filename = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            format!("{lib_name}.lib")
        } else {
            format!("lib{lib_name}.a")
        };
        println!("cargo:rerun-if-changed={}", dir.join(filename).display());
    }
    println!("cargo:rustc-link-lib=static={lib_name}");

    // Platform-specific runtime/system dependencies of libcronet.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "linux" => {
            // Chromium bundles its C++ runtime in the complete archive.
            for lib in
                build_target::linux_libraries(&env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default())
            {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        "windows" => {
            assert_eq!(
                env::var("CARGO_CFG_TARGET_ENV").as_deref(),
                Ok("msvc"),
                "cronet-sys: Windows static archives require an MSVC target"
            );
            let flags = lib_dir
                .as_ref()
                .expect("set CRONET_LIB_DIR for Windows static linking")
                .join("link_flags.txt");
            println!("cargo:rerun-if-changed={}", flags.display());
            let contents = std::fs::read_to_string(&flags)
                .expect("provide the link_flags.txt generated alongside cronet.lib");
            let libraries =
                build_target::windows_libraries(&contents).expect("invalid Windows link_flags.txt");
            let crt_static = env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap_or_default()
                .split(',')
                .any(|feature| feature == "crt-static");
            build_target::validate_windows_crt(&libraries, crt_static)
                .expect("Cronet and Rust must use matching Windows CRTs");
            for lib in libraries {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
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
                let host = env::var("HOST").unwrap_or_default();
                let prebuilt = if host.contains("windows") {
                    "windows-x86_64"
                } else if host.contains("apple") {
                    "darwin-x86_64"
                } else {
                    "linux-x86_64"
                };
                let base = std::path::Path::new(&ndk_home)
                    .join("toolchains/llvm/prebuilt")
                    .join(prebuilt)
                    .join("sysroot/usr/lib");
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
