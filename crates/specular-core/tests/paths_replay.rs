//! Replays recorded timing tables through the path selector (DESIGN.md §12.3.1): the same
//! requests on two machines give different paths, deterministically.

use specular_core::paths::{Engine, Mode, Objective, PathSelector, Reason, Request, Task};

const RADEON_860M: &str = include_str!("paths/radeon-860m.txt");
const FAST_DGPU: &str = include_str!("paths/fast-dgpu.txt");

/// 24 MP (bucket 4) and 6 MP (bucket 3) photos.
const PHOTO: u64 = 6000 * 4000;
const SMALL: u64 = 3000 * 2000;

fn load(sel: &PathSelector, table: &str) {
    assert!(sel.load_table(table, None));
    assert!(sel.calibrated());
}

fn engine(sel: &PathSelector, task: Task, pixels: u64, variant: u8, o: Objective) -> Engine {
    sel.choose(task, &Request::new(pixels, variant).objective(o))
        .engine
}

#[test]
fn a_fast_discrete_gpu_takes_paths_the_860m_does_not() {
    let (igpu, dgpu) = (PathSelector::new(), PathSelector::new());
    load(&igpu, RADEON_860M);
    load(&dgpu, FAST_DGPU);
    for px in [PHOTO, SMALL] {
        // The image the user waits for: our decoder on the 860M (four times faster than its
        // hardware decoder), the hardware decoder on the discrete GPU.
        assert_eq!(
            engine(&igpu, Task::JpegDecode, px, 0, Objective::Interactive),
            Engine::Cpu
        );
        assert_eq!(
            engine(&dgpu, Task::JpegDecode, px, 0, Objective::Interactive),
            Engine::HwDecoder
        );
        // Prefetch: the hardware decoder on both (a fraction of the CPU time, keeps up).
        assert_eq!(
            engine(&igpu, Task::JpegDecode, px, 0, Objective::Background),
            Engine::HwDecoder
        );
        assert_eq!(
            engine(&dgpu, Task::JpegDecode, px, 0, Objective::Background),
            Engine::HwDecoder
        );
        // Shader colour conversion wins on both.
        for o in [Objective::Interactive, Objective::Background] {
            assert_eq!(engine(&igpu, Task::YcbcrToRgb, px, 0, o), Engine::Gpu);
            assert_eq!(engine(&dgpu, Task::YcbcrToRgb, px, 0, o), Engine::Gpu);
        }
    }
    let c = dgpu.choose(
        Task::JpegDecode,
        &Request::new(PHOTO, 0).objective(Objective::Interactive),
    );
    assert_eq!(c.reason, Reason::Faster);
    let c = igpu.choose(
        Task::JpegDecode,
        &Request::new(PHOTO, 0).objective(Objective::Background),
    );
    assert_eq!(c.reason, Reason::Cheaper);
}

#[test]
fn replay_is_deterministic_and_round_trips() {
    let a = PathSelector::new();
    load(&a, RADEON_860M);
    let b = PathSelector::new();
    load(&b, &a.to_table());
    assert_eq!(a.to_table(), b.to_table());
    for task in Task::ALL {
        for o in [Objective::Interactive, Objective::Background] {
            for px in [1 << 16, 1 << 20, SMALL, PHOTO, 100 << 20] {
                // The first 15 background requests never explore.
                let (x, y) = (
                    a.choose(task, &Request::new(px, 0).objective(o)),
                    b.choose(task, &Request::new(px, 0).objective(o)),
                );
                assert_eq!(
                    (x.engine, x.reason),
                    (y.engine, y.reason),
                    "{task:?} {o:?} {px}"
                );
            }
        }
    }
}

#[test]
fn modes_and_power_change_the_replayed_choice() {
    let s = PathSelector::new();
    load(&s, FAST_DGPU);
    s.set_mode(Mode::CpuOnly);
    let c = s.choose(
        Task::JpegDecode,
        &Request::new(PHOTO, 0).objective(Objective::Interactive),
    );
    assert_eq!((c.engine, c.reason), (Engine::Cpu, Reason::CpuOnly));
    s.set_mode(Mode::Automatic);
    // On battery the discrete GPU's decoder still wins: faster and far less CPU.
    s.set_power_saving(true);
    assert_eq!(
        engine(&s, Task::JpegDecode, PHOTO, 0, Objective::Interactive),
        Engine::HwDecoder
    );
}

#[test]
fn a_different_machine_does_not_load_the_table() {
    let s = PathSelector::new();
    assert!(!s.load_table(RADEON_860M, Some("another GPU|driver|CPU|0.1.0")));
    assert!(!s.calibrated());
}

#[test]
fn png_wavefront_wins_on_flat_images_only() {
    let s = PathSelector::new();
    load(&s, RADEON_860M);
    // Screenshot-like (32:1 and above): the wavefront halves the wall time here, and on this
    // machine it even costs less CPU time, so prefetch takes it too.
    assert_eq!(
        engine(&s, Task::PngUnfilter, 1 << 22, 3, Objective::Interactive),
        Engine::CpuParallel
    );
    assert_eq!(
        engine(&s, Task::PngUnfilter, 1 << 22, 3, Objective::Background),
        Engine::CpuParallel
    );
    // Photo-like (below 4:1): inflate dominates and the wavefront only adds overhead.
    for o in [Objective::Interactive, Objective::Background] {
        assert_eq!(engine(&s, Task::PngUnfilter, 1 << 22, 0, o), Engine::Cpu);
    }
}
