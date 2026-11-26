//! Dynamic loading backend — load libcronet at runtime via dlopen/LoadLibrary.
//!
//! Used on Linux, Windows, and Android where dlopen is available.
//! NOT available on macOS/iOS.

#![allow(non_snake_case, dead_code, unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub use libloading::Library;

struct LoadedLibrary {
    handle: Library,
    path: PathBuf,
}

static LIB: OnceLock<LoadedLibrary> = OnceLock::new();
// Serialize native library initialization and ABI probing. A failed attempt may be retried.
static LOAD_LOCK: Mutex<()> = Mutex::new(());

/// Filesystem path reported by the OS for the active Cronet module.
pub fn loaded_library_path() -> Option<&'static Path> {
    LIB.get().map(|lib| lib.path.as_path())
}

fn candidate(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        find_platform_library(path)
    } else {
        Some(path.to_owned())
    }
}

/// If an explicit file, directory or loader search name selects a different library,
/// return the path of the one already in use. Never load the requested library here.
pub fn conflicting_library(requested: &Path) -> Option<&'static Path> {
    let loaded = LIB.get()?;
    let matches = candidate(requested).is_some_and(|path| {
        if path.file_name() == Some(path.as_os_str()) {
            return matches_search_name(&path, loaded);
        }
        path == loaded.path || same_file::is_same_file(&path, &loaded.path).unwrap_or(false)
    });
    (!matches).then_some(loaded.path.as_path())
}

#[cfg(windows)]
fn matches_search_name(name: &Path, loaded: &LoadedLibrary) -> bool {
    libloading::os::windows::Library::open_already_loaded(name)
        .is_ok_and(|module| same_module(&module.into(), &loaded.handle))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd"
))]
fn matches_search_name(name: &Path, loaded: &LoadedLibrary) -> bool {
    // RTLD_NOLOAD only acquires a reference to an already resident module.
    unsafe { libloading::os::unix::Library::open(Some(name), libc::RTLD_NOLOAD | libc::RTLD_LAZY) }
        .is_ok_and(|module| same_module(&module.into(), &loaded.handle))
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd"
)))]
fn matches_search_name(name: &Path, loaded: &LoadedLibrary) -> bool {
    loaded.path.file_name() == Some(name.as_os_str())
}

fn same_module(a: &Library, b: &Library) -> bool {
    // Reading an exported symbol's address does not call it.
    unsafe {
        let (Ok(a), Ok(b)) = (
            a.get::<*const ()>(b"Cronet_Engine_Create"),
            b.get::<*const ()>(b"Cronet_Engine_Create"),
        ) else {
            return false;
        };
        *a == *b
    }
}

#[cfg(unix)]
fn module_path(symbol: *const ()) -> std::io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::uninit();
    // The symbol belongs to the live library and dladdr initializes info on success.
    unsafe {
        if libc::dladdr(symbol.cast(), info.as_mut_ptr()) == 0 {
            return Err(std::io::Error::other(
                "dladdr could not locate the Cronet module",
            ));
        }
        let name = info.assume_init().dli_fname;
        if name.is_null() {
            return Err(std::io::Error::other(
                "dladdr returned no Cronet module path",
            ));
        }
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(
            std::ffi::CStr::from_ptr(name).to_bytes(),
        )))
    }
}

#[cfg(windows)]
fn module_path(symbol: *const ()) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };
    let mut module = 0;
    let mut name = vec![0u16; 32768];
    // The library owns the module reference throughout these calls.
    unsafe {
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            symbol.cast(),
            &mut module,
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let len = GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
        if len == 0 || len == name.len() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(std::ffi::OsString::from_wide(&name[..len]).into())
    }
}

