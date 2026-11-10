//! Local copies' proxies ([`specular_develop::proxy`]) against an independent reference: a
//! synthetic Bayer DNG (and a LinearRaw one) is binned by the test itself from the samples it
//! wrote, and the proxy, read back through `specular-raw` as any DNG, must hold the same
//! linear values (half-float precision), at the long edge asked for, with the camera's
//! colour, white balance and orientation.

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::path::Path;

use common::{DngCodec, DngSpec, write_dng};
use specular_develop::proxy;
use specular_raw::sensor::Layout;

/// A smooth Bayer scene: each colour its own ramp, so binning errors show.
fn scene(w: u32, h: u32, black: u32, white: u32) -> Vec<u16> {
    let span = (white - black) as f32;
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let c = ((y & 1) * 2 + (x & 1)) as f32;
            let v =
                0.05 + 0.8 * (x as f32 / w as f32) * (0.5 + 0.5 * y as f32 / h as f32) + 0.03 * c;
            (black as f32 + v.min(1.0) * span) as u16
        })
        .collect()
}

/// The proxy's pixels as the decoder reads them, 0 … 1 against its white, times
/// 2^BaselineExposure (the proxy stores values divided by their brightest).
fn read_back(file: &[u8]) -> (u32, u32, u32, Vec<f32>, specular_raw::sensor::SensorMeta) {
    let f = specular_raw::parse(file).expect("the proxy parses as a raw");
    assert!(f.can_decode_sensor());
    let s = f
        .decode_sensor(file, &specular_raw::DecodeOptions::default())
        .unwrap();
    let gain = 2f64.powf(s.meta.dng.as_ref().map_or(0.0, |d| d.baseline_exposure)) as f32;
    let px: Vec<f32> = match &s.float {
        Some(fl) => fl.iter().map(|v| v / 65535.0 * gain).collect(),
        None => s
            .samples
            .iter()
            .map(|&v| f32::from(v) / s.meta.white * gain)
            .collect(),
    };
    (s.width, s.height, s.cpp, px, s.meta)
}

#[test]
fn a_bayer_proxy_holds_the_binned_scene() {
    let (w, h, black, white) = (1024u32, 640u32, 512u32, 16383u32);
    let mut spec = DngSpec::new(w, h, DngCodec::U16);
    spec.black = black;
    spec.white = white;
    spec.neutral = [0.5, 1.0, 0.7];
    spec.orientation = 6;
    let data = scene(w, h, black, white);
    let file = write_dng(&spec, &data);
    let out = proxy::make(Path::new("/card/IMG_0001.DNG"), &file, 256).unwrap();
    assert!(proxy::is_proxy(&out));
    let (pw, ph, cpp, px, meta) = read_back(&out);
    // 1024 / 256 = 4: blocks of 4 × 4 (two Bayer periods).
    assert_eq!((pw, ph, cpp), (256, 160, 3));
    assert_eq!(meta.layout, Layout::Linear);
    assert_eq!(meta.orientation, 6, "orientation kept");
    let wb = meta.wb;
    assert!(
        (wb[0] / wb[1] - 2.0).abs() < 0.02,
        "as-shot white balance kept: {wb:?}"
    );
    assert!((wb[2] / wb[1] - 1.0 / 0.7).abs() < 0.02, "{wb:?}");
    // The reference: per output pixel, the mean of each colour's samples in its block.
    let span = (white - black) as f32;
    let (mut worst, mut off) = (0f32, 0usize);
    for oy in 0..ph {
        for ox in 0..pw {
            let mut sum = [0f32; 3];
            let mut n = [0f32; 3];
            for y in oy * 4..oy * 4 + 4 {
                for x in ox * 4..ox * 4 + 4 {
                    let c = [0usize, 1, 1, 2][((y & 1) * 2 + (x & 1)) as usize];
                    sum[c] += (f32::from(data[(y * w + x) as usize]) - black as f32) / span;
                    n[c] += 1.0;
                }
            }
            for c in 0..3 {
                let want = sum[c] / n[c];
                let got = px[((oy * pw + ox) * 3) as usize + c];
                let e = (got - want).abs() / want.max(1e-3);
                worst = worst.max(e);
                // Half floats hold 11 significant bits.
                if e > 2e-3 {
                    off += 1;
                }
            }
        }
    }
    eprintln!(
        "bayer proxy: {off} of {} values off by more than 0.2 %, worst {worst:.2e}",
        pw * ph * 3
    );
    assert_eq!(off, 0, "worst relative error {worst}");
}

#[test]
fn a_linear_raw_proxy_bins_each_channel() {
    let (w, h) = (900u32, 600u32);
    let mut spec = DngSpec::new(w, h, DngCodec::U16);
    spec.linear_raw = true;
    spec.white = 65535;
    let data: Vec<u16> = (0..w * h * 3)
        .map(|i| (1000 + (i % 3) * 9000 + (i / 3 % w) * 40) as u16)
        .collect();
    let file = write_dng(&spec, &data);
    let out = proxy::make(Path::new("IMG.dng"), &file, 300).unwrap();
    let (pw, ph, cpp, px, _) = read_back(&out);
    assert_eq!((pw, ph, cpp), (300, 200, 3));
    let mut off = 0;
    for oy in 0..ph {
        for ox in 0..pw {
            for c in 0..3 {
                let mut s = 0f32;
                for y in oy * 3..oy * 3 + 3 {
                    for x in ox * 3..ox * 3 + 3 {
                        s += f32::from(data[((y * w + x) * 3 + c) as usize]) / 65535.0;
                    }
                }
                let want = s / 9.0;
                let got = px[((oy * pw + ox) * 3 + c) as usize];
                if (got - want).abs() / want > 2e-3 {
                    off += 1;
                }
            }
        }
    }
    eprintln!(
        "linear proxy: {off} of {} values off by more than 0.2 %",
        pw * ph * 3
    );
    assert_eq!(off, 0);
    // Already small: no proxy (the file is copied whole).
    assert_eq!(
        proxy::make(Path::new("IMG.dng"), &file, 2560),
        Err(proxy::Error::Small)
    );
}

#[test]
fn files_that_are_not_raws_get_no_proxy() {
    assert!(proxy::make(Path::new("a.jpg"), b"\xff\xd8\xff\xe0 not a raw", 2560).is_err());
}
