//! Builds the vendored UnRAR with Specular's handle-I/O and password patches.
//!
//! `vendor/unrar` stays pristine: it is copied to `OUT_DIR`, `patches/*.patch` are applied to the
//! copy (exact context, any mismatch fails the build), and the copy is compiled. Unlike upstream's
//! build script, target-specific sources and libraries follow the *target* (`CARGO_CFG_*`), not
//! the host, so cross builds and cross checks see the right file list.
//!
//! UnRAR source code may be used in any software to handle
//! RAR archives without limitations free of charge, but cannot be
//! used to develop RAR (WinRAR) compatible archiver and to
//! re-create RAR compression algorithm, which is proprietary.
//! Distribution of modified UnRAR source code in separate form
//! or as a part of other software is permitted, provided that
//! full text of this paragraph, starting from "UnRAR source code"
//! words, is included in license, or in documentation if license
//! is not available, and in source code comments of resulting package.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const PATCHES: &[&str] = &[
    "patches/specular-handle-io.patch",
    "patches/specular-password.patch",
    "patches/specular-no-win32k.patch",
];

/// Specular's own sources, copied next to UnRAR's (specular-no-win32k.patch includes the
/// header from os.hpp); the `.cpp` files are compiled for Windows only.
const SPECULAR_FILES: &[&str] = &["specular_k32.hpp", "specular_k32.cpp"];

const SOURCES: &[&str] = &[
    "strlist",
    "strfn",
    "pathfn",
    "smallfn",
    "global",
    "file",
    "filefn",
    "filcreat",
    "archive",
    "arcread",
    "unicode",
    "system",
    "crypt",
    "crc",
    "rawread",
    "encname",
    "match",
    "timefn",
    "rdwrfn",
    "consio",
    "options",
    "errhnd",
    "rarvm",
    "secpassword",
    "rijndael",
    "getbits",
    "sha1",
    "sha256",
    "blake2s",
    "hash",
    "extinfo",
    "extract",
    "volume",
    "list",
    "find",
    "unpack",
    "headers",
    "threadpool",
    "rs16",
    "cmddata",
    "ui",
    "filestr",
    "scantree",
    "dll",
    "qopen",
];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let windows = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let gnu = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|e| e == "gnu");
    // Must match the C++ runtime `src/lib.rs` links: libc++ only on macOS and Free/OpenBSD.
    // GCC >= 15 accepts `-stdlib=libc++` too, so probing support would wrongly enable it on Linux.
    let libcxx = env::var("CARGO_CFG_TARGET_OS")
        .is_ok_and(|os| matches!(os.as_str(), "macos" | "freebsd" | "openbsd"));

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor/unrar");
    println!("cargo:rerun-if-changed=specular");
    for patch in PATCHES {
        println!("cargo:rerun-if-changed={patch}");
    }

    let src = out.join("unrar");
    if src.exists() {
        fs::remove_dir_all(&src).expect("clear patched UnRAR copy");
    }
    copy_dir(&manifest.join("vendor/unrar"), &src);
    for patch in PATCHES {
        let text = fs::read_to_string(manifest.join(patch)).expect("read patch");
        apply(&text, &out, patch);
    }
    for f in SPECULAR_FILES {
        fs::copy(manifest.join("specular").join(f), src.join(f)).expect("copy Specular source");
    }

    if windows {
        println!("cargo:rustc-link-lib=powrprof");
        println!("cargo:rustc-link-lib=shell32");
        // Registry and CryptoAPI (GetRarDataPath, GetRnd). Rust's std no longer links advapi32,
        // so a build of the decode host alone left them unresolved (LNK2019).
        println!("cargo:rustc-link-lib=advapi32");
        if gnu {
            println!("cargo:rustc-link-lib=pthread");
        }
    } else {
        println!("cargo:rustc-link-lib=pthread");
    }

    let mut files: Vec<PathBuf> = SOURCES
        .iter()
        .map(|s| src.join(format!("{s}.cpp")))
        .collect();
    if windows {
        files.push(src.join("isnt.cpp"));
        files.extend(
            SPECULAR_FILES
                .iter()
                .filter(|f| f.ends_with(".cpp"))
                .map(|f| src.join(f)),
        );
    }
    let mut build = cc::Build::new();
    if libcxx {
        build.flag_if_supported("-stdlib=libc++");
    }
    build
        .cpp(true)
        .opt_level(2)
        .std("c++14")
        // cc would link the dynamic C++ runtime by default, which breaks windows-gnu.
        .cpp_link_stdlib(None)
        .warnings(false)
        .extra_warnings(false)
        .flag_if_supported("-fPIC")
        .flag_if_supported("-Wno-switch")
        .flag_if_supported("-Wno-parentheses")
        .flag_if_supported("-Wno-macro-redefined")
        .flag_if_supported("-Wno-dangling-else")
        .flag_if_supported("-Wno-logical-op-parentheses")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-variable")
        .flag_if_supported("-Wno-unused-function")
        .flag_if_supported("-Wno-missing-braces")
        .flag_if_supported("-Wno-unknown-pragmas")
        .flag_if_supported("-Wno-deprecated-declarations")
        .define("_FILE_OFFSET_BITS", Some("64"))
        .define("_LARGEFILE_SOURCE", None)
        .define("RAR_SMP", None)
        .define("RARDLL", None)
        .files(&files)
        .compile("libunrar.a");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create patched UnRAR dir");
    for entry in fs::read_dir(from).expect("read vendor/unrar") {
        let entry = entry.expect("read vendor/unrar entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy UnRAR source");
        }
    }
}

