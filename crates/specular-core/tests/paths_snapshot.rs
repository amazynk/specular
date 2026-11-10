//! The Settings snapshot (`PathSelector::snapshot`) over recorded timing tables: what each
//! task chose on a machine and the reason the Performance page shows for it.

use specular_core::paths::{
    Confidence, Engine, Failure, GpuClass, Mode, Override, PathSelector, Setting, Task, Trust, Why,
};

const RADEON_860M: &str = include_str!("paths/radeon-860m.txt");
const FAST_DGPU: &str = include_str!("paths/fast-dgpu.txt");
const BLOCKED: &str = include_str!("paths/blocked-hwdec.txt");
const SOFTWARE: &str = include_str!("paths/software-gpu.txt");

fn loaded(table: &str) -> PathSelector {
    let s = PathSelector::new();
    assert!(s.load_table(table, None));
    s
}

#[test]
fn the_860m_keeps_the_viewed_image_on_the_cpu_and_prefetches_in_hardware() {
    let s = loaded(RADEON_860M);
    let snap = s.snapshot();
    assert_eq!(snap.machine.gpu, "AMD Radeon 860M Graphics (RADV GFX1152)");
    assert_eq!(snap.machine.driver, "radv Mesa 25.2");
    assert_eq!(snap.machine.cpu, "AMD Ryzen AI 7 PRO 350 w/ Radeon 860M");
    assert_eq!((snap.gpu_class, snap.threads), (GpuClass::Integrated, 16));
    assert!(snap.calibrated && !snap.power_saving);

    let jpeg = snap.task(Task::JpegDecode);
    assert!(jpeg.active);
    assert_eq!(jpeg.interactive.engine, Engine::Cpu);
    let Why::Measured(c) = jpeg.interactive.why else {
        panic!("{:?}", jpeg.interactive.why)
    };
    assert_eq!(c.other_engine, Engine::HwDecoder);
    assert!(c.chosen.wall_us * 3.0 < c.other.wall_us, "{c:?}");
    assert_eq!(jpeg.background.engine, Engine::HwDecoder);
    let Why::Measured(c) = jpeg.background.why else {
        panic!("{:?}", jpeg.background.why)
    };
    assert!(c.chosen.cpu_us * 5.0 < c.other.cpu_us, "{c:?}");
    assert_eq!(jpeg.paths[0].samples, 130);
    assert_eq!(jpeg.paths[0].confidence, Confidence::High);
    assert_eq!(jpeg.paths[1].confidence, Confidence::High);

    let colour = snap.task(Task::YcbcrToRgb);
    assert_eq!(colour.interactive.engine, Engine::Gpu);
    assert!(matches!(colour.interactive.why, Why::Measured(_)));
    assert_eq!(colour.paths[1].confidence, Confidence::Low);

    // Hooks with nothing registered are not listed.
    assert!(!snap.task(Task::Inference).active);
    assert!(!snap.any_blocked());
}

#[test]
fn a_fast_discrete_gpu_decodes_the_viewed_image_in_hardware() {
    let snap = loaded(FAST_DGPU).snapshot();
    let jpeg = snap.task(Task::JpegDecode);
    assert_eq!(jpeg.interactive.engine, Engine::HwDecoder);
    assert!(matches!(jpeg.interactive.why, Why::Measured(c) if c.other_engine == Engine::Cpu));
}

#[test]
fn blocked_and_unchecked_paths_are_reported_with_when_and_why() {
    let s = loaded(BLOCKED);
    let snap = s.snapshot();
    assert!(snap.any_blocked());
    let jpeg = snap.task(Task::JpegDecode);
    for d in [&jpeg.interactive, &jpeg.background] {
        assert_eq!(d.engine, Engine::Cpu);
        let Why::Blocked { engine, blocked } = d.why else {
            panic!("{:?}", d.why)
        };
        assert_eq!(engine, Engine::HwDecoder);
        assert_eq!(blocked.failure, Failure::CheckFailed);
        assert_eq!(blocked.at_unix, 1_759_311_000);
    }
    let (path, b) = jpeg.blocked().next().unwrap();
    assert_eq!(
        (path.engine, path.trust),
        (Engine::HwDecoder, Trust::Blocked)
    );
    assert_eq!(b.failure, Failure::CheckFailed);

    // Not verified yet: the image on screen stays on the CPU while background work checks it.
    let colour = snap.task(Task::YcbcrToRgb);
    assert_eq!(colour.interactive.engine, Engine::Cpu);
    assert_eq!(
        colour.interactive.why,
        Why::Checking {
            engine: Engine::Gpu
        }
    );

    // Trusted but only one side measured: "not measured yet".
    let png = snap.task(Task::PngUnfilter);
    assert!(
        matches!(png.interactive.why, Why::NotMeasured(_)),
        "{:?}",
        png.interactive.why
    );
    assert_eq!(png.paths[1].confidence, Confidence::None);

    // Retry: back to unverified and checked again; the table no longer lists it.
    assert!(s.retry(Task::JpegDecode, Engine::HwDecoder));
    let snap = s.snapshot();
    assert!(!snap.any_blocked());
    assert_eq!(
        snap.task(Task::JpegDecode).interactive.why,
        Why::Checking {
            engine: Engine::HwDecoder
        }
    );
    assert!(!snap.calibrated);
}

