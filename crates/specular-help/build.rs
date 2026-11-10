//! Embeds the manual's pages (`manual/<locale>/*.md`, English required) so adding a page or a
//! locale needs no code change, and records which commit the build is of (`SPECULAR_BUILD`,
//! when set, wins: packagers set it for builds outside a git checkout).

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::{env, fs};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("manual");
    println!("cargo:rerun-if-changed=manual");

    let mut locales: Vec<String> = fs::read_dir(&root)
        .expect("manual directory")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    locales.sort_by_key(|l| (l != "en", l.clone()));
    assert_eq!(
        locales.first().map(String::as_str),
        Some("en"),
        "manual/en is required"
    );
    let mut out = String::from(
        "/// Embedded pages: (locale, [(file name, source)]), English first.\n\
         pub(crate) const PAGES: &[(&str, &[(&str, &str)])] = &[\n",
    );
    for locale in &locales {
        println!("cargo:rerun-if-changed=manual/{locale}");
        let mut files: Vec<_> = fs::read_dir(root.join(locale))
            .expect("locale directory")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        files.sort();
        let _ = writeln!(out, "    ({locale:?}, &[");
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let abs = f.canonicalize().expect("canonical path");
            let _ = writeln!(out, "        ({name:?}, include_str!({abs:?})),");
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");
    let dest = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("pages.rs");
    fs::write(dest, out).expect("write pages.rs");

    // The commit: short hash and its date, from git when this is a checkout.
    println!("cargo:rerun-if-env-changed=SPECULAR_BUILD");
    let build = env::var("SPECULAR_BUILD").ok().or_else(|| {
        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&manifest)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        };
        // Rebuilt when HEAD moves (the files git keeps it in).
        for p in ["HEAD", "logs/HEAD"] {
            if let Some(path) = git(&["rev-parse", "--path-format=absolute", "--git-path", p]) {
                println!("cargo:rerun-if-changed={path}");
            }
        }
        git(&["log", "-1", "--format=%h %cs"]).filter(|s| !s.is_empty())
    });
    println!(
        "cargo:rustc-env=SPECULAR_BUILD_INFO={}",
        build.unwrap_or_default()
    );
}
