//! The `Task::Inference` calibration hook: the self-test passes, the GPU path becomes
//! available (never trusted: that is up to a model's own verification), and both engines
//! get timed samples. Cancelling stops it.

mod common;

use specular_core::paths::{Engine, Task, Trust, selector};
use specular_infer::calibrate::calibrate;

#[test]
fn calibration_marks_available_and_times_both_engines() {
    let Some(ctx) = common::ctx() else { return };
    let sel = selector();
    let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
    assert!(!calibrate(ctx, 4, &|| true), "cancelled at once");
    assert!(specular_infer::calibrate::missing());
    let t = std::time::Instant::now();
    assert!(calibrate(ctx, 4, &|| false));
    // The cost the app's idle-time calibration adds (`--release --nocapture` for the real one).
    eprintln!(
        "inference calibration: {:.0} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    assert!(!specular_infer::calibrate::missing());
    assert_eq!(sel.trust(Task::Inference, gpu), Trust::Unverified);
    let key = sel.calibration(Task::Inference, gpu, 224 * 224, 0).key;
    assert!(sel.samples(Task::Inference, gpu, key) >= 2);
    assert!(sel.samples(Task::Inference, 0, key) >= 1);
    eprintln!("{}", sel.to_table());
}
