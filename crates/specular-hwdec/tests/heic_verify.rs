//! The first-decode check: a hardware result that differs from software is caught, the
//! software result is used and the path is switched off.

mod heic_common;
use heic_common::*;
use specular_core::paths::{Engine, Task, Trust, selector};
use specular_hwdec::heic::{Fault, VaapiCore};

#[test]
fn mismatch_is_caught_by_the_first_decode_check() {
    // Selector policy: the first batch is compared with software.
    let dir = scratch("hwheic-verify");
    if !hardware() {
        return;
    }
    let Some(p) = grid() else {
        return;
    };
    specular_hwdec::set_enabled(true);
    selector().set_available(Task::HeicDecode, Engine::HwDecoder);
    selector().set_override(Task::HeicDecode, specular_core::paths::Override::On);
    let sw = software(&p).unwrap();
    let core = VaapiCore::new().with_fault(Fault::Corrupt);
    let out = decode(&p, Some(&core)).unwrap();
    assert!(same(&out, &sw), "the checked batch delivers software");
    assert!(
        core.stats().0 > 0,
        "the override should have sent it to the hardware"
    );
    assert!(
        !specular_hwdec::heic::usable(),
        "a mismatch turns the path off"
    );
    let path = Task::HeicDecode.path_of(Engine::HwDecoder).unwrap();
    assert_eq!(selector().trust(Task::HeicDecode, path), Trust::Blocked);
    let _ = std::fs::remove_dir_all(dir);
}
