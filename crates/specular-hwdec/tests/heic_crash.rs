//! The crash canary: a decode that dies in the driver leaves it behind.

mod heic_common;
use heic_common::*;
use specular_hwdec::heic::{CANARY_FILE, Fault, Policy, VaapiCore};
use std::path::PathBuf;

const CHILD: &str = "SPECULAR_HWHEIC_ABORT_CHILD";

fn dir(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// Runs only as the child of `crash_leaves_canary`: aborts inside a hardware decode.
#[test]
fn abort_child() {
    let Some(d) = std::env::var_os(CHILD) else {
        return;
    };
    specular_hwdec::heic::set_cache_dir(PathBuf::from(d));
    let p = grid().unwrap();
    let core = VaapiCore::new()
        .with_policy(Policy::Always)
        .with_fault(Fault::Abort);
    let _ = decode(&p, Some(&core));
    unreachable!("the decode should have aborted");
}

#[test]
fn crash_leaves_canary() {
    scratch("hwheic-crash-parent");
    if !hardware() || grid().is_none() {
        return;
    }
    let d = dir("hwheic-crash");
    let _ = std::fs::remove_dir_all(&d);
    let st = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "abort_child", "--nocapture", "--test-threads=1"])
        .env(CHILD, &d)
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "the child should have died");
    assert!(d.join(CANARY_FILE).is_file(), "no canary after the crash");
    let _ = std::fs::remove_dir_all(&d);
}
