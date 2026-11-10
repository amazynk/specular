//! A hardware decode that does not come back in time: software result, device wedged.

mod heic_common;
use heic_common::*;
use specular_hwdec::heic::{Fault, Policy, VaapiCore};
use std::time::{Duration, Instant};

#[test]
fn hang_times_out_to_software() {
    let dir = scratch("hwheic-timeout");
    if !hardware() {
        return;
    }
    let Some(p) = grid() else {
        eprintln!("skipped: no grid sample");
        return;
    };
    let sw = software(&p).unwrap();
    let timeout = Duration::from_millis(300);
    let core = VaapiCore::new()
        .with_policy(Policy::Always)
        .with_timeout(timeout)
        .with_fault(Fault::Hang);
    let t0 = Instant::now();
    let hw = decode(&p, Some(&core)).unwrap();
    let took = t0.elapsed();
    assert!(same(&hw, &sw));
    assert!(specular_hwdec::heic::wedged());
    assert!(!specular_hwdec::heic::usable());
    // Timeout plus the software decode, not the 3 s the worker hangs.
    assert!(took < Duration::from_millis(2500), "took {took:?}");
    let _ = std::fs::remove_dir_all(dir);
}
