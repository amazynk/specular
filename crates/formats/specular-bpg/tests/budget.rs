//! The HEVC decoder's picture buffer reserves from the memory budget (`specular-hevc`'s
//! DPB): held while an animation keeps pictures for prediction, given back when it is
//! dropped, and refused as a memory need when the budget is too small.

use specular_bpg as bpg;
use specular_core::budget::{MemoryBudget, with_budget, with_limit};
use std::sync::Arc;

fn anim() -> Arc<[u8]> {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/anim-alpha-delays.bpg"
    );
    std::fs::read(p).unwrap().into()
}

#[test]
fn picture_buffer_is_held_while_frames_live() {
    let data = anim();
    let b = Arc::new(MemoryBudget::fixed(1 << 30));
    let frames = with_budget(b.clone(), || {
        let mut f = bpg::Frames::new(data, 1).unwrap();
        f.next_frame(None).unwrap().unwrap();
        f
    });
    assert!(
        b.reserved() > 0,
        "the decoded picture buffer holds no reservation"
    );
    drop(frames);
    assert_eq!(b.reserved(), 0);
}

#[test]
fn picture_buffer_refused_by_a_small_budget() {
    let data = anim();
    let r = with_limit(1024, || {
        let mut f = bpg::Frames::new(data, 1)?;
        f.next_frame(None).unwrap().map(|_| ())
    });
    assert!(matches!(r, Err(bpg::Error::NeedsMemory(_))), "{r:?}");
}
