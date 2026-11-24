//! Build script for `cronet-sys`.
//!
//! Responsibilities:
//!   1. (`download` feature) Fetch the matching prebuilt `libcronet` for the
//!      target from github.com/rust-proxy/cronet-binaries into `OUT_DIR`, with
//!      mandatory SHA-256 verification against a pinned checksum. No network is
//!      touched unless `download`, or `download-static` with the static backend,
//!      is enabled.
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
//! - `CRONET_DOWNLOAD_TAG`    — release tag to fetch (default `chromium-154.0.8037.49-1`).
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

/// Pinned SHA-256 checksums for the fixed `chromium-154.0.8037.49-1` release.
/// Update the release tag and this table together when adopting a new build.
#[cfg(any(feature = "download", feature = "download-static"))]
#[rustfmt::skip]
const CHECKSUMS: &[(&str, &str)] = &[
    ("libcronet-android-386.a", "c3427955d6ddf603bc572a9e9dc546972a5f5ad195a20289fab09b5365220991"),
    ("libcronet-android-amd64.a", "f015e90bd4ed965580215f36a493cd7f9346b5355bb5e164acec9109b0fa2f82"),
    ("libcronet-android-arm.a", "7718d4f5574b5e7a552790c38c3daedeb2ca6c499fce4f0c81d1409de3cdc6c1"),
    ("libcronet-android-arm64.a", "e5d6e58a6aa24287358376a76fd280b4d8d62472c4b215dbb853d3a27b2722ae"),
    ("libcronet-darwin-amd64.a", "dddb764dad4ea2008699f6b9dd0b70d3042d917097bd657200b149e3446307b2"),
    ("libcronet-darwin-arm64.a", "3c03d46121140a66044dc42b17197ac046870b945cbb2ef292223645a17d6034"),
    ("libcronet-ios-amd64_simulator.a", "090d37b81c9e9c028607ba129b6d3eba6d11a8e813cc0daa6baf248659da666f"),
    ("libcronet-ios-arm64.a", "825811d26ad6eaf6dfe9041ed7e097eb4412acaa1f9012226a30f94d9a6262a7"),
    ("libcronet-ios-arm64_simulator.a", "c9642da81f761c2a859cbf6ef97b41889a42fee27d3dac262c966914c3992582"),
    ("libcronet-linux-386.a", "62c5c85c34cf5b90796d98a25074bc4005a073a7a94a805b332ed357e6ca67fa"),
    ("libcronet-linux-386.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-386.so", "d15e359cd27f6a04313af363aa092eda1d02fe8b8131dce7ee51a860943fab56"),
    ("libcronet-linux-386_musl.a", "33e257b9cf9cc13e6673165c8855981adc46d3017dabcb47eed15f5d37805a00"),
    ("libcronet-linux-386_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-amd64.a", "b5819a90e0628daf23a4e5f72c085b5fd297090964556291bb1c045cf5892b68"),
    ("libcronet-linux-amd64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-amd64.so", "e66882e193318719f2ab880ef54e27fd0fd4e824a6d7829074770758af0e9409"),
    ("libcronet-linux-amd64_musl.a", "b16d40eb14b8ef7780336a271b7a1ceeecbb3bd5a41086c299f904829e1abb1e"),
    ("libcronet-linux-amd64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-arm.a", "8faff0742e7a3c8b5d73c222dcd0815ba0f02b7c8f6289adbdb288baafe67d82"),
    ("libcronet-linux-arm.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-arm.so", "b0ea1dc4ffe88e33b2912d5d950b2388632930f9f1f115e534a754a0f10bdaf4"),
    ("libcronet-linux-arm64.a", "8947ed692d9c6006c3a31e88cf2f272be03e2c07d6673bc3d7a8157c483b4d40"),
    ("libcronet-linux-arm64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-arm64.so", "2b32ce765358ed57123df3409583b3b0cf022e0324692ab167bcfef764daff5a"),
    ("libcronet-linux-arm64_musl.a", "c42e69c678d71a616232488c004e0eba81c166c7fbe3bfd52502fe3a08f166bc"),
    ("libcronet-linux-arm64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-arm_musl.a", "9bf40169d35c760e2c425c99c2d99b4b2aa3ceb3fb3d283b3cfaabd07eaaa729"),
    ("libcronet-linux-arm_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-loong64.a", "ddb409653b1ee0fde3b7e5523d2fe727ceed26e0051ac31319f9ff18535c397c"),
    ("libcronet-linux-loong64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-loong64.so", "f9062a1c6076bc4588fe7d6b3503f9d0575579a84a193f6770ba4a8c13b9e45f"),
    ("libcronet-linux-loong64_musl.a", "e97571dbb0a2b1aff7df1db09a2f4ceeef008a2ea1b5eacfdca3a2cc5f29c3bb"),
    ("libcronet-linux-loong64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-linux-mips64le.a", "363b43ad7c5c57e84c3e4aa42176ccafa078d4db72be88b212686f055c5df29d"),
    ("libcronet-linux-mips64le.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-mips64le.so", "e49e029ed8c7ba46515285ad634ab0337b39ca5a91fa8c5df9d5b174f7700342"),
    ("libcronet-linux-mipsle.a", "6e497a21e291ed532208d66effc94640fa234362d286ab368ebda850751227ec"),
    ("libcronet-linux-mipsle.link-flags.txt", "2906b769664b8caba306bd9027ea645f7831df4a60accdac9380e0ca915b31c5"),
    ("libcronet-linux-mipsle.so", "23410260427e84a6aafa91242886a95c683dfcbced14dfc6b9292be73f5a5fff"),
    ("libcronet-linux-mipsle_musl.a", "3b87dd49d85712313089f1dd4d747fbd4954c12f70d323b436f7b5a8b5dafb46"),
    ("libcronet-linux-mipsle_musl.link-flags.txt", "7c4f3c9ad180baeaf47dc3f42d868f648bf93749e28a235ffc3981cc08379e75"),
    ("libcronet-linux-riscv64.a", "053174c3cd4644cc66a7ef9741a92d05921f9979294c0703701c9f930192cf6c"),
    ("libcronet-linux-riscv64.link-flags.txt", "eb76fb6666e3939472c0bc30d26186496f40e1e722f4ba4f971315cefa7005a3"),
    ("libcronet-linux-riscv64.so", "ffd9eff98dc70fc3d425b94f15d17cfb05145b1a965631491321b1f86c1ffe23"),
    ("libcronet-linux-riscv64_musl.a", "f7c2a710399f8a561bc2bddca27fd43a0d45b7e54a6c93ae9bcac8a5a0830346"),
    ("libcronet-linux-riscv64_musl.link-flags.txt", "6a314d8af0d1f1512f0bb9227c8071e5e28c5b1dc8af57b6d7382edc52525e9f"),
    ("libcronet-tvos-amd64_simulator.a", "0b68b67817e1a7b73f6f5f127448610b2ecafba53e4d1c573f6d5e35eb936c19"),
    ("libcronet-tvos-arm64.a", "77105e2c081195f9eda140b4791091b6ae1263f9056e0d1ca936c0ed0c459db2"),
    ("libcronet-tvos-arm64_simulator.a", "1dce25ed61503ce289408a41a81fc34397dfb757c9e75c345c58814529d8107c"),
    ("libcronet-windows-amd64.dll", "ec80234fcef453bbbedf826da2d906b5a5d65a0a53de08164577fb64cdf074fe"),
    ("libcronet-windows-amd64.lib", "018b05fd8c774268d75a89b670137b11c357de2a8b8fb4da8dc6c06729382976"),
    ("libcronet-windows-amd64.link-flags.txt", "e186384087ef385a1038abfecc8585d1f6ab7f9160d341ec73b272a5ce53d6d0"),
    ("libcronet-windows-amd64_md.dll", "a3f874828f12d75484effc3db2fe5e5423d4237620c3a2db1c322d5716e1b235"),
    ("libcronet-windows-amd64_md.lib", "6f8d002fd8387909d1738d8d963c047e7040f1f6591cf7ef7722990491d58e5e"),
    ("libcronet-windows-amd64_md.link-flags.txt", "5e66d459cabf06246fbdc1561bf1d6e04ccf34ed91a30dabfc25f7cd39a03334"),
    ("libcronet-windows-arm64.dll", "0b84a8cb62f050fe8884c0466e3226058519a4c94727c20e42515314b9f05b04"),
    ("libcronet-windows-arm64.lib", "1717aa3c4883a4c58ca5e271bb22e5596e466b8cd1b4869d5e36d4d2eee1a5f5"),
    ("libcronet-windows-arm64.link-flags.txt", "e186384087ef385a1038abfecc8585d1f6ab7f9160d341ec73b272a5ce53d6d0"),
    ("libcronet-windows-arm64_md.dll", "6929a88656585a885e5b44c57c8ff9968936cf9982275dbd8e7d1cbcf18ce7f5"),
    ("libcronet-windows-arm64_md.lib", "97ee59f6ac111113689fbf4ccba016b5645458074b43c54a031ebf5323c5d954"),
    ("libcronet-windows-arm64_md.link-flags.txt", "5e66d459cabf06246fbdc1561bf1d6e04ccf34ed91a30dabfc25f7cd39a03334"),
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

    let static_link = build_target::uses_static_link(
        &env::var("CARGO_CFG_TARGET_VENDOR").expect("Cargo sets the target vendor"),
        &env::var("CARGO_CFG_TARGET_OS").expect("Cargo sets the target OS"),
        cfg!(feature = "dynamic"),
        cfg!(feature = "static-link"),
    );
    println!("cargo:rustc-check-cfg=cfg(cronet_static_link)");
    if static_link {
        println!("cargo:rustc-cfg=cronet_static_link");
    }

    // 1. Optionally download (and verify) the prebuilt for this target.
    // A supplied native archive takes precedence over automatic downloads.
    let downloaded = if build_target::download_enabled(
        cfg!(feature = "download"),
        cfg!(feature = "download-static"),
        static_link,
    ) && !(static_link && env::var_os("CRONET_LIB_DIR").is_some())
    {
        maybe_download(static_link)
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
fn asset_name(static_link: bool) -> Option<String> {
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
        static_link,
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
fn maybe_download(_static_link: bool) -> Option<PathBuf> {
    None
}

#[cfg(any(feature = "download", feature = "download-static"))]
fn maybe_download(static_link: bool) -> Option<PathBuf> {
    let asset = asset_name(static_link).unwrap_or_else(|| {
        panic!(
            "cronet-sys: cannot determine prebuilt asset for target os={:?} arch={:?}; \
             set CRONET_DOWNLOAD_ASSET explicitly",
            env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
        )
    });

    let archive = download_asset(&asset, "CRONET_DOWNLOAD_SHA256");
    if static_link && env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
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

    let tag = env::var("CRONET_DOWNLOAD_TAG").unwrap_or_else(|_| "chromium-154.0.8037.49-1".to_string());

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
