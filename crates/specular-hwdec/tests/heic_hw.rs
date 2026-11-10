//! The hardware HEVC decoder (VA-API, D3D12) against the software core: bit-exact over the
//! HEIC corpus. Skipped without a hardware HEVC decoder.

mod heic_common;
use heic_common::*;
use specular_hwdec::heic::{Policy, VaapiCore};

#[test]
fn bit_exact_over_corpus() {
    scratch("hwheic-hw");
    if !hardware() {
        return;
    }
    let files = corpus();
    let (mut checked, mut on_hw, mut mismatched) = (0, 0, Vec::new());
    for p in &files {
        let Some(sw) = software(p) else {
            continue;
        };
        let core = VaapiCore::new().with_policy(Policy::Always);
        let hw = decode(p, Some(&core)).expect("software decodes it, so the hardware core must");
        let (h, s) = core.stats();
        checked += 1;
        on_hw += usize::from(h > 0);
        if std::env::var_os("HWHEIC_VERBOSE").is_some() {
            eprintln!("{:>4} hw {:>4} sw  {}", h, s, p.display());
        }
        if !same(&hw, &sw) {
            mismatched.push(p.display().to_string());
        }
    }
    eprintln!(
        "{checked} files checked ({} in the corpus), {on_hw} decoded at least partly on the hardware",
        files.len()
    );
    assert!(
        mismatched.is_empty(),
        "differs from software: {mismatched:#?}"
    );
    if checked > 0 {
        assert!(on_hw > 0, "nothing went to the hardware");
    }
}

/// Unsupported chroma (4:4:4, 4:2:2, monochrome …) goes to software, per picture.
#[test]
fn unsupported_chroma_falls_back() {
    scratch("hwheic-hw");
    if !hardware() {
        return;
    }
    let Some(p) = find(&[
        "target/agent-jpeg/heic-eval/gen/mid_444.heic",
        "target/agent-jpeg/heic-eval/gen/mid_422.heic",
    ]) else {
        eprintln!("skipped: no 4:4:4 / 4:2:2 sample");
        return;
    };
    let sw = software(&p).unwrap();
    let core = VaapiCore::new().with_policy(Policy::Always);
    let hw = decode(&p, Some(&core)).unwrap();
    assert!(same(&hw, &sw));
    let (h, s) = core.stats();
    assert_eq!(h, 0, "4:4:4 must not go to the hardware");
    assert!(s > 0);
}
