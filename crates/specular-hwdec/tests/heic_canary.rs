//! The crash canary: one left by a previous run turns the hardware HEVC path off (and is
//! consumed), and decoding carries on in software.

mod heic_common;
use heic_common::*;
use specular_core::paths::{Engine, Task, Trust, selector};
use specular_hwdec::heic::{CANARY_FILE, Policy, VaapiCore};

#[test]
fn canary_disables_the_path() {
    let d = scratch("hwheic-canary");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(CANARY_FILE), "1\n").unwrap();
    assert!(specular_hwdec::heic::check_canary());
    assert!(!specular_hwdec::heic::usable());
    assert!(!d.join(CANARY_FILE).exists(), "the canary is consumed");
    let path = Task::HeicDecode.path_of(Engine::HwDecoder).unwrap();
    assert_eq!(selector().trust(Task::HeicDecode, path), Trust::Blocked);
    // Decoding still works: in software.
    if let Some(p) = grid() {
        let core = VaapiCore::new().with_policy(Policy::Always);
        let out = decode(&p, Some(&core)).unwrap();
        assert!(same(&out, &software(&p).unwrap()));
        assert_eq!(core.stats().0, 0);
    }
    let _ = std::fs::remove_dir_all(&d);
}
