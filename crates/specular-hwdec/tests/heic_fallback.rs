//! A driver error falls back to software for the whole batch and blocklists the path.

mod heic_common;
use heic_common::*;
use specular_core::paths::{Engine, Task, Trust, selector};
use specular_hwdec::heic::{Fault, Policy, VaapiCore};

#[test]
fn driver_error_falls_back_and_blocks() {
    let dir = scratch("hwheic-fallback");
    if !hardware() {
        return;
    }
    let Some(p) = grid() else {
        eprintln!("skipped: no grid sample");
        return;
    };
    let sw = software(&p).unwrap();
    let core = VaapiCore::new()
        .with_policy(Policy::Always)
        .with_fault(Fault::SubmitError);
    let hw = decode(&p, Some(&core)).unwrap();
    assert!(same(&hw, &sw), "the fallback must give the software result");
    assert_eq!(core.stats().0, 0);
    let path = Task::HeicDecode.path_of(Engine::HwDecoder).unwrap();
    assert_eq!(selector().trust(Task::HeicDecode, path), Trust::Blocked);
    assert!(
        !dir.join(specular_hwdec::heic::CANARY_FILE).exists(),
        "canary left behind"
    );
    let _ = std::fs::remove_dir_all(dir);
}