#[test]
fn settings_override_the_measurements() {
    let s = loaded(RADEON_860M);
    s.set_mode(Mode::CpuOnly);
    let snap = s.snapshot();
    for task in [Task::JpegDecode, Task::YcbcrToRgb] {
        let t = snap.task(task);
        assert_eq!(t.interactive.engine, Engine::Cpu);
        assert_eq!(
            t.interactive.why,
            Why::Setting(Setting::Processing(Mode::CpuOnly))
        );
        assert_eq!(
            t.background.why,
            Why::Setting(Setting::Processing(Mode::CpuOnly))
        );
    }
    // CPU only leaves the CPU variants to the measurements.
    assert!(matches!(
        snap.task(Task::PngUnfilter).interactive.why,
        Why::Measured(_)
    ));
    s.set_mode(Mode::PreferGpu);
    let jpeg = s.snapshot().task(Task::JpegDecode).interactive;
    assert_eq!(jpeg.engine, Engine::HwDecoder);
    assert_eq!(jpeg.why, Why::Setting(Setting::Processing(Mode::PreferGpu)));
    s.set_mode(Mode::Automatic);
    s.set_override(Task::JpegDecode, Override::On);
    let jpeg = s.snapshot().task(Task::JpegDecode).interactive;
    assert_eq!(jpeg.engine, Engine::HwDecoder);
    assert_eq!(
        jpeg.why,
        Why::Setting(Setting::Override(Task::JpegDecode, Override::On))
    );
}

#[test]
fn nothing_but_the_cpu_on_a_software_renderer() {
    let snap = loaded(SOFTWARE).snapshot();
    assert_eq!(snap.gpu_class, GpuClass::Software);
    assert!(!snap.calibrated);
    let jpeg = snap.task(Task::JpegDecode);
    assert!(jpeg.active);
    assert_eq!(
        jpeg.interactive.why,
        Why::Unavailable {
            engine: Engine::HwDecoder
        }
    );
    assert_eq!(jpeg.paths[0].confidence, Confidence::Low);
    assert!(!snap.task(Task::YcbcrToRgb).active);
}

#[test]
fn measure_again_resets_the_table_and_the_snapshot() {
    let s = loaded(BLOCKED);
    s.reset_measurements();
    let table = s.to_table();
    assert!(!table.contains("\ncell "), "{table}");
    assert!(!table.contains("\nblocked "), "{table}");
    assert!(table.contains("calibrated false"));
    assert!(table.contains("trust jpeg_decode hw_decoder unverified"));
    assert!(table.contains("trust png_unfilter cpu_parallel unverified"));
    // Same machine: the reset table loads for it.
    assert!(PathSelector::new().load_table(&table, Some(&s.fingerprint())));
    let snap = s.snapshot();
    assert!(!snap.any_blocked());
    for t in &snap.tasks {
        assert!(
            t.paths
                .iter()
                .all(|p| p.samples == 0 && p.confidence == Confidence::None)
        );
    }
}

#[test]
fn snapshots_leave_the_selector_alone() {
    let s = loaded(RADEON_860M);
    let before = s.to_table();
    let _ = s.take_dirty();
    for _ in 0..64 {
        let _ = s.snapshot();
    }
    assert_eq!(s.to_table(), before);
    assert_eq!(s.take_dirty(), 0);
    assert_eq!(s.inflight(), 0);
}
