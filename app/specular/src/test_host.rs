//! The sandboxed decode host for the viewer's tests that need it (SQLite for MBTiles and
//! GeoPackage). `cargo test -p specular` builds only the viewer's test binary, so in a fresh
//! checkout or worktree the host is not next to it and every database read fails ("not an
//! MBTiles file: it has no tiles table"). [`ensure`] builds it into the same target folder
//! and profile first, offline.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Makes sure `specular-decode-host` is where the host client looks (next to the test
/// binary's folder), building it when it is missing. Panics with the reason when it cannot.
pub(crate) fn ensure() {
    static DONE: OnceLock<Result<(), String>> = OnceLock::new();
    if let Err(e) = DONE.get_or_init(build_if_missing) {
        panic!("these tests need the decode host: {e}");
    }
}

fn build_if_missing() -> Result<(), String> {
    let host = specular_sandbox::Host::new(specular_sandbox::Config::from_env());
    if host.exe().is_ok() {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // target/<profile>/deps/specular-<hash>
    let deps = exe.parent().ok_or("no test binary folder")?;
    let profile_dir = if deps.file_name().is_some_and(|n| n == "deps") {
        deps.parent().ok_or("no profile folder")?
    } else {
        deps
    };
    let target = profile_dir.parent().ok_or("no target folder")?;
    let profile = match profile_dir.file_name().and_then(|n| n.to_str()) {
        Some("debug") | None => "dev",
        Some(p) => p,
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args([
            "build",
            "--offline",
            "--quiet",
            "-p",
            "specular-decode-host",
        ])
        .arg("--profile")
        .arg(profile)
        .arg("--target-dir")
        .arg(target)
        .current_dir(workspace())
        .output()
        .map_err(|e| format!("cargo could not run: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().rev().take(20).collect();
        return Err(format!(
            "building specular-decode-host failed:\n{}",
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }
    host.exe().map(|_| ()).map_err(|e| e.to_string())
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_decode_host_is_there_or_built() {
    ensure();
    let host = specular_sandbox::Host::new(specular_sandbox::Config::from_env());
    assert!(host.exe().is_ok_and(|p| p.is_file()));
}
