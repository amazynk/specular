//! The export encoder as the app makes it ([`VideoEncoder::new`]): after the hardware probe
//! Automatic takes the GPU's encoder and checks it on the first picture; "CPU only" never
//! takes it. Skipped without a hardware H.264 encoder (`SPECULAR_REQUIRE_VAAPI_ENC=1` makes
//! it a failure). A file of its own: it needs the process-wide path selector untouched.

use specular_core::paths::{Engine, Mode, Task, Trust, selector};
use specular_h264enc::{Config, Frame};
use specular_video::encode::{Used, VideoEncoder};

#[test]
fn automatic_takes_the_hardware_and_cpu_only_does_not() {
    if specular_hwdec::probe()
        .and_then(|c| c.h264_encode.as_ref())
        .is_none()
    {
        let required = std::env::var("SPECULAR_REQUIRE_VAAPI_ENC").is_ok_and(|v| v == "1");
        assert!(
            !required,
            "SPECULAR_REQUIRE_VAAPI_ENC=1 but no hardware H.264 encoder"
        );
        eprintln!("no hardware H.264 encoder; skipped");
        return;
    }
    let (w, h) = (1280usize, 720usize);
    let y: Vec<u8> = (0..w * h)
        .map(|i| (16 + (i % w + i / w) % 200) as u8)
        .collect();
    let c = vec![128u8; w * h / 4];
    let f = Frame {
        y: &y,
        u: &c,
        v: &c,
        y_stride: w,
        c_stride: w / 2,
    };
    let cfg = Config::new(w as u32, h as u32, 30, 1, 24);

    assert_eq!(selector().trust(Task::VideoEncode, 1), Trust::Unverified);
    let mut enc = VideoEncoder::new(cfg.clone()).unwrap();
    assert_eq!(enc.engine(), Engine::HwEncoder);
    for i in 0..3 {
        assert!(enc.encode(&f, false).key == (i == 0));
    }
    assert_eq!(enc.used(), Used::Hardware, "{:?}", enc.fell_back());
    assert_eq!(selector().trust(Task::VideoEncode, 1), Trust::Trusted);

    selector().set_mode(Mode::CpuOnly);
    let enc = VideoEncoder::new(cfg).unwrap();
    assert_eq!(enc.engine(), Engine::Cpu);
}
