/// The static downloader is deliberately independent of backend selection.
pub fn download_enabled(legacy_download: bool, download_static: bool, static_link: bool) -> bool {
    legacy_download || (download_static && static_link)
}

// Shared with standalone build-script tests (rustc --test).
#[allow(dead_code)]
pub fn asset_name(
    os: &str,
    arch: &str,
    abi: &str,
    env: &str,
    endian: &str,
    static_link: bool,
) -> Option<String> {
    let arch = match arch {
        "x86" => "386",
        "x86_64" => "amd64",
        "arm" => "arm",
        "aarch64" => "arm64",
        "loongarch64" => "loong64",
        "riscv64" => "riscv64",
        "mips64" if endian == "little" => "mips64le",
        "mips" if endian == "little" => "mipsle",
        _ => return None,
    };
    let (platform, suffix, extension) = match os {
        "linux" if static_link => ("linux", if env == "musl" { "_musl" } else { "" }, "a"),
        "linux" if env != "musl" => ("linux", "", "so"),
        "windows" if static_link && env == "msvc" => ("windows", "", "lib"),
        "windows" if !static_link => ("windows", "", "dll"),
        "android" if static_link => ("android", "", "a"),
        "macos" if static_link => ("darwin", "", "a"),
        "ios" | "tvos" if static_link => (
            os,
            if abi == "sim" || arch == "amd64" {
                "_simulator"
            } else {
                ""
            },
            "a",
        ),
        _ => return None,
    };
    Some(format!("libcronet-{platform}-{arch}{suffix}.{extension}"))
}

// musl implements the POSIX libraries in libc. Rust's self-contained musl
// sysroot has libc.a but no separate (empty) libdl.a/libpthread.a/etc.
pub fn linux_libraries(env: &str) -> &'static [&'static str] {
    // The complete archives already contain their required atomic helpers.
    if env == "musl" {
        &[]
    } else {
        &["dl", "pthread", "rt", "m", "resolv"]
    }
}

pub fn windows_libraries(contents: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for line in contents
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
    {
        for token in line.split_whitespace() {
            let name = token
                .trim_matches('"')
                .strip_suffix(".lib")
                .ok_or_else(|| format!("unsupported native link flag: {token}"))?;
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(format!("invalid system library: {token}"));
            }
            result.push(name.to_string());
        }
    }
    if result.is_empty() {
        return Err("no Windows system libraries found".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn musl_uses_libc_without_separate_posix_libraries() {
        assert!(linux_libraries("musl").is_empty());
        assert_eq!(
            linux_libraries("gnu"),
            ["dl", "pthread", "rt", "m", "resolv"]
        );
    }

    #[test]
    fn static_download_requires_both_features() {
        assert!(!download_enabled(false, false, false));
        assert!(!download_enabled(false, true, false));
        assert!(!download_enabled(false, false, true));
        assert!(download_enabled(false, true, true));
        // Existing download consumers keep their original behavior.
        for static_link in [false, true] {
            for download_static in [false, true] {
                assert!(download_enabled(true, download_static, static_link));
            }
        }
    }

    #[test]
    fn linux_linkage_and_libc_have_distinct_assets() {
        for arch in [
            "x86",
            "x86_64",
            "arm",
            "aarch64",
            "loongarch64",
            "riscv64",
            "mips",
            "mips64",
        ] {
            let shared = asset_name("linux", arch, "", "gnu", "little", false).unwrap();
            let archive = asset_name("linux", arch, "", "gnu", "little", true).unwrap();
            let musl = asset_name("linux", arch, "", "musl", "little", true).unwrap();
            assert!(shared.ends_with(".so"));
            assert_eq!(archive, shared.replace(".so", ".a"));
            assert_eq!(musl, archive.replace(".a", "_musl.a"));
        }
        assert!(asset_name("linux", "mips", "", "gnu", "big", true).is_none());
        assert!(asset_name("linux", "x86_64", "", "musl", "little", false).is_none());
    }

    #[test]
    fn windows_static_requires_msvc() {
        assert_eq!(
            asset_name("windows", "x86_64", "", "msvc", "little", true).as_deref(),
            Some("libcronet-windows-amd64.lib")
        );
        assert!(asset_name("windows", "x86_64", "", "gnu", "little", true).is_none());
    }

    #[test]
    fn mobile_archives_are_not_dynamic_libraries() {
        assert_eq!(
            asset_name("ios", "aarch64", "sim", "", "little", true).as_deref(),
            Some("libcronet-ios-arm64_simulator.a")
        );
        assert_eq!(
            asset_name("ios", "x86_64", "", "", "little", true).as_deref(),
            Some("libcronet-ios-amd64_simulator.a")
        );
        for os in ["android", "macos", "ios", "tvos"] {
            assert!(asset_name(os, "aarch64", "", "", "little", false).is_none());
        }
    }

    #[test]
    fn parse_system_libraries_without_accepting_linker_options() {
        assert_eq!(
            windows_libraries("# libs\nws2_32.lib \"crypt32.lib\"\n").unwrap(),
            ["ws2_32", "crypt32"]
        );
        for flags in [
            "",
            "# libs",
            "/wholearchive:foo.lib",
            "../foo.lib",
            "-Wl,foo",
        ] {
            assert!(windows_libraries(flags).is_err());
        }
    }
}
