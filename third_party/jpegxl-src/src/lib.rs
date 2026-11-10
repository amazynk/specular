/*
 * This file is part of jpegxl-rs.
 *
 * jpegxl-rs is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * jpegxl-rs is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with jpegxl-rs.  If not, see <https://www.gnu.org/licenses/>.
 */

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use core::panic;
use std::{
    env,
    path::{Path, PathBuf},
};

fn source_dir() -> PathBuf {
    env::var("DEP_JXL_PATH").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("libjxl"),
        PathBuf::from,
    )
}

/// Specular: the generator the cmake crate will use, looked up in its order:
/// `CMAKE_GENERATOR_<target>`, the same with underscores, `HOST_`/`TARGET_CMAKE_GENERATOR`, then
/// `CMAKE_GENERATOR`.
fn cmake_generator() -> Option<String> {
    let target = env::var("TARGET").unwrap_or_default();
    let host = env::var("HOST").unwrap_or_default();
    let kind = if target == host { "HOST" } else { "TARGET" };
    let vars = [
        format!("CMAKE_GENERATOR_{target}"),
        format!("CMAKE_GENERATOR_{}", target.replace('-', "_")),
        format!("{kind}_CMAKE_GENERATOR"),
        "CMAKE_GENERATOR".to_owned(),
    ];
    // The cmake crate does not ask for a rerun when these change; switching generators must.
    for v in &vars {
        println!("cargo:rerun-if-env-changed={v}");
    }
    vars.iter().find_map(|v| env::var(v).ok())
}

/// Specular: absolute path of `exe` on PATH, with forward slashes (CMake misreads `\` in compiler
/// paths, and a bare name makes it think the compiler changed on every reconfigure).
fn find_on_path(exe: &str) -> Option<String> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(exe))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// Specular: the cmake crate skips configuring whenever `CMakeCache.txt` exists, so a build
/// folder left by the other Windows generator (Ninja or Visual Studio, configured or failed)
/// would be reused and fail the same way again. Such a folder is removed to start afresh.
fn clear_other_generator(ninja: bool) {
    let Some(out) = env::var_os("OUT_DIR") else {
        return;
    };
    let dir = Path::new(&out).join("build");
    let cached = std::fs::read_to_string(dir.join("CMakeCache.txt")).ok().and_then(|c| {
        c.lines()
            .find_map(|l| l.strip_prefix("CMAKE_GENERATOR:INTERNAL="))
            .map(str::to_owned)
    });
    if cached.is_some_and(|g| g.contains("Ninja") != ninja) {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
/// Builds the JPEG XL library.
///
/// # Panics
///
/// This function will panic if the source directory does not exist or is not a directory.
pub fn build() {
    let source = source_dir();
    assert!(
        source.exists() && source.is_dir(),
        "Source directory {} does not exist",
        source.display()
    );

    let mut config = cmake::Config::new(source);
    config
        .define("BUILD_TESTING", "OFF")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("JPEGXL_ENABLE_TOOLS", "OFF")
        .define("JPEGXL_ENABLE_DOXYGEN", "OFF")
        .define("JPEGXL_ENABLE_MANPAGES", "OFF")
        .define("JPEGXL_ENABLE_BENCHMARK", "OFF")
        .define("JPEGXL_ENABLE_EXAMPLES", "OFF")
        .define("JPEGXL_ENABLE_JNI", "OFF")
        .define("JPEGXL_ENABLE_SJPEG", "OFF")
        .define("JPEGXL_ENABLE_OPENEXR", "OFF")
        .define("JPEGXL_BUNDLE_LIBPNG", "OFF");

    if let Ok(p) = std::thread::available_parallelism() {
        config.env("CMAKE_BUILD_PARALLEL_LEVEL", format!("{p}"));
    }

    if cfg!(asan) {
        config
            .env("SANITIZER", "asan")
            .cflag("-g -DADDRESS_SANITIZER -fsanitize=address")
            .cxxflag("-g -DADDRESS_SANITIZER -fsanitize=address")
            .define("JPEGXL_ENABLE_TCMALLOC", "OFF");
    } else if cfg!(tsan) {
        config
            .env("SANITIZER", "tsan")
            .cflag("-g -DTHREAD_SANITIZER -fsanitize=thread")
            .cxxflag("-g -DTHREAD_SANITIZER -fsanitize=thread")
            .define("JPEGXL_ENABLE_TCMALLOC", "OFF");
    }

    #[cfg(windows)]
    const {
        assert!(!cfg!(tsan), "Thread sanitizer is not supported on Windows");
    }
    if cfg!(windows) {
        // For CMake pre-checking
        let mut exeflags = "MSVCRTD.lib".to_string();
        if cfg!(asan) {
            exeflags.push_str(
                " clang_rt.asan_dynamic-x86_64.lib clang_rt.asan_dynamic_runtime_thunk-x86_64.lib",
            );
        }

        let ninja = cmake_generator().is_some_and(|g| g.contains("Ninja"));
        clear_other_generator(ninja);
        if ninja {
            // Specular: Ninja runs clang-cl itself, so Visual Studio's optional ClangCL MSBuild
            // toolset is not needed. Without explicit compilers the cmake crate would pick cl.exe.
            let clang_cl = find_on_path("clang-cl.exe").expect(
                "CMAKE_GENERATOR=Ninja builds libjxl with clang-cl: put LLVM's bin directory on PATH",
            );
            config
                .define("CMAKE_C_COMPILER", &clang_cl)
                .define("CMAKE_CXX_COMPILER", &clang_cl);
        } else {
            config
                .generator_toolset("ClangCL")
                .define(
                    "CMAKE_VS_GLOBALS",
                    "UseMultiToolTask=true;EnforceProcessCountAcrossBuilds=true",
                ); // Enable parallel builds
        }
        // Specular: the C runtime must be the one Rust links (/MD unless +crt-static). /Zl drops
        // the default-library directives, but the STL headers still stamp the runtime into every
        // C++ object, and link.exe refuses a mix with the other C code (LNK2038).
        let crt_static = env::var("CARGO_CFG_TARGET_FEATURE")
            .is_ok_and(|f| f.split(',').any(|f| f == "crt-static"));
        let runtime = if crt_static { "MultiThreaded" } else { "MultiThreadedDLL" };
        config
            .define("CMAKE_MSVC_RUNTIME_LIBRARY", runtime)
            .define("CMAKE_EXE_LINKER_FLAGS", exeflags)
            .cflag("/Zl");
    }

    let prefix = config.build();

    let lib_dir = {
        let mut lib_dir = prefix.join("lib");
        if lib_dir.exists() {
            lib_dir
        } else {
            lib_dir.pop();
            lib_dir.push("lib64");
            if lib_dir.exists() {
                lib_dir
            } else {
                panic!(
                    "Could not find the library directory, please check the files in {prefix:?}"
                );
            }
        }
    };
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    println!("cargo:rustc-link-lib=static=jxl");
    println!("cargo:rustc-link-lib=static=jxl_cms");
    println!("cargo:rustc-link-lib=static=jxl_threads");

    println!("cargo:rustc-link-lib=static=hwy");
    println!("cargo:rustc-link-lib=static=brotlidec");
    println!("cargo:rustc-link-lib=static=brotlienc");
    println!("cargo:rustc-link-lib=static=brotlicommon");

    if cfg!(any(target_vendor = "apple", target_os = "freebsd")) {
        println!("cargo:rustc-link-lib=c++");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=stdc++");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_source_dir() {
        let mut path = source_dir();
        assert!(path.is_dir());

        path.push("lib/include/jxl/codestream_header.h");
        assert!(path.exists());
    }
}