/// Load the libcronet shared library at runtime.
///
/// `path` may be either:
/// - a **file** — loaded directly (e.g. `"libcronet.so"`, `"C:\\libs\\libcronet.dll"`);
/// - a **directory** — scanned **recursively** for the loadable library that
///   best matches the current platform and architecture (see
///   [`find_platform_library`]). This is handy for pointing at a download/output
///   tree without knowing the exact asset file name or its sub-path
///   (`load_library("dylibs")` will find `dylibs/windows-amd64/libcronet.dll`).
///
/// Must be called **before** any other FFI function (engine creation, etc.).
/// On success, the library stays loaded for the lifetime of the process.
///
/// # Safety
///
/// Use a trusted SagerNet Cronet library. A failed load can be retried.
/// After a successful load the library stays active for the process lifetime.
pub unsafe fn load_library(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LOAD_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    if LIB.get().is_some() {
        return Err("library already loaded".into());
    }
    let resolved = candidate(Path::new(path)).ok_or_else(|| {
        format!("no loadable cronet library for this platform found in directory {path:?}")
    })?;
    let lib = Library::new(&resolved)?;
    // Both revisions export the same dialer names with incompatible signatures.
    let create = lib.get::<unsafe extern "C" fn() -> super::RawEngine>(b"Cronet_Engine_Create")?;
    let destroy = lib.get::<unsafe extern "C" fn(super::RawEngine)>(b"Cronet_Engine_Destroy")?;
    let version = lib.get::<unsafe extern "C" fn(super::RawEngine) -> *const std::ffi::c_char>(
        b"Cronet_Engine_GetVersionString",
    )?;
    let engine = create();
    if engine.0 == 0 {
        return Err("Cronet version probe could not create an engine".into());
    }
    let ptr = version(engine);
    let actual = if ptr.is_null() {
        String::from("<unknown>")
    } else {
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    };
    destroy(engine);
    if actual != "154.0.8037.49" {
        return Err(
            format!("incompatible Cronet {actual}, expected SagerNet 154.0.8037.49").into(),
        );
    }
    let path = module_path(*create as *const ())?;
    let path = path.canonicalize().unwrap_or(path);
    LIB.set(LoadedLibrary { handle: lib, path })
        .map_err(|_| "library already loaded".into())
}

/// Recursively search `dir` (and its sub-directories) for the loadable library
/// that best matches the current platform, returning its full path. Returns
/// `None` if no candidate with a loadable extension for this OS is found.
///
/// Candidates are ranked by: loadable extension for this OS (required) →
/// filename contains `cronet` → architecture match → OS match, with guards so a
/// 64-bit host never prefers a 32-bit artifact (or vice-versa). Ties break by
/// full path for deterministic selection.
///
/// The walk is bounded to [`MAX_SEARCH_DEPTH`] levels and does not follow
/// directory symlinks, so it terminates on cyclic or pathological trees.
pub fn find_platform_library(dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(i32, PathBuf)> = Vec::new();
    collect_candidates(dir, MAX_SEARCH_DEPTH, &mut candidates);
    // Highest score first; tie-break by path ascending for determinism.
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    candidates.into_iter().next().map(|(_, path)| path)
}

/// Maximum directory depth searched by [`find_platform_library`].
const MAX_SEARCH_DEPTH: usize = 8;

/// Depth-first collect `(score, path)` for every loadable-on-this-OS file under
/// `dir`. `depth` is the remaining levels to descend. Errors (unreadable dirs,
/// permission issues) are skipped rather than propagated. Directory symlinks are
/// not followed, which prevents cycles.
fn collect_candidates(dir: &Path, depth: usize, out: &mut Vec<(i32, PathBuf)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        // `file_type()` does not follow symlinks, so a symlinked directory is
        // reported as a symlink (not a dir) and is treated as a plain file
        // below — never recursed into.
        if file_type.is_dir() {
            if depth > 0 {
                collect_candidates(&path, depth - 1, out);
            }
        } else {
            let name = entry.file_name();
            if let Some(score) = rank_library(&name.to_string_lossy()) {
                out.push((score, path));
            }
        }
    }
}

/// Choose the best-ranked candidate file name from an iterator of names.
/// Pure (filesystem-independent) so it can be unit-tested.
fn choose_library<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut scored: Vec<(i32, String)> = names
        .into_iter()
        .filter_map(|n| rank_library(n).map(|s| (s, n.to_string())))
        .collect();
    // Highest score first; tie-break by name ascending for determinism.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().next().map(|(_, name)| name)
}

