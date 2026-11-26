# cronet-sys

Low-level FFI bindings to Chromium **Cronet** (`libcronet`), as used by
[naiveproxy](https://github.com/klzgrad/naiveproxy). This is the unsafe `-sys`
layer under [`cronet-rs`](https://crates.io/crates/cronet-rs); most users want
the high-level crate.

These bindings use the SagerNet Chromium 154.0.8037.49 dialer ABI. Load a matching shared library or supply the matching static archive. Dynamic loading reports incompatible versions before registering callbacks.

After a successful dynamic load, `loaded_library_path()` returns the module path reported by the OS. Use `conflicting_library(path)` to check a later explicit request against the active library without loading a second one. Both return `None` for static builds.


The vendored C headers (the ABI source of truth) live in [`include/`](include/).

## Backends

| Feature | Default | What it does |
|---|---|---|
| `dynamic` | ❌ | Resolve symbols at runtime via `dlopen`/`LoadLibrary` (`libloading`). |
| `static-link` | ❌ | Link `libcronet.a` or MSVC `cronet.lib` at compile time. Default on Apple targets. |
| `download-static` | ❌ | Download a verified archive only for the selected static backend. |
| `download` | ❌ | Build script downloads a prebuilt `libcronet` for the target. |

Backend selection is identical on every target: `dynamic` wins over `static-link`,
and `download-static` only fetches an archive when the static backend is selected.
Desktop and Android builds use runtime loading by default, including with
`--no-default-features`; `static-link` embeds Cronet and is the default on Apple.
Apple targets therefore switch to runtime loading only when `dynamic` is enabled,
which requires a shared library: macOS can dlopen a user-supplied dylib, while
iOS/tvOS have no shared Cronet build.
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
3. **System search path** — drop a loadable `libcronet.{so,dll,dylib}` somewhere the
   loader finds it and call `load_library("libcronet.so")` yourself (dynamic).

## Environment variables (build script)

| Variable | Purpose |
|---|---|
| `CRONET_LIB_DIR` | Directory containing the static archive and Windows link flags. |
| `CRONET_STATIC_NAME` | Static lib stem, default `cronet` → `libcronet.a`. |
| `CRONET_DOWNLOAD_TAG` | Release tag to fetch, default `v154.0.8037.49`. |
| `CRONET_DOWNLOAD_ASSET` | Override the auto-detected asset file name. |
| `CRONET_DOWNLOAD_SHA256` | Expected SHA-256 for a custom/unpinned asset. |
| `CRONET_DOWNLOAD_LINK_FLAGS_SHA256` | Expected SHA-256 for custom/unpinned Windows link flags. |
| `CRONET_DOWNLOAD_DIR` | Cache directory for downloads, default `OUT_DIR`. |

Downloads use the fixed `v154.0.8037.49` release and pinned SHA-256 checksums,
including the Windows linker metadata. The release includes Intel and ARM64 iOS
simulator archives. For the macOS archives, set a deployment target of macOS
13.0 or newer. Cached files are verified before reuse.
A mismatch fails the build; update the release tag and checksum table together
when intentionally adopting a new build. Environment checksums apply only to
assets without a pinned entry.

[`load_downloaded_library()`]: https://docs.rs/cronet-sys
[`downloaded_library_path()`]: https://docs.rs/cronet-sys
