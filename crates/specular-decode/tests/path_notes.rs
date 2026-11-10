//! The path notes a decode leaves for the open report (DESIGN.md §12.7) for decodes that run
//! before the hardware decoder probe has registered anything: a HEIC opened at start-up is
//! decoded in software, and the note says the hardware path was not offered yet.

use specular_core::paths::{Engine, Reason, Task, Trust, take_note};
use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn heic_before_the_hardware_probe_notes_the_software_path() {
    // Nothing in this process probes: the probe is still pending as at start-up.
    assert!(specular_hwdec::probed().is_none());
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/alpha/heic8.heic");
    let data = std::fs::read(&file).unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("heic8.heic", data)))
        .unwrap();
    let _ = take_note();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(r.width > 0);
    let n = take_note().expect("a HEIC decode notes its path");
    assert_eq!(n.choice.task, Task::HeicDecode);
    assert_eq!(n.choice.engine, Engine::Cpu);
    assert_eq!(n.choice.reason, Reason::Only);
    // The hardware path was not registered when it was decided (whatever this machine has).
    assert_eq!(n.alt_trust, Trust::Absent);
    assert_eq!(n.fallback, None);
    assert!(specular_hwdec::probed().is_none());
}