/// Applies a unified diff whose paths are `a/<path>` / `b/<path>` relative to the crate root,
/// rooted at `root` instead of the crate root, except that `vendor/` maps to `root`. Text before
/// the first `--- ` line is commentary. Every hunk must match exactly where it says it applies
/// (after earlier hunks' line shifts); otherwise the build fails.
fn apply(patch: &str, root: &Path, name: &str) {
    let lines: Vec<&str> = patch.lines().collect();
    let mut i = lines
        .iter()
        .position(|l| l.starts_with("--- "))
        .unwrap_or(lines.len());
    let mut applied = 0;
    while i < lines.len() {
        let old = lines[i]
            .strip_prefix("--- ")
            .unwrap_or_else(|| fail(name, i, "expected ---"));
        let new = lines
            .get(i + 1)
            .and_then(|l| l.strip_prefix("+++ "))
            .unwrap_or_else(|| fail(name, i + 1, "expected +++"));
        let old = path_of(old, "a/").unwrap_or_else(|| fail(name, i, "bad --- path"));
        let new = path_of(new, "b/").unwrap_or_else(|| fail(name, i + 1, "bad +++ path"));
        if old != new {
            fail(name, i, "renames are not supported");
        }
        let rel = old
            .strip_prefix("vendor/")
            .unwrap_or_else(|| fail(name, i, "path outside vendor/"));
        if rel
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == "..")
        {
            fail(name, i, "unsafe path");
        }
        let file = root.join(rel);
        let text = fs::read_to_string(&file).unwrap_or_else(|_| fail(name, i, "file missing"));
        let trailing_newline = text.ends_with('\n');
        let mut body: Vec<String> = text.lines().map(str::to_owned).collect();
        i += 2;
        let mut shift: isize = 0;
        while i < lines.len() && lines[i].starts_with("@@ ") {
            let (old_start, old_len, new_len) =
                hunk_header(lines[i]).unwrap_or_else(|| fail(name, i, "bad hunk header"));
            i += 1;
            let mut from = Vec::new();
            let mut to = Vec::new();
            while from.len() < old_len || to.len() < new_len {
                let line = *lines
                    .get(i)
                    .unwrap_or_else(|| fail(name, i, "hunk runs past the end"));
                match line.as_bytes().first() {
                    Some(b' ') => {
                        from.push(&line[1..]);
                        to.push(&line[1..]);
                    }
                    Some(b'-') => from.push(&line[1..]),
                    Some(b'+') => to.push(&line[1..]),
                    None => {
                        from.push("");
                        to.push("");
                    }
                    _ => fail(name, i, "bad hunk line"),
                }
                i += 1;
            }
            if from.len() != old_len || to.len() != new_len {
                fail(name, i, "hunk length mismatch");
            }
            if lines.get(i).is_some_and(|l| l.starts_with('\\')) {
                fail(name, i, "\"No newline at end of file\" is not supported");
            }
            let at = (old_start.saturating_sub(1) as isize + shift) as usize;
            let matches = body
                .get(at..at + old_len)
                .is_some_and(|have| have.iter().zip(&from).all(|(h, f)| h == f));
            if !matches {
                fail(
                    name,
                    i,
                    &format!("hunk at {}:{old_start} does not match", rel),
                );
            }
            body.splice(at..at + old_len, to.iter().map(|s| (*s).to_owned()));
            shift += new_len as isize - old_len as isize;
            applied += 1;
        }
        let mut out = body.join("\n");
        if trailing_newline {
            out.push('\n');
        }
        fs::write(&file, out).expect("write patched UnRAR source");
    }
    if applied == 0 {
        fail(name, 0, "no hunks");
    }
}

fn path_of<'a>(field: &'a str, prefix: &str) -> Option<&'a str> {
    field.split('\t').next()?.strip_prefix(prefix)
}

fn hunk_header(line: &str) -> Option<(usize, usize, usize)> {
    let mut parts = line.strip_prefix("@@ -")?.split(' ');
    let range = |s: &str| -> Option<(usize, usize)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (old_start, old_len) = range(parts.next()?)?;
    let (_, new_len) = range(parts.next()?.strip_prefix('+')?)?;
    Some((old_start, old_len, new_len))
}

fn fail(patch: &str, line: usize, why: &str) -> ! {
    panic!("{patch}: line {}: {why}", line + 1);
}
