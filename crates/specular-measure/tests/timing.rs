//! Timings on a 24-megapixel image (`cargo test -p specular-measure --test timing -- --ignored
//! --nocapture`): ROI statistics and a long line profile.

use std::time::Instant;

use specular_measure::calib::Calibration;
use specular_measure::geom::{P, Shape};
use specular_measure::raster::{Samples, View};
use specular_measure::{profile, stats};

fn best<T>(mut f: impl FnMut() -> T) -> (f64, T) {
    let mut out = f();
    let mut t = f64::INFINITY;
    for _ in 0..5 {
        let s = Instant::now();
        out = f();
        t = t.min(s.elapsed().as_secs_f64() * 1e3);
    }
    (t, out)
}

#[test]
#[ignore = "timing"]
fn timing_24mp() {
    let (w, h) = (6000u32, 4000u32);
    let rgb: Vec<u8> = (0..w as usize * h as usize * 3)
        .map(|i| ((i * 2654435761) >> 24) as u8)
        .collect();
    let gray16: Vec<u16> = (0..w as usize * h as usize)
        .map(|i| ((i * 2654435761) >> 16) as u16)
        .collect();
    let gray32: Vec<f32> = gray16.iter().map(|&v| f32::from(v) * 0.5).collect();
    let views = [
        ("RGB 8-bit", View::new(w, h, 3, Samples::U8(&rgb)).unwrap()),
        (
            "gray 16-bit",
            View::new(w, h, 1, Samples::U16(&gray16)).unwrap(),
        ),
        (
            "gray f32",
            View::new(w, h, 1, Samples::F32(&gray32)).unwrap(),
        ),
    ];
    let shapes = [
        (
            "whole-image rect",
            Shape::rect(P::new(0.0, 0.0), P::new(6000.0, 4000.0)),
        ),
        (
            "inscribed ellipse",
            Shape::ellipse_in(P::new(0.0, 0.0), P::new(6000.0, 4000.0)),
        ),
        (
            "freehand, 2000 points",
            Shape::Freehand(
                (0..2000)
                    .map(|i| {
                        let t = i as f64 / 2000.0 * std::f64::consts::TAU;
                        let r = 1.0 + 0.1 * (t * 13.0).sin();
                        P::new(
                            3000.0 + 2900.0 * r * t.cos() / 1.1,
                            2000.0 + 1900.0 * r * t.sin() / 1.1,
                        )
                    })
                    .collect(),
            ),
        ),
    ];
    for (vn, v) in &views {
        for (sn, s) in &shapes {
            let (ms, st) = best(|| stats::roi(s, v, 256).unwrap());
            println!("stats {vn:12} {sn:22} {ms:8.1} ms  ({} px)", st[0].count);
        }
    }
    let cal = Calibration::uncalibrated();
    let line = [P::new(0.5, 0.5), P::new(5999.5, 3999.5)];
    for width in [1, 5] {
        let (ms, p) = best(|| profile::along(&line, &views[0].1, &cal, 1.0, width));
        println!(
            "profile RGB diagonal width {width}: {ms:.2} ms ({} samples)",
            p.len()
        );
    }
}
