# cronet-sys

Low-level FFI bindings to Chromium **Cronet** (`libcronet`), as used by
[naiveproxy](https://github.com/klzgrad/naiveproxy). This is the unsafe `-sys`
layer under [`cronet-rs`](https://crates.io/crates/cronet-rs); most users want
the high-level crate.

The vendored C headers (the ABI source of truth) live in [`include/`](include/).

## Backends

| Feature | Default | What it does |
|---|---|---|
| `dynamic` | ❌ | Resolve symbols at runtime via `dlopen`/`LoadLibrary` (`libloading`). |
| `static-link` | ❌ | Link `libcronet.a` or MSVC `cronet.lib` at compile time. Required on macOS/iOS. |
| `download-static` | ❌ | Download a verified archive only for the selected static backend. |
| `download` | ❌ | Build script downloads a prebuilt `libcronet` for the target. |

Desktop builds use runtime loading by default, including with `--no-default-features`.
Enable `static-link` to embed Cronet. An explicit `dynamic` selects runtime loading
on desktop even when `static-link` is enabled, so `download-static` skips the download.
Apple targets always link statically. Android honors an explicit `static-link` even with `dynamic`.
For automatic static linking, use `--features static-link,download-static`.
Linux archives match the target's glibc or musl ABI. Windows requires MSVC.
Automatic downloads match Rust's CRT: the default uses the `/MD` archive and
needs no extra `RUSTFLAGS`; `+crt-static` selects the `/MT` archive instead.
Cronet itself remains statically linked in both cases. `/MD` uses the MSVC
runtime DLLs. Cronet's private BoringSSL symbols are isolated so another
BoringSSL consumer can coexist in the same application.
When explicitly using `+crt-static`, also pass `--target` so host build tools
and procedural macros keep their normal CRT configuration.

## Providing the native library

Three options, in order of manual effort:

1. **`download` feature** — `build.rs` fetches the matching prebuilt from
   [`rust-proxy/cronet-binaries`](https://github.com/rust-proxy/cronet-binaries/releases)
   into `OUT_DIR` and verifies its **SHA-256** against a pinned checksum. No
   network is touched unless `download`, or `download-static` with the static backend, is enabled.
   - Dynamic mode: load it with [`load_downloaded_library()`]; the path is also
     exposed via [`downloaded_library_path()`].
   - Static mode: the archive is staged and linked automatically.
2. **`CRONET_LIB_DIR`** — point it at a directory containing `libcronet.a`
   (or Windows `cronet.lib` plus `link_flags.txt`) and build with
   `--features static-link`. Match the archive's `/MD` or `/MT` CRT to Rust on
   Windows. This bypasses automatic downloads.
3. **System search path** — drop a loadable `libcronet.{so,dll}` somewhere the
   loader finds it and call `load_library("libcronet.so")` yourself (dynamic).

## Environment variables (build script)

| Variable | Purpose |
|---|---|
| `CRONET_LIB_DIR` | Directory containing the static archive and Windows link flags. |
| `CRONET_STATIC_NAME` | Static lib stem, default `cronet` → `libcronet.a`. |
| `CRONET_DOWNLOAD_TAG` | Release tag to fetch, default `static-20261116.1`. |
| `CRONET_DOWNLOAD_ASSET` | Override the auto-detected asset file name. |
| `CRONET_DOWNLOAD_SHA256` | Expected SHA-256 for a custom/unpinned asset. |
| `CRONET_DOWNLOAD_LINK_FLAGS_SHA256` | Expected SHA-256 for custom/unpinned Windows link flags. |
| `CRONET_DOWNLOAD_DIR` | Cache directory for downloads, default `OUT_DIR`. |

Downloads use the fixed `static-20261116.1` release and pinned SHA-256 checksums,
including the Windows linker metadata. The iOS x64 simulator archive is temporarily
omitted from this release; other Apple targets remain available. Cached files
are verified before reuse.
A mismatch fails the build; update the release tag and checksum table together
when intentionally adopting a new build. Environment checksums apply only to
assets without a pinned entry.

[`load_downloaded_library()`]: https://docs.rs/cronet-sys
[`downloaded_library_path()`]: https://docs.rs/cronet-sys
