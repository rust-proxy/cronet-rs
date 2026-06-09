# cronet-sys

Low-level FFI bindings to Chromium **Cronet** (`libcronet`), as used by
[naiveproxy](https://github.com/klzgrad/naiveproxy). This is the unsafe `-sys`
layer under [`cronet-rs`](https://crates.io/crates/cronet-rs); most users want
the high-level crate.

The vendored C headers (the ABI source of truth) live in [`include/`](include/).

## Backends

| Feature | Default | What it does |
|---|---|---|
| `dynamic` | ✅ | Resolve symbols at runtime via `dlopen`/`LoadLibrary` (`libloading`). |
| `static-link` | ❌ | Link `libcronet.a` at compile time. Required on macOS/iOS. |
| `download` | ❌ | Build script downloads a prebuilt `libcronet` for the target. |

`dynamic` and `static-link` are mutually exclusive; pick one
(`default-features = false` + `static-link` for the static path).

## Providing the native library

Three options, in order of manual effort:

1. **`download` feature** — `build.rs` fetches the matching prebuilt from
   [`rust-proxy/cronet-binaries`](https://github.com/rust-proxy/cronet-binaries/releases)
   into `OUT_DIR` and verifies its **SHA-256** against a pinned checksum. No
   network is touched unless this feature is enabled.
   - Dynamic mode: load it with [`load_downloaded_library()`]; the path is also
     exposed via [`downloaded_library_path()`].
   - Static mode: the archive is staged and linked automatically.
2. **`CRONET_LIB_DIR`** — point it at a directory containing `libcronet.a`
   (static mode) and build with `--features static-link`.
3. **System search path** — drop a loadable `libcronet.{so,dll}` somewhere the
   loader finds it and call `load_library("libcronet.so")` yourself (dynamic).

## Environment variables (build script)

| Variable | Purpose |
|---|---|
| `CRONET_LIB_DIR` | Directory containing `libcronet.a` (static mode). |
| `CRONET_STATIC_NAME` | Static lib stem, default `cronet` → `libcronet.a`. |
| `CRONET_DOWNLOAD_TAG` | Release tag to fetch, default `latest`. |
| `CRONET_DOWNLOAD_ASSET` | Override the auto-detected asset file name. |
| `CRONET_DOWNLOAD_SHA256` | Expected SHA-256 for a custom/unpinned asset. |
| `CRONET_DOWNLOAD_DIR` | Cache directory for downloads, default `OUT_DIR`. |

> The `download` feature pins checksums for the rolling `latest` release. If
> upstream re-rolls that tag, the build fails on checksum mismatch by design —
> update the pinned table in `build.rs` (or set `CRONET_DOWNLOAD_SHA256`) when
> intentionally bumping.

[`load_downloaded_library()`]: https://docs.rs/cronet-sys
[`downloaded_library_path()`]: https://docs.rs/cronet-sys