/// Score a file name as a load candidate for the current platform.
/// Returns `None` if it is not a loadable library on this OS.
fn rank_library(name: &str) -> Option<i32> {
    let lower = name.to_ascii_lowercase();
    if !is_loadable_lib(&lower) {
        return None;
    }

    let mut score = 0;
    if lower.contains("cronet") {
        score += 8;
    }
    score += arch_bonus(&lower);
    score += os_bonus(&lower);
    // A device/desktop host should not prefer a simulator build.
    if lower.contains("simulator") {
        score -= 4;
    }
    // Exact canonical name (e.g. `libcronet.so` / `cronet.dll`) is a strong signal.
    let canonical = format!(
        "{}cronet{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    )
    .to_ascii_lowercase();
    if lower == canonical {
        score += 2;
    }
    Some(score)
}

/// True if `name` (already lowercased) carries this OS's dynamic-library
/// extension, including versioned forms like `libcronet.so.119`.
fn is_loadable_lib(name: &str) -> bool {
    let suffix = std::env::consts::DLL_SUFFIX; // ".so" | ".dll" | ".dylib"
    name.ends_with(suffix) || name.contains(&format!("{suffix}."))
}

/// Architecture-match bonus, with cross-width guards.
fn arch_bonus(lower: &str) -> i32 {
    let positives: &[&str] = match std::env::consts::ARCH {
        "x86_64" => &["amd64", "x86_64", "x64"],
        "x86" => &["386", "i686", "i386", "x86"],
        "aarch64" => &["arm64", "aarch64"],
        "arm" => &["armv7", "armhf", "armeabi", "arm"],
        "loongarch64" => &["loong64", "loongarch64"],
        "riscv64" => &["riscv64"],
        _ => &[],
    };
    let mut bonus = 0;
    if positives.iter().any(|t| lower.contains(t)) {
        bonus += 4;
    }
    // Guard against substring confusion between widths.
    match std::env::consts::ARCH {
        "x86" if lower.contains("x86_64") || lower.contains("amd64") || lower.contains("x64") => {
            bonus -= 6;
        }
        "arm" if lower.contains("arm64") || lower.contains("aarch64") => {
            bonus -= 6;
        }
        _ => {}
    }
    bonus
}

/// OS-match bonus: reward the running OS's token, mildly penalize a foreign one.
fn os_bonus(lower: &str) -> i32 {
    let (mine, others): (&[&str], &[&str]) = match std::env::consts::OS {
        "windows" => (&["windows", "win"], &["linux", "android", "darwin", "macos", "ios"]),
        "linux" => (&["linux"], &["windows", "android", "darwin", "macos", "ios"]),
        "android" => (&["android"], &["windows", "darwin", "macos", "ios"]),
        "macos" => (&["darwin", "macos", "osx"], &["windows", "android", "ios"]),
        "ios" => (&["ios"], &["windows", "android", "linux"]),
        _ => (&[], &[]),
    };
    let mut bonus = 0;
    if mine.iter().any(|t| lower.contains(t)) {
        bonus += 2;
    }
    if others.iter().any(|t| lower.contains(t)) {
        bonus -= 3;
    }
    bonus
}

// ---------------------------------------------------------------------------
// Helper: look up and call a C function
// ---------------------------------------------------------------------------

macro_rules! ffi_call {
    ($name:literal, $sig:ty, $($arg:expr),* $(,)?) => {{
        let lib = LIB.get().expect("libcronet not loaded; call load_library() first");
        let func: libloading::Symbol<$sig> = lib.handle.get($name).unwrap_or_else(|e| {
            panic!("symbol {} not found: {}", String::from_utf8_lossy($name), e)
        });
        func($($arg),*)
    }};
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub unsafe fn Cronet_Engine_Create() -> super::RawEngine {
    ffi_call!(b"Cronet_Engine_Create", unsafe extern "C" fn() -> super::RawEngine,)
}

pub unsafe fn Cronet_Engine_Destroy(engine: super::RawEngine) {
    ffi_call!(b"Cronet_Engine_Destroy", unsafe extern "C" fn(super::RawEngine), engine)
}

pub unsafe fn Cronet_Engine_StartWithParams(
    engine: super::RawEngine, params: super::RawEngineParams,
) -> i32 {
    ffi_call!(b"Cronet_Engine_StartWithParams", unsafe extern "C" fn(super::RawEngine, super::RawEngineParams) -> i32, engine, params)
}

pub unsafe fn Cronet_Engine_Shutdown(engine: super::RawEngine) -> i32 {
    ffi_call!(b"Cronet_Engine_Shutdown", unsafe extern "C" fn(super::RawEngine) -> i32, engine)
}

pub unsafe fn Cronet_Engine_GetVersionString(engine: super::RawEngine) -> *const std::os::raw::c_char {
    ffi_call!(b"Cronet_Engine_GetVersionString", unsafe extern "C" fn(super::RawEngine) -> *const std::os::raw::c_char, engine)
}

pub unsafe fn Cronet_Engine_GetDefaultUserAgent(engine: super::RawEngine) -> *const std::os::raw::c_char {
    ffi_call!(b"Cronet_Engine_GetDefaultUserAgent", unsafe extern "C" fn(super::RawEngine) -> *const std::os::raw::c_char, engine)
}

pub unsafe fn Cronet_Engine_GetStreamEngine(engine: super::RawEngine) -> super::RawStreamEngine {
    ffi_call!(b"Cronet_Engine_GetStreamEngine", unsafe extern "C" fn(super::RawEngine) -> super::RawStreamEngine, engine)
}

pub unsafe fn Cronet_Engine_CloseAllConnections(engine: super::RawEngine) {
    ffi_call!(b"Cronet_Engine_CloseAllConnections", unsafe extern "C" fn(super::RawEngine), engine)
}

pub unsafe fn Cronet_CreateCertVerifierWithRootCerts(
    pem: *const std::os::raw::c_char,
) -> super::RawCertVerifier {
    ffi_call!(b"Cronet_CreateCertVerifierWithRootCerts", unsafe extern "C" fn(*const std::os::raw::c_char) -> super::RawCertVerifier, pem)
}

pub unsafe fn Cronet_Engine_SetMockCertVerifierForTesting(
    engine: super::RawEngine, verifier: super::RawCertVerifier,
) {
    ffi_call!(b"Cronet_Engine_SetMockCertVerifierForTesting", unsafe extern "C" fn(super::RawEngine, super::RawCertVerifier), engine, verifier)
}

pub unsafe fn Cronet_Engine_SetDialer(
    engine: super::RawEngine,
    callback: Option<super::RawDialer>,
    context: *mut std::ffi::c_void,
) {
    ffi_call!(b"Cronet_Engine_SetDialer", unsafe extern "C" fn(super::RawEngine, Option<super::RawDialer>, *mut std::ffi::c_void), engine, callback, context)
}

pub unsafe fn Cronet_Engine_SetUdpDialer(
    engine: super::RawEngine,
    callback: Option<super::RawUdpDialer>,
    context: *mut std::ffi::c_void,
    on_close: Option<super::RawUdpSocketClose>,
) {
    ffi_call!(
        b"Cronet_Engine_SetUdpDialer",
        unsafe extern "C" fn(
            super::RawEngine,
            Option<super::RawUdpDialer>,
            *mut std::ffi::c_void,
            Option<super::RawUdpSocketClose>,
        ),
        engine,
        callback,
        context,
        on_close
    )
}

// ---------------------------------------------------------------------------
// EngineParams
// ---------------------------------------------------------------------------

pub unsafe fn Cronet_EngineParams_Create() -> super::RawEngineParams {
    ffi_call!(b"Cronet_EngineParams_Create", unsafe extern "C" fn() -> super::RawEngineParams,)
}

pub unsafe fn Cronet_EngineParams_Destroy(params: super::RawEngineParams) {
    ffi_call!(b"Cronet_EngineParams_Destroy", unsafe extern "C" fn(super::RawEngineParams), params)
}

pub unsafe fn Cronet_EngineParams_enable_quic_set(params: super::RawEngineParams, enable: bool) {
    ffi_call!(b"Cronet_EngineParams_enable_quic_set", unsafe extern "C" fn(super::RawEngineParams, bool), params, enable)
}

pub unsafe fn Cronet_EngineParams_enable_http2_set(params: super::RawEngineParams, enable: bool) {
    ffi_call!(b"Cronet_EngineParams_enable_http2_set", unsafe extern "C" fn(super::RawEngineParams, bool), params, enable)
}

pub unsafe fn Cronet_EngineParams_enable_brotli_set(params: super::RawEngineParams, enable: bool) {
    ffi_call!(b"Cronet_EngineParams_enable_brotli_set", unsafe extern "C" fn(super::RawEngineParams, bool), params, enable)
}

pub unsafe fn Cronet_EngineParams_user_agent_set(
    params: super::RawEngineParams, ua: *const std::os::raw::c_char,
) {
    ffi_call!(b"Cronet_EngineParams_user_agent_set", unsafe extern "C" fn(super::RawEngineParams, *const std::os::raw::c_char), params, ua)
}

pub unsafe fn Cronet_EngineParams_experimental_options_set(
    params: super::RawEngineParams, opts: *const std::os::raw::c_char,
) {
    ffi_call!(b"Cronet_EngineParams_experimental_options_set", unsafe extern "C" fn(super::RawEngineParams, *const std::os::raw::c_char), params, opts)
}

pub unsafe fn Cronet_EngineParams_http_cache_mode_set(params: super::RawEngineParams, mode: i32) {
    ffi_call!(b"Cronet_EngineParams_http_cache_mode_set", unsafe extern "C" fn(super::RawEngineParams, i32), params, mode)
}

pub unsafe fn Cronet_EngineParams_http_cache_max_size_set(params: super::RawEngineParams, size: i64) {
    ffi_call!(b"Cronet_EngineParams_http_cache_max_size_set", unsafe extern "C" fn(super::RawEngineParams, i64), params, size)
}

// ---------------------------------------------------------------------------
// BidirectionalStream
// ---------------------------------------------------------------------------

pub unsafe fn bidirectional_stream_create(
    engine: super::RawStreamEngine,
    annotation: usize,
    callback: *const super::RawBidirectionalStreamCallback,
) -> super::RawBidirectionalStream {
    ffi_call!(b"bidirectional_stream_create", unsafe extern "C" fn(super::RawStreamEngine, usize, *const super::RawBidirectionalStreamCallback) -> super::RawBidirectionalStream, engine, annotation, callback)
}

pub unsafe fn bidirectional_stream_destroy(stream: super::RawBidirectionalStream) -> i32 {
    ffi_call!(b"bidirectional_stream_destroy", unsafe extern "C" fn(super::RawBidirectionalStream) -> i32, stream)
}

pub unsafe fn bidirectional_stream_start(
    stream: super::RawBidirectionalStream,
    url: *const std::os::raw::c_char,
    priority: i32,
    method: *const std::os::raw::c_char,
    headers: *const super::RawBidirectionalStreamHeaderArray,
    end_of_stream: bool,
) -> i32 {
    ffi_call!(b"bidirectional_stream_start", unsafe extern "C" fn(super::RawBidirectionalStream, *const std::os::raw::c_char, i32, *const std::os::raw::c_char, *const super::RawBidirectionalStreamHeaderArray, bool) -> i32, stream, url, priority, method, headers, end_of_stream)
}

pub unsafe fn bidirectional_stream_read(
    stream: super::RawBidirectionalStream,
    buffer: *mut u8,
    size: i32,
) -> i32 {
    ffi_call!(b"bidirectional_stream_read", unsafe extern "C" fn(super::RawBidirectionalStream, *mut u8, i32) -> i32, stream, buffer, size)
}

pub unsafe fn bidirectional_stream_write(
    stream: super::RawBidirectionalStream,
    buffer: *const u8,
    size: i32,
    end_of_stream: bool,
) -> i32 {
    ffi_call!(b"bidirectional_stream_write", unsafe extern "C" fn(super::RawBidirectionalStream, *const u8, i32, bool) -> i32, stream, buffer, size, end_of_stream)
}

pub unsafe fn bidirectional_stream_flush(stream: super::RawBidirectionalStream) {
    ffi_call!(b"bidirectional_stream_flush", unsafe extern "C" fn(super::RawBidirectionalStream), stream)
}

pub unsafe fn bidirectional_stream_cancel(stream: super::RawBidirectionalStream) {
    ffi_call!(b"bidirectional_stream_cancel", unsafe extern "C" fn(super::RawBidirectionalStream), stream)
}

pub unsafe fn bidirectional_stream_disable_auto_flush(stream: super::RawBidirectionalStream, disable: bool) {
    ffi_call!(b"bidirectional_stream_disable_auto_flush", unsafe extern "C" fn(super::RawBidirectionalStream, bool), stream, disable)
}

pub unsafe fn bidirectional_stream_delay_request_headers_until_flush(
    stream: super::RawBidirectionalStream,
    delay: bool,
) {
    ffi_call!(b"bidirectional_stream_delay_request_headers_until_flush", unsafe extern "C" fn(super::RawBidirectionalStream, bool), stream, delay)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// This OS's dynamic library suffix (".so" / ".dll" / ".dylib").
    fn suf() -> &'static str {
        std::env::consts::DLL_SUFFIX
    }

    #[test]
    fn rejects_non_loadable_extensions() {
        // Static archives, import libs, headers — never loadable, on any OS.
        assert!(choose_library(["libcronet.a", "cronet.lib", "cronet_c.h", "notes.txt"]).is_none());
    }

    #[test]
    fn prefers_cronet_named_file() {
        let s = suf();
        let other = format!("libfoo{s}");
        let cronet = format!("libcronet{s}");
        let chosen = choose_library([other.as_str(), cronet.as_str()]).unwrap();
        assert_eq!(chosen, cronet);
    }

    #[test]
    fn single_canonical_file_in_dir_is_picked() {
        let s = suf();
        let only = format!("libcronet{s}");
        assert_eq!(choose_library([only.as_str()]).unwrap(), only);
    }

    #[test]
    fn ties_break_deterministically() {
        let s = suf();
        // Two equally-scored generic libs → lexicographically smallest wins.
        let a = format!("libaaa{s}");
        let b = format!("libbbb{s}");
        assert_eq!(choose_library([b.as_str(), a.as_str()]).unwrap(), a);
    }

    #[test]
    fn matches_release_asset_naming_for_this_target() {
        // The real rust-proxy/cronet-binaries asset for this host should be
        // selected over a foreign-arch sibling sharing the same extension.
        let s = suf();
        let os = std::env::consts::OS;
        let (mine, foreign): (&str, &str) = match std::env::consts::ARCH {
            "x86_64" => ("amd64", "arm64"),
            "aarch64" => ("arm64", "amd64"),
            "x86" => ("386", "amd64"),
            _ => return, // skip exotic arches
        };
        let mine_file = format!("libcronet-{os}-{mine}{s}");
        let foreign_file = format!("libcronet-{os}-{foreign}{s}");
        let chosen = choose_library([foreign_file.as_str(), mine_file.as_str()]).unwrap();
        assert_eq!(chosen, mine_file);
    }

    #[test]
    fn find_in_directory_resolves_full_path() {
        // Build a temp dir with a canonical lib and confirm we get its path back.
        let s = suf();
        let dir = std::env::temp_dir().join(format!("cronet_sys_find_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let lib = dir.join(format!("libcronet{s}"));
        std::fs::write(&lib, b"not a real lib").unwrap();
        std::fs::write(dir.join("readme.txt"), b"ignore me").unwrap();

        let found = find_platform_library(&dir).expect("should find the cronet lib");
        assert_eq!(found, lib);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_recurses_into_subdirectories() {
        // Mirror the repo layout: <root>/<platform>/libcronet.<ext>.
        let s = suf();
        let os = std::env::consts::OS;
        let root =
            std::env::temp_dir().join(format!("cronet_sys_recurse_test_{}", std::process::id()));
        let sub = root.join(format!("{os}-host")).join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(root.join("notes.txt"), b"ignore").unwrap();
        let lib = sub.join(format!("libcronet{s}"));
        std::fs::write(&lib, b"not a real lib").unwrap();

        // Searching from the *root* should descend and find the nested lib.
        let found = find_platform_library(&root).expect("should find nested cronet lib");
        assert_eq!(found, lib);

        let _ = std::fs::remove_dir_all(&root);
    }
}
